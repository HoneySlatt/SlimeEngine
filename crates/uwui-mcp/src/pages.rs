//! One way for every tool to read a long list: the program's pages gathered into one for the
//! agent, within the bytes that it asks and a time, its cursors passed through untouched
//! (`docs/adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md`).

use std::time::{Duration, Instant};

use rmcp::RoleServer;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use uwui_agent::{Fault, Value};

use crate::carrier::{Carrier, ask};
use crate::result::{failure, fault};

/// The most bytes of JSON that a page of a list holds, so that what a tool prints stays well
/// within what an agent's client takes whole: Claude Code moves an output past 25,000 tokens, or
/// 50,000 characters, into a file.
pub const PAGE_BYTES: u64 = 32 << 10;

/// How long a tool gathers the program's pages before it gives what it has, and their `next`.
pub const LIST_TIME: Duration = Duration::from_secs(5);

/// The bytes of JSON of a page that an agent asks `bytes` of: as many, at most [`PAGE_BYTES`];
/// [`PAGE_BYTES`] when it asks none.
pub fn page_bytes(bytes: Option<u64>) -> u64 {
    bytes.map_or(PAGE_BYTES, |bytes| bytes.min(PAGE_BYTES))
}

/// A page of a list, as a program answers it, which a tool gathers with the pages after it.
pub trait Page: Sized {
    /// The cursor to the rest, while the list goes on.
    fn next(&self) -> Option<&str>;
    /// Adds `more`, the page after this one: its items after these, and its `next` in place of
    /// this one's.
    fn extend(&mut self, more: Self);
    /// How many items it holds, and the bytes of JSON that they take.
    fn size(&self) -> (u64, u64);
}

/// A request of a list, of a family of uwui's core for agents, which a tool reads page after page.
pub trait List: Clone + Send + 'static {
    /// The page that its answer gives.
    type Page: Page;
    /// Its method, `family/name`, and its parameters.
    ///
    /// # Errors
    ///
    /// Why it cannot be written, which the agent reads.
    fn call(&self) -> Result<(String, Value), String>;
    /// The request of the rest of the list, from `cursor`, the last page's `next`, once `count`
    /// items were gathered: of at most `bytes` of JSON and of what its own limit leaves; none
    /// once that limit is reached.
    fn resumed(&self, cursor: String, count: u64, bytes: u64) -> Option<Self>;
    /// The page that an answer's `result` gives.
    ///
    /// # Errors
    ///
    /// A fault when it is no such page.
    fn page(result: Value) -> Result<Self::Page, Fault>;
}

/// A page of a list for an agent, from `first`, a request of the list's start or of a cursor that
/// the agent gave, which asks `bytes` of JSON at most: the program's pages, each after the last,
/// gathered until they hold half of `bytes` or more, the list ends, its limit is reached or
/// [`LIST_TIME`] passed, so that a page that the program cut short, or that found none, does not
/// reach the agent alone. It gives the last page's `next`.
///
/// # Errors
///
/// The tool's error that the agent reads: the program's fault, `STALE` saying to list again, or
/// why no answer came.
pub async fn gather<C: Carrier, L: List>(
    carrier: &C,
    context: &RequestContext<RoleServer>,
    first: L,
    bytes: u64,
) -> Result<L::Page, CallToolResult> {
    let started = Instant::now();
    let mut page = one_page(carrier, context, &first).await?;
    loop {
        let (count, held) = page.size();
        let Some(next) = page.next().map(str::to_owned) else {
            break;
        };
        if started.elapsed() >= LIST_TIME || held >= bytes / 2 {
            break;
        }
        let Some(rest) = first.resumed(next, count, bytes - held) else {
            break;
        };
        let more = one_page(carrier, context, &rest).await?;
        page.extend(more);
    }
    Ok(page)
}

/// The program's page that `request` asks, or the tool's error that says why none.
async fn one_page<C: Carrier, L: List>(
    carrier: &C,
    context: &RequestContext<RoleServer>,
    request: &L,
) -> Result<L::Page, CallToolResult> {
    let (method, params) = request.call().map_err(failure)?;
    let answer = ask(carrier, &method, params, context)
        .await
        .map_err(failure)?;
    L::page(answer.outcome.map_err(fault)?).map_err(fault)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_asks_less_than_a_page_never_more() {
        assert_eq!(
            (
                page_bytes(None),
                page_bytes(Some(10)),
                page_bytes(Some(u64::MAX))
            ),
            (PAGE_BYTES, 10, PAGE_BYTES)
        );
    }
}
