//! An agent's sessions with a server on uwui's MCP library, spoken as MCP's JSON-RPC through a
//! pipe in memory: the interface's tools on an interface in memory of two targets, which `uwui`
//! answers through both ends of the core's session; through MCP's handshake and through MCP
//! `2026-07-28`'s requests, which name their client each; a call cancelled by its agent; a cursor
//! that no longer holds. Each read of the server's answers is bounded by a time.

#[path = "common/memory.rs"]
mod memory;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use base64::Engine as _;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router,
};
use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines, ReadHalf, WriteHalf,
};
use uwui_agent::Fault;
use uwui_mcp::{Answer, Carrier, Sent, Tools, agent_of, interface_family, text};

use memory::{Memory, buttons, chooser, panel};

/// How long a test waits for any one message of the server.
const PATIENCE: Duration = Duration::from_secs(10);

/// What sends a request of the core, as a carrier does.
type Sends = Arc<dyn Fn(&str, Value) -> Result<Sent, String> + Send + Sync>;

/// A server of the interface's tools, and one of its own that names the agent, whose requests go
/// where `send` sends them.
#[derive(Clone)]
struct Server {
    send: Sends,
    tools: Arc<Tools<Self>>,
}

/// The first paragraph of the tests' server's instructions.
const OWN: &str = "A server of the tests.";

impl Server {
    fn new(send: Sends) -> Self {
        Self {
            send,
            tools: Arc::new(
                Tools::new(OWN, Self::own_tools())
                    .with(interface_family())
                    .expect("no name taken twice"),
            ),
        }
    }
}

impl Carrier for Server {
    fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        (self.send)(method, params)
    }
}

#[tool_router(router = own_tools)]
impl Server {
    #[tool(description = "Names the agent as the library names it to a program.")]
    async fn agent(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        let agent = agent_of(&context);
        Ok(text(format!("{} {}", agent.name, agent.version)))
    }
}

#[tool_handler(router = self.tools.router())]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("uwui-mcp's tests", "1"))
            .with_instructions(self.tools.instructions())
    }
}

/// The agent's end of a session with a [`Server`].
struct Agent {
    write: WriteHalf<DuplexStream>,
    lines: Lines<BufReader<ReadHalf<DuplexStream>>>,
    next: u64,
    /// The answers read while another was awaited, by their numbers.
    early: HashMap<u64, Value>,
}

impl Agent {
    /// A session with a server whose requests go where `send` sends them, not yet opened.
    fn start(send: Sends) -> Self {
        let (ours, theirs) = tokio::io::duplex(1 << 22);
        let server = Server::new(send);
        tokio::spawn(async move {
            if let Ok(running) = server.serve(theirs).await {
                let _ = running.waiting().await;
            }
        });
        let (read, write) = tokio::io::split(ours);
        Self {
            write,
            lines: BufReader::new(read).lines(),
            next: 0,
            early: HashMap::new(),
        }
    }

    /// A session opened by MCP's handshake, by an agent named `name`.
    async fn handshake(send: Sends, name: &str) -> Self {
        let mut agent = Self::start(send);
        agent
            .call(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": name, "version": "1" },
                }),
            )
            .await;
        agent.notify("notifications/initialized", json!({})).await;
        agent
    }

    /// Writes `message`, a line of JSON.
    async fn write(&mut self, message: Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.write
            .write_all(line.as_bytes())
            .await
            .expect("the server reads");
    }

    /// Sends the notification `method` with `params`.
    async fn notify(&mut self, method: &str, params: Value) {
        self.write(json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await;
    }

    /// Sends `method` with `params` without waiting for its answer: its number.
    async fn send(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let id = self.next;
        self.write(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        id
    }

    /// The server's next message, within [`PATIENCE`].
    async fn next_message(&mut self, waiting: &str) -> Value {
        let line = tokio::time::timeout(PATIENCE, self.lines.next_line())
            .await
            .unwrap_or_else(|_| panic!("no message while {waiting} waits"))
            .expect("the pipe reads")
            .unwrap_or_else(|| panic!("the server ended before {waiting}"));
        serde_json::from_str(&line).expect("JSON")
    }

    /// The answer numbered `id`, the notifications before it passed over, and the answers of other
    /// requests kept for when they are awaited.
    async fn answer(&mut self, id: u64, waiting: &str) -> Value {
        if let Some(answer) = self.early.remove(&id) {
            return answer;
        }
        loop {
            let message = self.next_message(waiting).await;
            match message["id"].as_u64() {
                Some(answered) if answered == id => return message,
                Some(answered) => {
                    self.early.insert(answered, message);
                }
                None => {}
            }
        }
    }

    /// Calls the tool `name` with `arguments` without waiting for its answer: its number.
    async fn send_tool(&mut self, name: &str, arguments: Value) -> u64 {
        self.send(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
        .await
    }

    /// The texts of the answer of the tool's call numbered `id`, and its result.
    async fn tool_answer(&mut self, id: u64) -> (Vec<String>, Value) {
        let answer = self.answer(id, "a tool's call").await;
        assert!(answer["error"].is_null(), "{answer}");
        (texts(&answer["result"]), answer["result"].clone())
    }

    /// Sends `method` with `params`, and gives its answer's result.
    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params).await;
        let answer = self.answer(id, method).await;
        assert!(answer["error"].is_null(), "{method}: {answer}");
        answer["result"].clone()
    }

    /// Calls the tool `name` with `arguments`: the texts of its result, and the result.
    async fn tool(&mut self, name: &str, arguments: Value) -> (Vec<String>, Value) {
        let result = self
            .call(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await;
        (texts(&result), result)
    }
}

/// The texts of a tool's `result`.
fn texts(result: &Value) -> Vec<String> {
    result["content"]
        .as_array()
        .expect("content")
        .iter()
        .filter_map(|block| block["text"].as_str().map(str::to_owned))
        .collect()
}

/// What sends to a program in memory, of the interface that [`panel`] makes.
fn in_memory() -> Sends {
    let memory = Memory::start(panel);
    Arc::new(move |method, params| memory.send(method, params))
}

/// The lines of `said` that name elements, without the ones that say where the list goes on or
/// that a page holds none: a page may be empty, and still give `next`.
fn element_lines(said: &[String]) -> Vec<String> {
    said[0]
        .lines()
        .filter(|line| !line.starts_with("next: ") && *line != "no element on this page")
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn the_server_s_instructions_are_its_own_then_each_family_s() {
    let mut agent = Agent::start(in_memory());
    let initialized = agent
        .call(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "a test", "version": "1" },
            }),
        )
        .await;
    let instructions = initialized["instructions"].as_str().expect("instructions");
    let (own, interface) = instructions.split_once("\n\n").expect("two paragraphs");
    assert_eq!(own, OWN);
    assert!(
        interface.starts_with("A program's interface is read and used as a person uses it")
            && interface.contains("`find_elements`"),
        "{instructions}"
    );
}

#[tokio::test]
async fn the_interface_s_tools_read_and_use_an_interface_of_two_targets() {
    let mut agent = Agent::handshake(in_memory(), "a test").await;
    let listed = agent.call("tools/list", json!({})).await;
    let names: Vec<&str> = listed["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name"))
        .collect();
    // In rmcp's order, by name, which does not change between calls.
    assert_eq!(
        names,
        [
            "act_on_interface",
            "agent",
            "capture_element",
            "element_at",
            "element_style",
            "find_elements",
            "wait_for_elements"
        ]
    );

    // The agent, as MCP's handshake named it.
    let (said, _) = agent.tool("agent", json!({})).await;
    assert_eq!(said, ["a test 1"]);

    let (said, _) = agent
        .tool("find_elements", json!({ "role": "button" }))
        .await;
    let whole = element_lines(&said);
    assert_eq!(whole.len(), 4, "{whole:?}");
    assert!(
        whole[0].starts_with("main/open — Button `open`"),
        "{whole:?}"
    );
    assert!(
        whole[3].starts_with("side/help — Button on target 2 `help`"),
        "the element of target 2 names its target: {whole:?}"
    );
    // A button a page, each page's cursor given back for the next: the same list, its cursor first.
    let mut paged = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let mut arguments = json!({ "role": "button", "limit": 1 });
        if let Some(cursor) = cursor.take() {
            arguments["cursor"] = json!(cursor);
        }
        let (said, _) = agent.tool("find_elements", arguments).await;
        cursor = said[0]
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("next: "))
            .and_then(|rest| rest.split(' ').next())
            .map(str::to_owned);
        paged.extend(element_lines(&said));
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(paged, whole, "the list read a button at a time");

    // The pixel of target 2 at its button, and none of the window's there.
    let (said, _) = agent
        .tool("element_at", json!({ "x": 10, "y": 10, "target": 2 }))
        .await;
    assert!(
        said[0].starts_with("side/help — Button on target 2"),
        "{said:?}"
    );
    let (said, _) = agent.tool("element_at", json!({ "x": 10, "y": 10 })).await;
    assert!(said[0].starts_with("main/open — Button"), "{said:?}");

    let (said, result) = agent
        .tool(
            "act_on_interface",
            json!({ "action": "focus", "element": "main/save" }),
        )
        .await;
    assert_eq!(said, ["done"], "{result}");
    let (said, _) = agent.tool("find_elements", json!({ "name": "save" })).await;
    assert!(said[0].contains("focused"), "{said:?}");

    let (said, result) = agent
        .tool("capture_element", json!({ "element": "main/close" }))
        .await;
    assert_eq!(said, ["100 × 40 pixels"], "{result}");
    let image = result["content"]
        .as_array()
        .expect("content")
        .iter()
        .find(|block| block["type"] == json!("image"))
        .expect("an image");
    assert_eq!(image["mimeType"], json!("image/png"));
    let png = base64::engine::general_purpose::STANDARD
        .decode(image["data"].as_str().expect("base64"))
        .expect("base64");
    let decoder = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .expect("a PNG image");
    assert_eq!((decoder.info().width, decoder.info().height), (100, 40));

    // The button's own style, written by `sized` in the interface's code.
    let (said, _) = agent
        .tool("element_style", json!({ "element": "main/open" }))
        .await;
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(&format!("{}:", memory::FILE)),
        "{said:?}"
    );
    assert!(
        said[0].ends_with("— layout.size[0], layout.size[1], layout.shrink"),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_program_s_refusal_and_a_cursor_that_does_not_read_are_tools_errors() {
    let mut agent = Agent::handshake(in_memory(), "a test").await;
    let (said, result) = agent
        .tool(
            "act_on_interface",
            json!({ "action": "press", "element": "main/nowhere" }),
        )
        .await;
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("main/nowhere"), "{said:?}");
    let (said, result) = agent
        .tool("find_elements", json!({ "cursor": "not a cursor" }))
        .await;
    assert_eq!(result["isError"], json!(true), "{said:?}");
    // An action that the tool reads wrong never reaches the program.
    let (said, result) = agent
        .tool(
            "act_on_interface",
            json!({ "action": "scroll", "element": "main/open" }),
        )
        .await;
    assert_eq!(result["isError"], json!(true));
    assert_eq!(said, ["`scroll` needs `x` and `y`"]);
}

#[tokio::test]
async fn an_answer_that_does_not_read_is_the_program_s_fault() {
    let wrong: Sends = Arc::new(|_, _| {
        Ok(Sent::new(
            || {
                Ok(Answer {
                    outcome: Ok(json!({ "reply": "nothing of the family's" })),
                    bytes: Vec::new(),
                })
            },
            || {},
        ))
    });
    let mut agent = Agent::handshake(wrong, "a test").await;
    let (said, result) = agent
        .tool("element_style", json!({ "element": "main/open" }))
        .await;
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(
        said[0].starts_with("the program's answer does not read as its family's reply: "),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_stale_cursor_says_to_list_again() {
    let stale: Sends = Arc::new(|_, _| {
        Ok(Sent::new(
            || {
                Ok(Answer {
                    outcome: Err(Fault::new(Fault::STALE, "the interface is another")),
                    bytes: Vec::new(),
                })
            },
            || {},
        ))
    });
    let mut agent = Agent::handshake(stale, "a test").await;
    let (said, result) = agent
        .tool("find_elements", json!({ "cursor": "a cursor of before" }))
        .await;
    assert_eq!(result["isError"], json!(true));
    assert!(
        said[0].contains("list it again from its start, without `cursor`"),
        "{said:?}"
    );
    assert!(said[0].contains("the interface is another"), "{said:?}");
}

#[tokio::test]
async fn mcp_2026_names_its_client_in_each_request() {
    let mut agent = Agent::start(in_memory());
    let meta = |name: &str| {
        json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": { "name": name, "version": "7" },
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    };
    let discovered = agent
        .call("server/discover", json!({ "_meta": meta("one") }))
        .await;
    assert!(
        discovered.to_string().contains("uwui-mcp's tests"),
        "{discovered}"
    );
    for name in ["one", "two"] {
        let result = agent
            .call(
                "tools/call",
                json!({ "name": "agent", "arguments": {}, "_meta": meta(name) }),
            )
            .await;
        assert_eq!(texts(&result), [format!("{name} 7")], "{result}");
    }
    let result = agent
        .call(
            "tools/call",
            json!({ "name": "find_elements", "arguments": { "name": "help" }, "_meta": meta("one") }),
        )
        .await;
    assert!(
        texts(&result)[0].starts_with("side/help — Button"),
        "{result}"
    );
}

#[tokio::test]
async fn a_call_that_its_agent_cancels_cancels_its_request_and_gives_no_answer() {
    // A request that waits until the program is told to cancel it, as an edit waits for a drag.
    let cancels = Arc::new(AtomicUsize::new(0));
    let (cancelled, waits) = mpsc::channel::<()>();
    let waits = Arc::new(std::sync::Mutex::new(waits));
    let send: Sends = {
        let cancels = Arc::clone(&cancels);
        Arc::new(move |_, _| {
            let waits = Arc::clone(&waits);
            let cancels = Arc::clone(&cancels);
            let cancelled = cancelled.clone();
            Ok(Sent::new(
                move || {
                    let _ = waits.lock().expect("not poisoned").recv_timeout(PATIENCE);
                    Ok(Answer {
                        outcome: Err(Fault::new(Fault::CANCELLED, "cancelled")),
                        bytes: Vec::new(),
                    })
                },
                move || {
                    cancels.fetch_add(1, Ordering::SeqCst);
                    let _ = cancelled.send(());
                },
            ))
        })
    };
    let mut agent = Agent::handshake(send, "a test").await;
    let id = agent
        .send(
            "tools/call",
            json!({ "name": "act_on_interface", "arguments": { "action": "accept" } }),
        )
        .await;
    // Cancelled once the request is in flight.
    tokio::time::sleep(Duration::from_millis(200)).await;
    agent
        .notify("notifications/cancelled", json!({ "requestId": id }))
        .await;
    // The next answer is a later request's: none comes for the call cancelled.
    let listed = agent.send("tools/list", json!({})).await;
    let message = agent.next_message("the list").await;
    assert_eq!(message["id"], json!(listed), "{message}");
    for _ in 0..50 {
        if cancels.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        cancels.load(Ordering::SeqCst),
        1,
        "the program was told once"
    );
}

#[tokio::test]
async fn a_tool_gathers_the_program_s_short_pages_into_one() {
    // The program gives a button a page: the tool gathers them all, and the list ends there.
    let memory = Memory::paged(panel, Some(1));
    let send: Sends = Arc::new(move |method, params| memory.send(method, params));
    let mut agent = Agent::handshake(send, "a test").await;
    let (said, _) = agent
        .tool("find_elements", json!({ "role": "button" }))
        .await;
    assert!(!said[0].starts_with("next: "), "the list's end: {said:?}");
    assert_eq!(element_lines(&said).len(), 4, "{said:?}");
}

#[tokio::test]
async fn a_tool_stops_gathering_past_half_its_bytes_and_gives_the_rest_by_its_cursor() {
    // 400 buttons of long names, some 34 KB of JSON, given 50 a page: past half of the tool's
    // 32 KiB, it gives what it gathered and the cursor to the rest.
    let named = "a button of a long name, so that the list of them passes half a page";
    let memory = Memory::paged(buttons(400, named), Some(50));
    let send: Sends = Arc::new(move |method, params| memory.send(method, params));
    let mut agent = Agent::handshake(send, "a test").await;
    let mut listed = Vec::new();
    let mut cursor: Option<String> = None;
    let mut calls = 0;
    loop {
        let mut arguments = json!({ "role": "button" });
        if let Some(cursor) = cursor.take() {
            arguments["cursor"] = json!(cursor);
        }
        let (said, _) = agent.tool("find_elements", arguments).await;
        calls += 1;
        let page = element_lines(&said);
        assert!(
            page.len() < 400,
            "a call gathers part of the list: {}",
            page.len()
        );
        listed.extend(page);
        cursor = said[0]
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("next: "))
            .and_then(|rest| rest.split(' ').next())
            .map(str::to_owned);
        if cursor.is_none() || calls == 20 {
            break;
        }
    }
    assert!(calls > 1, "the rest by its cursor");
    assert_eq!(listed.len(), 400, "each button once");
    for (at, line) in listed.iter().enumerate() {
        assert!(
            line.starts_with(&format!("main/#{at} — Button")),
            "in order: {line}"
        );
    }
}

#[tokio::test]
async fn a_call_cancelled_while_it_waits_for_a_place_is_cancelled_once_sent() {
    // A program whose places in flight are all taken: the request is sent once the test frees one.
    let cancels = Arc::new(AtomicUsize::new(0));
    let (free, freed) = mpsc::channel::<()>();
    let freed = Arc::new(std::sync::Mutex::new(freed));
    let send: Sends = {
        let cancels = Arc::clone(&cancels);
        Arc::new(move |_, _| {
            let _ = freed.lock().expect("not poisoned").recv_timeout(PATIENCE);
            let cancels = Arc::clone(&cancels);
            Ok(Sent::new(
                || {
                    Ok(Answer {
                        outcome: Err(Fault::new(Fault::CANCELLED, "cancelled")),
                        bytes: Vec::new(),
                    })
                },
                move || {
                    cancels.fetch_add(1, Ordering::SeqCst);
                },
            ))
        })
    };
    let mut agent = Agent::handshake(send, "a test").await;
    let id = agent
        .send(
            "tools/call",
            json!({ "name": "act_on_interface", "arguments": { "action": "accept" } }),
        )
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    agent
        .notify("notifications/cancelled", json!({ "requestId": id }))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(cancels.load(Ordering::SeqCst), 0, "nothing sent yet");
    free.send(()).expect("the carrier waits");
    for _ in 0..50 {
        if cancels.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(cancels.load(Ordering::SeqCst), 1, "cancelled once sent");
}

#[tokio::test]
async fn a_fault_on_a_later_page_is_the_tool_s_error() {
    // The first page goes on; the next one's cursor no longer holds.
    let pages = Arc::new(AtomicUsize::new(0));
    let send: Sends = Arc::new(move |_, _| {
        let page = pages.fetch_add(1, Ordering::SeqCst);
        Ok(Sent::new(
            move || {
                let outcome = if page == 0 {
                    Ok(json!({ "reply": "elements", "elements": [], "next": "1" }))
                } else {
                    Err(Fault::new(Fault::STALE, "the interface is another"))
                };
                Ok(Answer {
                    outcome,
                    bytes: Vec::new(),
                })
            },
            || {},
        ))
    });
    let mut agent = Agent::handshake(send, "a test").await;
    let (said, result) = agent.tool("find_elements", json!({})).await;
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("list it again from its start"), "{said:?}");
}

/// Polls `memory` until it holds `count` waits, within [`PATIENCE`]: a cancel reaches it on its own
/// thread after the call's answer.
async fn until_held(memory: &Memory, count: usize) {
    let started = std::time::Instant::now();
    while memory.held() != count {
        assert!(
            started.elapsed() < PATIENCE,
            "the program holds {} waits, not {count}",
            memory.held()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The `after` that a wait's first line gives.
fn after_of(said: &[String]) -> String {
    let line = said[0].lines().next().expect("a line");
    let after = line
        .strip_prefix("after: ")
        .unwrap_or_else(|| panic!("no `after`: {said:?}"));
    after.split(' ').next().expect("a handle").to_owned()
}

/// A list to choose from opened, then closed, is waited for through the tool: its options' presence
/// held until a press opens them, their absence from that answer held until a cancel closes them,
/// and seen once, a wait from that answer held. A bound passed and an agent's cancel each let the
/// program's wait go.
#[tokio::test]
async fn a_list_to_choose_from_closed_is_waited_for_and_seen_once() {
    let memory = Memory::start(chooser);
    let carried = memory.clone();
    let send: Sends = Arc::new(move |method, params| carried.send(method, params));
    let mut agent = Agent::handshake(send, "a test").await;
    // Nothing opened: the bound passes, and the program no longer holds the wait.
    let (said, result) = agent
        .tool(
            "wait_for_elements",
            json!({ "role": "ListBox", "seconds": 0 }),
        )
        .await;
    assert_ne!(result["isError"], json!(true), "{said:?}");
    assert!(
        said[0].starts_with("nothing met the wait within 1 s"),
        "{said:?}"
    );
    until_held(&memory, 0).await;
    // Held while a press opens the options.
    let opening = agent
        .send_tool("wait_for_elements", json!({ "role": "listbox" }))
        .await;
    until_held(&memory, 1).await;
    let (said, _) = agent
        .tool(
            "act_on_interface",
            json!({ "action": "press", "element": "main/pick" }),
        )
        .await;
    assert_eq!(said, ["done"]);
    let (said, _) = agent.tool_answer(opening).await;
    let lines: Vec<&str> = said[0].lines().collect();
    assert_eq!(lines.len(), 2, "{said:?}");
    assert!(lines[1].contains("— ListBox"), "{said:?}");
    let opened = after_of(&said);
    // Their absence from that answer, held until a cancel closes them.
    let closing = agent
        .send_tool(
            "wait_for_elements",
            json!({ "role": "ListBox", "absent": true, "after": opened }),
        )
        .await;
    until_held(&memory, 1).await;
    let (said, _) = agent
        .tool(
            "act_on_interface",
            json!({ "action": "focus", "element": "main/pick" }),
        )
        .await;
    assert_eq!(said, ["done"]);
    let (said, _) = agent
        .tool("act_on_interface", json!({ "action": "cancel" }))
        .await;
    assert_eq!(said, ["done"]);
    let (said, _) = agent.tool_answer(closing).await;
    assert!(said[0].ends_with("no element matches"), "{said:?}");
    let closed = after_of(&said);
    // Seen once: from that answer, the absence is not met again.
    let (said, _) = agent
        .tool(
            "wait_for_elements",
            json!({ "role": "ListBox", "absent": true, "after": closed, "seconds": 1 }),
        )
        .await;
    assert!(
        said[0].starts_with(&format!("after: {closed} (nothing met the wait within 1 s")),
        "{said:?}"
    );
    until_held(&memory, 0).await;
    // An agent's cancel lets the program's wait go too.
    let cancelled = agent
        .send_tool("wait_for_elements", json!({ "role": "Dialog" }))
        .await;
    until_held(&memory, 1).await;
    agent
        .notify(
            "notifications/cancelled",
            json!({ "requestId": cancelled, "reason": "a test" }),
        )
        .await;
    until_held(&memory, 0).await;
}

/// A role or a state that the family does not name, and an `after` of another interface, are tools'
/// errors that say so.
#[tokio::test]
async fn a_wait_on_what_no_element_could_be_is_refused_and_another_interface_s_after_too() {
    let mut agent = Agent::handshake(in_memory(), "a test").await;
    for (arguments, said) in [
        (json!({ "role": "Buton" }), "no role `Buton`"),
        (json!({ "holding": ["pressd"] }), "no state `pressd`"),
    ] {
        let (texts, result) = agent.tool("wait_for_elements", arguments).await;
        assert_eq!(result["isError"], json!(true), "{texts:?}");
        assert!(texts[0].contains(said), "{texts:?}");
    }
    let (said, _) = agent
        .tool("wait_for_elements", json!({ "role": "Button" }))
        .await;
    let after = after_of(&said);
    let mut other = Agent::handshake(in_memory(), "a test").await;
    let (said, result) = other
        .tool(
            "wait_for_elements",
            json!({ "role": "Button", "after": after, "seconds": 1 }),
        )
        .await;
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("wait again without `after`"), "{said:?}");
    assert!(!said[0].contains("list it again"), "{said:?}");
}

/// More elements met than the program's page holds: the tool says so, after `after`, and the next
/// wait from it gives the rest at once, none twice.
#[tokio::test]
async fn a_wait_met_past_its_page_says_more_and_gives_the_rest_from_its_after() {
    let memory = Memory::paged(buttons(80, "button"), Some(50));
    let send: Sends = Arc::new(move |method, params| memory.send(method, params));
    let mut agent = Agent::handshake(send, "a test").await;
    let (said, _) = agent
        .tool("wait_for_elements", json!({ "role": "Button" }))
        .await;
    let lines: Vec<&str> = said[0].lines().collect();
    assert!(lines[1].starts_with("more elements met it"), "{said:?}");
    assert_eq!(lines.len() - 2, 50, "{said:?}");
    let (said, _) = agent
        .tool(
            "wait_for_elements",
            json!({ "role": "Button", "after": after_of(&said) }),
        )
        .await;
    let lines: Vec<&str> = said[0].lines().collect();
    assert!(!said[0].contains("more elements"), "{said:?}");
    assert_eq!(lines.len() - 1, 30, "{said:?}");
}
