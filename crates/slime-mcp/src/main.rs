//! The engine's MCP server (`docs/adr/0099-ai-agents-drive-the-engine.md`,
//! `docs/adr/0122-the-development-channel.md`): an agent, through the Model Context Protocol on this
//! program's standard input and output, starts a game built with the engine, drives it through its
//! development channel, pauses and steps it, reads and changes its world, plays it with scripted
//! keys, sees its image or one of its views and changes how a view draws, names the entity at a
//! pixel, reads its reports and its log, saves its run and loads it, uses its interface as a
//! person does, edits an editor's level; and cooks assets and runs tests. `README.md` beside this
//! crate says how an agent is given it. Run it from the repository's root, where cooking and the
//! tests run.
//!
//! Its tools stand on uwui's MCP library, by families of the core for agents, each in a module of
//! its own: `play`, `world`, `editing`, and the library's `interface`; this file holds the server's
//! own tools and the carrier that takes every family's requests to the game
//! (`docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md`).

mod describe;
mod editing;
mod play;
mod world;

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, schemars, tool, tool_handler,
    tool_router,
};
use serde::{Deserialize, Serialize};
use slime_channel::{
    ChannelError, Client, EDITING, Edit, Named, Refusal, Reply, Token, launch_unless, why,
};
use uwui_mcp::{
    Answer, Carrier, Sent, Taken, Tools, Value, agent_of, failure, fault, interface_family, text,
};

use describe::describe;

/// How long a game is given to say where it listens, by default: `cargo run` may build it first.
const START_TIMEOUT: Duration = Duration::from_secs(600);
/// How long a game is given to end once its standard input closed, before it is killed: it ends
/// at its next frame, which takes tens of seconds for *Hunna Wood*'s zone on a CPU driver such as
/// CI's lavapipe.
const STOP_TIMEOUT: Duration = Duration::from_secs(120);
/// How many bytes of the end of a game's log, or of a command's output, a failure shows.
const TAIL: u64 = 4096;
/// The server's version, the workspace's.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The first paragraph of the server's instructions: what it drives, its own tools, and what
/// crosses its families; each family's paragraph follows.
const INSTRUCTIONS: &str = "Drives a game built with SlimeEngine, or its editor, through its \
     development channel. `start_game` starts one, `stop_game` ends it; an editor is started as a \
     game is (`cargo run --example editor -- --level <a copy of a level> --journal <a file of \
     its own>`), and opens a window unless it is started with `--headless`. Once an editor plays \
     its level (`run_operation` `play.start`), `drive_played_game` points the game's tools, and \
     the interface's, at the game that it plays, and back at the editor; the editor's tools \
     reach the editor all along, and a step that it makes reaches the game a moment after it \
     answers. `stop_game` ends the editor, and the game that it plays with it; `play.stop` ends \
     that game alone. `cook` and `run_tests` run in the repository's root, where the server runs.";

/// The numbers that the server gives each start of a game and each program that it connects to, as
/// its tools name them: an agent's cursor of a log names the program whose log it read.
static PROGRAMS: AtomicU64 = AtomicU64::new(1);

/// A new number of a program, never given before.
fn numbered() -> u64 {
    PROGRAMS.fetch_add(1, Ordering::Relaxed)
}

/// The server's game, in one state, behind the server's lock, which no tool holds while it waits:
/// for a game to start, to stop or to answer, nor for a connection. One game at a time: a start
/// takes the state from [`GameState::Absent`], and a tool that finds it in another state says so
/// at once (`docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md`).
enum GameState {
    /// No game runs.
    Absent,
    /// `start_game` starts the game numbered `number` ([`numbered`]), off the lock. The start is
    /// given up once the state is no longer this one, as `stop_game` and the call's cancel leave
    /// it: the program that it started is then killed.
    Starting { number: u64 },
    /// The game runs, boxed: the other states are a number.
    Running(Box<Game>),
    /// `stop_game`, or the server's end, stops the game numbered `number`, off the lock.
    Stopping { number: u64 },
}

/// The server's lock on its game's state, a moment at a time.
fn locked(game: &Mutex<GameState>) -> MutexGuard<'_, GameState> {
    game.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether `state` is the start numbered `number`, not yet given up.
fn starting(state: &GameState, number: u64) -> bool {
    matches!(state, GameState::Starting { number: now } if *now == number)
}

/// Forgets a game that ended on its own, and says how it ended, with the end of its log.
fn forget_ended(state: &mut GameState) -> Option<String> {
    let GameState::Running(running) = state else {
        return None;
    };
    let ended = running.ended()?;
    *state = GameState::Absent;
    Some(ended)
}

/// The game that runs, a game that ended forgotten; or why none runs, which the agent reads.
fn running(state: &mut GameState) -> Result<&mut Game, String> {
    if let Some(ended) = forget_ended(state) {
        return Err(ended);
    }
    match state {
        GameState::Running(running) => Ok(running),
        GameState::Absent => Err("no game runs: start one with `start_game`".to_owned()),
        GameState::Starting { .. } => {
            Err("the game starts: call this again once `start_game` answered".to_owned())
        }
        GameState::Stopping { .. } => {
            Err("the game stops (`stop_game`): start another once it answered".to_owned())
        }
    }
}

/// The game that the server drives. Its connections are shared, so that an agent's calls go side
/// by side: a tool that asks the game takes the server's lock to choose one, never while its
/// request waits for its answer.
struct Game {
    child: Child,
    client: Arc<Client>,
    /// Where it listens, and its token: a new connection replaces one that broke.
    address: SocketAddr,
    token: Token,
    /// The agent that started it, as its MCP client names itself, which each connection names to
    /// the game.
    agent: Named,
    /// The file that holds the game's standard error, its log.
    log: PathBuf,
    /// Its number, its start's ([`numbered`]), which a new connection to it keeps.
    program: u64,
    /// The game that the program, an editor, plays, while the game's tools drive it
    /// (`drive_played_game`, `docs/adr/0165-editing-while-playing.md`).
    played: Option<Played>,
    /// How many calls of `drive_played_game` came for it: the last one wins, the one whose count
    /// this still is once it connected.
    drives: u64,
    /// Whether a request whose connection broke connects to it again, off the lock
    /// ([`recovered`]): the other requests that the break failed then do not connect too.
    reconnecting: bool,
}

/// The game that an editor plays, while the game's tools drive it.
struct Played {
    client: Arc<Client>,
    /// Its number ([`numbered`]).
    program: u64,
}

/// The server, as it names itself to the games that it connects to.
fn server() -> Named {
    Named::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

/// The server: at most one game at a time ([`GameState`]). It is the carrier of uwui's MCP library, which takes
/// every family's requests to the game, and of its tools, the families' and its own
/// (`docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md`).
#[derive(Clone)]
struct Server {
    game: Arc<Mutex<GameState>>,
    /// Its tools and its instructions, shared by its clones, one for each request.
    tools: Arc<Tools<Self>>,
}

impl Server {
    /// The server, its tools gathered by families.
    ///
    /// # Errors
    ///
    /// [`Taken`] when two tools take one name.
    fn new() -> Result<Self, Taken> {
        let tools = Tools::new(INSTRUCTIONS, Self::own_tools())
            .with(play::family())?
            .with(world::family())?
            .with(editing::family())?
            .with(interface_family())?;
        Ok(Self {
            game: Arc::new(Mutex::new(GameState::Absent)),
            tools: Arc::new(tools),
        })
    }

    /// Asks the game `request`, a request of one of the engine's families, `play`, `world` or
    /// `editing`, through uwui's MCP library, which a call that its agent cancels cancels, and
    /// gives its reply as a result, or why none came.
    async fn answer(
        &self,
        request: &(impl Serialize + Sync),
        context: &RequestContext<RoleServer>,
    ) -> CallToolResult {
        match uwui_mcp::request::<Self, Reply>(self, request, context).await {
            Ok(Answer {
                outcome: Ok(reply),
                bytes,
            }) => describe(reply, bytes).await,
            Ok(Answer {
                outcome: Err(refused),
                ..
            }) => fault(refused),
            Err(why) => failure(why),
        }
    }
}

/// What `start_game` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StartGame {
    /// The program and its arguments, as a shell would split them: for example `["cargo", "run",
    /// "-p", "hunnawood", "--", "--new", "--save", "/tmp/agent-run.toml"]`. Give a game a save of
    /// its own, never the player's, and an editor a copy of a level and a journal of its own.
    command: Vec<String>,
    /// How many seconds the game is given to start listening; 600 if left out, `cargo run` building
    /// it first.
    timeout_seconds: Option<u64>,
}

/// What `drive_played_game` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DrivePlayed {
    /// Whether the game's tools drive the game that the editor plays, `true`, or the editor again,
    /// `false`.
    played: bool,
}

/// What `cook` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Cook {
    /// The directory of source assets: `games/hunnawood/assets/zone`.
    sources: String,
    /// The directory of cooked data: `target/cooked/hunnawood`. A game started with `--watch`
    /// loads again what cooking changes there.
    output: String,
}

/// What `run_tests` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RunTests {
    /// The arguments of `cargo test`: `["-p", "hunnawood"]`, `["-p", "slime-world", "saves"]`.
    arguments: Vec<String>,
}

/// The last bytes of the file at `path`, as text.
fn tail(path: &PathBuf) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map_or(0, |metadata| metadata.len());
    let _ = file.seek(SeekFrom::Start(length.saturating_sub(TAIL)));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Runs `command` to its end, and says how it ended, with the end of its output.
fn run_to_end(command: &mut Command) -> CallToolResult {
    match command.stdin(Stdio::null()).output() {
        Ok(output) => {
            let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
            said.push_str(&String::from_utf8_lossy(&output.stderr));
            let start = said.len().saturating_sub(TAIL as usize);
            let start = (start..said.len())
                .find(|&at| said.is_char_boundary(at))
                .unwrap_or(said.len());
            let message = format!("{}\n{}", output.status, &said[start..]);
            if output.status.success() {
                text(message)
            } else {
                failure(message)
            }
        }
        Err(error) => failure(format!("cannot run it: {error}")),
    }
}

impl Game {
    /// How the game ended, with the end of its log, if it has.
    fn ended(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(format!("the game ended: {status}\n{}", tail(&self.log))),
            _ => None,
        }
    }
}

/// What `start_game` answers once its start was given up.
const GIVEN_UP: &str = "the game's start was given up, by `stop_game` or by the call's cancel: the \
                        program that it started is being stopped";

/// Starts the game numbered `number` as `command` runs it, at most `timeout`, connecting to it as
/// `agent`, off the server's lock, and makes it the server's game, or says why not. A start given
/// up meanwhile ([`GameState::Starting`]) kills the program that it started.
fn start(
    game: &Mutex<GameState>,
    number: u64,
    command: &[String],
    timeout: Duration,
    agent: Named,
) -> Result<String, String> {
    let log = std::env::temp_dir().join(format!(
        "slime-mcp-{}-{}.log",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    // Every failure goes on to the state's change below, which ends the start.
    let started = command
        .split_first()
        .ok_or_else(|| "the command is empty: give a program".to_owned())
        .and_then(|(program, arguments)| {
            let log_file = File::create(&log).map_err(|error| error.to_string())?;
            launch_unless(
                Command::new(program).args(arguments).stderr(log_file),
                timeout,
                || !starting(&locked(game), number),
            )
            .map_err(|error| format!("{}\n{}", why(&error), tail(&log)))
        })
        .and_then(|mut launched| {
            match Client::connect(launched.address, &launched.token, &agent, &server()) {
                Ok(client) => Ok((launched, client)),
                Err(error) => {
                    killed(&mut launched.child);
                    Err(why(&error))
                }
            }
        });
    let mut state = locked(game);
    if !starting(&state, number) {
        drop(state);
        if let Ok((mut launched, _)) = started {
            killed(&mut launched.child);
        }
        return Err(GIVEN_UP.to_owned());
    }
    let (launched, client) = match started {
        Ok(started) => started,
        Err(why) => {
            *state = GameState::Absent;
            return Err(why);
        }
    };
    let said = format!(
        "{} runs as process {}, its development channel at {}; its log in {}",
        client.program(),
        launched.child.id(),
        launched.address,
        log.display()
    );
    *state = GameState::Running(Box::new(Game {
        child: launched.child,
        client: Arc::new(client),
        address: launched.address,
        token: launched.token,
        agent,
        log,
        program: number,
        played: None,
        drives: 0,
        reconnecting: false,
    }));
    Ok(said)
}

/// Gives up the start numbered `number`: while the state is still that start, its thread then kills
/// the program that it started; once the game that it started runs, it is the game, which the state
/// holds as stopping, for the caller to stop ([`stop_off_thread`]).
fn give_up(game: &Mutex<GameState>, number: u64) -> Option<Box<Game>> {
    let mut state = locked(game);
    match std::mem::replace(&mut *state, GameState::Absent) {
        GameState::Starting { number: now } if now == number => None,
        GameState::Running(running) if running.program == number => {
            *state = GameState::Stopping { number };
            Some(running)
        }
        other => {
            *state = other;
            None
        }
    }
}

/// Stops `running`, which the state holds as stopping, on a thread that may block, and ends that
/// state, even when the stop's thread failed, so that a start may follow.
async fn stop_off_thread(game: &Mutex<GameState>, running: Box<Game>) -> Result<String, String> {
    let number = running.program;
    let said = tokio::task::spawn_blocking(move || stopped(*running)).await;
    let mut state = locked(game);
    if matches!(*state, GameState::Stopping { number: now } if now == number) {
        *state = GameState::Absent;
    }
    said.map_err(|error| format!("the stop's thread failed: {error}"))
}

/// Stops `running` by closing its standard input, which it reads to its end
/// (`slime_channel::launch`): it ends as at a termination signal, needing no place in flight, which
/// long requests may all hold. Killed if it did not end within [`STOP_TIMEOUT`]. An editor ends
/// with the game that it plays.
fn stopped(mut running: Game) -> String {
    drop(running.child.stdin.take());
    match wait(&mut running.child, STOP_TIMEOUT) {
        Some(status) => format!("the game ended: {status}"),
        None => {
            killed(&mut running.child);
            format!(
                "the game did not end within {STOP_TIMEOUT:?}, and was killed\n{}",
                tail(&running.log)
            )
        }
    }
}

/// Kills `child`, and waits for its end, so that it leaves no process behind.
fn killed(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Waits for `child` to end, at most `timeout`.
fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let until = Instant::now() + timeout;
    while Instant::now() < until {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

impl Carrier for Server {
    /// Sends a request of the core to the game: an editor's request, of the family `editing`, to
    /// the program that `start_game` started; any other to the game that an editor plays while the
    /// game's tools drive it, to that program otherwise
    /// (`docs/adr/0165-editing-while-playing.md`). A game that ended is forgotten, its end told;
    /// once the connection to the program is lost, a new one replaces it; once the one to the game
    /// that an editor plays is, the game's tools drive the editor again. A family that the
    /// program does not serve is refused in the server's words.
    fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        let route = connection(&self.game, family_of(method))?;
        send_on(&self.game, &route, method, params)
    }
}

/// The server's carrier held to one route, the program that a request of a family reached when it
/// was made: each request of a long list goes to that program, whatever the routing does
/// meanwhile, so that a cursor that names its program names the one that answered.
#[derive(Clone)]
pub(crate) struct Pinned {
    game: Arc<Mutex<GameState>>,
    route: Route,
}

impl Server {
    /// The carrier held to the route that a request of `family` takes now.
    pub(crate) fn pinned(&self, family: &'static str) -> Result<Pinned, String> {
        Ok(Pinned {
            game: Arc::clone(&self.game),
            route: connection(&self.game, family)?,
        })
    }
}

impl Pinned {
    /// The number of the program that it reaches.
    fn program(&self) -> u64 {
        self.route.program
    }
}

impl Carrier for Pinned {
    /// Sends a request of the core on the route that it holds, as [`Server`] sends it.
    fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        send_on(&self.game, &self.route, method, params)
    }
}

/// The family that `method`, `family/name`, belongs to.
fn family_of(method: &str) -> &str {
    method.split_once('/').map_or(method, |(family, _)| family)
}

/// Sends `method` with `params` on `route`, as [`Carrier::send`] says: a failure as the server
/// recovers from it, a family not served in the server's words.
fn send_on(
    game: &Arc<Mutex<GameState>>,
    route: &Route,
    method: &str,
    params: Value,
) -> Result<Sent, String> {
    let route = route.clone();
    let game = Arc::clone(game);
    let client = &route.client;
    let pending = client.send_call(method, params).map_err(|error| {
        not_served(
            &error,
            client.program(),
            client.refused(),
            family_of(method),
        )
        .unwrap_or_else(|| recovered(&game, &route, &why(&error)))
    })?;
    let canceller = pending.canceller();
    Ok(Sent::new(
        move || match pending.wait_outcome() {
            Ok((outcome, bytes)) => Ok(Answer { outcome, bytes }),
            Err(error) => Err(recovered(&game, &route, &why(&error))),
        },
        move || canceller.cancel(),
    ))
}

/// The connection that a request goes to, as [`connection`] chooses it.
#[derive(Clone)]
struct Route {
    client: Arc<Client>,
    /// Whether it is the one to the game that an editor plays.
    to_played: bool,
    /// The number of the program that it reaches.
    program: u64,
    /// The number of the game that the server started, whose state [`recovered`] reads: the
    /// editor's when it reaches the game that the editor plays.
    game: u64,
}

impl Route {
    /// The route to the program that the server started, `running`.
    fn to(running: &Game) -> Self {
        Self {
            client: Arc::clone(&running.client),
            to_played: false,
            program: running.program,
            game: running.program,
        }
    }
}

/// The connection that a request of `family` goes to, chosen under the server's lock, which the
/// request does not hold while it waits for its answer: what is not an editor's request reaches
/// the game that an editor plays while the game's tools drive it. A game that ended is forgotten,
/// its end told; a game that starts or stops is told so.
fn connection(game: &Mutex<GameState>, family: &str) -> Result<Route, String> {
    let mut state = locked(game);
    let running = running(&mut state)?;
    Ok(match &running.played {
        Some(played) if family != EDITING => Route {
            client: Arc::clone(&played.client),
            to_played: true,
            program: played.program,
            game: running.program,
        },
        _ => Route::to(running),
    })
}

/// What the server says of a request of `family` refused as it was sent, when `program` does not
/// serve that family, as it `refused` those that it does not: the two versions, and which side to
/// build again, the older one
/// (`docs/adr/0275-a-familys-refusal-gives-the-version-that-the-program-serves.md`). None for a
/// family served, and for a failure of another kind than the session's: the core refuses a family
/// that the welcome refused before sending anything, but a connection broken fails first, and is
/// recovered from. A refusal through the channel is not tested: a program that speaks another
/// version of a family needs a listener of other families than the channel's.
fn not_served(
    error: &ChannelError,
    program: &str,
    refused: &[Refusal],
    family: &str,
) -> Option<String> {
    let ChannelError::Session(_) = error else {
        return None;
    };
    refusal_words(program, refused, family)
}

/// The words of [`not_served`], for a request of `family` that `program` refused.
fn refusal_words(program: &str, refused: &[Refusal], family: &str) -> Option<String> {
    let refused = refused
        .iter()
        .find(|refused| refused.family.name == family)?;
    let ours = refused.family.version;
    Some(match refused.served {
        Some(theirs) if theirs > ours => format!(
            "`{program}` serves version {theirs} of this tool's family, `{family}`, and this server \
             speaks version {ours}: build this server again as the workspace stands, then start \
             it again from the agent's client"
        ),
        Some(theirs) => format!(
            "`{program}` serves version {theirs} of this tool's family, `{family}`, and this server \
             speaks version {ours}: build the program again as the workspace stands, and start it \
             again"
        ),
        None => format!(
            "`{program}` does not serve this tool's family, `{family}`, which this server speaks at \
             version {ours}: {}",
            refused.reason
        ),
    })
}

/// What a request that failed with `error` on `route` says, once what that failure needs is done:
/// a request that failed on a connection that still serves, as one that timed out, leaves it as it
/// is; once the connection to the program is lost, a new one replaces it, connected off the lock;
/// once the one to the game that an editor plays is, the game's tools drive the editor again. A
/// game that stops, was stopped or ended is told so, and not connected to again.
fn recovered(game: &Mutex<GameState>, route: &Route, error: &str) -> String {
    if !route.client.is_broken() {
        return error.to_owned();
    }
    let (address, token, agent) = {
        let mut state = locked(game);
        let running = match &mut *state {
            GameState::Running(running) if running.program == route.game => running,
            GameState::Stopping { number } if *number == route.game => {
                return format!(
                    "{error}: the game stops (`stop_game`), and is not connected to again"
                );
            }
            _ => {
                return format!(
                    "{error}: the game that it asked was stopped or ended, and is not connected to \
                     again"
                );
            }
        };
        if let Some(ended) = running.ended() {
            *state = GameState::Absent;
            return format!("{error}\n{ended}");
        }
        if route.to_played {
            let replaced = !running
                .played
                .as_ref()
                .is_some_and(|played| Arc::ptr_eq(&played.client, &route.client));
            if replaced {
                return format!(
                    "the game that the editor plays does not answer ({error}); the game's tools \
                     drive another game meanwhile"
                );
            }
            running.played = None;
            return format!(
                "the game that the editor plays does not answer ({error}): the game's tools drive \
                 the editor again"
            );
        }
        if !Arc::ptr_eq(&running.client, &route.client) {
            return format!(
                "{error}; the server connected to the game again meanwhile: the request may have run"
            );
        }
        if running.reconnecting {
            return format!(
                "{error}; the server connects to the game again: the request may have run"
            );
        }
        running.reconnecting = true;
        (
            running.address,
            running.token.clone(),
            running.agent.clone(),
        )
    };
    // Off the lock: a game may take a while to welcome a connection. Only this request connects:
    // the state says so to the others until it is done.
    let connected = Client::connect(address, &token, &agent, &server());
    // Declared after `connected`, the lock is let go before a connection not kept closes.
    let mut state = locked(game);
    let GameState::Running(running) = &mut *state else {
        return format!("{error}: the game was stopped meanwhile, and is not connected to again");
    };
    if running.program != route.game {
        return format!("{error}: the game was stopped meanwhile, and is not connected to again");
    }
    running.reconnecting = false;
    match connected {
        Ok(client) => {
            running.client = Arc::new(client);
            format!("{error}; the server connected to the game again: the request may have run")
        }
        Err(again) => format!(
            "{error}; connecting to the game again failed: {}",
            why(&again)
        ),
    }
}

#[tool_router(router = own_tools)]
impl Server {
    #[tool(
        description = "Starts a game built with SlimeEngine, as `command` runs it, with a secret \
                       token, and connects to its development channel. One game at a time; give it \
                       a save of its own (`--new --save /tmp/agent-run.toml` for Hunna Wood), and \
                       an editor a copy of a level and a journal of its own (`--level`, \
                       `--journal`). A call cancelled, or `stop_game` meanwhile, gives the start \
                       up, and stops the program that it started."
    )]
    async fn start_game(
        &self,
        Parameters(StartGame {
            command,
            timeout_seconds,
        }): Parameters<StartGame>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let number = numbered();
        {
            let mut state = locked(&self.game);
            // A game that ended since is forgotten.
            let _ = forget_ended(&mut state);
            let refused = match &*state {
                GameState::Absent => None,
                GameState::Starting { .. } => Some(
                    "a game starts already: wait for its `start_game`, or give it up with \
                     `stop_game`",
                ),
                GameState::Running(_) => Some("a game runs already: stop it with `stop_game`"),
                GameState::Stopping { .. } => {
                    Some("a game stops: start another once its `stop_game` answered")
                }
            };
            if let Some(refused) = refused {
                return Ok(failure(refused));
            }
            *state = GameState::Starting { number };
        }
        let game = Arc::clone(&self.game);
        let agent = agent_of(&context);
        let timeout = timeout_seconds.map_or(START_TIMEOUT, Duration::from_secs);
        let mut starting =
            tokio::task::spawn_blocking(move || start(&game, number, &command, timeout, agent));
        let started = tokio::select! {
            started = &mut starting => started
                .unwrap_or_else(|error| Err(format!("the start's thread failed: {error}"))),
            () = context.ct.cancelled() => Err(GIVEN_UP.to_owned()),
        };
        if context.ct.is_cancelled() || started.is_err() {
            // A start cancelled, even as it answered, or failed, is given up: its thread kills
            // what it started, and a game that it made run is stopped. MCP answers no cancelled
            // call.
            if let Some(running) = give_up(&self.game, number) {
                let _ = stop_off_thread(&self.game, running).await;
            }
        }
        Ok(started.map_or_else(failure, text))
    }

    #[tool(
        description = "Stops the game that `start_game` started, as a termination signal would, \
                       and waits for its end: an editor ends with the game that it plays. A game \
                       that does not end within 120 s is killed. A start still under way is \
                       given up, and the program that it started stopped after this answer."
    )]
    async fn stop_game(&self) -> Result<CallToolResult, McpError> {
        let running = {
            let mut state = locked(&self.game);
            match std::mem::replace(&mut *state, GameState::Absent) {
                GameState::Absent => return Ok(failure("no game runs")),
                GameState::Starting { .. } => {
                    return Ok(text(
                        "the game's start is given up: the program that it started is being \
                         stopped",
                    ));
                }
                GameState::Stopping { number } => {
                    *state = GameState::Stopping { number };
                    return Ok(failure(
                        "the game stops already: another `stop_game` answers once it ended",
                    ));
                }
                GameState::Running(running) => {
                    *state = GameState::Stopping {
                        number: running.program,
                    };
                    running
                }
            }
        };
        Ok(stop_off_thread(&self.game, running)
            .await
            .map_or_else(failure, text))
    }

    #[tool(
        description = "In an editor that plays its level (`play.start`, once the game runs), points \
                       the game's tools, those of the world and of the interface with them, at \
                       the game that it plays, `played: true`, or back at the editor, \
                       `played: false`; the last call wins. The editor's tools always reach the \
                       editor, and `stop_game` stops the editor, `play.stop` the game. A step that \
                       the editor makes reaches the game a moment after the editor answers. The \
                       server connects to the game with what the editor gives of it."
    )]
    async fn drive_played_game(
        &self,
        Parameters(DrivePlayed { played }): Parameters<DrivePlayed>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (editor, agent, drive) = {
            let mut state = locked(&self.game);
            let running = match running(&mut state) {
                Ok(running) => running,
                Err(why) => return Ok(failure(why)),
            };
            running.drives += 1;
            if !played {
                running.played = None;
                return Ok(text("the game's tools drive the editor again"));
            }
            let editor = Pinned {
                game: Arc::clone(&self.game),
                route: Route::to(running),
            };
            (editor, running.agent.clone(), running.drives)
        };
        let (address, token) =
            match uwui_mcp::request::<Pinned, Reply>(&editor, &Edit::Played, &context).await {
                Ok(Answer {
                    outcome: Ok(Reply::Played { address, token }),
                    ..
                }) => (address, token),
                Ok(Answer {
                    outcome: Ok(Reply::Failed { message, .. }),
                    ..
                }) => return Ok(failure(message)),
                Ok(Answer { outcome: Ok(_), .. }) => {
                    return Ok(failure("the program gave no game that it plays"));
                }
                Ok(Answer {
                    outcome: Err(refused),
                    ..
                }) => return Ok(fault(refused)),
                Err(why) => return Ok(failure(why)),
            };
        let address: SocketAddr = match address.parse() {
            Ok(address) => address,
            Err(error) => return Ok(failure(format!("the editor gave no address: {error}"))),
        };
        let token: Token = match token.parse() {
            Ok(token) => token,
            Err(error) => return Ok(failure(format!("the editor gave no token: {error}"))),
        };
        let connected = tokio::task::spawn_blocking(move || {
            Client::connect(address, &token, &agent, &server())
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let client = match connected {
            Ok(client) => Arc::new(client),
            Err(error) => return Ok(failure(why(&error))),
        };
        let mut state = locked(&self.game);
        let GameState::Running(running) = &mut *state else {
            return Ok(failure("the editor was stopped meanwhile"));
        };
        if running.program != editor.program() {
            return Ok(failure("the editor was stopped meanwhile"));
        }
        if running.drives != drive || context.ct.is_cancelled() {
            return Ok(failure(
                "a later call of `drive_played_game`, or this call's cancel, took this one's place",
            ));
        }
        running.played = Some(Played {
            client,
            program: numbered(),
        });
        Ok(text(format!(
            "the game's tools drive the game that the editor plays, which listens at {address}"
        )))
    }

    #[tool(
        description = "Cooks a directory of source assets into a directory of cooked data, with the \
                       engine's cooker, again only what changed; a game started with `--watch` \
                       loads again what changed."
    )]
    async fn cook(
        &self,
        Parameters(Cook { sources, output }): Parameters<Cook>,
    ) -> Result<CallToolResult, McpError> {
        tokio::task::spawn_blocking(move || {
            run_to_end(
                Command::new("cargo")
                    .args(["run", "--release", "-p", "slime-cook", "--", "cook"])
                    .args([sources, output]),
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))
    }

    #[tool(
        description = "Runs `cargo test` with the arguments given, and gives the end of its output."
    )]
    async fn run_tests(
        &self,
        Parameters(RunTests { arguments }): Parameters<RunTests>,
    ) -> Result<CallToolResult, McpError> {
        tokio::task::spawn_blocking(move || {
            run_to_end(Command::new("cargo").arg("test").args(arguments))
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))
    }
}

#[tool_handler(router = self.tools.router())]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("slime-mcp", VERSION))
            .with_instructions(self.tools.instructions())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = match Server::new() {
        Ok(server) => server,
        Err(taken) => {
            // Told as a person reads it, not as `main`'s error would print it.
            eprintln!("slime-mcp cannot start: {taken}");
            std::process::exit(1);
        }
    };
    let service = server.clone().serve(rmcp::transport::stdio()).await?;
    let served = service.waiting().await;
    // The agent is gone, or the service failed: a game still running goes with it, stopped as
    // `stop_game` stops it; a start under way is given up, and its thread kills what it started; a
    // game that stops already is waited for, as the runtime waits for its thread.
    let left = std::mem::replace(&mut *locked(&server.game), GameState::Absent);
    if let GameState::Running(running) = left {
        let _ = stopped(*running);
    }
    served?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slime_channel::Family;

    /// The server's tools, its own and its families', each name given once.
    #[test]
    fn the_server_is_built_each_tool_s_name_given_once() {
        let server = Server::new().expect("no name taken twice");
        assert_eq!(server.tools.router().list_all().len(), 38);
        let instructions = server.tools.instructions();
        let paragraphs: Vec<&str> = instructions.split("\n\n").collect();
        assert_eq!(
            paragraphs.len(),
            5,
            "its own, then play, world, editing, interface"
        );
        assert!(paragraphs[0].starts_with("Drives a game built with SlimeEngine"));
    }

    /// `stop_game`'s description, which no constant can reach, gives the time after which it
    /// kills a game.
    #[test]
    fn stop_game_s_description_gives_its_time() {
        let server = Server::new().expect("no name taken twice");
        let stop = server
            .tools
            .router()
            .list_all()
            .into_iter()
            .find(|tool| tool.name == "stop_game")
            .expect("stop_game");
        let description = stop.description.expect("a description");
        let within = format!("within {} s", STOP_TIMEOUT.as_secs());
        assert!(description.contains(&within), "{description}");
    }

    /// A family that the program serves at another version is refused in words that give both
    /// versions and name the side to build again, the older; one that it lacks says so; a family
    /// served is not refused.
    #[test]
    fn a_family_not_served_is_refused_naming_the_side_to_build_again() {
        let refusal = |served| Refusal {
            family: Family::new("editing", 3),
            reason: "the program does not serve \"editing\"".to_owned(),
            served,
        };
        let older = refusal_words("hunnawood", &[refusal(Some(2))], "editing").expect("refused");
        assert!(
            older.starts_with("`hunnawood` serves version 2 of this tool's family, `editing`")
                && older.contains("version 3")
                && older.contains("build the program again"),
            "{older}"
        );
        let newer = refusal_words("hunnawood", &[refusal(Some(4))], "editing").expect("refused");
        assert!(newer.contains("build this server again"), "{newer}");
        let lacked = refusal_words("hunnawood", &[refusal(None)], "editing").expect("refused");
        assert!(
            lacked.contains("does not serve this tool's family")
                && lacked.contains("the program does not serve \"editing\""),
            "{lacked}"
        );
        assert_eq!(
            refusal_words("hunnawood", &[refusal(Some(2))], "play"),
            None
        );
        // A broken connection fails before the session refuses, and is recovered from.
        assert_eq!(
            not_served(
                &ChannelError::Broken,
                "hunnawood",
                &[refusal(Some(2))],
                "editing"
            ),
            None
        );
    }
}
