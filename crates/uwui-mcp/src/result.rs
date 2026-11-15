//! What a tool gives its agent: a text, or an error that the agent's model reads.
//!
//! A program's faults are tools' errors, marked `isError`, as MCP asks of whatever a tool's
//! arguments make fail, so that a model reads them and corrects itself; JSON-RPC's errors are left
//! to a tool unknown or a call malformed
//! (`docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md`).

use rmcp::model::{CallToolResult, ContentBlock};
use uwui_agent::Fault;

/// A result of text.
pub fn text(message: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.into())])
}

/// A tool's error that says what failed, which the agent reads.
pub fn failure(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// The tool's error of a program's `fault`: one of a cursor that no longer holds, `STALE`, says to
/// list again from the start.
pub fn fault(fault: Fault) -> CallToolResult {
    if fault.code == Fault::STALE {
        failure(format!(
            "the list changed whole since this cursor was given: list it again from its start, \
             without `cursor` ({})",
            fault.message
        ))
    } else {
        failure(fault.message)
    }
}

/// The line that starts a page of a list that goes on: its cursor, first, as an agent's client
/// may cut a long output.
pub fn next_line(next: &str) -> String {
    format!("next: {next} (give it as `cursor`, with the same other arguments, for the rest)")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts of `result`.
    fn texts(result: &CallToolResult) -> Vec<String> {
        result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect()
    }

    #[test]
    fn a_stale_cursor_says_to_list_again_as_a_tool_s_error() {
        let result = fault(Fault::new(Fault::STALE, "another world"));
        assert_eq!(result.is_error, Some(true));
        let said = texts(&result);
        assert!(said[0].contains("list it again from its start"), "{said:?}");
        assert!(said[0].contains("another world"), "{said:?}");
    }

    #[test]
    fn another_fault_is_a_tool_s_error_of_its_message() {
        let result = fault(Fault::new(Fault::REFUSED, "no element `main/x`"));
        assert_eq!(result.is_error, Some(true));
        assert_eq!(texts(&result), ["no element `main/x`"]);
    }
}
