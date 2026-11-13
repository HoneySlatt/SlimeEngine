//! A tool's end: it starts a game with a token, connects to it, and asks.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use uwui_agent::{
    AgentError, CORE_VERSION, Family, Fault, FromProgram, Hello, Limits, Named, Reader,
    ToolSession, Value, Welcome,
};

use crate::error::ChannelError;
use crate::locked;
use crate::message::{Event, Reply, Request};
use crate::token::Token;
use crate::wire::{MAX_BYTES, MAX_LINE, receive, send, why};
use crate::{ANNOUNCEMENT, TOKEN_ENV_VAR};

/// How long a request waits for a place in flight, then for the game's answer, before it gives up
/// and cancels it: a game answers at its next frame, which takes seconds on a CPU's renderer, and
/// a step of many fixed steps once the frames have run them all.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// What a request's answer is: the reply, and the bytes that follow it.
type Answer = Result<(Reply, Vec<u8>), ChannelError>;

/// An answer as the core's session read it: its outcome, and the bytes that follow it.
type Raw = Result<(Result<Value, Fault>, Vec<u8>), ChannelError>;

/// What a client tells its owner, on the thread that reads the connection
/// ([`Client::connect_with`]).
#[derive(Debug)]
pub enum Notice {
    /// An event that the game sent, of a subscription ([`Client::subscribe`]).
    Event(Event),
    /// The connection closed or broke, and why: told once, the last notice, as when the game
    /// ended or the client was dropped.
    Closed(ChannelError),
}

/// What a client's reader calls with each notice.
type Notify = Box<dyn Fn(Notice) + Send + Sync>;

/// A connection to a game's development channel, open once the game welcomed its hello: it carries
/// the tool's end of the session of uwui's core for agents (`uwui_agent::ToolSession`).
///
/// Several threads may ask through one client at once: each request is in flight until its answer
/// comes, in any order, and a thread of the client's own reads the answers and hands each to its
/// request (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
/// A request past those in flight that the game takes waits for an answer to be read.
pub struct Client {
    shared: Arc<Shared>,
    /// A clone of the connection, which the client shuts down when dropped, so that its reader
    /// ends.
    stream: TcpStream,
    reader: Option<JoinHandle<()>>,
    program: String,
    /// The events that the game declares, as its welcome listed them.
    events: Vec<String>,
}

/// What the client's callers and its reader share.
struct Shared {
    state: Mutex<State>,
    /// Told when an answer came, which frees a place in flight, or the connection broke.
    answered: Condvar,
    /// The connection, written under its own lock so that no line is cut by another, and so that
    /// the reader, which never takes it, reads on while a caller's write waits for the game.
    writer: Mutex<TcpStream>,
}

struct State {
    session: ToolSession,
    /// Where each request in flight waits for its answer, by number; a request given up is no
    /// longer here, and its answer, which still comes, is dropped.
    waiting: HashMap<u64, mpsc::Sender<Raw>>,
    /// Why the connection broke, once it did.
    broken: Option<Broken>,
}

/// Why a connection broke, which each request that waited is told.
#[derive(Clone)]
enum Broken {
    /// Reading or writing failed, of this kind, as the system said.
    Io(io::ErrorKind, String),
    /// The game sent what is not a message of the protocol, or said that it could not read the
    /// tool's.
    Malformed(String),
}

impl Broken {
    fn error(&self) -> ChannelError {
        match self {
            Broken::Io(kind, why) => ChannelError::Io(io::Error::new(*kind, why.clone())),
            Broken::Malformed(why) => ChannelError::Malformed(why.clone()),
        }
    }
}

/// A request in flight, whose answer comes later ([`Pending::wait`]).
pub struct Pending {
    id: u64,
    answer: mpsc::Receiver<Raw>,
    shared: Arc<Shared>,
}

impl Client {
    /// Connects to the game that listens at `address`, with `token`, for `agent`, whose requests
    /// `client` carries, an MCP server, as they name themselves: the game logs both. It speaks every family of
    /// [`FAMILIES`](crate::FAMILIES).
    ///
    /// # Errors
    ///
    /// [`ChannelError::Refused`] with the game's reason when it refuses the token or the version of
    /// the core, or serves as many connections as it takes; [`ChannelError::Io`] when no game
    /// listens there or the connection breaks; [`ChannelError::Malformed`] or
    /// [`ChannelError::Session`] when what answers does not speak the protocol.
    pub fn connect(
        address: SocketAddr,
        token: &Token,
        agent: &Named,
        client: &Named,
    ) -> Result<Self, ChannelError> {
        Self::connect_with(address, token, agent, client, |_| {})
    }

    /// Connects as [`Client::connect`] does, and tells `notify` each event that the game sends and
    /// the connection's end ([`Notice`]), on the thread that reads the connection: `notify` must
    /// not wait, as for an answer of this client, which that thread would read.
    ///
    /// # Errors
    ///
    /// As [`Client::connect`].
    pub fn connect_with(
        address: SocketAddr,
        token: &Token,
        agent: &Named,
        client: &Named,
        notify: impl Fn(Notice) + Send + Sync + 'static,
    ) -> Result<Self, ChannelError> {
        Self::connect_speaking(
            address,
            token,
            agent,
            client,
            crate::families(),
            Box::new(notify),
        )
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
        notify: Notify,
    ) -> Result<(Self, Welcome), ChannelError> {
        let mut stream = TcpStream::connect(address)?;
        // Each message goes out at once, rather than waiting for the acknowledgement of the last.
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        let limits = Limits {
            line: MAX_LINE,
            bytes: MAX_BYTES,
        };
        let mut reader = Reader::new(limits);
        let mut session = ToolSession::new();
        let hello = session.hello(&Hello {
            core: CORE_VERSION,
            agent: agent.clone(),
            client: client.clone(),
            secret: token.to_string(),
            families,
            limits,
        })?;
        send(&mut stream, &hello, &[])?;
        let frame = receive(&mut stream, &mut reader)?;
        let welcome = match session.receive(frame)? {
            FromProgram::Welcomed(welcome) => welcome,
            FromProgram::Refused(reason) => return Err(ChannelError::Refused(reason)),
            FromProgram::Unreadable(fault) => return Err(ChannelError::Refused(fault.message)),
            other => {
                return Err(ChannelError::Malformed(format!(
                    "the game answered {other:?} to a hello"
                )));
            }
        };
        // Events may not come for hours: each request keeps its own time instead.
        stream.set_read_timeout(None)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                session,
                waiting: HashMap::new(),
                broken: None,
            }),
            answered: Condvar::new(),
            writer: Mutex::new(stream.try_clone()?),
        });
        let reading = stream.try_clone()?;
        let reader = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name("slime dev channel client".to_owned())
                .spawn(move || read_answers(reading, reader, &shared, &notify))?
        };
        let program = welcome.program.clone();
        let events = welcome.events.clone();
        Ok((
            Self {
                shared,
                stream,
                reader: Some(reader),
                program,
                events,
            },
            welcome,
        ))
    }

    /// The name of the game's program.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The events that the game declares, which [`Client::subscribe`] may name: `play/time`; none
    /// for a game of before them.
    pub fn events(&self) -> &[String] {
        &self.events
    }

    /// Subscribes to `events`, each `family/name`, and waits for the answer, within the time that a
    /// request has: then each event comes, its current value first, to the notices given at
    /// [`Client::connect_with`].
    ///
    /// # Errors
    ///
    /// [`ChannelError::Session`] for an event that the game does not declare, which sends
    /// nothing; [`ChannelError::Refused`] when the game refuses it, as once it stopped; and as
    /// [`Client::send`] and [`Pending::wait`].
    pub fn subscribe(&self, events: &[&str]) -> Result<(), ChannelError> {
        subscribed(
            self.send_with(|session| session.subscribe(events))?
                .wait_raw(ANSWER_TIMEOUT),
        )
    }

    /// Ends the subscriptions to `events`, and waits for the answer: none of them comes after it.
    ///
    /// # Errors
    ///
    /// As [`Client::subscribe`].
    pub fn unsubscribe(&self, events: &[&str]) -> Result<(), ChannelError> {
        subscribed(
            self.send_with(|session| session.unsubscribe(events))?
                .wait_raw(ANSWER_TIMEOUT),
        )
    }

    /// Whether the connection is given up: it broke, or the game sent what is not a message of
    /// the protocol. Every request fails with [`ChannelError::Broken`] then.
    pub fn is_broken(&self) -> bool {
        locked(&self.shared.state).broken.is_some()
    }

    /// Sends `request`, and waits for the game's answer: the reply, and the bytes that follow it,
    /// the pixels of an image. A request holds at most 1 MiB; an answer at most 16 MiB, and its
    /// bytes 1 GiB, past which the game answers that it is too large. A request that finds no
    /// place in flight, or no answer, within ten minutes, fails then, and is cancelled.
    ///
    /// # Errors
    ///
    /// As [`Client::send`], and [`Pending::wait`]. A request that the game refuses is answered
    /// [`Reply::Failed`], which is no error.
    pub fn request(&self, request: &Request) -> Answer {
        self.send(request)?.wait()
    }

    /// Sends `request` without waiting for its answer: it is in flight until [`Pending::wait`]
    /// reads it. It waits first, at most ten minutes, while the requests in flight are as many as
    /// the game takes.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Session`] when the game does not serve the request's family on this
    /// connection, or the request is larger than 1 MiB, which sends nothing and leaves the
    /// connection usable; [`ChannelError::Io`] of the kind `TimedOut` when no place freed in time,
    /// which sends nothing either; [`ChannelError::Broken`] once the connection broke;
    /// [`ChannelError::Io`] when it breaks now.
    pub fn send(&self, request: &Request) -> Result<Pending, ChannelError> {
        let (method, params) = request.call()?;
        self.send_with(|session| session.request(&method, params.clone()))
    }

    /// Sends the request that `numbered` makes of the session, once a place in flight is free, as
    /// [`Client::send`] says.
    fn send_with(
        &self,
        mut numbered: impl FnMut(&mut ToolSession) -> Result<(u64, Vec<u8>), AgentError>,
    ) -> Result<Pending, ChannelError> {
        let until = Instant::now() + ANSWER_TIMEOUT;
        let mut state = locked(&self.shared.state);
        let (id, line) = loop {
            if state.broken.is_some() {
                return Err(ChannelError::Broken);
            }
            match numbered(&mut state.session) {
                Err(AgentError::Full) => {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::Error::from(io::ErrorKind::TimedOut).into());
                    }
                    state = self
                        .shared
                        .answered
                        .wait_timeout(state, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
                sent => break sent?,
            }
        };
        let (answer, answered) = mpsc::channel();
        state.waiting.insert(id, answer);
        drop(state);
        let written = send(&mut *locked(&self.shared.writer), &line, &[]);
        if let Err(error) = written {
            let mut state = locked(&self.shared.state);
            state.waiting.remove(&id);
            state.broken = Some(Broken::Io(error.kind(), error.to_string()));
            self.shared.answered.notify_all();
            return Err(error.into());
        }
        Ok(Pending {
            id,
            answer: answered,
            shared: Arc::clone(&self.shared),
        })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // The reader, blocked on the socket, ends.
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Pending {
    /// The request's number.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Waits for the game's answer, at most ten minutes, after which the request is cancelled.
    ///
    /// # Errors
    ///
    /// [`ChannelError::Io`] when the connection breaks, with the system's reason, or the game does
    /// not answer in time, of the kind `TimedOut`; [`ChannelError::Malformed`] when the game sent
    /// what is not a message of the protocol, after which the connection is given up and every
    /// later request fails with [`ChannelError::Broken`], or when this answer alone is not a reply
    /// of the channel, the connection kept; [`ChannelError::Cancelled`] for a request cancelled
    /// before the game did it.
    pub fn wait(self) -> Answer {
        self.wait_for(ANSWER_TIMEOUT)
    }

    /// Waits for the game's answer as [`Pending::wait`] does, at most `timeout`; past it, it cancels
    /// the request unless another thread is writing on the connection.
    ///
    /// # Errors
    ///
    /// As [`Pending::wait`].
    pub fn wait_for(self, timeout: Duration) -> Answer {
        self.wait_raw(timeout)
            .and_then(|(outcome, bytes)| read_reply(outcome).map(|reply| (reply, bytes)))
    }

    /// Waits for the answer as the core's session read it, as [`Pending::wait_for`] does.
    fn wait_raw(self, timeout: Duration) -> Raw {
        match self.answer.recv_timeout(timeout) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Given up: an answer that came meanwhile is taken still, and one that comes later
                // dropped.
                locked(&self.shared.state).waiting.remove(&self.id);
                if let Ok(answer) = self.answer.try_recv() {
                    return answer;
                }
                // Not waiting for another thread's write, which may be blocked on a game that
                // reads nothing: the cancel is given up then, and the answer dropped when it comes.
                if let Ok(mut writer) = self.shared.writer.try_lock() {
                    self.send_cancel(&mut writer);
                }
                Err(io::Error::from(io::ErrorKind::TimedOut).into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ChannelError::Broken),
        }
    }

    /// Asks the game to cancel the request: it still answers it, with its result if it had changed
    /// something, [`ChannelError::Cancelled`] if it had not. Nothing is sent once the answer came,
    /// nor on a connection broken.
    pub fn cancel(&self) {
        self.send_cancel(&mut locked(&self.shared.writer));
    }

    /// Sends the cancel on `writer`, the connection, locked.
    fn send_cancel(&self, writer: &mut TcpStream) {
        let line = {
            let state = locked(&self.shared.state);
            if state.broken.is_some() {
                return;
            }
            match state.session.cancel(self.id) {
                Ok(Some(line)) => line,
                _ => return,
            }
        };
        if let Err(error) = send(writer, &line, &[]) {
            locked(&self.shared.state).broken = Some(Broken::Io(error.kind(), error.to_string()));
            self.shared.answered.notify_all();
        }
    }
}

/// Reads the game's answers until the connection closes or breaks, and hands each to the request
/// that waits for it, and each event to `notify`, then the connection's end. It never takes the
/// writer's lock, nor calls `notify` while it holds the client's.
fn read_answers(mut stream: TcpStream, mut reader: Reader, shared: &Shared, notify: &Notify) {
    let broken = loop {
        let frame = match receive(&mut stream, &mut reader) {
            Ok(frame) => frame,
            Err(ChannelError::Io(error)) => break Broken::Io(error.kind(), error.to_string()),
            Err(error) => break Broken::Malformed(why(&error)),
        };
        let mut state = locked(&shared.state);
        let (id, answer) = match state.session.receive(frame) {
            Ok(FromProgram::Response { id, outcome, bytes }) => (id, Ok((outcome, bytes))),
            Ok(FromProgram::Event { method, params, .. }) => {
                drop(state);
                // An event of a later version of its family is not one of this client's.
                if let Ok(event) = Event::read(&method, params) {
                    notify(Notice::Event(event));
                }
                continue;
            }
            Ok(FromProgram::Unreadable(fault)) => break Broken::Malformed(fault.message),
            Ok(other) => break Broken::Malformed(format!("the game answered {other:?}")),
            Err(error) => break Broken::Malformed(why(&error)),
        };
        if let Some(waiting) = state.waiting.remove(&id) {
            let _ = waiting.send(answer);
        }
        shared.answered.notify_all();
    };
    {
        let mut state = locked(&shared.state);
        for (_, waiting) in state.waiting.drain() {
            let _ = waiting.send(Err(broken.error()));
        }
        state.broken = Some(broken.clone());
        shared.answered.notify_all();
    }
    notify(Notice::Closed(broken.error()));
}

/// The outcome of a subscription's answer, or of its end's.
fn subscribed(answer: Raw) -> Result<(), ChannelError> {
    match answer?.0 {
        Ok(_) => Ok(()),
        Err(fault) if fault.code == Fault::CANCELLED => Err(ChannelError::Cancelled),
        Err(fault) => Err(ChannelError::Refused(fault.message)),
    }
}

/// The reply of an answer's outcome: a request cancelled before the game did it is
/// [`ChannelError::Cancelled`].
fn read_reply(outcome: Result<Value, Fault>) -> Result<Reply, ChannelError> {
    match outcome {
        Err(fault) if fault.code == Fault::CANCELLED => Err(ChannelError::Cancelled),
        outcome => Reply::read(outcome),
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
