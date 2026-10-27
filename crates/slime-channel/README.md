# slime-channel

The development channel: the link between a running game and the tools that drive it, the
engine's MCP server, tests, and later the editor
(`docs/adr/0110-how-the-editor-runs-a-games-code.md`, `docs/adr/0122-the-development-channel.md`).

## Status

The protocol, version 9: the host's controls (pause, resume, speed, steps, stop), commands that set
a part of a component, questions on the world, captures of the game's image or of one of its views
(`docs/adr/0126-views-apart-from-the-renderer.md`), the settings of its views read and changed
(`docs/adr/0127-the-settings-of-a-view.md`), its reports read (`docs/adr/0128-asset-requests.md`),
the entity at a pixel of a view picked (`docs/adr/0130-picking.md`), the elements of its interface
listed by role and name, with what its fields hold, found at a pixel, acted on, a field, a slider,
two panels, a list to choose from or a color well given a value, and captured
(`docs/adr/0143-what-agents-see-of-the-interface.md`, `docs/adr/0146-screen-readers.md`), scripted
keys, the game's state, log and frame times, its run saved to a file and loaded from one
(`docs/adr/0125-saves-per-scene.md`), and an editor's requests (`Request::Edit`): steps of commands
as text, undo and redo, its history, its journal, its selection and its level written back
(`docs/adr/0135-the-editors-program.md`). Only programs built with `slime-app`'s `development`
feature listen; a build for shipping holds no channel.

## Role

- A game started with a token in its environment, `TOKEN_ENV_VAR`, listens on `127.0.0.1`, on a
  port that the system chooses, which it writes on its standard output (`ANNOUNCEMENT`). It
  accepts a connection only with the token and the same version of the protocol, and tells a
  connection that it refuses why.
- Messages are reflected values in the engine's binary encoding (`slime-serialize`), each followed
  by bytes of its own, empty but for an image's pixels. Entities are named by persistent identities, values
  given and answered as text in the engine's notation, which the game reads with its types: a tool
  needs none of them.
- Who starts a game draws its token, 128 bits of the system's randomness, and gives it to the game
  in its environment only, never on its command line.

## Entry points

```rust
use std::thread;
use slime_channel::{Client, Listener, Reply, Request, Token};

let token = Token::generate().expect("random bits");
let listener = Listener::open(token.clone(), "my game").expect("a port");
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
let mut client = Client::connect(address, &token).expect("the token is the game's");
assert_eq!(client.program(), "my game");
let (reply, _) = client.request(&Request::Pause).expect("an answer");
assert_eq!(reply, Reply::Done);
game.join().expect("the game answered");
```

- `Token`: the secret, drawn with `Token::generate`, written as 32 hexadecimal digits, compared in
  a time that does not depend on where two tokens differ; its `Debug` form hides it.
- `Listener::open(token, program)`: the game's end, which `slime-app`'s `run` opens when the
  environment gives a token; `Listener::next` gives a request that waits, which the game answers
  with `Incoming::answer`, or with `Incoming::answer_with` when the reply carries an image's
  pixels.
- `launch(&mut command, timeout)`: starts a game with a new token, and waits for it to say where it
  listens; `Client::connect(address, &token)` then opens a connection, and `Client::request`
  sends a `Request` and waits for its `Reply`. The game's standard input is a pipe that the tool
  holds, and the game ends once it closes, so that no game outlives the tool that started it.
  Both ends send each message at once, Nagle's algorithm off, so that a request is answered
  within the game's next frame rather than after tens of milliseconds of acknowledgements.

The rustdoc holds the contracts: each request and its reply, the limits, the errors.

## Dependencies

- `getrandom`: the token's random bits, which the standard library does not give yet.
- `slime-reflect` and `slime-serialize`: the messages are reflected values, in the binary encoding.
- `thiserror`: derives `ChannelError`'s `Display` and `Error` implementations.
- `tracing`: the connections accepted and refused.

## Must not

- Listen on another address than the machine's own, or accept a connection without the token.
- Depend on the windowing system, the GPU or a game's types: the MCP server links it, and needs
  none of them.
- Write a token into a log, a message other than the hello, or a command line.
