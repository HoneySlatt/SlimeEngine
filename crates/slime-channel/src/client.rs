//! A tool's end: it starts a game with a token, connects to it, and asks.

use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use uwui_agent::{
    CORE_VERSION, Family, FromProgram, Hello, Limits, Named, Reader, ToolSession, Welcome,
};

use crate::error::ChannelError;
use crate::message::{Reply, Request};
use crate::token::Token;
use crate::wire::{MAX_BYTES, MAX_LINE, receive, send};
use crate::{ANNOUNCEMENT, TOKEN_ENV_VAR};

/// How long a connection waits for the game's answer to a request before it gives up: a game
/// answers at its next frame, which takes seconds on a CPU's renderer, and a step of many fixed
/// steps once the frames have run them all.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// A connection to a game's development channel, open once the game welcomed its hello: it carries
/// the tool's end of the session of uwui's core for agents (`uwui_agent::ToolSession`).
pub struct Client {
    stream: TcpStream,
    reader: Reader,
    session: ToolSession,
    program: String,
    /// Whether a request failed: the connection is given up then ([`ChannelError::Broken`]).
    broken: bool,
}

impl Client {
    /// Connects to the game that listens at `address`, with `token`, for `agent`, whose requests
    /// `client` carries, an MCP server, as they name themselves: the game logs both. It speaks every family of
    /// [`FAMILIES`](crate::FAMILIES).
    ///
    /// # Errors
    ///
    /// [`ChannelError::Refused`] with the game's reason when it refuses the token or the version of
    /// the core; [`ChannelError::Io`] when no game listens there or the connection breaks;
    /// [`ChannelError::Malformed`] or [`ChannelError::Session`] when what answers does not speak
    /// the protocol.
    pub fn connect(
        address: SocketAddr,
        token: &Token,
        agent: &Named,
        client: &Named,
    ) -> Result<Self, ChannelError> {
        Self::connect_speaking(address, token, agent, client, crate::families())
            .map(|(connection, _)| connection)
    }

    /// Connects as [`Client::connect`] does, speaking `families`: the client, and the game's
    /// welcome, with the families that it serves and those that it refuses.
    pub(crate) fn connect_speaking(
        address: SocketAddr,
        token: &Token,
        agent: &Named,
        client: &Named,
        families: Vec<Family>,
    ) -> Result<(Self, Welcome), ChannelError> {
        let stream = TcpStream::connect(address)?;
        // Each message goes out at once, rather than waiting for the acknowledgement of the last.
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        let mut connection = Self {
            stream,
            reader: Reader::new(Limits {
                line: MAX_LINE,
                bytes: MAX_BYTES,
            }),
            session: ToolSession::new(),
            program: String::new(),
            broken: false,
        };
        let hello = connection.session.hello(&Hello {
            core: CORE_VERSION,
            agent: agent.clone(),
            client: client.clone(),
            secret: token.to_string(),
            families,
        })?;
        send(&mut connection.stream, &hello, &[])?;
        let frame = receive(&mut connection.stream, &mut connection.reader)?;
        match connection.session.receive(frame)? {
            FromProgram::Welcomed(welcome) => {
                connection.program.clone_from(&welcome.program);
                Ok((connection, welcome))
            }
            FromProgram::Refused(reason) => Err(ChannelError::Refused(reason)),
            FromProgram::Unreadable(fault) => Err(ChannelError::Refused(fault.message)),
            other => Err(ChannelError::Malformed(format!(
                "the game answered {other:?} to a hello"
            ))),
        }
    }

    /// The name of the game's program.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Sends `request`, and waits for the game's answer: the reply, and the bytes that follow it,
    /// the pixels of an image. A request holds at most 1 MiB, which the game refuses past it, closing
    /// the connection; an answer at most 16 MiB, and its bytes 1 GiB.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Session`] when the game does not serve the request's family on this
    /// connection, which sends nothing and leaves the connection usable. [`ChannelError::Io`] when
    /// the connection breaks, or the game does not answer within ten minutes;
    /// [`ChannelError::Malformed`] or [`ChannelError::Session`] when its answer is not a reply of
    /// the protocol. After such an error, the game's answer may still come, and would be read as
    /// the answer to the next request: every later request fails with [`ChannelError::Broken`],
    /// and a new connection is needed. A request that the game refuses is answered
    /// [`Reply::Failed`], which is no error.
    pub fn request(&mut self, request: &Request) -> Result<(Reply, Vec<u8>), ChannelError> {
        if self.broken {
            return Err(ChannelError::Broken);
        }
        let (method, params) = request.call()?;
        let (id, line) = self.session.request(&method, params)?;
        let answer = self.exchange(id, &line);
        self.broken = answer.is_err();
        answer
    }

    /// Sends a request's `line`, and waits for the answer numbered `id`.
    fn exchange(&mut self, id: u64, line: &[u8]) -> Result<(Reply, Vec<u8>), ChannelError> {
        send(&mut self.stream, line, &[])?;
        loop {
            let frame = receive(&mut self.stream, &mut self.reader)?;
            match self.session.receive(frame)? {
                FromProgram::Response {
                    id: answered,
                    outcome,
                    bytes,
                } if answered == id => return Ok((Reply::read(outcome)?, bytes)),
                // The channel's families send no event yet.
                FromProgram::Event { .. } => {}
                FromProgram::Unreadable(fault) => {
                    return Err(ChannelError::Malformed(fault.message));
                }
                other => {
                    return Err(ChannelError::Malformed(format!(
                        "the game answered {other:?}"
                    )));
                }
            }
        }
    }
}

/// A game started with a token, and where it listens.
pub struct Launched {
    /// The game's process. A thread of the tool reads its standard output to its end, looking for
    /// the announcement, and keeps none of it; its standard error is as the command set it. Its
    /// standard input is a pipe that `child` holds (`Child::stdin`): the game ends once it closes,
    /// as when the tool drops it or ends, crashed or not.
    pub child: Child,
    /// Where it listens.
    pub address: SocketAddr,
    /// The token that it asks.
    pub token: Token,
}

/// Starts the game that `command` runs, with a new token in its environment
/// ([`TOKEN_ENV_VAR`]), and waits, at most `timeout`, for it to say on its standard output where it
/// listens ([`ANNOUNCEMENT`]). The token is in the game's environment only, never on its command
/// line, which other users of the machine can read. Its standard input is a pipe that the tool
/// holds, whatever `command` set: a game started with a token ends once it closes, so that a tool
/// that ends leaves no game behind ([`Launched::child`]).
///
/// # Errors
///
/// [`ChannelError::Randomness`] when no token can be drawn; [`ChannelError::Io`] when the game
/// cannot be started; [`ChannelError::NotListening`] when it ends, or does not say where it listens,
/// within `timeout`: a program built without the development channel never does. The game is
/// stopped then.
pub fn launch(command: &mut Command, timeout: Duration) -> Result<Launched, ChannelError> {
    launch_unless(command, timeout, || false)
}

/// Starts the game that `command` runs, as [`launch`] does, but gives up once `cancelled` says so,
/// which it asks every twentieth of a second while it waits: what an editor that plays a game
/// does, whose user stops the game before it said where it listens
/// (`docs/adr/0164-play.md`).
///
/// # Errors
///
/// As [`launch`]; [`ChannelError::NotListening`] too once `cancelled` gave up. The game is stopped
/// then.
pub fn launch_unless(
    command: &mut Command,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<Launched, ChannelError> {
    let token = Token::generate()?;
    let mut child = command
        .env(TOKEN_ENV_VAR, token.to_string())
        .stdin(Stdio::piped())
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
    let until = Instant::now() + timeout;
    let error = loop {
        let left = until.saturating_duration_since(Instant::now());
        match address.recv_timeout(left.min(Duration::from_millis(50))) {
            Ok(address) => {
                return Ok(Launched {
                    child,
                    address,
                    token,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) if cancelled() => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ChannelError::NotListening(
                    "it was stopped before it said where it listens".to_owned(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) if !left.is_zero() => {}
            Err(error) => break error,
        }
    };
    let _ = child.kill();
    let why = match child.wait() {
        Ok(status) if error == mpsc::RecvTimeoutError::Disconnected => {
            format!("it ended ({status}) without saying where it listens")
        }
        _ => format!("it did not say where it listens within {timeout:?}"),
    };
    Err(ChannelError::NotListening(why))
}
