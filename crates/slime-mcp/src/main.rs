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
use std::sync::{Arc, Mutex, PoisonError};
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
    ChannelError, Client, EDITING, Edit, Named, Refusal, Reply, Request, Token, launch, why,
};
use uwui_mcp::{
    Answer, Carrier, Sent, Taken, Tools, Value, agent_of, failure, fault, interface_family, text,
};

use describe::describe;

/// How long a game is given to say where it listens, by default: `cargo run` may build it first.
const START_TIMEOUT: Duration = Duration::from_secs(600);
/// How long a game is given to end once asked to stop, before it is killed: it ends once the frame
/// that took the request is drawn, which takes tens of seconds for *Hunna Wood*'s zone on a CPU
/// driver such as CI's lavapipe.
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

/// The numbers that the server gives each program that it connects to, as its tools name it: an
/// agent's cursor of a log names the program whose log it read.
static PROGRAMS: AtomicU64 = AtomicU64::new(1);

/// A new number of a program, never given before.
fn numbered() -> u64 {
    PROGRAMS.fetch_add(1, Ordering::Relaxed)
}

/// The game that the server drives, if one runs. Its connections are shared, so that an agent's
/// calls go side by side: a tool that asks the game holds the server's lock to choose one, or to
/// replace one lost, never while its request waits for its answer. `start_game` holds it while
/// the game starts, `stop_game` and `drive_played_game` for their own requests, so that one game at
/// a time runs, and they wait for a place in flight like the others (`docs/debt.md`, until
/// M6d.9a2b2).
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
    /// Its number ([`numbered`]), which a new connection to it keeps.
    program: u64,
    /// The game that the program, an editor, plays, while the game's tools drive it
    /// (`drive_played_game`, `docs/adr/0165-editing-while-playing.md`).
    played: Option<Played>,
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

/// The server: at most one game at a time. It is the carrier of uwui's MCP library, which takes
/// every family's requests to the game, and of its tools, the families' and its own
/// (`docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md`).
#[derive(Clone)]
struct Server {
    game: Arc<Mutex<Option<Game>>>,
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
            game: Arc::default(),
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
            }) => describe(reply, &bytes),
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
    game: Arc<Mutex<Option<Game>>>,
    route: Route,
}

impl Server {
    /// The carrier held to the route that a request of `family` takes now, chosen on a thread
    /// that may block, as the server's lock may be held a while by a game that starts.
    pub(crate) async fn pinned(&self, family: &'static str) -> Result<Pinned, String> {
        let game = Arc::clone(&self.game);
        let route = tokio::task::spawn_blocking(move || connection(&game, family))
            .await
            .map_err(|error| format!("the request's thread failed: {error}"))??;
        Ok(Pinned {
            game: Arc::clone(&self.game),
            route,
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
    game: &Arc<Mutex<Option<Game>>>,
    route: &Route,
    method: &str,
    params: Value,
) -> Result<Sent, String> {
    let Route {
        client, to_played, ..
    } = route.clone();
    let game = Arc::clone(game);
    let pending = client.send_call(method, params).map_err(|error| {
        not_served(
            &error,
            client.program(),
            client.refused(),
            family_of(method),
        )
        .unwrap_or_else(|| recovered(&game, &client, to_played, &why(&error)))
    })?;
    let canceller = pending.canceller();
    Ok(Sent::new(
        move || match pending.wait_outcome() {
            Ok((outcome, bytes)) => Ok(Answer { outcome, bytes }),
            Err(error) => Err(recovered(&game, &client, to_played, &why(&error))),
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
}

/// The connection that a request of `family` goes to, chosen under the server's lock, which the
/// request does not hold while it waits for its answer: what is not an editor's request reaches
/// the game that an editor plays while the game's tools drive it. A game that ended is forgotten,
/// its end told.
fn connection(game: &Mutex<Option<Game>>, family: &str) -> Result<Route, String> {
    let mut slot = game.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(running) = slot.as_mut() else {
        return Err("no game runs: start one with `start_game`".to_owned());
    };
    if let Some(ended) = running.ended() {
        *slot = None;
        return Err(ended);
    }
    Ok(match &running.played {
        Some(played) if family != EDITING => Route {
            client: Arc::clone(&played.client),
            to_played: true,
            program: played.program,
        },
        _ => Route {
            client: Arc::clone(&running.client),
            to_played: false,
            program: running.program,
        },
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

/// What a request that failed with `error` on `client` says, once what that failure needs is done:
/// a request that failed on a connection that still serves, as one that timed out, leaves it as it
/// is; once the connection to the program is lost, a new one replaces it; once the one to the
/// game that an editor plays is, `to_played`, the game's tools drive the editor again.
fn recovered(
    game: &Mutex<Option<Game>>,
    client: &Arc<Client>,
    to_played: bool,
    error: &str,
) -> String {
    let error = error.to_owned();
    if !client.is_broken() {
        return error;
    }
    let mut slot = game.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(running) = slot.as_mut() else {
        return error;
    };
    if to_played {
        let replaced = !running
            .played
            .as_ref()
            .is_some_and(|played| Arc::ptr_eq(&played.client, client));
        if replaced {
            return format!(
                "the game that the editor plays does not answer ({error}); the game's tools drive \
                 another game meanwhile"
            );
        }
        running.played = None;
        return format!(
            "the game that the editor plays does not answer ({error}): the game's tools drive the \
             editor again"
        );
    }
    if let Some(ended) = running.ended() {
        *slot = None;
        return format!("{error}\n{ended}");
    }
    if !Arc::ptr_eq(&running.client, client) {
        return format!(
            "{error}; the server connected to the game again meanwhile: the request may have run"
        );
    }
    match Client::connect(running.address, &running.token, &running.agent, &server()) {
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
                       `--journal`)."
    )]
    async fn start_game(
        &self,
        Parameters(StartGame {
            command,
            timeout_seconds,
        }): Parameters<StartGame>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let game = Arc::clone(&self.game);
        let agent = agent_of(&context);
        let started = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let mut game = game.lock().unwrap_or_else(PoisonError::into_inner);
            // A game that ended since is forgotten.
            if game
                .as_mut()
                .is_some_and(|running| running.ended().is_none())
            {
                return Err("a game runs already: stop it with `stop_game`".to_owned());
            }
            let (program, arguments) = command
                .split_first()
                .ok_or("the command is empty: give a program")?;
            let log = std::env::temp_dir().join(format!(
                "slime-mcp-{}-{}.log",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos())
            ));
            let log_file = File::create(&log).map_err(|error| error.to_string())?;
            let timeout = timeout_seconds.map_or(START_TIMEOUT, Duration::from_secs);
            let mut launched = launch(
                Command::new(program).args(arguments).stderr(log_file),
                timeout,
            )
            .map_err(|error| format!("{}\n{}", why(&error), tail(&log)))?;
            let client = match Client::connect(launched.address, &launched.token, &agent, &server())
            {
                Ok(client) => client,
                Err(error) => {
                    let _ = launched.child.kill();
                    let _ = launched.child.wait();
                    return Err(why(&error));
                }
            };
            let said = format!(
                "{} runs as process {}, its development channel at {}; its log in {}",
                client.program(),
                launched.child.id(),
                launched.address,
                log.display()
            );
            *game = Some(Game {
                child: launched.child,
                client: Arc::new(client),
                address: launched.address,
                token: launched.token,
                agent,
                log,
                program: numbered(),
                played: None,
            });
            Ok(said)
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(started.map_or_else(failure, text))
    }

    #[tool(
        description = "Asks the game that `start_game` started to stop, as closing its window \
                       would, and waits for it: an editor ends with the game that it plays."
    )]
    async fn stop_game(&self) -> Result<CallToolResult, McpError> {
        let game = Arc::clone(&self.game);
        let stopped = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let mut game = game.lock().unwrap_or_else(PoisonError::into_inner);
            let mut running = game.take().ok_or("no game runs")?;
            let _ = running.client.request(&Request::Stop);
            Ok(match wait(&mut running.child, STOP_TIMEOUT) {
                Some(status) => format!("the game ended: {status}"),
                None => {
                    let _ = running.child.kill();
                    let _ = running.child.wait();
                    format!(
                        "the game did not end within {STOP_TIMEOUT:?}, and was killed\n{}",
                        tail(&running.log)
                    )
                }
            })
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(stopped.map_or_else(failure, text))
    }

    #[tool(
        description = "In an editor that plays its level (`play.start`, once the game runs), points \
                       the game's tools, those of the world and of the interface with them, at \
                       the game that it plays, `played: true`, or back at the editor, \
                       `played: false`; the editor's tools always reach the editor, and \
                       `stop_game` stops the editor, `play.stop` the game. A step that the editor \
                       makes reaches the game a moment after the editor answers. The server \
                       connects to the game with what the editor gives of it."
    )]
    async fn drive_played_game(
        &self,
        Parameters(DrivePlayed { played }): Parameters<DrivePlayed>,
    ) -> Result<CallToolResult, McpError> {
        let game = Arc::clone(&self.game);
        let answer = tokio::task::spawn_blocking(move || {
            let mut slot = game.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(running) = slot.as_mut() else {
                return Err("no program runs: start an editor with `start_game`".to_owned());
            };
            if !played {
                running.played = None;
                return Ok("the game's tools drive the editor again".to_owned());
            }
            let edit = Request::Edit { edit: Edit::Played };
            let (address, token) = match running.client.request(&edit) {
                Ok((Reply::Played { address, token }, _)) => (address, token),
                Ok((Reply::Failed { message, .. }, _)) => return Err(message),
                Ok(_) => return Err("the program gave no game that it plays".to_owned()),
                Err(error) => return Err(why(&error)),
            };
            let address: SocketAddr = address
                .parse()
                .map_err(|error| format!("the editor gave no address: {error}"))?;
            let token: Token = token
                .parse()
                .map_err(|error| format!("the editor gave no token: {error}"))?;
            let client = Client::connect(address, &token, &running.agent, &server())
                .map_err(|error| why(&error))?;
            running.played = Some(Played {
                client: Arc::new(client),
                program: numbered(),
            });
            Ok(format!(
                "the game's tools drive the game that the editor plays, which listens at {address}"
            ))
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(match answer {
            Ok(said) => text(said),
            Err(why) => failure(why),
        })
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
    // The agent is gone, or the service failed: a game still running goes with it.
    let mut game = server.game.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(mut running) = game.take() {
        let _ = running.client.request(&Request::Stop);
        if wait(&mut running.child, STOP_TIMEOUT).is_none() {
            let _ = running.child.kill();
            let _ = running.child.wait();
        }
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
        assert_eq!(server.tools.router().list_all().len(), 37);
        let instructions = server.tools.instructions();
        let paragraphs: Vec<&str> = instructions.split("\n\n").collect();
        assert_eq!(
            paragraphs.len(),
            5,
            "its own, then play, world, editing, interface"
        );
        assert!(paragraphs[0].starts_with("Drives a game built with SlimeEngine"));
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
