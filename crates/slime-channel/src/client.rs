//! A tool's end: it starts a game with a token, connects to it, and asks.

use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use slime_reflect::Reflect;
use slime_serialize::MAX_SIZE;

use crate::error::ChannelError;
use crate::message::{Hello, Reply, Request, decode, encode};
use crate::token::Token;
use crate::wire::{MAX_PAYLOAD, receive, send};
use crate::{ANNOUNCEMENT, PROTOCOL_VERSION, TOKEN_ENV_VAR};

/// How long a connection waits for the game's answer to a request before it gives up: a game
/// answers at its next frame, which takes seconds on a CPU's renderer, and a step of many fixed
/// steps once the frames have run them all.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// A connection to a game's development channel, open once the game accepted its token and its
/// version of the protocol.
pub struct Client {
    stream: TcpStream,
    program: String,
    /// Whether a request failed: the connection is given up then ([`ChannelError::Broken`]).
    broken: bool,
}

impl Client {
    /// Connects to the game that listens at `address`, with `token`.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Refused`] with the game's reason when it refuses the token or the version of
    /// the protocol; [`ChannelError::Io`] when no game listens there or the connection breaks;
    /// [`ChannelError::Malformed`] when what answers does not speak the protocol.
    pub fn connect(address: SocketAddr, token: &Token) -> Result<Self, ChannelError> {
        let stream = TcpStream::connect(address)?;
        // Each message goes out at once, rather than waiting for the acknowledgement of the last.
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        let mut client = Self {
            stream,
            program: String::new(),
            broken: false,
        };
        let hello = Hello {
            version: PROTOCOL_VERSION,
            token: token.to_string(),
        };
        match client.exchange(&hello)? {
            (Reply::Welcome { program, .. }, _) => {
                client.program = program;
                Ok(client)
            }
            (Reply::Refused { reason }, _) => Err(ChannelError::Refused(reason)),
            (other, _) => Err(ChannelError::Malformed(format!(
                "the game answered {other:?} to a hello"
            ))),
        }
    }

    /// The name of the game's program.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Sends `request`, and waits for the game's answer: the reply, and the bytes that follow it,
    /// the pixels of an image. Messages hold at most `slime_serialize::MAX_SIZE` bytes.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Io`] when the connection breaks, or the game does not answer within ten
    /// minutes; [`ChannelError::Malformed`] when its answer is not a reply of the protocol. After
    /// such an error, the game's answer may still come, and would be read as the answer to the next
    /// request: every later request fails with [`ChannelError::Broken`], and a new connection is
    /// needed. A request that the game refuses is answered [`Reply::Failed`], which is no error.
    pub fn request(&mut self, request: &Request) -> Result<(Reply, Vec<u8>), ChannelError> {
        self.exchange(request)
    }

    /// Sends `message`, and waits for the game's answer.
    fn exchange<T: Reflect>(&mut self, message: &T) -> Result<(Reply, Vec<u8>), ChannelError> {
        if self.broken {
            return Err(ChannelError::Broken);
        }
        let answer = send(&mut self.stream, &encode(message)?, &[])
            .map_err(ChannelError::from)
            .and_then(|()| receive(&mut self.stream, MAX_SIZE, MAX_PAYLOAD))
            .and_then(|(document, payload)| Ok((decode(&document)?, payload)));
        self.broken = answer.is_err();
        answer
    }
}

/// A game started with a token, and where it listens.
pub struct Launched {
    /// The game's process. A thread of the tool reads its standard output to its end, looking for
    /// the announcement, and keeps none of it; its standard error is as the command set it.
    pub child: Child,
    /// Where it listens.
    pub address: SocketAddr,
    /// The token that it asks.
    pub token: Token,
}

/// Starts the game that `command` runs, with a new token in its environment
/// ([`TOKEN_ENV_VAR`]), and waits, at most `timeout`, for it to say on its standard output where it
/// listens ([`ANNOUNCEMENT`]). The token is in the game's environment only, never on its command
/// line, which other users of the machine can read.
///
/// # Errors
///
/// [`ChannelError::Randomness`] when no token can be drawn; [`ChannelError::Io`] when the game
/// cannot be started; [`ChannelError::NotListening`] when it ends, or does not say where it listens,
/// within `timeout`: a program built without the development channel never does. The game is
/// stopped then.
pub fn launch(command: &mut Command, timeout: Duration) -> Result<Launched, ChannelError> {
    let token = Token::generate()?;
    let mut child = command
        .env(TOKEN_ENV_VAR, token.to_string())
        .stdout(Stdio::piped())
        .spawn()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ChannelError::NotListening(
            "its output cannot be read".to_owned(),
        ));
    };
    let (found, address) = mpsc::channel();
    // Reads the game's output until it ends, so that the game never waits on a full pipe: as
    // bytes, which need not be text.
    let reader = thread::Builder::new()
        .name("slime dev channel launch".to_owned())
        .spawn(move || {
            let mut found = Some(found);
            for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
                let line = String::from_utf8_lossy(&line);
                // Anywhere in the line: what the program wrote before it may lack its line's end.
                let at = line.split_once(ANNOUNCEMENT).and_then(|(_, rest)| {
                    rest.split_whitespace().next()?.parse::<SocketAddr>().ok()
                });
                if let (Some(at), Some(sender)) = (at, &found) {
                    let _ = sender.send(at);
                    found = None;
                }
            }
        });
    if let Err(error) = reader {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    match address.recv_timeout(timeout) {
        Ok(address) => Ok(Launched {
            child,
            address,
            token,
        }),
        Err(error) => {
            let _ = child.kill();
            let why = match child.wait() {
                Ok(status) if error == mpsc::RecvTimeoutError::Disconnected => {
                    format!("it ended ({status}) without saying where it listens")
                }
                _ => format!("it did not say where it listens within {timeout:?}"),
            };
            Err(ChannelError::NotListening(why))
        }
    }
}
