//! The engine's MCP server (`docs/adr/0099-ai-agents-drive-the-engine.md`,
//! `docs/adr/0122-the-development-channel.md`): an agent, through the Model Context Protocol on this
//! program's standard input and output, starts a game built with the engine, drives it through its
//! development channel, pauses and steps it, reads and changes its world, plays it with scripted
//! keys, sees its image or one of its views and changes how a view draws, names the entity at a
//! pixel, reads its reports and its log, saves its run and loads it; and cooks assets and runs
//! tests. `README.md` beside this
//! crate says how an agent is given it. Run it from the repository's root, where cooking and the
//! tests run.

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, schemars, tool, tool_handler,
    tool_router,
};
use serde::Deserialize;
use slime_channel::{Client, Edit, Fault, Named, Reply, Request, Token, json_size, launch};
use uwui_mcp::{
    Answer, Carrier, LIST_TIME, PAGE_BYTES, Sent, Value, agent_of, failure, fault, image,
    interface_tools, next_line, page_bytes, text,
};

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
/// What the server tells an agent of its tools.
const INSTRUCTIONS: &str = "Drives a game built with SlimeEngine through its development channel. \
     Start it with `start_game`; `pause` it, `step` it, `resume` it, `set_speed` changes the speed \
     of its game time, `game_state` says where it stands, with its views and its reports; \
     `list_entities` and `inspect_entity` read its world, the lists a page at a time, each page \
     that more follow starting with its `next`, which `cursor` takes, `set_property` changes it; `press_key` and \
     `release_key` play it; `capture` shows its image or one of its views, and \
     `set_view_settings` changes how a view draws, its scale, anti-aliasing, passes or a mode of \
     debugging that shows normals, shadows, the base color or the light alone; `pick` names the entity drawn at a pixel of its \
     image or of a view; `read_report` reads its reports, as its asset cache's; \
     `read_logs` reads its log; `save_game` and `load_game` save its run to a file and load it; \
     `minimize_game` hides its window, `stop_game` ends it. A game's interface is read and used as a person uses it: \
     `find_elements` lists its elements by role and name, each with its path, `element_at` names \
     the one at a pixel, `act_on_interface` presses, focuses or scrolls one, gives a field, a \
     slider, two panels, a list to choose from or a color well a value, or moves the focus, \
     `capture_element` shows one, and `element_style` says where the code that wrote its own \
     style is. An editor, started as a game is (`cargo run --example \
     editor -- --level <a copy of a level> --journal <a file of its own>`), edits its level: \
     `edit` makes a step of commands, `undo` and `redo` undo and make it again, `select` selects \
     entities, `editor_history` says its steps and selection, `editor_journal` gives its journal by pages, \
     `write_level` writes the level back to its file, `list_operations` lists every action of its \
     menus, shortcuts and palette, and `run_operation` runs one by its name, as a person does. \
     Once an editor plays its level (`run_operation` `play.start`), `drive_played_game` points \
     the game's tools at the game that it plays, and back at the editor; the editor's tools reach \
     the editor all along, and a step that it makes reaches the game a moment after it answers. \
     `stop_game` ends the editor, and the game that it plays with it; `play.stop` ends that game \
     alone. An editor opens a window unless it is started with `--headless`. `cook` and `run_tests` run \
     in the repository's root, where the server runs.";

/// The game that the server drives, if one runs. Its connections are shared, so that an agent's
/// calls go side by side: a tool that asks the game holds the server's lock to choose one, or to
/// replace one lost, never while its request waits for its answer. `stop_game` and
/// `drive_played_game` hold it for their own requests, so that one game at a time runs, and they
/// wait for a place in flight like the others.
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
    /// A connection to the game that the program, an editor, plays, while the game's tools drive
    /// it (`drive_played_game`, `docs/adr/0165-editing-while-playing.md`).
    played: Option<Arc<Client>>,
}

/// The server, as it names itself to the games that it connects to.
fn server() -> Named {
    Named::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

/// The server: at most one game at a time. It carries the requests of uwui's MCP library, the
/// interface's tools, to the game (`docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md`).
#[derive(Clone)]
struct Server {
    game: Arc<Mutex<Option<Game>>>,
    /// Its tools, its own and the library's, shared by its clones, one for each request.
    tools: Arc<ToolRouter<Self>>,
}

impl Server {
    fn new() -> Self {
        Self {
            game: Arc::default(),
            tools: Arc::new(Self::tool_router() + interface_tools::<Self>()),
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

/// What `edit` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EditStep {
    /// The step's name, which the history shows: `Light the gate`.
    label: String,
    /// The commands, each as text in the engine's notation, applied in their order, each as the
    /// world is when it comes: `[Create]\nentity = "yard/torch-0rdx"\nparent = "yard"\n\n
    /// [Create.components]\nName = "torch"\n`, `[Delete]\nentity = "yard/lamp 4"\n`. A table named
    /// after the command's kind: `SetProperty` (entity, component, path, value), `Create` (entity,
    /// parent, rank, components), `Place` (entity, parent, rank, prefab, its asset ID), `Delete`
    /// (entity), `Reparent` (entity, parent, rank), `Insert` (entity, component, value), `Remove`
    /// (entity, component). An entity that a command creates is named under the level's key,
    /// `yard/<a name and four letters or digits>`; entities are under the level's root.
    commands: Vec<String>,
    /// The entities to select after the step, by persistent identity; left out to keep the
    /// selection.
    selection: Option<Vec<String>>,
}

/// What `select` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Select {
    /// The entities to select, by persistent identity; empty for none.
    selection: Vec<String>,
}

/// What `run_operation` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RunOperation {
    /// The operation's name, as `list_operations` lists it: `edit.undo`, `file.open`.
    name: String,
    /// Its arguments, as text, in the order of its parameters: a level's path for `file.open`;
    /// left out for none.
    #[serde(default)]
    arguments: Vec<String>,
}

/// What `step` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Step {
    /// How many fixed steps of game time to run, 1/60 s each by default.
    steps: u32,
}

/// What `set_speed` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Speed {
    /// The speed of game time: 1 is real time, 0.25 a quarter of it, 2 twice; above 0.
    speed: f64,
}

/// What `list_entities` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListEntities {
    /// Text that the entities' names or persistent identities hold, whatever its case; empty for
    /// every entity.
    filter: String,
    /// Where the page starts: the `next` that the page before gave, with the same `filter`; left
    /// out for the list's start.
    cursor: Option<String>,
    /// The most entities wanted on the page; left out for as many as fit.
    limit: Option<u64>,
}

/// What `editor_journal` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EditorJournal {
    /// Where the page starts: the `next` that the page before gave; left out for the journal's
    /// start.
    cursor: Option<String>,
    /// The most bytes of the page, as JSON writes its text; left out for as many as the server
    /// gives, some 32 KiB.
    bytes: Option<u64>,
}

/// What `inspect_entity` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct InspectEntity {
    /// The entity's persistent identity, as `list_entities` gives it: `hero`, `zone/door`.
    entity: String,
}

/// What `set_property` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetProperty {
    /// The entity's persistent identity.
    entity: String,
    /// The component's type, as `inspect_entity` names it: `Transform`, `Light`.
    component: String,
    /// The part of the component, as a property path: `translation.x`, `intensity`; empty for the
    /// whole component.
    path: String,
    /// The new value, in the engine's notation, as `inspect_entity` writes values: `12.5`, `true`,
    /// `"text"`, `{ x = 1.0, y = 0.0, z = 2.0 }`.
    value: String,
}

/// What `set_view_settings` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetViewSettings {
    /// The view, by the name that `game_state` lists.
    view: String,
    /// The part of its settings, as a property path, as `game_state` writes them: `render_scale`,
    /// `anti_aliasing`, `debug`, `shadows`, `sky`, `bloom`; empty for the whole settings.
    path: String,
    /// The new value, in the engine's notation: `0.5`, `false`, `"Off"`, `"Msaa4"`, `"Normals"`,
    /// `"Shadows"`.
    value: String,
}

/// What `read_report` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadReport {
    /// The report, by the name that `game_state` lists: `assets`, the asset cache's requests and
    /// memory against its budgets; `views`, the statistics of each view's last frame, its times in
    /// seconds.
    name: String,
}

/// What `press_key` and `release_key` take.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Key {
    /// The key, by its position on a US keyboard: `KeyW`, `KeyA`, `ArrowLeft`, `Space`, `Escape`,
    /// `F5`.
    key: String,
}

/// What `capture` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Capture {
    /// The view whose part of the image to give, by the name that `game_state` lists; the whole
    /// image if left out.
    view: Option<String>,
}

/// What `pick` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Pick {
    /// The view, by the name that `game_state` lists, whose pixel `x` and `y` count from its top
    /// left corner; the whole image if left out, where the last view drawn at the pixel is picked.
    view: Option<String>,
    /// The pixel's column, from the left, as in the image that `capture` gives.
    x: u32,
    /// Its row, from the top.
    y: u32,
}

/// What `drive_played_game` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DrivePlayed {
    /// Whether the game's tools drive the game that the editor plays, `true`, or the editor again,
    /// `false`.
    played: bool,
}

/// What `read_logs` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadLogs {
    /// The number of the last line already read; 0 for the oldest that the game keeps.
    after: u64,
}

/// What `save_game` and `load_game` take.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SaveFile {
    /// The save's file, a path on the game's machine: one of the agent's own, never the player's
    /// save.
    file: String,
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

/// `error`'s message, followed by those of its causes, as `slime_app::error_chain` writes them,
/// which this program does not link.
fn causes(error: &dyn std::error::Error) -> String {
    let mut said = error.to_string();
    let mut cause = error.source();
    while let Some(error) = cause {
        said.push_str(": ");
        said.push_str(&error.to_string());
        cause = error.source();
    }
    said
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

impl Server {
    /// Asks the game `request`, on a thread that may wait, so that the server goes on answering,
    /// and gives its reply as a result: the game that an editor plays while the game's tools
    /// drive it, but for the editor's requests (`docs/adr/0165-editing-while-playing.md`), the
    /// program that `start_game` started otherwise. An agent's calls go side by side, each in
    /// flight on the connection at once (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
    /// A game that ended is forgotten, its end told; once the connection to the program is lost, a
    /// new one replaces it; once the one to the game that an editor plays is, the game's tools
    /// drive the editor again.
    async fn ask(&self, request: Request) -> Result<CallToolResult, McpError> {
        Ok(match self.reply(request).await? {
            Ok((reply, payload)) => describe(reply, &payload),
            Err(why) => failure(why),
        })
    }

    /// A page of a list for an agent, from `request`, a request of a list that asks `bytes` of
    /// JSON at most, [`PAGE_BYTES`] or fewer: the pages that the game gives, each after the last,
    /// gathered until they hold about `bytes` of JSON, the list ends or [`LIST_TIME`] passed, so
    /// that a page that the game cut short, or that found none, does not reach the agent alone. It
    /// gives the last page's `next`.
    async fn list(&self, first: Request, bytes: u64) -> Result<CallToolResult, McpError> {
        let started = Instant::now();
        let mut request = first.clone();
        let mut gathered: Option<Reply> = None;
        loop {
            let reply = match self.reply(request.clone()).await? {
                Ok((reply, _)) => reply,
                Err(why) => return Ok(failure(why)),
            };
            let Some(next) = paged(&mut gathered, reply) else {
                break;
            };
            if started.elapsed() >= LIST_TIME {
                break;
            }
            let Some(asked) = gathered
                .as_ref()
                .and_then(|gathered| rest_of(&first, next, gathered, bytes))
            else {
                break;
            };
            request = asked;
        }
        Ok(gathered.map_or_else(|| text("no page"), |reply| describe(reply, &[])))
    }

    /// The game's reply to `request`, as [`Server::ask`] asks it, or why there is none.
    async fn reply(&self, request: Request) -> Result<Result<(Reply, Vec<u8>), String>, McpError> {
        let game = Arc::clone(&self.game);
        tokio::task::spawn_blocking(move || {
            let to_editor = matches!(request, Request::Edit { .. });
            let (client, to_played) = connection(&game, to_editor)?;
            client
                .request(&request)
                .map_err(|error| recovered(&game, &client, to_played, &causes(&error)))
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))
    }
}

impl Carrier for Server {
    /// Sends the library's request to the game, as [`Server::ask`] sends the server's own: to the
    /// game that an editor plays while the game's tools drive it, but for the editor's requests.
    fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        let game = Arc::clone(&self.game);
        let (client, to_played) = connection(&game, method.starts_with("editing/"))?;
        let pending = client
            .send_call(method, params)
            .map_err(|error| recovered(&game, &client, to_played, &causes(&error)))?;
        let canceller = pending.canceller();
        Ok(Sent::new(
            move || match pending.wait_outcome() {
                Ok((outcome, bytes)) => Ok(Answer { outcome, bytes }),
                Err(error) => Err(recovered(&game, &client, to_played, &causes(&error))),
            },
            move || canceller.cancel(),
        ))
    }
}

/// The connection that a request goes to, chosen under the server's lock, which the request does
/// not hold while it waits for its answer, and whether it is the one to the game that an editor
/// plays: what is not an editor's request, `to_editor`, reaches that game while the game's tools
/// drive it. A game that ended is forgotten, its end told.
fn connection(game: &Mutex<Option<Game>>, to_editor: bool) -> Result<(Arc<Client>, bool), String> {
    let mut slot = game.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(running) = slot.as_mut() else {
        return Err("no game runs: start one with `start_game`".to_owned());
    };
    if let Some(ended) = running.ended() {
        *slot = None;
        return Err(ended);
    }
    Ok(match &running.played {
        Some(played) if !to_editor => (Arc::clone(played), true),
        _ => (Arc::clone(&running.client), false),
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
            .is_some_and(|played| Arc::ptr_eq(played, client));
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
            causes(&again)
        ),
    }
}

/// Adds the page `reply` to the pages `gathered`: its `next`, while the list goes on; none at its
/// end, or for a reply that is no page, as a fault, which then replaces them.
fn paged(gathered: &mut Option<Reply>, reply: Reply) -> Option<String> {
    match (gathered.as_mut(), reply) {
        (
            Some(Reply::Entities { entities, next }),
            Reply::Entities {
                entities: more,
                next: then,
            },
        ) => {
            entities.extend(more);
            *next = then;
            next.clone()
        }
        (
            Some(Reply::Journal { text, next, lossy }),
            Reply::Journal {
                text: more,
                next: then,
                lossy: replaced,
            },
        ) => {
            text.push_str(&more);
            *next = then;
            *lossy |= replaced;
            next.clone()
        }
        (_, reply) => {
            let next = match &reply {
                Reply::Entities { next, .. } | Reply::Journal { next, .. } => next.clone(),
                _ => None,
            };
            *gathered = Some(reply);
            next
        }
    }
}

/// How many items the pages gathered hold, and the bytes of JSON that they take.
fn page_size(gathered: &Reply) -> (u64, u64) {
    let (count, size) = match gathered {
        Reply::Entities { entities, .. } => (entities.len(), json_size(entities)),
        Reply::Journal { text, .. } => (0, json_size(text)),
        _ => (0, 0),
    };
    (count as u64, size as u64)
}

/// The request of the next page of the list that `first` asked within `bytes` of JSON, from
/// `next`, the last page's cursor, once the pages `gathered` hold what they hold: none once they
/// hold half of `bytes` or more, as the next page may be cut short, nor once the list's `limit` is
/// reached.
fn rest_of(first: &Request, next: String, gathered: &Reply, bytes: u64) -> Option<Request> {
    let (count, held) = page_size(gathered);
    if held >= bytes / 2 {
        return None;
    }
    resumed(first, next, count, bytes - held)
}

/// The rest of the list that `request` asked, its first page, once `count` items of it were
/// gathered: from `cursor`, of at most `bytes` of JSON and of what its `limit` leaves; none for a
/// request of no list, or whose `limit` is reached.
fn resumed(request: &Request, cursor: String, count: u64, bytes: u64) -> Option<Request> {
    let mut request = request.clone();
    match &mut request {
        Request::Entities {
            cursor: at,
            limit,
            bytes: most,
            ..
        } => {
            if let Some(limit) = limit {
                *limit = limit.checked_sub(count).filter(|&left| left > 0)?;
            }
            *at = Some(cursor);
            *most = Some(bytes);
        }
        Request::Edit {
            edit:
                Edit::Journal {
                    cursor: at,
                    bytes: most,
                },
        } => {
            *at = Some(cursor);
            *most = Some(bytes);
        }
        _ => return None,
    }
    Some(request)
}

/// The result that tells `reply`, with `payload`, an image's pixels, after it.
fn describe(reply: Reply, payload: &[u8]) -> CallToolResult {
    match reply {
        Reply::Done => text("done"),
        Reply::Failed { code, message } => fault(Fault::new(code, message)),
        Reply::State { state } => text(format!(
            "frame {}, game time {}: {} fixed steps run, {:.3} s of game time, at a speed of {}; \
             {:.3} s of real time; frames of {:.2} ms on average, {:.2} ms at the longest, over the \
             last ones; {} entities; {}; {}{}",
            state.frame,
            match (state.paused, state.held) {
                (true, true) => "paused, and held by the game, as while it loads",
                (true, false) => "paused",
                (false, true) => "held by the game, as while it loads",
                (false, false) => "running",
            },
            state.steps,
            state.game_seconds,
            state.speed,
            state.real_seconds,
            state.frame_ms,
            state.longest_frame_ms,
            state.entities,
            if state.views.is_empty() {
                "no view named".to_owned()
            } else {
                let names: Vec<&str> = state.views.iter().map(|view| view.name.as_str()).collect();
                format!("views `{}`", names.join("`, `"))
            },
            if state.reports.is_empty() {
                "no report".to_owned()
            } else {
                format!("reports `{}`", state.reports.join("`, `"))
            },
            state
                .views
                .iter()
                .map(|view| if view.settings.is_empty() {
                    format!("\n\nview `{}`: its settings are not given", view.name)
                } else {
                    format!(
                        "\n\nview `{}`, its settings:\n{}",
                        view.name,
                        view.settings.trim_end()
                    )
                })
                .collect::<String>(),
        )),
        Reply::Entities { entities, next } => {
            let mut lines: Vec<String> = next.as_deref().map(next_line).into_iter().collect();
            if entities.is_empty() {
                lines.push("no entity on this page".to_owned());
            }
            lines.extend(entities.into_iter().map(|entity| {
                let mut line = if entity.name.is_empty() {
                    entity.identity
                } else {
                    format!("{} ({})", entity.identity, entity.name)
                };
                if !entity.cut.is_empty() {
                    line.push_str(&format!(" [cut: {}]", entity.cut.join(", ")));
                }
                line
            }));
            text(lines.join("\n"))
        }
        Reply::Entity {
            entity,
            parent,
            components,
        } => {
            let mut said = entity.identity;
            if !entity.name.is_empty() {
                said.push_str(&format!(", named {}", entity.name));
            }
            if !parent.is_empty() {
                said.push_str(&format!(", child of {parent}"));
            }
            for component in components {
                said.push_str(&format!("\n\n[{}]\n{}", component.name, component.value));
            }
            text(said)
        }
        // The interface's tools ask through uwui's MCP library, which reads its family's replies.
        Reply::Interface { reply } => failure(format!("the game answered {reply:?}")),
        Reply::Report { text: report } => text(report),
        Reply::Picked { entity, found } => text(if !found {
            "nothing is picked there".to_owned()
        } else if entity.identity.is_empty() {
            format!(
                "an entity without a persistent identity, named `{}`",
                entity.name
            )
        } else if entity.name.is_empty() {
            entity.identity
        } else {
            format!("{} ({})", entity.identity, entity.name)
        }),
        Reply::Logs { lines, partial } => {
            let mut said: Vec<String> = Vec::new();
            if partial && let Some(last) = lines.last() {
                said.push(format!(
                    "more lines follow: read them with `after: {}`",
                    last.number
                ));
            }
            said.extend(lines.into_iter().map(|line| {
                let cut = if line.cut { " [cut]" } else { "" };
                format!("{}: {}{cut}", line.number, line.text)
            }));
            text(said.join("\n"))
        }
        Reply::Image { width, height } => image(width, height, payload),
        Reply::History { history } => {
            let list = |labels: &[String]| {
                if labels.is_empty() {
                    "none".to_owned()
                } else {
                    format!("`{}`", labels.join("`, `"))
                }
            };
            text(format!(
                "level {}; steps done, the oldest first: {}; steps undone, the next to redo first: \
                 {}; selected: {}; steps that the level's file does not hold: {}; entries of a \
                 journal recovered at the start: {}; journal {}",
                history.level,
                list(&history.done),
                list(&history.undone),
                list(&history.selection),
                history.unwritten,
                history.recovered,
                history.journal
            ))
        }
        Reply::Journal {
            text: journal,
            next,
            lossy,
        } => {
            // Where it goes on and how it was read first, apart from the journal's own text.
            let mut said: Vec<String> = next.as_deref().map(next_line).into_iter().collect();
            if lossy {
                said.push(
                    "bytes of this page that are not UTF-8 were each replaced, by run, with U+FFFD"
                        .to_owned(),
                );
            }
            let mut blocks: Vec<ContentBlock> = said.into_iter().map(ContentBlock::text).collect();
            blocks.push(ContentBlock::text(journal));
            CallToolResult::success(blocks)
        }
        Reply::Operations { operations } => text(
            operations
                .iter()
                .map(|operation| {
                    let parameters = if operation.parameters.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", operation.parameters.join(", "))
                    };
                    format!(
                        "{}{parameters}: {}; changes the {}",
                        operation.name, operation.label, operation.changes
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Reply::Written { left_out } => text(if left_out.is_empty() {
            "the level is written, whole".to_owned()
        } else {
            format!(
                "the level is written; its file could not hold:\n{}",
                left_out.join("\n")
            )
        }),
        Reply::Played { address, .. } => text(format!(
            "the editor plays a game, which listens at {address}"
        )),
    }
}

#[tool_router]
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
            .map_err(|error| format!("{}\n{}", causes(&error), tail(&log)))?;
            let client = match Client::connect(launched.address, &launched.token, &agent, &server())
            {
                Ok(client) => client,
                Err(error) => {
                    let _ = launched.child.kill();
                    let _ = launched.child.wait();
                    return Err(causes(&error));
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
        description = "The game's state: its frame, whether game time is paused, the fixed steps run, \
                       game time and real time, its frames' times, its entities, the views that \
                       its last frame drew and the reports it gives."
    )]
    async fn game_state(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::State).await
    }

    #[tool(description = "Pauses game time: the world stands still, but for commands.")]
    async fn pause(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Pause).await
    }

    #[tool(description = "Lets game time run again.")]
    async fn resume(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Resume).await
    }

    #[tool(
        description = "Minimizes the game's window, as the user would: the compositor hides it, \
                       and a game that rests while hidden draws nothing. Only the user shows it \
                       again."
    )]
    async fn minimize_game(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Minimize).await
    }

    #[tool(
        description = "While game time is paused, runs that many fixed steps of it, over the next \
                       frames, and answers once they have all run."
    )]
    async fn step(
        &self,
        Parameters(Step { steps }): Parameters<Step>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Step { steps }).await
    }

    #[tool(description = "Sets the speed of game time, whose fixed steps keep their duration.")]
    async fn set_speed(
        &self,
        Parameters(Speed { speed }): Parameters<Speed>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Speed { speed }).await
    }

    #[tool(
        description = "Lists the world's entities by persistent identity and name, a page at a \
                       time: a page that more follow starts with `next`, which `cursor` takes for \
                       the rest. Texts too long for a page are cut, and marked."
    )]
    async fn list_entities(
        &self,
        Parameters(ListEntities {
            filter,
            cursor,
            limit,
        }): Parameters<ListEntities>,
    ) -> Result<CallToolResult, McpError> {
        self.list(
            Request::Entities {
                filter,
                cursor,
                limit,
                bytes: Some(PAGE_BYTES),
            },
            PAGE_BYTES,
        )
        .await
    }

    #[tool(
        description = "An entity's parent and its components, each written as the engine's \
                       documents write it."
    )]
    async fn inspect_entity(
        &self,
        Parameters(InspectEntity { entity }): Parameters<InspectEntity>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Entity { entity }).await
    }

    #[tool(
        description = "Sets a part of an entity's component, as the editor's commands do: the value \
                       is checked against the component's type and its ranges."
    )]
    async fn set_property(
        &self,
        Parameters(SetProperty {
            entity,
            component,
            path,
            value,
        }): Parameters<SetProperty>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::SetProperty {
            entity,
            component,
            path,
            value,
        })
        .await
    }

    #[tool(
        description = "Puts a key down, as the player would, until `release_key`: the game reads it \
                       held from its next frame on."
    )]
    async fn press_key(
        &self,
        Parameters(Key { key }): Parameters<Key>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Press { key }).await
    }

    #[tool(description = "Lets a key go.")]
    async fn release_key(
        &self,
        Parameters(Key { key }): Parameters<Key>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Release { key }).await
    }

    #[tool(
        description = "The image that the game shows, as the player sees it, dev overlay included, \
                       once its next frame is drawn; or the part of it that one of its views \
                       draws, named as `game_state` lists them."
    )]
    async fn capture(
        &self,
        Parameters(Capture { view }): Parameters<Capture>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Capture {
            view: view.unwrap_or_default(),
        })
        .await
    }

    #[tool(
        description = "The entity drawn at a pixel of the game's image, or of one of its views, \
                       counted as in the image that `capture` gives: by its persistent identity \
                       and its name, as `list_entities` gives them, or nothing. The game picks it \
                       on the GPU, exactly as it draws it, a frame or more later; the editor \
                       picks nothing that its user hid or locked."
    )]
    async fn pick(
        &self,
        Parameters(Pick { view, x, y }): Parameters<Pick>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Pick {
            view: view.unwrap_or_default(),
            x,
            y,
        })
        .await
    }

    #[tool(
        description = "Changes a part of the settings of one of the game's views, from its next \
                       frame: its render scale, anti-aliasing, mode of debugging or passes, as \
                       `game_state` lists them, a new render scale once the view's size holds \
                       still; the value is checked against their type and ranges."
    )]
    async fn set_view_settings(
        &self,
        Parameters(SetViewSettings { view, path, value }): Parameters<SetViewSettings>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::SetViewSettings { view, path, value })
            .await
    }

    #[tool(
        description = "One of the game's reports, as `game_state` lists them, written as the \
                       engine's documents write values: `assets`, its asset cache's requests by \
                       priority and its memory against its budgets; `views`, its views' \
                       statistics."
    )]
    async fn read_report(
        &self,
        Parameters(ReadReport { name }): Parameters<ReadReport>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Report { name }).await
    }

    #[tool(
        description = "The lines of the game's log after a given line, oldest first, a page at a \
                       time: a page that more follow says so first. A line too long is cut, and \
                       marked."
    )]
    async fn read_logs(
        &self,
        Parameters(ReadLogs { after }): Parameters<ReadLogs>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Logs {
            after,
            limit: None,
            bytes: Some(PAGE_BYTES),
        })
        .await
    }

    #[tool(
        description = "Saves the game's run to a file, as the game saves its own runs, at the start \
                       of its next frame."
    )]
    async fn save_game(
        &self,
        Parameters(SaveFile { file }): Parameters<SaveFile>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Save { file }).await
    }

    #[tool(
        description = "Loads the run of a save's file in place of the game's run, as the game resumes \
                       its own saves, at the start of its next frame: a save that the game does not \
                       resume leaves the run as it is."
    )]
    async fn load_game(
        &self,
        Parameters(SaveFile { file }): Parameters<SaveFile>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Load { file }).await
    }

    #[tool(
        description = "In an editor, makes a step of commands given as text, whole or not at all, at \
                       the start of its next frame, and selects what the step gives to select."
    )]
    async fn edit(
        &self,
        Parameters(EditStep {
            label,
            commands,
            selection,
        }): Parameters<EditStep>,
    ) -> Result<CallToolResult, McpError> {
        let edit = Edit::Step {
            label,
            commands,
            selection,
        };
        self.ask(Request::Edit { edit }).await
    }

    #[tool(description = "In an editor, undoes the last step done.")]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit { edit: Edit::Undo }).await
    }

    #[tool(description = "In an editor, does again the last step undone.")]
    async fn redo(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit { edit: Edit::Redo }).await
    }

    #[tool(
        description = "In an editor, selects entities by their persistent identities, which is not \
                       a step: undoing a step selects again what was selected before it."
    )]
    async fn select(
        &self,
        Parameters(Select { selection }): Parameters<Select>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit {
            edit: Edit::Select { selection },
        })
        .await
    }

    #[tool(
        description = "An editor's level, its steps done and undone, what is selected, and what it \
                       recovered of a journal as it started."
    )]
    async fn editor_history(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit {
            edit: Edit::History,
        })
        .await
    }

    #[tool(
        description = "An editor's journal, as text, a page at a time: the level as the journal \
                       started, on one line that may take many pages, then each step made, undone \
                       and done again, each selection and each writing. A page that more follow \
                       starts with `next`, which `cursor` takes for the rest; one whose bytes were \
                       not text says so."
    )]
    async fn editor_journal(
        &self,
        Parameters(EditorJournal { cursor, bytes }): Parameters<EditorJournal>,
    ) -> Result<CallToolResult, McpError> {
        let bytes = page_bytes(bytes);
        self.list(
            Request::Edit {
                edit: Edit::Journal {
                    cursor,
                    bytes: Some(bytes),
                },
            },
            bytes,
        )
        .await
    }

    #[tool(
        description = "In an editor that plays its level (`play.start`, once the game runs), points \
                       the game's tools at the game that it plays, `played: true`, or back at the \
                       editor, `played: false`; the editor's tools always reach the editor, and \
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
                Err(error) => return Err(causes(&error)),
            };
            let address: SocketAddr = address
                .parse()
                .map_err(|error| format!("the editor gave no address: {error}"))?;
            let token: Token = token
                .parse()
                .map_err(|error| format!("the editor gave no token: {error}"))?;
            let client = Client::connect(address, &token, &running.agent, &server())
                .map_err(|error| causes(&error))?;
            running.played = Some(Arc::new(client));
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
        description = "Writes an editor's level back to its file, whole or not at all, its comments \
                       kept, and says what the file could not hold."
    )]
    async fn write_level(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit { edit: Edit::Write }).await
    }

    #[tool(
        description = "An editor's operations, every action that its menus, its shortcuts and its \
                       palette run: each name, its parameters, its label, and what it changes: \
                       the history, the level's file, the selection, the level by a step, what \
                       it copied, the window or the program."
    )]
    async fn list_operations(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit {
            edit: Edit::Operations,
        })
        .await
    }

    #[tool(
        description = "In an editor, runs an operation by its name, as `list_operations` lists it, \
                       with its arguments as text in the order of its parameters, as a person runs \
                       it from a menu, a shortcut or the palette."
    )]
    async fn run_operation(
        &self,
        Parameters(RunOperation { name, arguments }): Parameters<RunOperation>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit {
            edit: Edit::Run { name, arguments },
        })
        .await
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

#[tool_handler(router = self.tools)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("slime-mcp", VERSION))
            .with_instructions(INSTRUCTIONS)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::new();
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
    use slime_channel::EntitySummary;

    /// The server's own tools and the library's: none takes a name that another took, which the
    /// router would replace silently.
    #[test]
    fn no_tool_takes_another_s_name() {
        let own = Server::tool_router().list_all().len();
        let library = interface_tools::<Server>().list_all().len();
        assert_eq!(Server::new().tools.list_all().len(), own + library);
    }

    fn entity(name: &str) -> EntitySummary {
        EntitySummary {
            identity: String::new(),
            name: name.to_owned(),
            cut: Vec::new(),
        }
    }

    fn entities(names: &[&str], next: Option<&str>) -> Reply {
        Reply::Entities {
            entities: names.iter().map(|name| entity(name)).collect(),
            next: next.map(str::to_owned),
        }
    }

    /// The pages of a list gathered into one, with the last page's `next`; a fault in place of
    /// what was gathered.
    #[test]
    fn pages_are_gathered_with_the_last_next() {
        let mut gathered = None;
        assert_eq!(
            paged(&mut gathered, entities(&["a"], Some("1"))),
            Some("1".to_owned())
        );
        assert_eq!(
            paged(&mut gathered, entities(&[], Some("2"))),
            Some("2".to_owned())
        );
        assert_eq!(paged(&mut gathered, entities(&["b", "c"], None)), None);
        assert_eq!(gathered, Some(entities(&["a", "b", "c"], None)));
        assert_eq!(page_size(gathered.as_ref().expect("gathered")).0, 3);
        let stale = Reply::Failed {
            code: Fault::STALE,
            message: "again".to_owned(),
        };
        assert_eq!(paged(&mut gathered, stale.clone()), None);
        assert_eq!(gathered, Some(stale));
    }

    /// The rest of a list is asked from the last cursor, within what is left of its `limit`, and
    /// not at all once the limit is reached.
    #[test]
    fn the_rest_is_asked_within_the_limit_left() {
        let first = Request::Entities {
            filter: "lamp".to_owned(),
            cursor: None,
            limit: Some(5),
            bytes: Some(PAGE_BYTES),
        };
        assert_eq!(
            resumed(&first, "c".to_owned(), 2, 100),
            Some(Request::Entities {
                filter: "lamp".to_owned(),
                cursor: Some("c".to_owned()),
                limit: Some(3),
                bytes: Some(100),
            })
        );
        assert_eq!(
            resumed(&first, "c".to_owned(), 5, 100),
            None,
            "the limit reached"
        );
        assert_eq!(resumed(&Request::State, "c".to_owned(), 0, 100), None);
    }

    fn journal(text: &str, next: Option<&str>, lossy: bool) -> Reply {
        Reply::Journal {
            text: text.to_owned(),
            next: next.map(str::to_owned),
            lossy,
        }
    }

    /// The journal's pages gathered into one text, lossy when one was, with the last `next`; the
    /// rest asked from it within the bytes left; the agent shown `next` first, apart from the
    /// journal's text.
    #[test]
    fn the_journal_s_pages_are_gathered_its_next_shown_first() {
        let mut gathered = None;
        assert_eq!(
            paged(&mut gathered, journal("# level\n", Some("1"), false)),
            Some("1".to_owned())
        );
        assert_eq!(
            paged(&mut gathered, journal("\u{fffd}", Some("2"), true)),
            Some("2".to_owned())
        );
        paged(&mut gathered, journal("# do\n", Some("3"), false));
        let gathered = gathered.expect("gathered");
        assert_eq!(
            gathered,
            journal("# level\n\u{fffd}# do\n", Some("3"), true)
        );
        assert_eq!(page_size(&gathered).1, 20);
        let first = Request::Edit {
            edit: Edit::Journal {
                cursor: None,
                bytes: Some(4096),
            },
        };
        assert_eq!(
            resumed(&first, "3".to_owned(), 0, 4000),
            Some(Request::Edit {
                edit: Edit::Journal {
                    cursor: Some("3".to_owned()),
                    bytes: Some(4000),
                },
            })
        );
        let said: Vec<String> = describe(gathered, &[])
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect();
        assert_eq!(said.len(), 3, "{said:?}");
        assert!(said[0].starts_with("next: 3 "), "{said:?}");
        assert!(said[1].contains("not UTF-8"), "{said:?}");
        assert_eq!(said[2], "# level\n\u{fffd}# do\n");
        // A page short of half the agent's bytes is followed by one of the bytes left.
        let short = journal(&"x".repeat(500), Some("4"), false);
        assert_eq!(
            rest_of(&first, "4".to_owned(), &short, 2048),
            Some(Request::Edit {
                edit: Edit::Journal {
                    cursor: Some("4".to_owned()),
                    bytes: Some(2048 - 502),
                },
            })
        );
        let half = journal(&"x".repeat(1022), Some("5"), false);
        assert_eq!(rest_of(&first, "5".to_owned(), &half, 2048), None);
    }
}
