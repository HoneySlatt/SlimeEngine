# 0212. A function's body reloaded through subsecond and dx

- Status: Accepted; amends [0190](0190-uwuis-requirements.md) (Rust's code reloaded: through
  `subsecond` and `dx`, under `uwui dev`), [0202](0202-how-m6d1-compares-ways-of-writing-an-application.md)
  (M6d.1c3 cut in two) and [0210](0210-signals-named-for-a-reload.md) (the body's reload timed
  over kept state); amended by
  [0224](0224-a-files-change-reaches-the-application-through-the-development-tool.md) (one
  connection in development: `uwui dev`'s, which relays `dx`'s patches)
- Date: 2026-11-07

## Context

[ADR-0190](0190-uwuis-requirements.md) asks that an application's Rust code reload at once, its
state kept, and that a change of its interface be seen within 2 s.
[ADR-0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) answers three reloads apart:
a style or a layout, a function's body, and a type whose state migrates.
[ADR-0210](0210-signals-named-for-a-reload.md) decided the third: signals named by their authors,
kept through `serde`. M6d.1's bench had simulated the second with a table of the entry points'
addresses that a test swaps.

`subsecond`, by the Dioxus team, calls functions through a table of their latest addresses. Its
tool `dx` watches the sources, compiles a change, links it against the running program's
addresses, and sends the new table over a websocket. Iced, GPUI and Rune reload through it.
[uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#8-reloads-on-the-web-and-on-android)
found it the only tool for Rust's code that covers the desktop, the web and Android.

At its start, M6d.1c3 held two dependencies, the tool `dx`, a prototype of Slatty's size and
seven rivals to time. The owner decided on 2026-11-07, one question at a time.

## Decision

**M6d.1c3 is cut in two:**

- M6d.1c3a: a function's body reloaded through `subsecond` and `dx`, on a prototype of Slatty's
  size, timed; the web and Android weighed; this ADR;
- M6d.1c3b: the rivals' reloads timed from outside on their own examples, Dioxus, Iced, GPUI,
  Makepad, Slint, Flutter and Compose Hot Reload, by a step of uwui's `scripts/measure.py` from a
  file saved to the first frame that changes. It closes M6d.1c.

**A function's body is reloaded through `subsecond`, its patches compiled and sent by `dx`.**

- A development tool of uwui's connects to `dx`'s server, whose address `dx` gives the program,
  on a thread of its own. It is absent from a build that ships, as `subsecond` is without debug
  assertions.
- That thread gives each patch to the interface's thread through the host's waker. The host
  applies it between two frames, never during one.
- The host then runs each root's view again through `subsecond`'s table, as a reload of
  ADR-0210 does: the new body runs, each kept signal takes its value back by its name, and the
  others start again.

On the bench's branch, the patches come through `tungstenite` and some thirty lines that read
`dx` 0.7.10's messages, as GPUI does. uwui's first stage writes its own client, and pins the
version of `dx` whose protocol it reads.

**`uwui dev` compiles the application's crates with their line tables alone.** At Slatty's size,
full debug information puts the time over that budget, and line tables bring it back within. `dx`
builds into its own folder, apart from cargo's `dev`, so `cargo build` and `cargo run` keep full
debug information, and a debugger sees local variables there. Under `uwui dev`, it sees lines and
calls only.

**What a patch must see stays outside the crates that it links again.** On Linux, `dx` links the
patch of a changed crate as a shared library, from all that crate's code. In builds with debug
assertions, the only ones that patches reach:

- a thread-local defined in a crate that `dx` patches does not link: an application's crates
  hold none;
- one reached by code compiled in that crate, a generic function or one inlined, becomes an empty
  copy of the patch's own. uwui reaches its own thread-locals only through functions that are
  neither generic nor inlined, which the patch calls in the running program. Builds that ship
  may inline them.

**On the web and on Android, weighed from the survey, not tried:** `subsecond` patches wasm32 and
Android's arm64 and armv7, and `dx` serves both. uwui's hosts for them, which do not exist yet,
connect the same way: a page's websocket on the web, `adb reverse` on Android. A patch on Android
is a shared library too, so the rule on thread-locals holds there.

## Alternatives considered

- **A program started again, its kept values written to a file and read back:** no dependency,
  and it works on every platform. On the bench, cargo builds the same change again in about the
  patch's time. But the program loads its data again, its window and the GPU's resources are
  made again, and the core's threads, connections and caches start again.
- **Bodies in a dynamic library of the application's, reloaded:** no tool from outside, but every
  call that a reload reaches goes through a table that the framework writes by hand, the
  library's types must not change, and wasm loads no such library.
- **The logic in a language of scripts:** reloads without compiling, but ADR-0190 keeps Rust for
  the logic.
- **`dioxus-devtools` for the connection,** `connect_subsecond()` in one call: the least code,
  but some thirty crates, Dioxus's own signals among them.
- **A websocket client written by hand:** no crate of network, but a protocol rewritten, its
  fragmented frames and pings, for a client that a crate already gives.
- **Full debug information under `uwui dev`:** a debugger sees every variable during a reload,
  but the budget missed at Slatty's size. **No debug information:** faster still in a first try,
  not measured at rest, but a panic no longer says where it happened.

## Consequences

- The figures are in
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-functions-body-reloaded-through-subsecond-2026-11-07):
  from a change saved to the frame that shows it, within the budget at Slatty's size with line
  tables alone, the state kept; nearly all of it `dx`'s build and link. A patch links the whole
  crate that changed, so the time grows with that crate's size: smaller crates for an
  application's interface reload faster.
- The reload saves little of the compiler's time over cargo's own build. It saves the program's
  start, its data and the state that the author would have to find again.
- uwui's first stage builds `uwui dev` around `dx`, its client of `dx`'s protocol, and the rule on
  thread-locals in its core. `subsecond` and a websocket client become uwui's dependencies then,
  in a crate of development tools, each put to the owner with its version.
- The bench's thread-locals moved to a crate of their own, `slime-bench-locals`, reached through
  such functions; the prototype is thrown away once M6d.1 ends.
- A patch is unsafe: a table built for another program would replace functions with wrong ones.
  The client applies only patches that name its build and its process, as `dx` addresses them.
- Not tried: the web and Android, which have no host yet; Windows and macOS, whose patches `dx`
  links otherwise; a window, whose frame M6d.7's host adds to the time.
