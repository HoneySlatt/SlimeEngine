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

use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use slime_channel::{Client, Edit, InterfaceAction, Reply, Request, Token, launch};

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
     `list_entities` and `inspect_entity` read its world, `set_property` changes it; `press_key` and \
     `release_key` play it; `capture` shows its image or one of its views, and \
     `set_view_settings` changes how a view draws, its scale, anti-aliasing, passes or a mode of \
     debugging that shows normals or shadows; `pick` names the entity drawn at a pixel of its \
     image or of a view; `read_report` reads its reports, as its asset cache's; \
     `read_logs` reads its log; `save_game` and `load_game` save its run to a file and load it; \
     `stop_game` ends it. A game's interface is read and used as a person uses it: \
     `find_elements` lists its elements by role and name, each with its path, `element_at` names \
     the one at a pixel, `act_on_interface` presses, focuses or scrolls one, gives a field, a \
     slider or two panels a value, or moves the focus, \
     and `capture_element` shows one. An editor, started as a game is (`cargo run --example \
     editor -- --level <a copy of a level> --journal <a file of its own>`), edits its level: \
     `edit` makes a step of commands, `undo` and `redo` undo and make it again, `select` selects \
     entities, `editor_history` says its steps and selection, `editor_journal` gives its journal, \
     and `write_level` writes the level back to its file. `cook` and `run_tests` run in the \
     repository's root, where the server runs.";

/// The game that the server drives, if one runs.
struct Game {
    child: Child,
    client: Client,
    /// Where it listens, and its token: a new connection replaces one that broke.
    address: SocketAddr,
    token: Token,
    /// The file that holds the game's standard error, its log.
    log: PathBuf,
}

/// The server: at most one game at a time.
#[derive(Clone, Default)]
struct Server {
    game: Arc<Mutex<Option<Game>>>,
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

/// What `find_elements` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FindElements {
    /// The role of the elements wanted, as AccessKit names roles, whatever its case: `Button`,
    /// `Label`, `Image`, `List`, `ListItem`, `ScrollView`, `Dialog`, `Toolbar`, `Group`,
    /// `TextInput`, `SearchInput`, `MultilineTextInput`, `SpinButton`, `CheckBox`, `Slider`,
    /// `Details`, `TabList`, `Tab`, `TabPanel`, `Splitter`, `Menu`, `MenuBar`, `MenuItem`,
    /// `MenuItemCheckBox`, `ComboBox`, `ListBox`, `ListBoxOption`, `Pane`, `ToggleButton`,
    /// `RadioGroup`, `RadioButton`, `ProgressIndicator`, `Status`, `Alert`, `ColorWell`; any if
    /// left out.
    role: Option<String>,
    /// What their names hold, whatever its case: a button's label, a text; any if left out.
    name: Option<String>,
}

/// What `element_at` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ElementAt {
    /// The pixel's column, from the left, as in the image that `capture` gives.
    x: u32,
    /// Its row, from the top.
    y: u32,
}

/// What `act_on_interface` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ActOnInterface {
    /// What to do, whatever its case: `press`, `focus` or `scroll` an element, or `set_value` of a
    /// field, a slider, two panels or a list to choose from, which must take it, as `find_elements`
    /// lists; or, without an
    /// element, `next`, `previous`, `up`, `down`, `left`, `right` to move the focus as Tab and the
    /// arrows do, `accept` as Enter does, `cancel` as Escape does, `command` to give the command
    /// named `command` as a shortcut does.
    action: String,
    /// The element, by the path that `find_elements` gives: `#0/bar/open`.
    element: Option<String>,
    /// For `scroll`, the offset from the start of the content, in logical pixels.
    x: Option<f32>,
    /// For `scroll`, the offset from its top.
    y: Option<f32>,
    /// For `command`, the command's name: `undo`.
    command: Option<String>,
    /// For `set_value`, the text that the field holds then, as a person who replaced what it holds
    /// and committed would leave it; for a field of numbers, an expression of a finite number, in
    /// a unit of its family where it has one: `2.5`, `sqrt(2)/2`, `10cm`; for a slider, a finite
    /// number; for two panels, the share of the first, from 0 to 1; for a list to choose from, one
    /// of its options, `High`, or the place of one, from 0.
    value: Option<String>,
}

/// What `capture_element` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CaptureElement {
    /// The element, by the path that `find_elements` gives.
    element: String,
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

/// A result that says what failed, which the agent reads.
fn failure(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// A result of text.
fn text(message: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.into())])
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

/// `pixels`, `width` × `height` of four bytes each, as a PNG file.
fn png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(bytes)
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
    /// and gives its reply as a result. A game that ended is forgotten, its end told; after a
    /// request that failed, a new connection replaces the one that the game's late answer would
    /// throw out of step.
    async fn ask(&self, request: Request) -> Result<CallToolResult, McpError> {
        let game = Arc::clone(&self.game);
        let answer = tokio::task::spawn_blocking(move || {
            let mut slot = game.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(running) = slot.as_mut() else {
                return Err("no game runs: start one with `start_game`".to_owned());
            };
            if let Some(ended) = running.ended() {
                *slot = None;
                return Err(ended);
            }
            let error = match running.client.request(&request) {
                Ok(answer) => return Ok(answer),
                Err(error) => causes(&error),
            };
            if let Some(ended) = running.ended() {
                *slot = None;
                return Err(format!("{error}\n{ended}"));
            }
            Err(match Client::connect(running.address, &running.token) {
                Ok(client) => {
                    running.client = client;
                    format!(
                        "{error}; the server connected to the game again: the request may have run"
                    )
                }
                Err(again) => format!(
                    "{error}; connecting to the game again failed: {}",
                    causes(&again)
                ),
            })
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(match answer {
            Ok((reply, payload)) => describe(reply, &payload),
            Err(why) => failure(why),
        })
    }
}

/// The result that tells `reply`, with `payload`, an image's pixels, after it.
fn describe(reply: Reply, payload: &[u8]) -> CallToolResult {
    match reply {
        Reply::Done => text("done"),
        Reply::Failed { message } => failure(message),
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
        Reply::Entities { entities, more } => {
            let mut lines: Vec<String> = entities
                .into_iter()
                .map(|entity| {
                    if entity.name.is_empty() {
                        entity.identity
                    } else {
                        format!("{} ({})", entity.identity, entity.name)
                    }
                })
                .collect();
            if more > 0 {
                lines.push(format!("and {more} more"));
            }
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
        Reply::Elements { elements, more } => {
            let mut lines: Vec<String> = elements
                .into_iter()
                .map(|element| {
                    let mut line = format!("{} — {}", element.path, element.role);
                    if !element.name.is_empty() {
                        line.push_str(&format!(" `{}`", element.name));
                    }
                    if let Some(value) = &element.value {
                        line.push_str(&format!(", holding {value:?}"));
                    }
                    if let Some(description) = &element.description {
                        line.push_str(&format!(", described {description:?}"));
                    }
                    if let Some(placeholder) = &element.placeholder {
                        line.push_str(&format!(", showing {placeholder:?} while empty"));
                    }
                    if !element.states.is_empty() {
                        line.push_str(&format!(", {}", element.states.join(", ")));
                    }
                    line.push_str(&match element.bounds {
                        Some([x, y, width, height]) => {
                            format!("; at ({x}, {y}), {width} × {height} pixels")
                        }
                        None => "; does not show".to_owned(),
                    });
                    if !element.actions.is_empty() {
                        line.push_str(&format!("; takes {}", element.actions.join(", ")));
                    }
                    line
                })
                .collect();
            if lines.is_empty() {
                lines.push("no element".to_owned());
            }
            if more > 0 {
                lines.push(format!("and {more} more"));
            }
            text(lines.join("\n"))
        }
        Reply::Report { text: report } => text(report),
        Reply::Picked { entity, found } => text(if !found {
            "nothing is drawn there".to_owned()
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
        Reply::Logs { lines } => text(
            lines
                .into_iter()
                .map(|line| format!("{}: {}", line.number, line.text))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Reply::Image { width, height } => match png(width, height, payload) {
            Ok(bytes) => CallToolResult::success(vec![
                ContentBlock::text(format!("{width} × {height} pixels")),
                ContentBlock::image(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                    "image/png",
                ),
            ]),
            Err(error) => failure(format!("the image cannot be written as PNG: {error}")),
        },
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
                 {}; selected: {}; entries of a journal recovered at the start: {}; journal {}",
                history.level,
                list(&history.done),
                list(&history.undone),
                list(&history.selection),
                history.recovered,
                history.journal
            ))
        }
        Reply::Journal { text: journal } => text(journal),
        Reply::Written { left_out } => text(if left_out.is_empty() {
            "the level is written, whole".to_owned()
        } else {
            format!(
                "the level is written; its file could not hold:\n{}",
                left_out.join("\n")
            )
        }),
        Reply::Welcome { .. } | Reply::Refused { .. } => {
            failure("the game answered as to a connection's start")
        }
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
    ) -> Result<CallToolResult, McpError> {
        let game = Arc::clone(&self.game);
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
                Command::new(program)
                    .args(arguments)
                    .stdin(Stdio::null())
                    .stderr(log_file),
                timeout,
            )
            .map_err(|error| format!("{}\n{}", causes(&error), tail(&log)))?;
            let client = match Client::connect(launched.address, &launched.token) {
                Ok(client) => client,
                Err(error) => {
                    let _ = launched.child.kill();
                    let _ = launched.child.wait();
                    return Err(causes(&error));
                }
            };
            let said = format!(
                "{} runs, its development channel at {}; its log in {}",
                client.program(),
                launched.address,
                log.display()
            );
            *game = Some(Game {
                child: launched.child,
                client,
                address: launched.address,
                token: launched.token,
                log,
            });
            Ok(said)
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(started.map_or_else(failure, text))
    }

    #[tool(description = "Asks the game to stop, as closing its window would, and waits for it.")]
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

    #[tool(description = "Lists the world's entities by persistent identity and name.")]
    async fn list_entities(
        &self,
        Parameters(ListEntities { filter }): Parameters<ListEntities>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Entities { filter }).await
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
                       on the GPU, exactly as it draws it, a frame or more later."
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
        description = "The elements of the game's interface, in the order of its tree, as its \
                       last frame left them, whose role and name match: each by its path, which \
                       names it to the other tools, its role, its name, what a field, a slider, \
                       two panels or a list to choose from hold, its description (its tooltip), \
                       the text that a field shows while empty, \
                       its states (hovered, pressed, focused, disabled, checked, expanded, \
                       selected, has_popup for what opens a menu or a list beside it), where it \
                       shows in the image \
                       that `capture` gives, and what it takes now (press, focus, scroll, \
                       set_value). Boxes that only lay others out are left out."
    )]
    async fn find_elements(
        &self,
        Parameters(FindElements { role, name }): Parameters<FindElements>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Elements {
            role: role.unwrap_or_default(),
            name: name.unwrap_or_default(),
        })
        .await
    }

    #[tool(
        description = "The element of the game's interface at a pixel of its image, as the pointer \
                       would reach it, or the nearest above it that means something, as \
                       `find_elements` gives elements."
    )]
    async fn element_at(
        &self,
        Parameters(ElementAt { x, y }): Parameters<ElementAt>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::ElementAt { x, y }).await
    }

    #[tool(
        description = "Acts in the game's interface as a person would: presses a button, gives an \
                       element the focus, scrolls a box or a list, or gives a field, a slider or \
                       two panels a value, as \
                       if a person replaced what it holds and committed, which must take it now; \
                       or moves the focus, accepts, cancels or gives a command, as keys do. The \
                       game reads what it asks as its frame goes on."
    )]
    async fn act_on_interface(
        &self,
        Parameters(act): Parameters<ActOnInterface>,
    ) -> Result<CallToolResult, McpError> {
        let action = match act.action.to_lowercase().as_str() {
            "press" => InterfaceAction::Press,
            "focus" => InterfaceAction::Focus,
            "scroll" => match (act.x, act.y) {
                (Some(x), Some(y)) => InterfaceAction::Scroll { x, y },
                _ => return Ok(failure("`scroll` needs `x` and `y`")),
            },
            "set_value" => match act.value {
                Some(value) => InterfaceAction::SetValue { value },
                None => return Ok(failure("`set_value` needs `value`, the element's text")),
            },
            "next" => InterfaceAction::Next,
            "previous" => InterfaceAction::Previous,
            "up" => InterfaceAction::Up,
            "down" => InterfaceAction::Down,
            "left" => InterfaceAction::Left,
            "right" => InterfaceAction::Right,
            "accept" => InterfaceAction::Accept,
            "cancel" => InterfaceAction::Cancel,
            "command" => match act.command {
                Some(name) => InterfaceAction::Command { name },
                None => return Ok(failure("`command` needs `command`, the command's name")),
            },
            other => {
                return Ok(failure(format!(
                    "no action `{other}`: `press`, `focus`, `scroll`, `set_value`, `next`, \
                     `previous`, `up`, `down`, `left`, `right`, `accept`, `cancel` or `command`"
                )));
            }
        };
        self.ask(Request::Act {
            element: act.element.unwrap_or_default(),
            action,
        })
        .await
    }

    #[tool(
        description = "The part of the game's image where an element of its interface shows, by \
                       the path that `find_elements` gives, once its next frame is drawn."
    )]
    async fn capture_element(
        &self,
        Parameters(CaptureElement { element }): Parameters<CaptureElement>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::CaptureElement { element }).await
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

    #[tool(description = "The lines of the game's log after a given line, oldest first.")]
    async fn read_logs(
        &self,
        Parameters(ReadLogs { after }): Parameters<ReadLogs>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::Logs { after }).await
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
        description = "An editor's journal, as text: the level as the journal started, then each \
                       step made, undone and done again, each selection and each writing."
    )]
    async fn editor_journal(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit {
            edit: Edit::Journal,
        })
        .await
    }

    #[tool(
        description = "Writes an editor's level back to its file, whole or not at all, its comments \
                       kept, and says what the file could not hold."
    )]
    async fn write_level(&self) -> Result<CallToolResult, McpError> {
        self.ask(Request::Edit { edit: Edit::Write }).await
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

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("slime-mcp", VERSION))
            .with_instructions(INSTRUCTIONS)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::default();
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
