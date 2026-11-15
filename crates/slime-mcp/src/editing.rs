//! The family `editing` as tools: an agent edits an editor's level, by steps of commands, by its
//! history and its selection, by its operations, and reads its journal by pages.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, schemars, tool, tool_router};
use serde::Deserialize;
use slime_channel::{EDITING, Edit, Fault, Reply, json_size};
use uwui_mcp::{Family, List, Page, gather, next_line, page_bytes};

use crate::Server;

/// What the family `editing` tells an agent among the server's instructions.
const INSTRUCTIONS: &str = "An editor's tools reach the editor, whatever the game's tools drive: \
     `edit` makes a step of commands, `undo` and `redo` undo and make it again, `select` selects \
     entities, `editor_history` says its steps and selection, `editor_journal` gives its journal \
     by pages, `write_level` writes the level back to its file, `list_operations` lists every \
     action of its menus, shortcuts and palette, and `run_operation` runs one by its name, as a \
     person does.";

/// The family `editing`, for the server.
pub(crate) fn family() -> Family<Server> {
    Family::new(EDITING, INSTRUCTIONS, Server::editing_tools())
}

/// What `edit` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct EditStep {
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
pub(crate) struct Select {
    /// The entities to select, by persistent identity; empty for none.
    selection: Vec<String>,
}

/// What `run_operation` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RunOperation {
    /// The operation's name, as `list_operations` lists it: `edit.undo`, `file.open`.
    name: String,
    /// Its arguments, as text, in the order of its parameters: a level's path for `file.open`;
    /// left out for none.
    #[serde(default)]
    arguments: Vec<String>,
}

/// What `editor_journal` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct EditorJournal {
    /// Where the page starts: the `next` that the page before gave; left out for the journal's
    /// start.
    cursor: Option<String>,
    /// The most bytes of the page, as JSON writes its text; left out for as many as the server
    /// gives, some 32 KiB.
    bytes: Option<u64>,
}

#[tool_router(router = editing_tools, vis = "pub(crate)")]
impl Server {
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let edit = Edit::Step {
            label,
            commands,
            selection,
        };
        Ok(self.answer(&edit, &context).await)
    }

    #[tool(description = "In an editor, undoes the last step done.")]
    async fn undo(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Undo, &context).await)
    }

    #[tool(description = "In an editor, does again the last step undone.")]
    async fn redo(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Redo, &context).await)
    }

    #[tool(
        description = "In an editor, selects entities by their persistent identities, which is not \
                       a step: undoing a step selects again what was selected before it."
    )]
    async fn select(
        &self,
        Parameters(Select { selection }): Parameters<Select>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Select { selection }, &context).await)
    }

    #[tool(
        description = "An editor's level, its steps done and undone, what is selected, and what it \
                       recovered of a journal as it started."
    )]
    async fn editor_history(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::History, &context).await)
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let bytes = page_bytes(bytes);
        let first = Journal(Edit::Journal {
            cursor,
            bytes: Some(bytes),
        });
        Ok(match gather(self, &context, first, bytes).await {
            Ok(page) => described(page),
            Err(failed) => failed,
        })
    }

    #[tool(
        description = "Writes an editor's level back to its file, whole or not at all, its comments \
                       kept, and says what the file could not hold."
    )]
    async fn write_level(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Write, &context).await)
    }

    #[tool(
        description = "An editor's operations, every action that its menus, its shortcuts and its \
                       palette run: each name, its parameters, its label, and what it changes: \
                       the history, the level's file, the selection, the level by a step, what \
                       it copied, the window or the program."
    )]
    async fn list_operations(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Operations, &context).await)
    }

    #[tool(
        description = "In an editor, runs an operation by its name, as `list_operations` lists it, \
                       with its arguments as text in the order of its parameters, as a person runs \
                       it from a menu, a shortcut or the palette."
    )]
    async fn run_operation(
        &self,
        Parameters(RunOperation { name, arguments }): Parameters<RunOperation>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Edit::Run { name, arguments }, &context).await)
    }
}

/// A request of a page of an editor's journal, `editing/journal`, which `editor_journal` reads
/// page after page.
#[derive(Clone, Debug, PartialEq)]
struct Journal(Edit);

/// A page of an editor's journal, as `editing/journal` answers it.
#[derive(Debug, PartialEq)]
struct JournalPage {
    text: String,
    next: Option<String>,
    /// Whether bytes of a page that were not UTF-8 were replaced.
    lossy: bool,
}

impl Page for JournalPage {
    fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    fn extend(&mut self, more: Self) {
        self.text.push_str(&more.text);
        self.next = more.next;
        self.lossy |= more.lossy;
    }

    /// A journal's text is no list of items: none counted, its bytes alone.
    fn size(&self) -> (u64, u64) {
        (0, json_size(&self.text) as u64)
    }
}

impl List for Journal {
    type Request = Edit;
    type Reply = Reply;
    type Page = JournalPage;

    fn request(&self) -> &Edit {
        &self.0
    }

    fn resumed(&self, cursor: String, _count: u64, bytes: u64) -> Option<Self> {
        let Edit::Journal { .. } = self.0 else {
            return None;
        };
        Some(Self(Edit::Journal {
            cursor: Some(cursor),
            bytes: Some(bytes),
        }))
    }

    fn page(reply: Reply) -> Result<JournalPage, Fault> {
        match reply {
            Reply::Journal { text, next, lossy } => Ok(JournalPage { text, next, lossy }),
            other => Err(Fault::new(
                Fault::INVALID,
                format!("the editor answered a page of its journal with {other:?}"),
            )),
        }
    }
}

/// The result that gives `page`'s text, apart from where it goes on and how it was read, which
/// come first.
fn described(page: JournalPage) -> CallToolResult {
    let mut said: Vec<String> = page.next.as_deref().map(next_line).into_iter().collect();
    if page.lossy {
        said.push(
            "bytes of this page that are not UTF-8 were each replaced, by run, with U+FFFD"
                .to_owned(),
        );
    }
    let mut blocks: Vec<ContentBlock> = said.into_iter().map(ContentBlock::text).collect();
    blocks.push(ContentBlock::text(page.text));
    CallToolResult::success(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str, next: Option<&str>, lossy: bool) -> JournalPage {
        JournalPage {
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
        let mut gathered = page("# level\n", Some("1"), false);
        gathered.extend(page("\u{fffd}", Some("2"), true));
        gathered.extend(page("# do\n", Some("3"), false));
        assert_eq!(gathered, page("# level\n\u{fffd}# do\n", Some("3"), true));
        assert_eq!(gathered.size(), (0, 20));
        let first = Journal(Edit::Journal {
            cursor: None,
            bytes: Some(4096),
        });
        assert_eq!(
            first.resumed("3".to_owned(), 0, 4000),
            Some(Journal(Edit::Journal {
                cursor: Some("3".to_owned()),
                bytes: Some(4000),
            }))
        );
        assert_eq!(Journal(Edit::Undo).resumed("3".to_owned(), 0, 4000), None);
        let said: Vec<String> = described(gathered)
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect();
        assert_eq!(said.len(), 3, "{said:?}");
        assert!(said[0].starts_with("next: 3 "), "{said:?}");
        assert!(said[1].contains("not UTF-8"), "{said:?}");
        assert_eq!(said[2], "# level\n\u{fffd}# do\n");
    }

    /// A reply that is no page of the journal is a fault.
    #[test]
    fn a_reply_of_another_kind_is_no_page() {
        assert_eq!(
            Journal::page(Reply::Done).map_err(|fault| fault.code),
            Err(Fault::INVALID)
        );
    }
}
