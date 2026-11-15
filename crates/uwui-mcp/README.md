# uwui-mcp

uwui's MCP library: through it, an AI agent reads and drives a program on uwui with the Model
Context Protocol (MCP), the open protocol through which coding agents, Claude Code and Codex among
them, call the tools of other programs
([ADR-0218](../../docs/adr/0218-one-core-for-agents-in-uwui.md),
[ADR-0269](../../docs/adr/0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)).
It turns MCP's tools into requests of uwui's core for agents (`uwui-agent`), which a program
answers.

Its name is provisional: the names of uwui's crates are settled at M6d.10
([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

## Status

M6d.9a2b1: the library, the interface's family as its tools, and a server's tools gathered by
families. The engine's MCP server, `slime-mcp`, serves every tool through it, its own families
`play`, `world` and `editing` among them. No program of its own: uwui's MCP server as a program
comes with a carrier of uwui's own, `uwui dev`'s, at uwui's first stage. Tools that wait for a
change come at M6d.9a2c
([`stage-6d.md`](../../docs/stage-6d.md#m6d9a2--uwuis-mcp-library-and-families)).

## Role

- A *carrier* takes the library's requests to a program: `Carrier`, which the MCP server's own
  type implements, sends a request of the core, `family/name` with its parameters, to the program
  that serves it now, and gives what waits for its answer and what cancels it (`Sent`). The
  engine's carrier is its development channel; this crate's tests carry their requests in memory,
  to an interface that `uwui` answers.
- `ask` sends a request on a thread that may block, so that the server answers its other calls
  meanwhile. A call that its agent cancels, MCP's `notifications/cancelled`, cancels the request,
  and gives no answer: MCP answers no cancelled call. `request` does the same for a request of a
  family's own type, and reads its answer as that family's reply: every family's tools take it.
- A family of the core is a `Family`: its name, its tools and its paragraph of the server's
  instructions. `Tools` gathers a server's own tools, which send no request of a family, and its
  families, each name given once: a family that gives a name already taken is an error, `Taken`,
  where rmcp's routers, added up, would replace a tool without a word. It writes the server's
  instructions, its own paragraph first, then each family's. A family that the program does not
  serve is refused by the carrier, as it sends
  ([ADR-0274](../../docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md)).
- `interface_family` gives the interface's family, for any server's type that is a carrier:
  `find_elements`, `element_at`, `act_on_interface`, `capture_element` and `element_style`.
- `gather` reads a long list for an agent: the program's pages, each after the last, gathered
  within the bytes that the agent asks, at most `PAGE_BYTES`, and `LIST_TIME`, the cursor of the
  rest written first, as `next_line` writes it. A list's request (`List`) says how its rest is
  asked and how its page (`Page`) reads; the cursors pass through untouched.
- A program's faults are tools' errors, marked `isError`, which a model reads and corrects itself
  by, as MCP asks of whatever a tool's arguments make fail (`fault`): a cursor that no longer
  holds, `STALE`, says to list the list again from its start. JSON-RPC's errors are left to rmcp,
  for a tool unknown or a call malformed.
- `agent_of` names the agent as its MCP client names itself, in MCP's handshake or in each request
  of MCP `2026-07-28`, its name and version cut in bytes of JSON so that a program's hello holds
  them.

## Entry points

A server's type holds what reaches its program, and gathers its tools with the interface's:

```rust,no_run
use rmcp::handler::server::router::tool::ToolRouter;
use uwui_mcp::{Answer, Carrier, Sent, Tools, Value, interface_family};

#[derive(Clone)]
struct Server;

impl Carrier for Server {
    fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        // A real carrier sends the request on its connection, and waits there for its answer.
        let _ = (method, params);
        Ok(Sent::new(
            || Ok(Answer { outcome: Ok(Value::Null), bytes: Vec::new() }),
            || {},
        ))
    }
}

let tools = Tools::new("A server of a program.", ToolRouter::<Server>::new())
    .with(interface_family())
    .expect("no tool's name taken twice");
assert!(tools.router().has_route("find_elements"));
assert!(tools.instructions().starts_with("A server of a program.\n\n"));
```

The rustdoc holds each item's contract.

## Dependencies

- `uwui-agent`: the core's messages and the interface's family.
- `rmcp`, with `tokio`, its runtime: MCP's official SDK in Rust; `serde` and `serde_json` read the
  tools' parameters and the families' answers.
- `png` and `base64`: a capture returned as a PNG image, in base64 as MCP carries it.
- For its tests: `uwui` with its feature `agents`, whose answers give the interface in memory.

## Must not

- Depend on the engine: its families are the engine's to add, in `slime-mcp`.
- Be linked by an application: `rmcp` is under Apache-2.0 alone, and `tokio` is no runtime of
  uwui's ([ADR-0211](../../docs/adr/0211-the-executor-and-other-threads-through-channels.md)).
- Keep state for an agent between its calls: MCP `2026-07-28` has no session, so what follows a
  call is a handle that the agent passes back.
- Show a program's secret to an agent.
