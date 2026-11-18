//! What the tests that read an editor's journal through the MCP server share: the cursors that its
//! tool's first line gives.

use serde_json::json;

use crate::session::Session;

/// The cursor that `said`, a tool's first line, gives after `kind`, as
/// `next: <cursor> (how to use it)`; none when it gives another kind.
pub fn cursor(said: &str, kind: &str) -> Option<String> {
    let rest = said.strip_prefix(kind)?.strip_prefix(": ")?;
    rest.split(' ').next().map(str::to_owned)
}

/// The cursor at the end of the journal of the editor that `session` reaches, read to its end.
pub fn end(session: &mut Session) -> String {
    let mut next = None;
    loop {
        let (said, _) = session.tool("editor_journal", json!({ "cursor": next }));
        if let Some(end) = cursor(&said[0], "end") {
            return end;
        }
        next = Some(cursor(&said[0], "next").unwrap_or_else(|| panic!("next or end: {said:?}")));
    }
}
