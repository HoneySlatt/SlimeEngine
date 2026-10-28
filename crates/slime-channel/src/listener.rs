//! The game's end: it listens, checks each connection, and hands the requests to its host.

use std::io::{self, Read};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use slime_serialize::MAX_SIZE;

use crate::PROTOCOL_VERSION;
use crate::error::ChannelError;
use crate::message::{Hello, Reply, Request, decode, encode};
use crate::token::Token;
use crate::wire::{receive, send};

/// The largest first message that a connection may send: a [`Hello`] holds a number and a token.
/// Larger ones are refused unread, so that who has no token makes the game read little.
const MAX_HELLO: usize = 1024;
/// How long a connection has to send its whole first message: one that does not is closed then,
/// and its thread ends.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// How many connections may wait for their first message at once: more are closed unread, so that
/// who has no token cannot make the game start threads without end.
const MAX_WAITING: usize = 16;
/// How long the thread that accepts connections waits after the system failed to give one, as when
/// the process has no file left: the failure would come back at once otherwise.
const ACCEPT_PAUSE: Duration = Duration::from_millis(100);

/// A game's end of its development channel: it listens on the machine's own address, on a port
/// that the system chooses, accepts the connections that give its token and speak its version of
/// the protocol, and hands their requests to the game, which answers each
/// (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
///
/// A thread accepts the connections, and one more for each connection reads its requests and
/// writes the answers: threads that wait rather than compute, as the engine allows threads of their
/// own (`docs/adr/0107-the-execution-model.md`). A connection's requests are answered one at a
/// time, in their order; several connections are answered side by side.
pub struct Listener {
    address: SocketAddr,
    incoming: Receiver<Incoming>,
}

/// A request that a connection made, which the game answers ([`Incoming::answer`]).
pub struct Incoming {
    request: Request,
    answer: Sender<(Reply, Vec<u8>)>,
}

impl Listener {
    /// Listens on `127.0.0.1`, on a port that the system chooses, for connections that give
    /// `token`, from tools that speak [`PROTOCOL_VERSION`]; `program` names the game to those
    /// accepted. A connection sends its [`Hello`], of at most 1 KiB, within 5 seconds, or is
    /// refused; one refused is told why, and closed. At most 16 connections wait for their hello at
    /// once: more are closed unread. Later messages hold at most `slime_serialize::MAX_SIZE` bytes;
    /// a longer one closes its connection, with the reason.
    ///
    /// `wake` is called, on the thread of the connection, each time a request is queued: a game
    /// that waits for something to do runs a frame then, which answers it.
    ///
    /// # Errors
    ///
    /// When the system gives no port to listen on, or no thread to accept the connections.
    pub fn open(
        token: Token,
        program: &str,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let (requests, incoming) = mpsc::channel();
        let program = program.to_owned();
        let waiting = Arc::new(AtomicUsize::new(0));
        thread::Builder::new()
            .name("slime dev channel".to_owned())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else {
                        thread::sleep(ACCEPT_PAUSE);
                        continue;
                    };
                    // Each answer goes out at once, rather than waiting for the acknowledgement of
                    // the last message: a socket that cannot is still served, slower.
                    let _ = stream.set_nodelay(true);
                    if waiting.fetch_add(1, Ordering::Relaxed) >= MAX_WAITING {
                        waiting.fetch_sub(1, Ordering::Relaxed);
                        continue;
                    }
                    let waits = Waiting(Arc::clone(&waiting));
                    let (token, program, requests, wake) = (
                        token.clone(),
                        program.clone(),
                        requests.clone(),
                        Arc::clone(&wake),
                    );
                    let spawned = thread::Builder::new()
                        .name("slime dev channel connection".to_owned())
                        .spawn(move || serve(stream, waits, &token, &program, &requests, &*wake));
                    if let Err(error) = spawned {
                        tracing::warn!(%error, "a connection to the development channel is dropped");
                    }
                }
            })?;
        Ok(Self { address, incoming })
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The next request that a connection made, if one waits, without waiting for one.
    pub fn next(&self) -> Option<Incoming> {
        self.incoming.try_recv().ok()
    }
}

impl Incoming {
    /// The request.
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Answers the request with `reply`.
    pub fn answer(self, reply: Reply) {
        self.answer_with(reply, Vec::new());
    }

    /// Answers the request with `reply`, and `payload` after it, as [`Reply::Image`] has its
    /// pixels.
    pub fn answer_with(self, reply: Reply, payload: Vec<u8>) {
        // The connection may be gone: nothing waits for the answer then.
        let _ = self.answer.send((reply, payload));
    }
}

/// Counts a connection among those that wait for their first message, until it is dropped.
struct Waiting(Arc<AtomicUsize>);

impl Drop for Waiting {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Reads from a stream until a moment, whatever the number of reads: a peer that sends a byte at a
/// time cannot hold a connection past it.
struct Until<'a> {
    stream: &'a TcpStream,
    until: Instant,
}

impl Read for Until<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let left = self.until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(bytes)
    }
}

/// Serves a connection: checks its first message, then hands its requests to the game, through
/// `requests`, waking it, and writes the game's answers, until the connection or the game ends.
/// `waits` counts it among the connections that wait for their first message, until it came.
fn serve(
    mut stream: TcpStream,
    waits: Waiting,
    token: &Token,
    program: &str,
    requests: &Sender<Incoming>,
    wake: &(dyn Fn() + Send + Sync),
) {
    let checked = handshake(&mut stream, token);
    drop(waits);
    if let Err(reason) = checked {
        // Escaped: the reason may quote what the connection sent, which must not make lines of
        // the log.
        tracing::warn!("a connection to the development channel is refused: {reason:?}");
        let _ = reply(&mut stream, &Reply::Refused { reason }, &[]);
        return;
    }
    let welcome = Reply::Welcome {
        version: PROTOCOL_VERSION,
        program: program.to_owned(),
    };
    if reply(&mut stream, &welcome, &[]).is_err() {
        return;
    }
    tracing::info!("a tool connected to the development channel");
    loop {
        let request = match receive(&mut stream, MAX_SIZE, 0) {
            Ok((document, _)) => decode::<Request>(&document),
            // The tool closed the connection, or broke it.
            Err(ChannelError::Io(_)) => return,
            // A message refused by its length is left unread, so the next one cannot be found: the
            // connection is closed, with the reason.
            Err(error) => {
                let failed = Reply::Failed {
                    message: error.to_string(),
                };
                let _ = reply(&mut stream, &failed, &[]);
                return;
            }
        };
        let (answer, answered) = mpsc::channel();
        let (answer, payload) = match request {
            Ok(request) => {
                let sent = requests.send(Incoming { request, answer });
                if sent.is_ok() {
                    wake();
                }
                match sent.ok().and_then(|()| answered.recv().ok()) {
                    Some(answer) => answer,
                    None => (
                        Reply::Failed {
                            message: "the game has stopped".to_owned(),
                        },
                        Vec::new(),
                    ),
                }
            }
            // A hello among them too: the connection is open already.
            Err(error) => (
                Reply::Failed {
                    message: error.to_string(),
                },
                Vec::new(),
            ),
        };
        if reply(&mut stream, &answer, &payload).is_err() {
            return;
        }
    }
}

/// Reads a connection's first message, within [`HELLO_TIMEOUT`], and says why it is refused, if it
/// is: none in time, not a hello, another version of the protocol, another token.
fn handshake(stream: &mut TcpStream, token: &Token) -> Result<(), String> {
    let mut until = Until {
        stream,
        until: Instant::now() + HELLO_TIMEOUT,
    };
    let read = receive(&mut until, MAX_HELLO, 0);
    // A tool waits as long as it likes between its requests.
    stream
        .set_read_timeout(None)
        .map_err(|error| error.to_string())?;
    let (document, _) = read.map_err(|error| match error {
        ChannelError::Io(error) => format!("the hello could not be read in time: {error}"),
        error => error.to_string(),
    })?;
    let Hello {
        version,
        token: given,
    } = decode::<Hello>(&document)
        .map_err(|error| format!("the first message is not a hello: {error}"))?;
    if version != PROTOCOL_VERSION {
        Err(format!(
            "the tool speaks version {version} of the protocol, the game version \
             {PROTOCOL_VERSION}"
        ))
    } else if !token.matches(&given) {
        Err("the token is not the game's".to_owned())
    } else {
        Ok(())
    }
}

/// Writes `answer`, and `payload` after it.
fn reply(stream: &mut TcpStream, answer: &Reply, payload: &[u8]) -> Result<(), ChannelError> {
    send(stream, &encode(answer)?, payload)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use slime_reflect::Reflect;

    use super::*;
    use crate::Client;

    /// Sends `message` on `stream`: the reply.
    fn send_on<T: Reflect>(stream: &mut TcpStream, message: &T) -> Reply {
        send(stream, &encode(message).expect("encoded"), &[]).expect("sent");
        let (document, _) = receive(stream, MAX_SIZE, 0).expect("answered");
        decode(&document).expect("a reply")
    }

    /// Opens a connection to `address` and sends `message` as its first message: the reply.
    fn first<T: Reflect>(address: SocketAddr, message: &T) -> Reply {
        send_on(
            &mut TcpStream::connect(address).expect("listening"),
            message,
        )
    }

    /// A hello with `token`, of this version of the protocol.
    fn hello(token: &Token) -> Hello {
        Hello {
            version: PROTOCOL_VERSION,
            token: token.to_string(),
        }
    }

    #[test]
    fn a_connection_without_the_token_or_of_another_version_is_refused_with_a_reason() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let address = listener.address();
        assert!(address.ip().is_loopback());
        let refused = |reply: Reply, why: &str| match reply {
            Reply::Refused { reason } => assert!(reason.contains(why), "{reason}"),
            other => panic!("refused, not {other:?}"),
        };
        let other = Token::generate().expect("random bits");
        refused(first(address, &hello(&other)), "token");
        let later = Hello {
            version: PROTOCOL_VERSION + 1,
            ..hello(&token)
        };
        refused(first(address, &later), "version");
        refused(first(address, &Request::Pause), "hello");
        assert!(matches!(
            Client::connect(address, &other),
            Err(ChannelError::Refused(_))
        ));
        assert!(listener.next().is_none(), "no request reached the game");
    }

    #[test]
    fn connections_that_send_no_hello_are_bounded_in_number_and_in_time() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let silent: Vec<TcpStream> = (0..MAX_WAITING)
            .map(|_| TcpStream::connect(listener.address()).expect("listening"))
            .collect();
        // One more is closed unread, at once.
        let mut one_more = TcpStream::connect(listener.address()).expect("listening");
        one_more
            .set_read_timeout(Some(HELLO_TIMEOUT / 2))
            .expect("a timeout");
        let mut rest = Vec::new();
        one_more.read_to_end(&mut rest).expect("closed");
        assert!(rest.is_empty());
        // The others are refused once their time has passed, which lets new connections in.
        for mut stream in silent {
            let (document, _) = receive(&mut stream, MAX_SIZE, 0).expect("answered");
            let reply: Reply = decode(&document).expect("a reply");
            assert!(
                matches!(&reply, Reply::Refused { reason } if reason.contains("hello")),
                "{reply:?}"
            );
        }
        let mut accepted = None;
        for _ in 0..100 {
            if let Ok(client) = Client::connect(listener.address(), &token) {
                accepted = Some(client);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(accepted.is_some(), "a tool with the token connects again");
    }

    #[test]
    fn a_request_is_answered_in_well_under_the_delay_of_an_acknowledgement() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut client = Client::connect(listener.address(), &token).expect("accepted");
        let stop = Arc::new(AtomicBool::new(false));
        // The game's side: answers each request at once, with a payload as a capture has.
        let answering = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.next() {
                        Some(incoming) => incoming.answer_with(Reply::Done, vec![7; 64]),
                        None => thread::yield_now(),
                    }
                }
            })
        };
        let mut times: Vec<Duration> = (0..21)
            .map(|_| {
                let started = Instant::now();
                let (reply, payload) = client.request(&Request::State).expect("answered");
                assert_eq!((reply, payload.len()), (Reply::Done, 64));
                started.elapsed()
            })
            .collect();
        stop.store(true, Ordering::Relaxed);
        answering.join().expect("answered");
        times.sort();
        let median = times[times.len() / 2];
        println!(
            "a request answered in {median:?}, the median of {}",
            times.len()
        );
        // A message written in pieces, on a socket that waits for the acknowledgement of the last
        // piece before it sends the next, takes 40 ms or more.
        assert!(median < Duration::from_millis(20), "{median:?}");
    }

    #[test]
    fn a_request_to_a_game_that_stopped_is_answered_so() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut client = Client::connect(listener.address(), &token).expect("accepted");
        drop(listener);
        let (reply, _) = client.request(&Request::State).expect("answered");
        assert_eq!(
            reply,
            Reply::Failed {
                message: "the game has stopped".to_owned()
            }
        );
    }

    #[test]
    fn a_second_hello_is_no_request() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        let welcome = send_on(&mut stream, &hello(&token));
        assert!(matches!(welcome, Reply::Welcome { .. }), "{welcome:?}");
        let again = send_on(&mut stream, &hello(&token));
        assert!(matches!(again, Reply::Failed { .. }), "{again:?}");
        assert!(listener.next().is_none());
    }

    #[test]
    fn a_message_too_long_closes_the_connection_with_the_reason() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        let welcome = send_on(&mut stream, &hello(&token));
        assert!(matches!(welcome, Reply::Welcome { .. }), "{welcome:?}");
        let length = u32::try_from(MAX_SIZE + 1).expect("past the largest document");
        std::io::Write::write_all(&mut stream, &length.to_le_bytes()).expect("sent");
        let (document, _) = receive(&mut stream, MAX_SIZE, 0).expect("answered");
        let reply: Reply = decode(&document).expect("a reply");
        assert!(matches!(reply, Reply::Failed { .. }), "{reply:?}");
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).expect("closed");
        assert!(rest.is_empty(), "nothing follows the reason");
        assert!(listener.next().is_none());
    }
}
