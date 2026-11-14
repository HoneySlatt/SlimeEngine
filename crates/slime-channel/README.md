# slime-channel

The development channel: the link between a running game and the tools that drive it, the
engine's MCP server, tests, and later the editor
(`docs/adr/0110-how-the-editor-runs-a-games-code.md`, `docs/adr/0122-the-development-channel.md`).

## Status

The engine's families on the session of uwui's core for agents (`uwui-agent`,
`docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md`): `play`,
`world`, `editing` and `interface`, each at its version (`FAMILIES`). They hold the host's
controls (pause, resume, speed, steps, stop), the game's window minimized
(`docs/adr/0187-a-hidden-window-at-rest.md`), commands that set
a part of a component, the world's commands applied from their texts, which an editor sends the game
that it plays (`docs/adr/0165-editing-while-playing.md`), questions on the world, captures of the
game's image or of one of its views (`docs/adr/0126-views-apart-from-the-renderer.md`), the settings
of its views read and changed (`docs/adr/0127-the-settings-of-a-view.md`), its reports read
(`docs/adr/0128-asset-requests.md`), the entity at a pixel of a view picked
(`docs/adr/0130-picking.md`), the interface's family, scripted keys, the game's state, log and frame times, its run
saved to a file and loaded from one (`docs/adr/0125-saves-per-scene.md`), and an editor's requests
(`Request::Edit`): steps of commands as text, undo and redo, its history, its journal, its selection
and its level written back (`docs/adr/0135-the-editors-program.md`), its operations listed and run
by name (`docs/adr/0156-the-editors-window-and-its-operations.md`), and where the game that it plays
listens, with its token (`docs/adr/0165-editing-while-playing.md`). The interface's family is
uwui's own: its messages are `uwui_agent::interface`'s, re-exported as `interface`, carried as
`Request::Interface` and `Reply::Interface`, and `uwui` answers them with its feature `agents`
(`docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`):
the elements of an interface listed by role and name, with their targets and what their fields
hold, found at a pixel, acted on, a field, a slider, two panels, a list to choose from or a color
well given a value, an element dropped onto another
(`docs/adr/0168-rearranging-the-hierarchy.md`), captured
(`docs/adr/0143-what-agents-see-of-the-interface.md`, `docs/adr/0146-screen-readers.md`), and where
the code that wrote an element's own style is. Its one event, `play/time`,
tells how game time stands to a connection that subscribed to it. Only programs built with
`slime-app`'s `development` feature listen; a build for shipping holds no channel.

## Role

- A game started with a token in its environment, `TOKEN_ENV_VAR`, listens on `127.0.0.1`, on a
  port that the system chooses, which it writes on its standard output (`ANNOUNCEMENT`). It
  accepts a connection only with the token and the same version of the core, and tells a
  connection that it refuses why.
- It carries the session of uwui's core for agents, `uwui-agent`: JSON-RPC 2.0 a message a line,
  a request named by its method, `family/name`, `play/pause`. The first message, the hello, names
  the agent and the program that carries it, as they name themselves, which the game logs, gives
  the token, and lists the families that the tool speaks: one of another version is refused alone,
  and the rest served. `Request` and `Reply` are the families' messages, written through `serde`;
  a reply carries bytes of its own after its line, an image's pixels; `Reply::Failed` crosses as an
  error of the core's. Entities are named by persistent identities, values given and answered as
  text in the engine's notation, which the game reads with its types: a tool needs none of them.
- A connection may have several requests in flight
  ([ADR-0250](../../docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)).
  The game takes them in their order, one of each connection in turn, and answers each when it is
  done, in any order: a read passes an edit that waits for a person's drag. A request cancelled
  before it changed anything is answered `CANCELLED` and never done; those of a connection that
  closed are never done. The game welcomes a bounded number of connections, and takes none of a
  connection's requests while what it was sent and did not read passes a bound, nor of any
  connection holding something unwritten while all of them pass another: a tool that stops
  reading stops being served. The bounds are kept by the listener until a second carrier
  ([ADR-0252](../../docs/adr/0252-the-channels-bounds-of-memory-kept-by-its-carrier-until-a-second-one.md));
  `Listener::open`'s rustdoc gives their figures.
- A connection subscribes to the events that the families declare (`FAMILIES`, `Event`), which
  the welcome lists
  ([ADR-0254](../../docs/adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md)).
  Each subscription has a *cell* in its connection: the value that the game published last and
  the writer has not taken, which a newer one replaces, and the last value written. The writer
  starts a subscription as it takes the answer to `subscribe`, so that the first value follows
  the answer, and ends it as it takes the answer to `unsubscribe`, so that nothing follows. It
  writes a value only when it differs from the last written, and the cells that wait once between
  two answers: a tool that stops reading holds one value of each event, and an event waits behind
  the one message being written.
- Who starts a game draws its token, 128 bits of the system's randomness, and gives it to the game
  in its environment only, never on its command line.

## Entry points

```rust
use std::thread;
use slime_channel::{Client, Listener, Named, Reply, Request, Token};

let token = Token::generate().expect("random bits");
let listener = Listener::open(token.clone(), "my game", || {}).expect("a port");
let address = listener.address();
// A game answers at the start of its frames; here, a thread answers one request.
let game = thread::spawn(move || loop {
    if let Some(incoming) = listener.next() {
        assert_eq!(incoming.request(), &Request::Pause);
        incoming.answer(Reply::Done);
        break;
    }
    thread::yield_now();
});
let agent = Named::new("my agent", "1.0");
let server = Named::new("my MCP server", "0.1");
let client =
    Client::connect(address, &token, &agent, &server).expect("the token is the game's");
assert_eq!(client.program(), "my game");
let (reply, _) = client.request(&Request::Pause).expect("an answer");
assert_eq!(reply, Reply::Done);
game.join().expect("the game answered");
```

- `Token`: the secret, drawn with `Token::generate`, written as 32 hexadecimal digits, compared in
  a time that does not depend on where two tokens differ; its `Debug` form hides it.
- `Listener::open(token, program, wake)`: the game's end, which `slime-app`'s `run` opens when the
  environment gives a token, and which calls `wake` as each request comes or is cancelled, so that
  a game that waits runs a frame; `Listener::round` gives a request of each connection that has
  one, `Listener::next` one alone, which the game answers with `Incoming::answer`, or with
  `Incoming::answer_with` when the reply carries an image's pixels, shared; `Incoming::cancelled`
  says whether its tool gave it up, and `Incoming::cancel` answers it so. `Listener::wants` says
  whether a connection subscribed to an event, and `Listener::publish` leaves it for each, which a
  game does at the end of a frame.
- `launch(&mut command, timeout)`: starts a game with a new token, and waits for it to say where it
  listens; `Client::connect(address, &token, &agent, &client)` then opens a connection for an
  agent and the program that carries its requests, and `Client::request`
  sends a `Request` and waits for its `Reply`, from any thread; `Client::send` sends it without
  waiting, its `Pending` answer read with `Pending::wait` or given up with `Pending::cancel`.
  `Client::connect_with` hands each event and the connection's end to a function of the tool's,
  on the client's own thread, and `Client::subscribe` subscribes. The game's standard input is a pipe that the tool
  holds, and the game ends once it closes, so that no game outlives the tool that started it.
  Both ends send each message at once, Nagle's algorithm off, so that a request is answered
  within the game's next frame rather than after tens of milliseconds of acknowledgements.
  `launch_unless` gives the start up once the caller says so, as an editor's Play does when its
  user stops a game that is still starting (`docs/adr/0164-play.md`).

The rustdoc holds the contracts: each request and its reply, the limits, the errors.

## Dependencies

- `getrandom`: the token's random bits, which the standard library does not give yet.
- `serde`: the families' messages, which `uwui-agent` writes as JSON.
- `uwui-agent`: the session that the channel carries.
- `thiserror`: derives `ChannelError`'s `Display` and `Error` implementations.
- `tracing`: the connections accepted and refused.

## Must not

- Listen on another address than the machine's own, or accept a connection without the token.
- Depend on the windowing system, the GPU or a game's types: the MCP server links it, and needs
  none of them.
- Write a token into a log, a message other than the hello, or a command line.
