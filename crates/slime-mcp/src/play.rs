//! The family `play` as tools: an agent pauses, steps and plays the game, sees its image and its
//! views, picks what it draws, reads its reports and its log, and saves and loads its run.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, schemars, tool, tool_router};
use serde::Deserialize;
use slime_channel::{Fault, LogLine, PLAY, Reply, Request, json_size};
use uwui_mcp::{Family, List, PAGE_BYTES, Page, failure, gather, next_line, text};

use crate::Server;

/// What the family `play` tells an agent among the server's instructions.
const INSTRUCTIONS: &str = "The game's tools drive the game: `pause` it, `step` it, `resume` it, \
     `set_speed` changes the speed of its game time, `game_state` says where it stands, with its \
     views and its reports; `press_key` and `release_key` play it; `capture` shows its image or \
     one of its views, and `set_view_settings` changes how a view draws, its scale, \
     anti-aliasing, passes or a mode of debugging that shows normals, shadows, the base color or \
     the light alone; `pick` names the entity drawn at a pixel of its image or of a view; \
     `read_report` reads its reports, as its asset cache's; `read_logs` reads its log by pages; \
     `save_game` and `load_game` save its run to a file and load it; `minimize_game` hides its \
     window.";

/// The family `play`, for the server.
pub(crate) fn family() -> Family<Server> {
    Family::new(PLAY, INSTRUCTIONS, Server::play_tools())
}

/// What `step` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct Step {
    /// How many fixed steps of game time to run, 1/60 s each by default.
    steps: u32,
}

/// What `set_speed` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct Speed {
    /// The speed of game time: 1 is real time, 0.25 a quarter of it, 2 twice; above 0.
    speed: f64,
}

/// What `set_view_settings` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SetViewSettings {
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
pub(crate) struct ReadReport {
    /// The report, by the name that `game_state` lists: `assets`, the asset cache's requests and
    /// memory against its budgets; `views`, the statistics of each view's last frame, its times in
    /// seconds.
    name: String,
}

/// What `press_key` and `release_key` take.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct Key {
    /// The key, by its position on a US keyboard: `KeyW`, `KeyA`, `ArrowLeft`, `Space`, `Escape`,
    /// `F5`.
    key: String,
}

/// What `capture` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct Capture {
    /// The view whose part of the image to give, by the name that `game_state` lists; the whole
    /// image if left out.
    view: Option<String>,
}

/// What `pick` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct Pick {
    /// The view, by the name that `game_state` lists, whose pixel `x` and `y` count from its top
    /// left corner; the whole image if left out, where the last view drawn at the pixel is picked.
    view: Option<String>,
    /// The pixel's column, from the left, as in the image that `capture` gives.
    x: u32,
    /// Its row, from the top.
    y: u32,
}

/// What `read_logs` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ReadLogs {
    /// Where the page starts: the `next` or the `end` that the page before gave; left out for the
    /// oldest line that the game keeps.
    cursor: Option<String>,
    /// No longer taken: `cursor` holds where a page starts.
    #[schemars(skip)]
    after: Option<u64>,
}

/// What `save_game` and `load_game` take.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SaveFile {
    /// The save's file, a path on the game's machine: one of the agent's own, never the player's
    /// save.
    file: String,
}

#[tool_router(router = play_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "The game's state: its frame, whether game time is paused, the fixed steps run, \
                       game time and real time, its frames' times, its entities, the views that \
                       its last frame drew and the reports it gives."
    )]
    async fn game_state(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::State, &context).await)
    }

    #[tool(description = "Pauses game time: the world stands still, but for commands.")]
    async fn pause(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Pause, &context).await)
    }

    #[tool(description = "Lets game time run again.")]
    async fn resume(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Resume, &context).await)
    }

    #[tool(
        description = "Minimizes the game's window, as the user would: the compositor hides it, \
                       and a game that rests while hidden draws nothing. Only the user shows it \
                       again."
    )]
    async fn minimize_game(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Minimize, &context).await)
    }

    #[tool(
        description = "While game time is paused, runs that many fixed steps of it, over the next \
                       frames, and answers once they have all run."
    )]
    async fn step(
        &self,
        Parameters(Step { steps }): Parameters<Step>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Step { steps }, &context).await)
    }

    #[tool(description = "Sets the speed of game time, whose fixed steps keep their duration.")]
    async fn set_speed(
        &self,
        Parameters(Speed { speed }): Parameters<Speed>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Speed { speed }, &context).await)
    }

    #[tool(
        description = "Puts a key down, as the player would, until `release_key`: the game reads it \
                       held from its next frame on."
    )]
    async fn press_key(
        &self,
        Parameters(Key { key }): Parameters<Key>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Press { key }, &context).await)
    }

    #[tool(description = "Lets a key go.")]
    async fn release_key(
        &self,
        Parameters(Key { key }): Parameters<Key>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Release { key }, &context).await)
    }

    #[tool(
        description = "The image that the game shows, as the player sees it, dev overlay included, \
                       once its next frame is drawn; or the part of it that one of its views \
                       draws, named as `game_state` lists them."
    )]
    async fn capture(
        &self,
        Parameters(Capture { view }): Parameters<Capture>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let request = Request::Capture {
            view: view.unwrap_or_default(),
        };
        Ok(self.answer(&request, &context).await)
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let request = Request::Pick {
            view: view.unwrap_or_default(),
            x,
            y,
        };
        Ok(self.answer(&request, &context).await)
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let request = Request::SetViewSettings { view, path, value };
        Ok(self.answer(&request, &context).await)
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Report { name }, &context).await)
    }

    #[tool(
        description = "The lines of the game's log, oldest first, a page at a time: a page that \
                       more follow starts with `next`, which `cursor` takes for the rest; the \
                       page that reaches the log's end starts with `end`, which `cursor` takes \
                       later for the lines written after it. A line too long is cut, and marked."
    )]
    async fn read_logs(
        &self,
        Parameters(ReadLogs { cursor, after }): Parameters<ReadLogs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if after.is_some() {
            return Ok(failure(
                "`read_logs` takes `cursor`, the `next` or the `end` that a page gave, no longer \
                 `after`: leave both out for the oldest line that the game keeps",
            ));
        }
        // Every page asked of the program that the routing reaches now, whose number binds the
        // cursors given and taken, whatever the routing does meanwhile.
        let pinned = match self.pinned(PLAY).await {
            Ok(pinned) => pinned,
            Err(why) => return Ok(failure(why)),
        };
        let program = pinned.program();
        let after = match cursor.as_deref().map(|cursor| line_of(cursor, program)) {
            None => 0,
            Some(Ok(after)) => after,
            Some(Err(why)) => return Ok(failure(why)),
        };
        let first = Logs(Request::Logs {
            after,
            limit: None,
            bytes: Some(PAGE_BYTES),
        });
        let page = match gather(&pinned, &context, first, PAGE_BYTES).await {
            Ok(page) => page,
            Err(failed) => return Ok(failed),
        };
        Ok(lines(page, after, program))
    }

    #[tool(
        description = "Saves the game's run to a file, as the game saves its own runs, at the start \
                       of its next frame."
    )]
    async fn save_game(
        &self,
        Parameters(SaveFile { file }): Parameters<SaveFile>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Save { file }, &context).await)
    }

    #[tool(
        description = "Loads the run of a save's file in place of the game's run, as the game resumes \
                       its own saves, at the start of its next frame: a save that the game does not \
                       resume leaves the run as it is."
    )]
    async fn load_game(
        &self,
        Parameters(SaveFile { file }): Parameters<SaveFile>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Load { file }, &context).await)
    }
}

/// What a cursor of another program's log says, as `STALE` says of a list that changed whole.
const OTHER_PROGRAM: &str = "this cursor is of the log of another program than the one that the \
     game's tools reach now: read the log again from its start, without `cursor`";

/// A cursor of `read_logs`, the program whose log it reads, by the server's number of it, then the
/// number of the last line read: `3.120`.
fn cursor(program: u64, line: u64) -> String {
    format!("{program}.{line}")
}

/// The number of the last line read that `cursor` gives, a cursor of `program`'s log.
fn line_of(cursor: &str, program: u64) -> Result<u64, String> {
    let read = cursor
        .split_once('.')
        .and_then(|(of, line)| Some((of.parse::<u64>().ok()?, line.parse::<u64>().ok()?)));
    match read {
        Some((of, line)) if of == program => Ok(line),
        Some(_) => Err(OTHER_PROGRAM.to_owned()),
        None => Err(format!(
            "`{cursor}` is no cursor of `read_logs`: give the `next` or the `end` that a page gave"
        )),
    }
}

/// A request of a page of the game's log, `play/logs`, which `read_logs` reads page after page.
/// Its cursors, the number of the last line read, as text, stay within the server: the agent's
/// name their program too ([`cursor`]).
#[derive(Clone, Debug, PartialEq)]
struct Logs(Request);

/// A page of the game's log, as `play/logs` answers it.
#[derive(Debug, PartialEq)]
struct LogPage {
    lines: Vec<LogLine>,
    /// The number of the last line, while lines kept after it were left for the next page.
    next: Option<String>,
}

impl Page for LogPage {
    fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    fn extend(&mut self, more: Self) {
        self.lines.extend(more.lines);
        self.next = more.next;
    }

    fn size(&self) -> (u64, u64) {
        (self.lines.len() as u64, json_size(&self.lines) as u64)
    }
}

impl List for Logs {
    type Request = Request;
    type Reply = Reply;
    type Page = LogPage;

    fn request(&self) -> &Request {
        &self.0
    }

    fn resumed(&self, cursor: String, _count: u64, bytes: u64) -> Option<Self> {
        let Request::Logs { limit, .. } = self.0 else {
            return None;
        };
        Some(Self(Request::Logs {
            after: cursor.parse().ok()?,
            limit,
            bytes: Some(bytes),
        }))
    }

    fn page(reply: Reply) -> Result<LogPage, Fault> {
        match reply {
            Reply::Logs { lines, partial } => {
                // A game gives `partial` with one line at least.
                let next = lines
                    .last()
                    .filter(|_| partial)
                    .map(|last| last.number.to_string());
                Ok(LogPage { lines, next })
            }
            other => Err(Fault::new(
                Fault::INVALID,
                format!("the game answered a page of its log with {other:?}"),
            )),
        }
    }
}

/// The result that gives `page`'s lines, of `program`'s log read after the line `after`: first
/// `next` while more follow, else `end`, where the lines written later start.
fn lines(page: LogPage, after: u64, program: u64) -> CallToolResult {
    let last = page.lines.last().map_or(after, |line| line.number);
    let mut said = vec![match page.next {
        Some(_) => next_line(&cursor(program, last)),
        None => format!(
            "end: {} (give it as `cursor` later for the lines written after these)",
            cursor(program, last)
        ),
    }];
    if page.lines.is_empty() {
        said.push("no line after this cursor yet".to_owned());
    }
    said.extend(page.lines.into_iter().map(|line| {
        let cut = if line.cut { " [cut]" } else { "" };
        format!("{}: {}{cut}", line.number, line.text)
    }));
    text(said.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(number: u64) -> LogLine {
        LogLine {
            number,
            text: format!("line {number}"),
            cut: false,
        }
    }

    fn said(result: &CallToolResult) -> String {
        result.content[0]
            .as_text()
            .map(|text| text.text.clone())
            .unwrap_or_default()
    }

    /// A page left partial goes on after its last line; one that reaches the log's end does not.
    #[test]
    fn a_partial_page_goes_on_after_its_last_line() {
        let partial = Reply::Logs {
            lines: vec![line(4), line(5)],
            partial: true,
        };
        let page = Logs::page(partial).expect("a page");
        assert_eq!(page.next(), Some("5"));
        let first = Logs(Request::Logs {
            after: 3,
            limit: None,
            bytes: Some(PAGE_BYTES),
        });
        assert_eq!(
            first.resumed("5".to_owned(), 2, 100),
            Some(Logs(Request::Logs {
                after: 5,
                limit: None,
                bytes: Some(100),
            }))
        );
        let whole = Logs::page(Reply::Logs {
            lines: vec![line(6)],
            partial: false,
        })
        .expect("a page");
        assert_eq!(whole.next(), None);
        assert_eq!(
            Logs::page(Reply::Done).map_err(|fault| fault.code),
            Err(Fault::INVALID)
        );
    }

    /// The agent's cursor names the program whose log it reads, and the last line read: a cursor
    /// of another program's log, or that is no cursor, is refused.
    #[test]
    fn a_cursor_names_its_program_and_its_line() {
        assert_eq!(line_of(&cursor(3, 120), 3), Ok(120));
        assert_eq!(line_of(&cursor(2, 120), 3), Err(OTHER_PROGRAM.to_owned()));
        let refused = line_of("120", 3).expect_err("no program named");
        assert!(refused.contains("no cursor"), "{refused}");
        let going_on = lines(
            LogPage {
                lines: vec![line(4), line(5)],
                next: Some("5".to_owned()),
            },
            3,
            7,
        );
        let said_going_on = said(&going_on);
        assert!(said_going_on.starts_with("next: 7.5 "), "{said_going_on}");
        assert!(
            said_going_on.ends_with("4: line 4\n5: line 5"),
            "{said_going_on}"
        );
        let at_end = said(&lines(
            LogPage {
                lines: Vec::new(),
                next: None,
            },
            9,
            7,
        ));
        assert!(at_end.starts_with("end: 7.9 "), "{at_end}");
        assert!(at_end.contains("no line after this cursor yet"), "{at_end}");
    }
}
