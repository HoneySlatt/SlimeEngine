//! The game's end: it listens, checks each connection, and hands the requests to its host.

use std::io::{self, Read};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use uwui_agent::{Fault, FromTool, Limits, ProgramSession, Reader, Value};

use crate::error::ChannelError;
use crate::message::{Reply, Request};
use crate::token::Token;
use crate::wire::{MAX_REQUEST, receive, send};

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
/// that the system chooses, accepts the connections whose hello gives its token and speaks its
/// version of the core, serves them the [`FAMILIES`](crate::FAMILIES) that they speak at the same
/// versions, and hands their requests to the game, which answers each
/// (`docs/adr/0110-how-the-editor-runs-a-games-code.md`,
/// `docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md`).
///
/// It carries the session of uwui's core for agents (`uwui_agent::ProgramSession`): a thread
/// accepts the connections, and one more for each connection reads its requests and writes the
/// answers, threads that wait rather than compute, as the engine allows threads of their own
/// (`docs/adr/0107-the-execution-model.md`). A connection's requests are answered one at a time, in
/// their order; several connections are answered side by side.
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
    /// Listens on `127.0.0.1`, on a port that the system chooses, for connections whose hello
    /// gives `token` and speaks `uwui_agent::CORE_VERSION`; `program` names the game to those
    /// welcomed. A connection sends its hello, of at most 4 KiB, within 5 seconds, or is refused;
    /// one refused is told why, and closed. At most 16 connections wait for their hello at once:
    /// more are closed unread. Later requests hold at most 1 MiB, and announce no bytes; a line
    /// that is not a message, or past that, closes its connection, with the reason. The game logs
    /// the agent of each connection welcomed, and the program that carries it, as they name
    /// themselves.
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

/// `error` and its causes, on one line. `slime_app::error_chain` and `slime-mcp`'s `causes` write
/// the same for their own errors: `slime-channel` lies below the first and links no logging.
fn why(error: &dyn std::error::Error) -> String {
    let mut why = error.to_string();
    let mut cause = error.source();
    while let Some(error) = cause {
        why.push_str(": ");
        why.push_str(&error.to_string());
        cause = error.source();
    }
    why
}

/// Serves a connection: welcomes or refuses its hello, then hands its requests to the game, through
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
    let mut session = ProgramSession::new(program, crate::families());
    let mut reader = Reader::new(Limits::HELLO);
    let first = receive(
        &mut Until {
            stream: &stream,
            until: Instant::now() + HELLO_TIMEOUT,
        },
        &mut reader,
    );
    drop(waits);
    // A tool waits as long as it likes between its requests.
    if stream.set_read_timeout(None).is_err() {
        return;
    }
    let welcomed = match first {
        Ok(frame) => session.receive(frame, |secret| token.matches(secret)),
        Err(ChannelError::Io(error)) => {
            close(
                &mut stream,
                &session,
                &format!("the hello could not be read in time: {error}"),
            );
            return;
        }
        Err(error) => {
            close(&mut stream, &session, &why(&error));
            return;
        }
    };
    match welcomed {
        Ok(FromTool::Welcomed {
            agent,
            client,
            reply,
        }) => {
            // Escaped, as fields: what a connection says it is must not make lines of the log.
            tracing::info!(
                agent = ?agent.to_string(),
                client = ?client.to_string(),
                "an agent connected to the development channel"
            );
            if send(&mut stream, &reply, &[]).is_err() {
                return;
            }
        }
        Ok(FromTool::Refused { reason, reply }) => {
            tracing::warn!("a connection to the development channel is refused: {reason:?}");
            let _ = send(&mut stream, &reply, &[]);
            return;
        }
        _ => return,
    }
    reader.set_limits(Limits {
        line: MAX_REQUEST,
        bytes: 0,
    });
    loop {
        let frame = match receive(&mut stream, &mut reader) {
            Ok(frame) => frame,
            // The tool closed the connection, or broke it.
            Err(ChannelError::Io(_)) => return,
            // A line that is not a message loses the framing: the next one cannot be found, and
            // the connection is closed, with the reason.
            Err(error) => {
                close(&mut stream, &session, &why(&error));
                return;
            }
        };
        let answer = match session.receive(frame, |_| false) {
            Ok(FromTool::Request {
                id,
                family,
                name,
                params,
                ..
            }) => {
                let (outcome, payload) = answered(&family, &name, params, requests, wake);
                session
                    .answer(id, outcome, payload.len())
                    .map(|line| (line, payload))
            }
            Ok(FromTool::Answered { reply }) => Ok((reply, Vec::new())),
            Ok(_) => continue,
            Err(_) => return,
        };
        match answer {
            Ok((line, payload)) if send(&mut stream, &line, &payload).is_ok() => {}
            _ => return,
        }
    }
}

/// The game's answer to the request `name` of `family` with `params`: the outcome, and the bytes
/// that follow it.
fn answered(
    family: &str,
    name: &str,
    params: Value,
    requests: &Sender<Incoming>,
    wake: &(dyn Fn() + Send + Sync),
) -> (Result<Value, Fault>, Vec<u8>) {
    let request = match Request::read(family, &format!("{family}/{name}"), params) {
        Ok(request) => request,
        Err(fault) => return (Err(fault), Vec::new()),
    };
    let (answer, answers) = mpsc::channel();
    let sent = requests.send(Incoming { request, answer });
    if sent.is_ok() {
        wake();
    }
    let (reply, payload) = match sent.ok().and_then(|()| answers.recv().ok()) {
        Some(answer) => answer,
        None => (
            Reply::Failed {
                message: "the game has stopped".to_owned(),
            },
            Vec::new(),
        ),
    };
    match reply.outcome() {
        Ok(outcome) => (outcome, payload),
        Err(error) => (Err(Fault::new(Fault::REFUSED, why(&error))), Vec::new()),
    }
}

/// Tells the tool why its connection is closed, as an answer to no request.
fn close(stream: &mut TcpStream, session: &ProgramSession, reason: &str) {
    // Escaped: the reason may quote what the connection sent.
    tracing::warn!("a connection to the development channel is closed: {reason:?}");
    if let Ok(line) = session.unreadable(reason) {
        let _ = send(stream, &line, &[]);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::atomic::AtomicBool;

    use uwui_agent::{
        CORE_VERSION, Family, FromProgram, Hello, Message, Named, ToolSession, encode,
    };

    use super::*;
    use crate::Client;
    use crate::wire::{MAX_BYTES, MAX_LINE};

    fn agent() -> Named {
        Named::new("a test", "1")
    }

    fn client() -> Named {
        Named::new("slime-channel's tests", "")
    }

    /// A hello with `token`, of this version of the core, speaking every family.
    fn hello(token: &Token) -> Hello {
        Hello {
            core: CORE_VERSION,
            agent: agent(),
            client: client(),
            secret: token.to_string(),
            families: crate::families(),
        }
    }

    /// Reads the next message on `stream`, as a tool's session does.
    fn read(stream: &mut TcpStream, session: &mut ToolSession) -> FromProgram {
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: MAX_BYTES,
        });
        session
            .receive(receive(stream, &mut reader).expect("a message"))
            .expect("a message of the session")
    }

    /// Opens a connection to `address` and sends `line` as its first message: what answers.
    fn first(address: SocketAddr, line: &[u8]) -> FromProgram {
        let mut stream = TcpStream::connect(address).expect("listening");
        stream.write_all(line).expect("sent");
        let mut session = ToolSession::new();
        // The session reads the answer to its own hello, numbered 0.
        session
            .hello(&hello(&Token::generate().expect("random bits")))
            .expect("written");
        read(&mut stream, &mut session)
    }

    #[test]
    fn a_connection_without_the_token_or_of_another_core_is_refused_with_a_reason() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let address = listener.address();
        assert!(address.ip().is_loopback());
        let refused = |answer: FromProgram, why: &str| match answer {
            FromProgram::Refused(reason) => assert!(reason.contains(why), "{reason}"),
            other => panic!("refused, not {other:?}"),
        };
        let other = Token::generate().expect("random bits");
        let line = ToolSession::new().hello(&hello(&other)).expect("written");
        refused(first(address, &line), "secret");
        let later = Hello {
            core: CORE_VERSION + 1,
            ..hello(&token)
        };
        let line = ToolSession::new().hello(&later).expect("written");
        refused(first(address, &line), "version");
        let pause = encode(
            &Message::Request {
                id: 0,
                method: "play/pause".to_owned(),
                params: uwui_agent::Value::Null,
            },
            0,
        )
        .expect("written");
        refused(first(address, &pause), "hello");
        assert!(matches!(
            Client::connect(address, &other, &agent(), &client()),
            Err(ChannelError::Refused(_))
        ));
        assert!(listener.next().is_none(), "no request reached the game");
    }

    #[test]
    fn a_family_of_another_version_is_refused_alone_and_the_rest_served() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut families = crate::families();
        for family in &mut families {
            if family.name == "editing" {
                family.version += 1;
            }
        }
        let (mut client, welcome) =
            Client::connect_speaking(listener.address(), &token, &agent(), &client(), families)
                .expect("welcomed");
        assert_eq!(welcome.refused.len(), 1, "{welcome:?}");
        assert_eq!(welcome.refused[0].family.name, "editing");
        assert_eq!(welcome.served.len(), crate::FAMILIES.len() - 1);
        let game = thread::spawn(move || {
            loop {
                if let Some(incoming) = listener.next() {
                    assert_eq!(incoming.request(), &Request::Pause);
                    incoming.answer(Reply::Done);
                    return listener;
                }
                thread::yield_now();
            }
        });
        let edit = Request::Edit {
            edit: crate::Edit::Undo,
        };
        assert!(
            matches!(
                client.request(&edit),
                Err(ChannelError::Session(uwui_agent::AgentError::NotServed(family))) if family == "editing"
            ),
            "the tool's end sends nothing of a family not served"
        );
        let (reply, _) = client.request(&Request::Pause).expect("the rest is served");
        assert_eq!(reply, Reply::Done);
        let listener = game.join().expect("answered");
        assert!(listener.next().is_none(), "the edit never reached the game");
    }

    #[test]
    fn the_game_answers_a_family_not_served_or_parameters_that_do_not_read_and_serves_on() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        let mut session = ToolSession::new();
        let mut speaking = hello(&token);
        speaking.families = vec![Family::new("play", 1)];
        stream
            .write_all(&session.hello(&speaking).expect("written"))
            .expect("sent");
        assert!(matches!(
            read(&mut stream, &mut session),
            FromProgram::Welcomed(_)
        ));
        let mut ask = |method: &str, params| {
            let line = encode(
                &Message::Request {
                    id: 41,
                    method: method.to_owned(),
                    params,
                },
                0,
            )
            .expect("written");
            stream.write_all(&line).expect("sent");
            let mut reader = Reader::new(Limits {
                line: MAX_LINE,
                bytes: 0,
            });
            match receive(&mut stream, &mut reader).expect("answered").message {
                Message::Response {
                    id: Some(41),
                    outcome: Err(fault),
                } => fault.code,
                other => panic!("a failure, not {other:?}"),
            }
        };
        assert_eq!(
            ask("editing/undo", uwui_agent::Value::Null),
            Fault::NOT_SERVED
        );
        assert_eq!(
            ask("play/rewind", uwui_agent::Value::Null),
            Fault::NO_METHOD
        );
        let steps = uwui_agent::write(&"three").expect("written");
        assert_eq!(ask("play/step", steps), Fault::BAD_PARAMS);
        assert!(listener.next().is_none(), "none reached the game");
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
        // The others are told why once their time has passed, which lets new connections in.
        for mut stream in silent {
            let mut reader = Reader::new(Limits::HELLO);
            let frame = receive(&mut stream, &mut reader).expect("answered");
            assert!(
                matches!(&frame.message, Message::Response { id: None, outcome: Err(fault) } if fault.message.contains("hello")),
                "{frame:?}"
            );
        }
        let mut accepted = None;
        for _ in 0..100 {
            if let Ok(client) = Client::connect(listener.address(), &token, &agent(), &client()) {
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
        let mut client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
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
        let mut client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
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
        let mut session = ToolSession::new();
        stream
            .write_all(&session.hello(&hello(&token)).expect("written"))
            .expect("sent");
        assert!(matches!(
            read(&mut stream, &mut session),
            FromProgram::Welcomed(_)
        ));
        let again = encode(
            &Message::Request {
                id: 1,
                method: uwui_agent::HELLO.to_owned(),
                params: uwui_agent::write(&hello(&token)).expect("written"),
            },
            0,
        )
        .expect("written");
        stream.write_all(&again).expect("sent");
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: 0,
        });
        let frame = receive(&mut stream, &mut reader).expect("answered");
        assert!(
            matches!(&frame.message, Message::Response { id: Some(1), outcome: Err(fault) } if fault.code == Fault::INVALID),
            "{frame:?}"
        );
        assert!(listener.next().is_none());
    }

    #[test]
    fn a_request_line_too_long_closes_the_connection_with_the_reason() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        let mut session = ToolSession::new();
        stream
            .write_all(&session.hello(&hello(&token)).expect("written"))
            .expect("sent");
        assert!(matches!(
            read(&mut stream, &mut session),
            FromProgram::Welcomed(_)
        ));
        // The game may close before it has read all of it: what is left unsent is no failure.
        let _ = stream.write_all(&vec![b' '; MAX_REQUEST + 1]);
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: 0,
        });
        let frame = receive(&mut stream, &mut reader).expect("answered");
        assert!(
            matches!(
                &frame.message,
                Message::Response {
                    id: None,
                    outcome: Err(_)
                }
            ),
            "{frame:?}"
        );
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
        assert!(rest.is_empty(), "nothing follows the reason");
        assert!(listener.next().is_none());
    }

    #[test]
    fn a_first_line_past_the_hello_s_limit_is_refused() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        // The hello's limit, 4 KiB, which the listener's documentation promises, and one more.
        let _ = stream.write_all(&[b' '; 4 * 1024 + 1]);
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: 0,
        });
        let frame = receive(&mut stream, &mut reader).expect("answered");
        assert!(
            matches!(&frame.message, Message::Response { id: None, outcome: Err(fault) } if fault.message.contains("line")),
            "{frame:?}"
        );
    }

    #[test]
    fn a_request_past_the_hello_s_limit_and_a_long_answer_are_served() {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        let mut client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let report = "r".repeat(256 * 1024);
        let game = {
            let report = report.clone();
            thread::spawn(move || {
                loop {
                    if let Some(incoming) = listener.next() {
                        let Request::Apply { commands } = incoming.request() else {
                            panic!("the commands");
                        };
                        assert_eq!(commands[0].len(), 64 * 1024);
                        incoming.answer(Reply::Report { text: report });
                        return;
                    }
                    thread::yield_now();
                }
            })
        };
        let request = Request::Apply {
            commands: vec!["c".repeat(64 * 1024)],
        };
        let (reply, _) = client.request(&request).expect("answered");
        assert_eq!(reply, Reply::Report { text: report });
        game.join().expect("answered");
    }
}
