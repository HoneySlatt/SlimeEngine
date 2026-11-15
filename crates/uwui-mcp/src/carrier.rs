//! How the library reaches a program: through the carrier that its host gives, at the level of
//! uwui's core for agents
//! (`docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md`).

use rmcp::RoleServer;
use rmcp::service::RequestContext;
use serde::Serialize;
use serde::de::DeserializeOwned;
use uwui_agent::{Fault, Value};

/// What a program answered a request: its outcome, the answer's result or the program's fault,
/// and the bytes that followed its line, an image's pixels. Its result is JSON as a carrier gives
/// it, or a family's own reply as [`request`] reads it.
#[derive(Debug)]
pub struct Answer<T = Value> {
    /// The answer's result, or the fault that the program answered, `STALE` among them.
    pub outcome: Result<T, Fault>,
    /// The bytes that followed the answer's line; none for most answers.
    pub bytes: Vec<u8>,
}

/// A request sent to a program, in flight until its answer comes: what waits for that answer, and
/// what cancels the request meanwhile.
pub struct Sent {
    wait: Box<dyn FnOnce() -> Result<Answer, String> + Send>,
    cancel: Box<dyn Fn() + Send + Sync>,
}

impl Sent {
    /// A request in flight: `wait` blocks until its answer, or says why none comes, as a
    /// connection lost; `cancel` asks the program to cancel it, from another thread while `wait`
    /// blocks. Each runs on a thread that may block: a cancel may wait for another request's
    /// write on the same connection.
    pub fn new(
        wait: impl FnOnce() -> Result<Answer, String> + Send + 'static,
        cancel: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            wait: Box::new(wait),
            cancel: Box::new(cancel),
        }
    }
}

/// What carries the library's requests to a program: the MCP server's own type, which its host
/// writes, and which chooses the program that each request goes to.
///
/// The engine's carrier is its development channel (`slime-mcp`); a carrier of uwui's own comes
/// with `uwui dev`.
///
/// The library clones it for each request, which it sends from another thread: its clone is
/// cheap, its state behind `Arc`s.
pub trait Carrier: Clone + Send + Sync + 'static {
    /// Sends the request `method`, `family/name`, with `params`, to the program that serves it
    /// now. It may block while the program's places in flight are all taken.
    ///
    /// # Errors
    ///
    /// Why nothing was sent, as no program running or a connection lost, which the agent reads.
    fn send(&self, method: &str, params: Value) -> Result<Sent, String>;
}

/// Why a call cancelled by its agent gives no answer: MCP answers no cancelled call, so that no
/// agent reads it.
const CANCELLED: &str = "the agent cancelled the call";

/// Asks `method` with `params` of the program that `carrier` reaches, on threads that may block,
/// so that the server answers its other calls meanwhile, and gives the program's answer. A call
/// that its agent cancels (`context`) cancels the request, which the program then does not do if it
/// had changed nothing yet, and gives no answer.
///
/// # Errors
///
/// Why no answer came, as [`Carrier::send`] and the carrier's wait say, or that the agent
/// cancelled the call.
pub async fn ask<C: Carrier>(
    carrier: &C,
    method: &str,
    params: Value,
    context: &RequestContext<RoleServer>,
) -> Result<Answer, String> {
    if context.ct.is_cancelled() {
        // Cancelled before it was sent, as a list's next page: never sent.
        return Err(CANCELLED.to_owned());
    }
    let carrier = carrier.clone();
    let method = method.to_owned();
    let mut sending = tokio::task::spawn_blocking(move || carrier.send(&method, params));
    let sent = tokio::select! {
        sent = &mut sending => joined(sent)?,
        () = context.ct.cancelled() => {
            // Sent once a place in flight frees: cancelled then, its answer dropped.
            tokio::spawn(async move {
                if let Ok(Ok(sent)) = sending.await {
                    let _ = tokio::task::spawn_blocking(sent.cancel).await;
                }
            });
            return Err(CANCELLED.to_owned());
        }
    };
    let Sent { wait, cancel } = sent;
    let mut waiting = tokio::task::spawn_blocking(wait);
    tokio::select! {
        answer = &mut waiting => joined(answer),
        () = context.ct.cancelled() => {
            // The program still answers it, once; the thread that waits drops that answer. The
            // cancel may wait for another request's write: not on the runtime's own thread.
            tokio::task::spawn_blocking(cancel);
            Err(CANCELLED.to_owned())
        }
    }
}

/// Asks `request`, a request of a family's own type, `R` its reply, of the program that `carrier`
/// reaches, as [`ask`] does: written by the core as its family writes it, and its answer read as
/// an `R`. Each family's tools take this way, the interface's and a host's own alike.
///
/// # Errors
///
/// As [`ask`]; and why `request` cannot be written as a request of the core. An answer that does
/// not read as an `R` is an [`Answer::outcome`] of a fault, `INVALID`, that says so, not an error:
/// the program's answer is wrong, not the agent's arguments.
pub async fn request<C: Carrier, R: DeserializeOwned>(
    carrier: &C,
    request: &impl Serialize,
    context: &RequestContext<RoleServer>,
) -> Result<Answer<R>, String> {
    let (method, params) = uwui_agent::call(request).map_err(|error| error.to_string())?;
    let answer = ask(carrier, &method, params, context).await?;
    Ok(Answer {
        outcome: answer.outcome.and_then(|result| {
            uwui_agent::read::<R>(result).map_err(|unread| {
                Fault::new(
                    Fault::INVALID,
                    format!(
                        "the program's answer does not read as its family's reply: {}",
                        unread.message
                    ),
                )
            })
        }),
        bytes: answer.bytes,
    })
}

/// The result of work run on tokio's threads for blocking work: a panic of that work is a failure
/// that says so.
fn joined<T>(joined: Result<Result<T, String>, tokio::task::JoinError>) -> Result<T, String> {
    joined.unwrap_or_else(|error| Err(format!("the request's thread failed: {error}")))
}
