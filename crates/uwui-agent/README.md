# uwui-agent

uwui's core for agents: the session through which an agent, or any tool, reads and drives a
program on uwui, and the engine's development channel speaks
([ADR-0218](../../docs/adr/0218-one-core-for-agents-in-uwui.md),
[ADR-0246](../../docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)).

Its name is provisional: the names of uwui's crates are settled at M6d.10
([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

## Status

M6d.9a1b1: the session and its messages, both ends, several requests in flight, answered in any
order, bounded and cancellable
([ADR-0250](../../docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)).
Events that a program sends, long lists read by pages, attaching to a program already running,
consent, rights and previews come with M6d.9a's later parts
([`stage-6d.md`](../../docs/stage-6d.md#m6d9a--one-core-for-agents-and-its-mcp-server)).

## Role

- A session without input or output. It opens no socket, starts no thread and reads no clock: its
  *carrier*, the code that owns the connection, gives it the bytes that came and sends the bytes
  that it gives back. Whoever dialled, the program answers. The engine's carrier is
  `slime-channel`'s, on the machine's own address with a token; the web's, Android's and
  `uwui dev`'s come later, the session unchanged.
- Messages are JSON-RPC 2.0, one a line. A request has its number, at most `MAX_EXACT_INTEGER`,
  its method, `family/name`, and its parameters; an answer has the number, and a result or a `Fault`, with a
  code; an event is a notification. A message that carries bytes, an image's pixels, counts them
  in its envelope, `"bytes": 8294400`, and they follow its line at once, written by the
  connection's one writer.
- `Reader` reads messages from bytes as they come, in pieces of any size, within `Limits`: a line
  or bytes past them, or a line that is not JSON-RPC 2.0, are refused, and the connection closed.
- The first message, `hello`, names the agent and the program that carries it, as they say they
  are, gives the carrier's secret, says how large an answer the tool reads (`Limits`), and lists
  the *families* that the tool speaks, each at its version: a family is a named group of requests and events, `play` for `play/pause`. The
  `ProgramSession` welcomes it with the families that the program serves at the same versions, and
  refuses the others one by one, or refuses the hello whole for another version of the core
  (`CORE_VERSION`) or another secret. A request of a family not served is answered `NOT_SERVED`.
- `ToolSession` numbers the requests, refuses to send one of a family not served, and matches each
  answer to the request that waits for it.
- A request is *in flight* from when the tool sends it until its answer comes: several may be, the
  welcome stating how many and how many bytes of their lines (`InFlight`, whose rustdoc gives the
  default). The program begins them in their order and answers each when it is done, in any order,
  by its number. `ToolSession::request` refuses one past the bounds, `AgentError::Full`, so that
  its carrier waits for an answer; the program answers one that passes them anyway `BUSY`, and
  one whose number is in flight already `INVALID`. A request's place frees once its carrier takes
  its answer to write it, and says so, `ProgramSession::written`.
- The tool cancels a request in flight with the core's event `cancel`. The program still answers
  it once: with its result if it had changed something, `CANCELLED` if it had not, and then never
  does it. An answer larger than the tool reads becomes the fault `TOO_LARGE`, without its bytes.

## Entry points

```rust
use serde_json::Value;
use uwui_agent::{
    CORE_VERSION, Family, FromProgram, FromTool, Hello, InFlight, Limits, Named, ProgramSession,
    Reader, ToolSession,
};

// Each end reads what the other wrote through a reader of its own.
fn read(bytes: &[u8]) -> uwui_agent::Frame {
    let mut reader = Reader::new(Limits { line: 1 << 20, bytes: 0 });
    reader.push(bytes);
    reader.message().expect("a message").expect("whole")
}

let mut program = ProgramSession::new("my game", vec![Family::new("play", 1)], InFlight::DEFAULT);
let mut tool = ToolSession::new();
let hello = tool
    .hello(&Hello {
        core: CORE_VERSION,
        agent: Named::new("my agent", "1.0"),
        client: Named::new("my server", "0.1"),
        secret: "the token".to_owned(),
        families: vec![Family::new("play", 1)],
        limits: Limits { line: 1 << 20, bytes: 1 << 30 },
    })
    .expect("written");
let FromTool::Welcomed { reply, .. } = program
    // A real carrier compares a token in a time that does not depend on where it differs.
    .receive(read(&hello), |secret| secret == "the token")
    .expect("read")
else {
    panic!("welcomed");
};
assert!(matches!(tool.receive(read(&reply)), Ok(FromProgram::Welcomed(_))));
let (id, request) = tool.request("play/pause", Value::Null).expect("served");
let FromTool::Request { name, .. } = program.receive(read(&request), |_| false).expect("read")
else {
    panic!("a request");
};
assert_eq!(name, "pause");
let (answer, _) = program.answer(id, Ok(Value::Null), 0).expect("written");
assert!(matches!(tool.receive(read(&answer)), Ok(FromProgram::Response { .. })));
```

The rustdoc holds each item's contract: the codes of a `Fault`, the limits, the errors.

## Dependencies

- `serde` and `serde_json`: the messages, JSON-RPC 2.0, the format that MCP, the web and Android
  carry as they are (ADR-0246).
- `thiserror`: derives `AgentError`'s `Display` and `Error` implementations (ADR-0008).

## Must not

- Depend on another crate of uwui's or of the engine: a tool speaks the protocol without building
  uwui's text and layout.
- Open a socket, start a thread or wait: its carrier does.
- Read past its limits, or panic on what a connection sends.
- Write a hello's secret anywhere but in the hello: its `Debug` form hides it.
