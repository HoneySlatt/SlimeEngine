# 0224. A file's change reaches the application through the development tool

- Status: Accepted; amends [0212](0212-a-functions-body-reloaded-through-subsecond-and-dx.md)
  (one connection in development: `uwui dev`'s, which relays `dx`'s patches)
- Date: 2026-11-09

## Context

The themes' values, rules and parts are files ([ADR-0222](0222-what-a-theme-is.md)), as an
application's style sheets are, and as M6d.1d2's layouts will be. A change of one must be seen
within 2 s ([ADR-0190](0190-uwuis-requirements.md)), the state kept. `Ui::set_style` already takes
new sheets and themes while the interface runs and keeps the style it had when one does not read
([ADR-0141](0141-style-sheets-and-themes.md)). No program reads them again when their files
change.

Rust's code reloads through `dx`, which `uwui dev` runs. `dx` sends its patches to the application
over a connection: a websocket on the desktop and the web, through `adb reverse` on Android
([ADR-0212](0212-a-functions-body-reloaded-through-subsecond-and-dx.md)). On the web and Android,
the application cannot see the author's files. `dx` reloads files other than Rust's only where they
are declared through Dioxus's `asset!` macro (`serve/runner.rs` of dioxus-cli 0.7.10).

M6d.1d1 measured a style sheet's change on M6d.1's bench, on 2026-11-09: Slatty's library on
signals, played to a game's page, its sheet saved with its title's size changed. A thread of the
bench watched the file each millisecond, standing for a watcher. From the save to the end of the
frame that shows the change, the median of 21 was 2.9 ms in a release build and 13.6 ms in a debug
build, an author's, the state kept at each run
([uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-style-sheets-change-seen-2026-11-09)).
The program's own part, the rules made again and the frame, took some 2.4 ms and 13.0 ms of it,
by the medians of its parts.

The first proposal was attacked before it reached the owner: it opened a second connection beside
`dx`'s, and applied files saved together one at a time. A value renamed in a theme and its use in a
sheet would then be refused between the two saves. The owner decided on 2026-11-09.

## Decision

**`uwui dev` watches the files and sends their text to the application.** These are the themes'
and the style sheets' files that the application names, and layouts once M6d.1d2 decides them. The
application receives a text and styles again between two frames. It holds no watcher and no
dependency for it.

- *One connection.* The application connects to `uwui dev` alone. `uwui dev` connects to `dx`'s
  server in the application's name and passes its patches on. The desktop, the web and Android
  take the same path.
- *Files saved together are applied together.* `uwui dev` gathers the files that changed until
  they stay as they are for a short interval, as `slime-cook watch` does. It sends them in one
  message, which the application applies in one call.
- *A file that does not read keeps the last good style.* The error, with its file and line, is
  shown by the application in development, by `uwui dev`, and in the answer given to agents.
- *A host may give texts itself.* uwui's call that takes them is the host's API. The engine may
  feed it from its cooker.
- *Nothing of it ships.* A release build holds its files and no connection, and no request to
  style again from outside.

Until `uwui dev` and its connection exist, the prototypes stand in for them, as the bench's
watching thread does.

## Alternatives considered

- **The application watches its own files,** in development, through `notify`: it works without a
  tool. But it adds a dependency to uwui, and it works on the desktop alone: the web and Android do
  not see the author's files.
- **The application looks at its files' times at an interval,** with the standard library alone:
  no dependency. But the desktop alone, a wake-up at each interval while developing, and the
  interval's delay.
- **Two connections, `dx`'s and `uwui dev`'s:** nothing to relay. But two clients, two ways to fail,
  on each platform.

## Consequences

- `uwui dev` holds a watcher of files; its library, `notify` or another, is put to the owner when
  the tool is built, in uwui's first stage.
- Relaying `dx` ties `uwui dev` to `dx`'s protocol, which already ties uwui (ADR-0212): a version
  of `dx` changes both.
- The watcher's own delay, the system's event to the text sent, is measured when the tool is built;
  the bench's figure bounds the program's part alone.
- The editor's and the dev overlay's look, read when they start and not from the disk while they
  run ([`debt.md`](../debt.md)), takes the same path once they run on uwui's tool or their host
  gives the texts.
