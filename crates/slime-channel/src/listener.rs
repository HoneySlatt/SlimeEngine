//! The game's end: it listens, checks each connection, and hands the requests to its host.

use std::collections::{BTreeSet, VecDeque};
use std::io::{self, Read};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use uwui_agent::{Fault, FromTool, InFlight, Limits, ProgramSession, Reader};

use crate::error::ChannelError;
use crate::message::{Reply, Request};
use crate::token::Token;
use crate::wire::{MAX_REQUEST, receive, send, why};
use crate::{locked, waited};

/// How long a connection has to send its whole first message: one that does not is closed then,
/// and its thread ends.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// How many connections may wait for their first message at once: more are closed unread, so that
/// who has no token cannot make the game start threads without end.
const MAX_WAITING: usize = 16;
/// How many connections the game serves at once: a hello past them is refused, with the reason.
/// The editor's Play holds one to the game it plays, and an MCP server one to each program
/// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
const MAX_WELCOMED: usize = 8;
/// The most bytes that a connection's answers and refusals hold unwritten before the game takes
/// none of its requests, and reads none, until its writer wrote: a tool that stops reading stops
/// being served. What the game took already is answered still, one answer larger than this alone
/// among them.
const MAX_UNWRITTEN: u64 = 64 << 20;
/// The most bytes that every connection's answers hold unwritten together before the game takes
/// requests only of the connections that hold none: those that read what they are sent are served
/// on.
const MAX_UNWRITTEN_ALL: u64 = 256 << 20;
/// How long the thread that accepts connections waits after the system failed to give one, as when
/// the process has no file left: the failure would come back at once otherwise.
const ACCEPT_PAUSE: Duration = Duration::from_millis(100);
/// What a request that reaches no game is answered.
const STOPPED: &str = "the game has stopped";

/// A game's end of its development channel: it listens on the machine's own address, on a port
/// that the system chooses, accepts the connections whose hello gives its token and speaks its
/// version of the core, serves them the [`FAMILIES`](crate::FAMILIES) that they speak at the same
/// versions, and hands their requests to the game, which answers each
/// (`docs/adr/0110-how-the-editor-runs-a-games-code.md`,
/// `docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md`).
///
/// It carries the session of uwui's core for agents (`uwui_agent::ProgramSession`): a thread
/// accepts the connections, and each connection has a thread that reads its requests and one that
/// writes its answers, threads that wait rather than compute, as the engine allows threads of their
/// own (`docs/adr/0107-the-execution-model.md`). A connection may have
/// [`InFlight::DEFAULT`] requests in flight; the game takes them in their order, one of each
/// connection in turn ([`Listener::round`]), and answers each when it is done, in any order
/// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
/// The game's thread never writes to a socket, waits on one, nor waits for a lock that a thread
/// holds while it does: it leaves each answer to the connection's writer.
pub struct Listener {
    address: SocketAddr,
    shared: Arc<Shared>,
    /// Where the next round starts, so that no connection is always first.
    turn: AtomicUsize,
}

/// What the connections' threads and the game share.
struct Shared {
    /// The connections welcomed and not yet closed, in the order they came.
    connections: Mutex<Vec<Arc<Connection>>>,
    /// Whether the game dropped its listener: a request is answered at once then.
    stopped: AtomicBool,
    /// How many connections are welcomed.
    welcomed: Arc<AtomicUsize>,
    /// The bytes unwritten, every connection together.
    unwritten: AtomicU64,
    /// Wakes the game: a request came or was cancelled, a connection closed, or one whose writer
    /// fell behind caught up.
    wake: Box<dyn Fn() + Send + Sync>,
}

/// A connection welcomed: its session, the requests that wait for the game, the answers that wait
/// for its writer.
struct Connection {
    /// Its session, which its reader and its writer lock, the game never.
    session: Mutex<ProgramSession>,
    /// What its threads and the game change, under a lock that none holds while it reads or writes
    /// the socket, encodes or decodes.
    state: Mutex<State>,
    /// Told each time its state changes in a way that a thread waits for: an answer left to write,
    /// bytes written, the connection closed.
    changed: Condvar,
    /// The bytes unwritten on this connection.
    unwritten: AtomicU64,
}

#[derive(Default)]
struct State {
    /// The requests that the game has not taken yet, in their order.
    inbox: VecDeque<(u64, Request)>,
    /// What waits for the writer, in its order.
    outbox: VecDeque<Outgoing>,
    /// The requests in flight that the tool cancelled, until their answers leave for the writer.
    cancelled: BTreeSet<u64>,
    /// Whether the connection closed: its requests are cancelled, and nothing more is written.
    closed: bool,
    /// Whether the reader ended on a line that is not a message: the game does none of its requests
    /// left, nothing more is posted, and the writer writes what it was left, the reason last, then
    /// ends.
    finishing: bool,
}

/// What a connection's writer writes.
enum Outgoing {
    /// A line that the session wrote itself, of no request in flight.
    Line(Vec<u8>),
    /// The answer to the request in flight numbered `id`, the bytes that follow it, and what both
    /// hold unwritten.
    Answer {
        id: u64,
        outcome: Result<Reply, Fault>,
        payload: Arc<Vec<u8>>,
        size: u64,
    },
}

impl Outgoing {
    /// The answer to the request `id`: `outcome` and `payload`, counted as the bytes of the JSON
    /// that it writes and those that follow it.
    fn answer(id: u64, outcome: Result<Reply, Fault>, payload: Arc<Vec<u8>>) -> Self {
        let line = match &outcome {
            Ok(reply) => uwui_agent::json_size(reply),
            Err(fault) => fault.message.len(),
        };
        let size = (line + payload.len()) as u64;
        Outgoing::Answer {
            id,
            outcome,
            payload,
            size,
        }
    }

    /// The bytes that it holds unwritten: a line, or an answer's JSON and the bytes after it.
    fn size(&self) -> u64 {
        match self {
            Outgoing::Line(line) => line.len() as u64,
            Outgoing::Answer { size, .. } => *size,
        }
    }
}

/// A request that a connection made, which the game answers once ([`Incoming::answer`]); one
/// dropped unanswered is answered that the game has stopped.
pub struct Incoming {
    request: Request,
    id: u64,
    connection: Arc<Connection>,
    shared: Arc<Shared>,
    answered: bool,
}

impl Listener {
    /// Listens on `127.0.0.1`, on a port that the system chooses, for connections whose hello
    /// gives `token` and speaks `uwui_agent::CORE_VERSION`; `program` names the game to those
    /// welcomed. A connection sends its hello, of at most 4 KiB, within 5 seconds, or is refused;
    /// one refused is told why, and closed. At most 16 connections wait for their hello at once:
    /// more are closed unread. At most 8 are welcomed at once: a hello with the token past them is
    /// refused, with the reason. The game logs the agent of each connection welcomed, and the
    /// program that carries it, as they name themselves.
    ///
    /// A connection welcomed may have [`InFlight::DEFAULT`] requests in flight, 16, of 1 MiB of
    /// lines; a request past them is answered `BUSY`, the connection kept. A request's place frees
    /// once the connection's writer takes its answer. A request holds at most 1 MiB, and announces
    /// no bytes; a line that is not a message, or past that, closes its connection, after what was
    /// left to write and the reason. Once 64 MiB of what a connection is sent wait to be written,
    /// the game takes and reads none of its requests until its writer wrote; once 256 MiB of every
    /// connection's do, it takes requests only of the connections that hold nothing unwritten. A
    /// connection that closes cancels its requests in flight.
    ///
    /// `wake` is called, on a thread of the connection, each time a request is queued or
    /// cancelled, when a connection closes, and when a connection whose writer fell behind
    /// catches up: a game that waits for something to do runs a frame then, which takes it.
    ///
    /// # Errors
    ///
    /// When the system gives no port to listen on, or no thread to accept the connections.
    pub fn open(
        token: Token,
        program: &str,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let shared = Arc::new(Shared {
            connections: Mutex::new(Vec::new()),
            stopped: AtomicBool::new(false),
            welcomed: Arc::new(AtomicUsize::new(0)),
            unwritten: AtomicU64::new(0),
            wake: Box::new(wake),
        });
        let program = program.to_owned();
        let waiting = Arc::new(AtomicUsize::new(0));
        let accepting = Arc::clone(&shared);
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
                    let waits = Counted(Arc::clone(&waiting));
                    let (token, program, shared) =
                        (token.clone(), program.clone(), Arc::clone(&accepting));
                    let spawned = thread::Builder::new()
                        .name("slime dev channel connection".to_owned())
                        .spawn(move || serve(stream, waits, &token, &program, &shared));
                    if let Err(error) = spawned {
                        tracing::warn!(%error, "a connection to the development channel is dropped");
                    }
                }
            })?;
        Ok(Self {
            address,
            shared,
            turn: AtomicUsize::new(0),
        })
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The next request that a connection made, if one waits, without waiting for one: the
    /// connections take turns, as [`Listener::round`]'s.
    pub fn next(&self) -> Option<Incoming> {
        self.take(1).pop()
    }

    /// A request of each connection that has one waiting, the oldest of each, without waiting for
    /// one; empty when none waits. The first connection of a round is the one after the first of
    /// the last, so that each comes first in turn. A connection whose writer is behind gives none
    /// ([`Listener::open`]). A request that its tool cancelled before the game took it is answered
    /// so, and not given.
    pub fn round(&self) -> Vec<Incoming> {
        self.take(usize::MAX)
    }

    /// Whether a request waits for the game to take it, of a connection that gives one now.
    pub fn waiting(&self) -> bool {
        locked(&self.shared.connections).iter().any(|connection| {
            !self.shared.behind(connection) && !locked(&connection.state).inbox.is_empty()
        })
    }

    /// At most `most` requests, one of each connection, starting from the connection whose turn it
    /// is.
    fn take(&self, most: usize) -> Vec<Incoming> {
        let mut connections = locked(&self.shared.connections);
        connections.retain(|connection| !locked(&connection.state).closed);
        let count = connections.len();
        let first = self.turn.load(Ordering::Relaxed);
        let mut taken = Vec::new();
        for at in 0..count {
            if taken.len() == most {
                break;
            }
            let connection = &connections[(first + at) % count];
            if self.shared.behind(connection) {
                continue;
            }
            let mut state = locked(&connection.state);
            // After a line that is not a message, nothing more is done.
            if state.finishing {
                state.inbox.clear();
                continue;
            }
            while let Some((id, request)) = state.inbox.pop_front() {
                if state.cancelled.contains(&id) {
                    post(
                        connection,
                        &self.shared,
                        &mut state,
                        Outgoing::answer(id, Err(cancelled()), Arc::default()),
                    );
                    continue;
                }
                if taken.is_empty() {
                    self.turn.store((first + at + 1) % count, Ordering::Relaxed);
                }
                taken.push(Incoming {
                    request,
                    id,
                    connection: Arc::clone(connection),
                    shared: Arc::clone(&self.shared),
                    answered: false,
                });
                break;
            }
        }
        taken
    }
}

impl Shared {
    /// Whether `connection`'s writer is too far behind for the game to take its requests.
    fn behind(&self, connection: &Connection) -> bool {
        let own = connection.unwritten.load(Ordering::Relaxed);
        own > MAX_UNWRITTEN
            || (own > 0 && self.unwritten.load(Ordering::Relaxed) > MAX_UNWRITTEN_ALL)
    }

    /// Counts `bytes` of `connection` written or dropped, and wakes the game when a connection may
    /// give requests again.
    fn release(&self, connection: &Connection, bytes: u64) {
        let own = connection.unwritten.fetch_sub(bytes, Ordering::Relaxed);
        let all = self.unwritten.fetch_sub(bytes, Ordering::Relaxed);
        // Behind no longer: past its own bound before, or past the program's while it held
        // something, which it no longer does.
        let caught_up = (own > MAX_UNWRITTEN && own - bytes <= MAX_UNWRITTEN)
            || (all > MAX_UNWRITTEN_ALL && all - bytes <= MAX_UNWRITTEN_ALL)
            || (own > 0 && own == bytes && all > MAX_UNWRITTEN_ALL);
        if caught_up {
            (self.wake)();
        }
    }
}

impl Drop for Listener {
    /// Answers every request that the game did not take that it has stopped, and every one that
    /// comes later.
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Relaxed);
        for connection in locked(&self.shared.connections).iter() {
            let mut state = locked(&connection.state);
            while let Some((id, _)) = state.inbox.pop_front() {
                post(
                    connection,
                    &self.shared,
                    &mut state,
                    Outgoing::answer(id, Ok(stopped()), Arc::default()),
                );
            }
        }
    }
}

/// The fault of a request that its tool cancelled before it changed anything.
fn cancelled() -> Fault {
    Fault::new(
        Fault::CANCELLED,
        "the tool cancelled the request before the game did it",
    )
}

fn stopped() -> Reply {
    Reply::Failed {
        message: STOPPED.to_owned(),
    }
}

/// Leaves `outgoing` for `connection`'s writer, `state` being its own, locked; dropped when the
/// connection closed, where nothing waits for it, or once its reader finished, so that the reason
/// is written last.
fn post(connection: &Connection, shared: &Shared, state: &mut State, outgoing: Outgoing) {
    if state.closed || state.finishing {
        return;
    }
    let size = outgoing.size();
    connection.unwritten.fetch_add(size, Ordering::Relaxed);
    shared.unwritten.fetch_add(size, Ordering::Relaxed);
    state.outbox.push_back(outgoing);
    connection.changed.notify_all();
}

impl Incoming {
    /// The request.
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Whether its tool cancelled it, or closed its connection: a request that has changed nothing
    /// yet is then answered [`Incoming::cancel`], and never done
    /// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
    pub fn cancelled(&self) -> bool {
        let state = locked(&self.connection.state);
        state.closed || state.finishing || state.cancelled.contains(&self.id)
    }

    /// Answers the request with `reply`.
    pub fn answer(self, reply: Reply) {
        self.answer_with(reply, Arc::default());
    }

    /// Answers the request with `reply`, and `payload` after it, as [`Reply::Image`] has its
    /// pixels: shared, so that the captures of one frame hold its image once.
    pub fn answer_with(mut self, reply: Reply, payload: Arc<Vec<u8>>) {
        self.finish(Ok(reply), payload);
    }

    /// Answers the request that its tool cancelled before it changed anything with the fault
    /// `CANCELLED`, without doing it.
    pub fn cancel(mut self) {
        self.finish(Err(cancelled()), Arc::default());
    }

    fn finish(&mut self, outcome: Result<Reply, Fault>, payload: Arc<Vec<u8>>) {
        self.answered = true;
        let mut state = locked(&self.connection.state);
        post(
            &self.connection,
            &self.shared,
            &mut state,
            Outgoing::answer(self.id, outcome, payload),
        );
    }
}

impl Drop for Incoming {
    fn drop(&mut self) {
        if !self.answered {
            self.finish(Ok(stopped()), Arc::default());
        }
    }
}

/// Counts a connection among others while it lives: those that wait for their hello, those
/// welcomed.
struct Counted(Arc<AtomicUsize>);

impl Drop for Counted {
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

/// Serves a connection: welcomes or refuses its hello, then reads its requests and hands them to
/// the game, while a writer of its own writes the answers, until the connection or the game ends.
/// `waits` counts it among the connections that wait for their first message, until it came.
fn serve(stream: TcpStream, waits: Counted, token: &Token, program: &str, shared: &Arc<Shared>) {
    let mut session = ProgramSession::new(program, crate::families(), InFlight::DEFAULT);
    let mut reader = Reader::new(Limits::HELLO);
    let mut stream = stream;
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
    let frame = match first {
        Ok(frame) => frame,
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
    let Some((place, welcomed)) = welcome(&mut stream, &mut session, frame, token, shared) else {
        return;
    };
    reader.set_limits(Limits {
        line: MAX_REQUEST,
        bytes: 0,
    });
    let Ok(writing) = stream.try_clone() else {
        return;
    };
    let connection = Arc::new(Connection {
        session: Mutex::new(session),
        state: Mutex::new(State::default()),
        changed: Condvar::new(),
        unwritten: AtomicU64::new(0),
    });
    // Among the game's connections before the tool reads its welcome, which the writer writes
    // first: a request that follows it finds its connection there.
    {
        let mut state = locked(&connection.state);
        post(&connection, shared, &mut state, Outgoing::Line(welcomed));
    }
    locked(&shared.connections).push(Arc::clone(&connection));
    let writer = {
        let (connection, shared) = (Arc::clone(&connection), Arc::clone(shared));
        thread::Builder::new()
            .name("slime dev channel writer".to_owned())
            .spawn(move || write_answers(writing, &connection, &shared))
    };
    let mut writer = writer.ok();
    if writer.is_some() {
        read_requests(&mut stream, &mut reader, &connection, shared);
    }
    // After a line that is not a message, the writer writes what it was left and the reason, then
    // ends, or ends once the tool is gone.
    if locked(&connection.state).finishing
        && let Some(writer) = writer.take()
    {
        let _ = writer.join();
    }
    // The tool closed or broke the connection: its requests are cancelled, and the game told, so
    // that what waits for a frame for them is dropped.
    let (inbox, outbox) = {
        let mut state = locked(&connection.state);
        state.closed = true;
        connection.changed.notify_all();
        (
            std::mem::take(&mut state.inbox),
            std::mem::take(&mut state.outbox),
        )
    };
    // Dropped out of the lock that the game's thread takes: an answer may hold a frame's image.
    drop(inbox);
    shared.release(&connection, outbox.iter().map(Outgoing::size).sum());
    drop(outbox);
    // A writer blocked on the socket ends with it.
    let _ = stream.shutdown(Shutdown::Both);
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    (shared.wake)();
    drop(place);
}

/// Reads the hello `frame`: the connection's place among those welcomed and the welcome to send,
/// or `None` once it is refused and told why. Who has not the token learns nothing of the others.
fn welcome(
    stream: &mut TcpStream,
    session: &mut ProgramSession,
    frame: uwui_agent::Frame,
    token: &Token,
    shared: &Shared,
) -> Option<(Counted, Vec<u8>)> {
    let id = match &frame.message {
        uwui_agent::Message::Request { id, .. } => Some(*id),
        _ => None,
    };
    match session.receive(frame, |secret| token.matches(secret)) {
        Ok(FromTool::Welcomed {
            agent,
            client,
            reply,
        }) => {
            if shared.welcomed.fetch_add(1, Ordering::Relaxed) >= MAX_WELCOMED {
                shared.welcomed.fetch_sub(1, Ordering::Relaxed);
                let reason = format!(
                    "the game serves {MAX_WELCOMED} connections already: close one, then connect \
                     again"
                );
                tracing::warn!("a connection to the development channel is refused: {reason}");
                if let Ok(reply) = session.refuse(id, &reason) {
                    let _ = send(stream, &reply, &[]);
                }
                return None;
            }
            let place = Counted(Arc::clone(&shared.welcomed));
            // Escaped, as fields: what a connection says it is must not make lines of the log.
            tracing::info!(
                agent = ?agent.to_string(),
                client = ?client.to_string(),
                "an agent connected to the development channel"
            );
            Some((place, reply))
        }
        Ok(FromTool::Refused { reason, reply }) => {
            tracing::warn!("a connection to the development channel is refused: {reason:?}");
            let _ = send(stream, &reply, &[]);
            None
        }
        _ => None,
    }
}

/// Reads the connection's requests until it closes: hands each one that reads to the game, answers
/// itself what the session answers, and marks what the tool cancels. While what it was sent and did
/// not write passes its bound, it reads nothing, and handles none of what it read.
fn read_requests(
    stream: &mut TcpStream,
    reader: &mut Reader,
    connection: &Connection,
    shared: &Shared,
) {
    loop {
        if !wait_for_room(connection) {
            return;
        }
        let frame = match receive(stream, reader) {
            Ok(frame) => frame,
            // The tool closed the connection, or broke it, or its writer did.
            Err(ChannelError::Io(_)) => return,
            // A line that is not a message loses the framing: the next one cannot be found, and
            // the connection is closed, with the reason, once what was left to write is written.
            Err(error) => {
                let reason = why(&error);
                tracing::warn!("a connection to the development channel is closed: {reason:?}");
                let line = locked(&connection.session).unreadable(&reason);
                let mut state = locked(&connection.state);
                if let Ok(line) = line {
                    post(connection, shared, &mut state, Outgoing::Line(line));
                }
                state.finishing = true;
                connection.changed.notify_all();
                return;
            }
        };
        // A reader blocked on the socket while the bytes unwritten passed their bound holds the
        // one message it read until they no longer do.
        if !wait_for_room(connection) {
            return;
        }
        let received = locked(&connection.session).receive(frame, |_| false);
        match received {
            Ok(FromTool::Request {
                id,
                family,
                name,
                params,
                ..
            }) => {
                // Read into its type before the lock, which the game's thread takes.
                let request = Request::read(&family, &format!("{family}/{name}"), params);
                let mut state = locked(&connection.state);
                let answer = |outcome| Outgoing::answer(id, outcome, Arc::default());
                match request {
                    Err(fault) => post(connection, shared, &mut state, answer(Err(fault))),
                    Ok(_) if shared.stopped.load(Ordering::Relaxed) => {
                        post(connection, shared, &mut state, answer(Ok(stopped())));
                    }
                    Ok(request) => {
                        state.inbox.push_back((id, request));
                        drop(state);
                        (shared.wake)();
                    }
                }
            }
            Ok(FromTool::Answered { reply }) => {
                let mut state = locked(&connection.state);
                post(connection, shared, &mut state, Outgoing::Line(reply));
            }
            Ok(FromTool::Cancel { id }) => {
                // A cancel read just as its answer leaves for the writer may stay here, the request
                // no longer in flight: harmless, as the core's tool never uses a number twice
                // (`docs/debt.md`).
                locked(&connection.state).cancelled.insert(id);
                (shared.wake)();
            }
            Ok(_) => {}
            Err(_) => return,
        }
    }
}

/// Waits while what the connection was sent and did not write passes its bound: one answer larger
/// is written alone, and the reading goes on once it is. Whether the connection is still open.
fn wait_for_room(connection: &Connection) -> bool {
    let mut state = locked(&connection.state);
    while connection.unwritten.load(Ordering::Relaxed) > MAX_UNWRITTEN && !state.closed {
        state = waited(&connection.changed, state);
    }
    !state.closed
}

/// Writes the connection's answers in the order they were left, each answer's line then its
/// bytes, until the connection closes, a write fails, or the reader finished and nothing is left;
/// frees each request's place once it takes its answer.
fn write_answers(mut stream: TcpStream, connection: &Connection, shared: &Shared) {
    loop {
        let outgoing = {
            let mut state = locked(&connection.state);
            loop {
                if state.closed {
                    return;
                }
                if let Some(outgoing) = state.outbox.pop_front() {
                    if let Outgoing::Answer { id, .. } = &outgoing {
                        state.cancelled.remove(id);
                    }
                    break outgoing;
                }
                if state.finishing {
                    return;
                }
                state = waited(&connection.changed, state);
            }
        };
        let size = outgoing.size();
        let written = match outgoing {
            Outgoing::Line(line) => send(&mut stream, &line, &[]),
            Outgoing::Answer {
                id,
                outcome,
                payload,
                ..
            } => {
                let outcome = match outcome.map(|reply| reply.outcome()) {
                    Ok(Ok(outcome)) => outcome,
                    Ok(Err(error)) => Err(Fault::new(Fault::REFUSED, why(&error))),
                    Err(fault) => Err(fault),
                };
                // Its place freed before the tool can read the answer, so that a tool that sends
                // again as soon as it read one is never refused.
                let answer = {
                    let mut session = locked(&connection.session);
                    session.written(id);
                    session.answer(id, outcome, payload.len())
                };
                answer
                    .map_err(io::Error::other)
                    .and_then(|(line, follow)| send(&mut stream, &line, &payload[..follow]))
            }
        };
        shared.release(connection, size);
        {
            let mut state = locked(&connection.state);
            if written.is_err() {
                state.closed = true;
            }
            connection.changed.notify_all();
        }
        if written.is_err() {
            // The reader, blocked on the socket, ends with it.
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
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
    use std::collections::VecDeque;
    use std::io::Write;

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
            limits: Limits {
                line: MAX_LINE,
                bytes: MAX_BYTES,
            },
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
        let (client, welcome) =
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
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let stop = Arc::new(AtomicBool::new(false));
        // The game's side: answers each request at once, with a payload as a capture has.
        let answering = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.next() {
                        Some(incoming) => incoming.answer_with(Reply::Done, Arc::new(vec![7; 64])),
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
        let client =
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
        let client =
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

    /// The next request that `listener` gives, waiting at most ten seconds for one.
    fn taken(listener: &Listener) -> Incoming {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(incoming) = listener.next() {
                return incoming;
            }
            assert!(Instant::now() < until, "a request reached the game");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Waits at most ten seconds for `holds` to hold.
    fn eventually(what: &str, holds: impl Fn() -> bool) {
        let until = Instant::now() + Duration::from_secs(10);
        while !holds() {
            assert!(Instant::now() < until, "{what}");
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn opened() -> (Listener, Token) {
        let token = Token::generate().expect("random bits");
        let listener = Listener::open(token.clone(), "test", || {}).expect("a port");
        (listener, token)
    }

    #[test]
    fn two_requests_of_a_connection_are_answered_out_of_their_order() {
        let (listener, token) = opened();
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let first = client.send(&Request::Pause).expect("sent");
        let second = client.send(&Request::State).expect("sent");
        let (held, asked) = (taken(&listener), taken(&listener));
        assert_eq!(
            (held.request(), asked.request()),
            (&Request::Pause, &Request::State),
            "taken in their order"
        );
        // The second answered while the first is held.
        asked.answer(Reply::Done);
        let (reply, _) = second
            .wait_for(Duration::from_secs(10))
            .expect("the second's answer, first");
        assert_eq!(reply, Reply::Done);
        held.answer(Reply::Failed {
            message: "held".to_owned(),
        });
        let (reply, _) = first.wait_for(Duration::from_secs(10)).expect("answered");
        assert!(matches!(reply, Reply::Failed { .. }));
    }

    #[test]
    fn a_request_cancelled_before_or_while_the_game_holds_it_is_answered_so() {
        let (listener, token) = opened();
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        // Cancelled before the game took it: answered so, and never given to the game.
        let pending = client.send(&Request::Pause).expect("sent");
        let id = pending.id();
        let connection = || Arc::clone(&locked(&listener.shared.connections)[0]);
        eventually("the request reached its connection's queue", || {
            locked(&connection().state).inbox.len() == 1
        });
        pending.cancel();
        eventually("the cancel was read", || {
            locked(&connection().state).cancelled.contains(&id)
        });
        assert!(listener.round().is_empty(), "the game never sees it");
        assert!(matches!(
            pending.wait_for(Duration::from_secs(10)),
            Err(ChannelError::Cancelled)
        ));
        // Cancelled while the game holds it, as an edit that waits for a drag's end.
        let pending = client.send(&Request::Pause).expect("sent");
        let held = taken(&listener);
        assert!(!held.cancelled());
        pending.cancel();
        eventually("the game sees it cancelled", || held.cancelled());
        held.cancel();
        assert!(matches!(
            pending.wait_for(Duration::from_secs(10)),
            Err(ChannelError::Cancelled)
        ));
        // The connection serves on.
        let pending = client.send(&Request::State).expect("sent");
        taken(&listener).answer(Reply::Done);
        assert_eq!(
            pending
                .wait_for(Duration::from_secs(10))
                .expect("answered")
                .0,
            Reply::Done
        );
    }

    #[test]
    fn a_connection_that_closes_cancels_what_the_game_holds_of_it() {
        let (listener, token) = opened();
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let _pending = client.send(&Request::Pause).expect("sent");
        let held = taken(&listener);
        assert!(!held.cancelled());
        drop(client);
        eventually("the game sees it cancelled", || held.cancelled());
    }

    /// Opens a connection by hand with `token`, welcomed, that reads nothing more.
    fn deaf(listener: &Listener, token: &Token) -> TcpStream {
        let mut stream = TcpStream::connect(listener.address()).expect("listening");
        let mut session = ToolSession::new();
        stream
            .write_all(&session.hello(&hello(token)).expect("written"))
            .expect("sent");
        assert!(matches!(
            read(&mut stream, &mut session),
            FromProgram::Welcomed(_)
        ));
        stream
    }

    /// The line of the request `id` to `play/state`, written by hand.
    fn state_line(id: u64) -> Vec<u8> {
        encode(
            &Message::Request {
                id,
                method: "play/state".to_owned(),
                params: uwui_agent::Value::Null,
            },
            0,
        )
        .expect("written")
    }

    #[test]
    fn eight_connections_are_welcomed_and_the_ninth_refused_with_the_reason() {
        let (listener, token) = opened();
        let connect = || Client::connect(listener.address(), &token, &agent(), &client());
        let mut welcomed: Vec<Client> = (0..MAX_WELCOMED)
            .map(|_| connect().expect("welcomed"))
            .collect();
        assert!(matches!(
            connect(),
            Err(ChannelError::Refused(reason)) if reason.contains("connections already")
        ));
        welcomed.pop();
        let until = Instant::now() + Duration::from_secs(10);
        while connect().is_err() {
            assert!(Instant::now() < until, "a place freed once one closed");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn the_connections_take_turns_each_first_in_its_turn() {
        let (listener, token) = opened();
        let connect =
            || Client::connect(listener.address(), &token, &agent(), &client()).expect("welcomed");
        let (one, two) = (connect(), connect());
        let mut pending = Vec::new();
        for client in [&one, &two] {
            pending.push(client.send(&Request::Pause).expect("sent"));
            pending.push(client.send(&Request::Resume).expect("sent"));
        }
        eventually("all four reached their queues", || {
            locked(&listener.shared.connections)
                .iter()
                .map(|connection| locked(&connection.state).inbox.len())
                .sum::<usize>()
                == 4
        });
        let first = listener.round();
        let second = listener.round();
        let requests = |round: &[Incoming]| {
            round
                .iter()
                .map(|incoming| incoming.request().clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            requests(&first),
            [Request::Pause, Request::Pause],
            "one of each"
        );
        assert_eq!(requests(&second), [Request::Resume, Request::Resume]);
        assert!(
            !Arc::ptr_eq(&first[0].connection, &second[0].connection),
            "the other connection comes first in the next round"
        );
        assert!(listener.round().is_empty() && !listener.waiting());
    }

    /// Reads the answers on `stream` as a tool's session does, until `count` came: each one's
    /// number and its fault's code, if it failed.
    fn answers(stream: &mut TcpStream, count: usize) -> Vec<(u64, Option<i64>)> {
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: MAX_BYTES,
        });
        (0..count)
            .map(
                |_| match receive(stream, &mut reader).expect("answered").message {
                    Message::Response {
                        id: Some(id),
                        outcome,
                    } => (id, outcome.err().map(|fault| fault.code)),
                    other => panic!("an answer, not {other:?}"),
                },
            )
            .collect()
    }

    #[test]
    fn a_connection_has_sixteen_requests_in_flight_and_one_more_is_answered_busy() {
        let (listener, token) = opened();
        let mut stream = deaf(&listener, &token);
        for id in 1..=17 {
            stream.write_all(&state_line(id)).expect("sent");
        }
        // The game holds the sixteen it is given, and the seventeenth is refused.
        let mut held: Vec<_> = (0..InFlight::DEFAULT.requests)
            .map(|_| taken(&listener))
            .collect();
        assert_eq!(answers(&mut stream, 1), [(17, Some(Fault::BUSY))]);
        assert!(listener.next().is_none(), "no more reached the game");
        // An answer frees a place: the next one is taken.
        held.remove(0).answer(Reply::Done);
        assert_eq!(answers(&mut stream, 1), [(1, None)]);
        stream.write_all(&state_line(18)).expect("sent");
        taken(&listener).answer(Reply::Done);
        assert_eq!(answers(&mut stream, 1), [(18, None)]);
        drop(held);
    }

    #[test]
    fn a_tool_that_sends_as_soon_as_it_reads_an_answer_is_never_answered_busy() {
        let (listener, token) = opened();
        let client = Arc::new(
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted"),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let game = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let round = listener.round();
                    if round.is_empty() {
                        thread::yield_now();
                    }
                    for incoming in round {
                        incoming.answer(Reply::Done);
                    }
                }
            })
        };
        // Four threads, each keeping five requests in flight: twenty asked for sixteen places.
        let askers: Vec<_> = (0..4)
            .map(|_| {
                let client = Arc::clone(&client);
                thread::spawn(move || {
                    let mut flying = VecDeque::new();
                    let mut busy = 0;
                    for _ in 0..500 {
                        if flying.len() == 5 {
                            let pending: crate::Pending = flying.pop_front().expect("in flight");
                            if let Reply::Failed { .. } = pending.wait().expect("answered").0 {
                                busy += 1;
                            }
                        }
                        flying.push_back(client.send(&Request::State).expect("sent"));
                    }
                    for pending in flying {
                        if let Reply::Failed { .. } = pending.wait().expect("answered").0 {
                            busy += 1;
                        }
                    }
                    busy
                })
            })
            .collect();
        let busy: usize = askers
            .into_iter()
            .map(|asker| asker.join().expect("asked"))
            .sum();
        stop.store(true, Ordering::Relaxed);
        game.join().expect("the game ends");
        assert_eq!(busy, 0, "requests answered busy");
    }

    #[test]
    fn the_game_takes_and_reads_none_of_a_connection_whose_writer_is_behind() {
        let (listener, token) = opened();
        let mut stream = deaf(&listener, &token);
        let other =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        for id in 1..=7 {
            stream.write_all(&state_line(id)).expect("sent");
        }
        let connection = Arc::clone(&locked(&listener.shared.connections)[0]);
        eventually("the seven reached their connection's queue", || {
            locked(&connection.state).inbox.len() == 7
        });
        // Six answers of 16 MiB, one image shared, pass the 64 MiB that a connection holds
        // unwritten, even with one of them in the socket's buffers already.
        let payload = Arc::new(vec![0_u8; 16 << 20]);
        let held: Vec<_> = (0..6).map(|_| taken(&listener)).collect();
        for incoming in held {
            incoming.answer_with(Reply::Done, Arc::clone(&payload));
        }
        assert!(!listener.waiting(), "nothing that the game may take");
        // The other connection is served meanwhile.
        let pending = other.send(&Request::Pause).expect("sent");
        let served = taken(&listener);
        assert_eq!(served.request(), &Request::Pause);
        served.answer(Reply::Done);
        pending.wait_for(Duration::from_secs(10)).expect("answered");
        // One more line, which the reader would queue at once if it read it, is not read.
        stream.write_all(&state_line(8)).expect("sent");
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            locked(&connection.state).inbox.len(),
            1,
            "the eighth was not read"
        );
        // Once the tool reads, the seventh is taken.
        let mut reading = stream.try_clone().expect("a clone");
        let reading = thread::spawn(move || {
            let mut rest = Vec::new();
            let _ = reading.read_to_end(&mut rest);
        });
        assert_eq!(taken(&listener).request(), &Request::State);
        let _ = stream.shutdown(Shutdown::Both);
        let _ = reading.join();
    }

    #[test]
    fn past_the_program_s_bound_only_connections_with_nothing_unwritten_are_served() {
        let token = Token::generate().expect("random bits");
        let wakes = Arc::new(AtomicUsize::new(0));
        let listener = {
            let wakes = Arc::clone(&wakes);
            Listener::open(token.clone(), "test", move || {
                wakes.fetch_add(1, Ordering::Relaxed);
            })
            .expect("a port")
        };
        // Seven connections with fifteen answers of 4 MiB each, one image shared: 420 MiB, past
        // the program's 256 even with one answer of each in the socket's buffers, each within its
        // own 64. Taken before any is answered, so that each is taken.
        let mut streams: Vec<_> = (0..7).map(|_| deaf(&listener, &token)).collect();
        for stream in &mut streams {
            for id in 1..=15 {
                stream.write_all(&state_line(id)).expect("sent");
            }
        }
        let held: Vec<_> = (0..7 * 15).map(|_| taken(&listener)).collect();
        let payload = Arc::new(vec![0_u8; 4 << 20]);
        for incoming in held {
            incoming.answer_with(Reply::Done, Arc::clone(&payload));
        }
        // One more on each, none of which is given, while a connection that reads is served.
        for stream in &mut streams {
            stream.write_all(&state_line(16)).expect("sent");
        }
        eventually("the sixteenths reached their queues", || {
            locked(&listener.shared.connections)
                .iter()
                .map(|connection| locked(&connection.state).inbox.len())
                .sum::<usize>()
                == 7
        });
        assert!(!listener.waiting());
        let reader =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let pending = reader.send(&Request::Pause).expect("sent");
        let served = taken(&listener);
        assert_eq!(served.request(), &Request::Pause);
        // Its answer, written, leaves it holding nothing again: the game is woken for what it
        // may take now, though the program stays past its bound.
        let before = wakes.load(Ordering::Relaxed);
        served.answer_with(Reply::Done, Arc::clone(&payload));
        pending.wait_for(Duration::from_secs(10)).expect("answered");
        eventually("the game is woken once it caught up", || {
            wakes.load(Ordering::Relaxed) > before
        });
        assert!(
            listener.round().is_empty(),
            "none of the sixteenths was given"
        );
    }

    #[test]
    fn a_malformed_line_after_answers_waits_for_them_then_its_reason_and_frees_its_place() {
        let (listener, token) = opened();
        let mut stream = deaf(&listener, &token);
        let payload = Arc::new(vec![0_u8; 8 << 20]);
        for id in 1..=5 {
            stream.write_all(&state_line(id)).expect("sent");
        }
        for _ in 0..4 {
            taken(&listener).answer_with(Reply::Done, Arc::clone(&payload));
        }
        let held = taken(&listener);
        // A line that is not a message, the tool reading nothing: the connection stays open, its
        // writer blocked, and what the game holds of it changes nothing more.
        stream.write_all(b"not json\n").expect("sent");
        let connection = Arc::clone(&held.connection);
        eventually("the reader finished", || {
            locked(&connection.state).finishing
        });
        assert!(!locked(&connection.state).closed && held.cancelled());
        drop(held);
        // Then the tool goes before reading what it was sent.
        drop(stream);
        eventually("the connection's place is free", || {
            listener.shared.welcomed.load(Ordering::Relaxed) == 0
        });
        // A tool that reads gets the answers, then the reason, then the end; what it asked and the
        // game had not taken is never done.
        let mut stream = deaf(&listener, &token);
        stream.write_all(&state_line(1)).expect("sent");
        taken(&listener).answer(Reply::Done);
        stream.write_all(&state_line(3)).expect("sent");
        let held = taken(&listener);
        stream.write_all(&state_line(2)).expect("sent");
        let connection = Arc::clone(&locked(&listener.shared.connections)[0]);
        eventually("the second reached its connection's queue", || {
            locked(&connection.state).inbox.len() == 1
        });
        stream.write_all(b"not json\n").expect("sent");
        eventually("the reader finished", || {
            locked(&connection.state).finishing
        });
        assert!(listener.round().is_empty(), "the second is not done");
        assert!(held.cancelled(), "what the game holds changes nothing more");
        drop(held);
        let mut reader = Reader::new(Limits {
            line: MAX_LINE,
            bytes: MAX_BYTES,
        });
        let first = receive(&mut stream, &mut reader).expect("answered").message;
        assert!(
            matches!(first, Message::Response { id: Some(1), .. }),
            "{first:?}"
        );
        let reason = receive(&mut stream, &mut reader)
            .expect("the reason")
            .message;
        assert!(
            matches!(
                reason,
                Message::Response {
                    id: None,
                    outcome: Err(_)
                }
            ),
            "{reason:?}"
        );
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).expect("closed");
        assert!(rest.is_empty());
    }

    #[test]
    fn a_request_the_game_did_not_take_or_dropped_is_answered_that_it_stopped() {
        let (listener, token) = opened();
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let held = client.send(&Request::Pause).expect("sent");
        drop(taken(&listener));
        assert_eq!(
            held.wait_for(Duration::from_secs(10)).expect("answered").0,
            stopped()
        );
        let queued = client.send(&Request::Pause).expect("sent");
        let connection = Arc::clone(&locked(&listener.shared.connections)[0]);
        eventually("it reached its connection's queue", || {
            locked(&connection.state).inbox.len() == 1
        });
        drop(listener);
        assert_eq!(
            queued
                .wait_for(Duration::from_secs(10))
                .expect("answered")
                .0,
            stopped()
        );
    }

    #[test]
    fn a_request_and_its_cancel_wake_the_game_and_an_answer_after_a_close_is_dropped() {
        let token = Token::generate().expect("random bits");
        let wakes = Arc::new(AtomicUsize::new(0));
        let listener = {
            let wakes = Arc::clone(&wakes);
            Listener::open(token.clone(), "test", move || {
                wakes.fetch_add(1, Ordering::Relaxed);
            })
            .expect("a port")
        };
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let pending = client.send(&Request::Pause).expect("sent");
        eventually("the request woke the game", || {
            wakes.load(Ordering::Relaxed) == 1
        });
        let held = taken(&listener);
        pending.cancel();
        eventually("the cancel woke it", || wakes.load(Ordering::Relaxed) == 2);
        drop(client);
        eventually("the close woke it", || wakes.load(Ordering::Relaxed) == 3);
        let connection = Arc::clone(&held.connection);
        held.answer(Reply::Done);
        assert!(
            locked(&connection.state).outbox.is_empty(),
            "nothing left to write"
        );
        assert_eq!(connection.unwritten.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_number_used_again_once_answered_is_not_cancelled() {
        let (listener, token) = opened();
        let mut stream = deaf(&listener, &token);
        stream.write_all(&state_line(7)).expect("sent");
        let held = taken(&listener);
        let cancel = encode(
            &Message::Event {
                method: uwui_agent::CANCEL.to_owned(),
                params: uwui_agent::write(&std::collections::BTreeMap::from([("id", 7)]))
                    .expect("written"),
            },
            0,
        )
        .expect("written");
        stream.write_all(&cancel).expect("sent");
        eventually("the cancel was read", || held.cancelled());
        held.answer(Reply::Done);
        assert_eq!(answers(&mut stream, 1), [(7, None)]);
        stream.write_all(&state_line(7)).expect("sent");
        assert!(!taken(&listener).cancelled(), "the old cancel is gone");
    }

    #[test]
    fn a_request_that_waits_past_its_time_is_cancelled_and_fails_so() {
        let (listener, token) = opened();
        let client =
            Client::connect(listener.address(), &token, &agent(), &client()).expect("accepted");
        let pending = client.send(&Request::Pause).expect("sent");
        let held = taken(&listener);
        assert!(matches!(
            pending.wait_for(Duration::from_millis(50)),
            Err(ChannelError::Io(error)) if error.kind() == io::ErrorKind::TimedOut
        ));
        eventually("the game sees it cancelled", || held.cancelled());
        held.cancel();
        // The connection serves on, the late answer dropped.
        let pending = client.send(&Request::State).expect("sent");
        taken(&listener).answer(Reply::Done);
        assert_eq!(
            pending
                .wait_for(Duration::from_secs(10))
                .expect("answered")
                .0,
            Reply::Done
        );
    }

    #[test]
    fn answers_of_text_alone_count_against_the_bound_too() {
        let (listener, token) = opened();
        let mut stream = deaf(&listener, &token);
        for id in 1..=8 {
            stream.write_all(&state_line(id)).expect("sent");
        }
        let connection = Arc::clone(&locked(&listener.shared.connections)[0]);
        eventually("the eight reached their connection's queue", || {
            locked(&connection.state).inbox.len() == 8
        });
        // Seven reports of 12 MiB of text, and no bytes after them: past 64 MiB, even with one
        // of them in the socket's buffers already.
        let held: Vec<_> = (0..7).map(|_| taken(&listener)).collect();
        for incoming in held {
            incoming.answer(Reply::Report {
                text: "r".repeat(12 << 20),
            });
        }
        assert!(!listener.waiting(), "the eighth is held back");
        let _ = stream.shutdown(Shutdown::Both);
    }
}
