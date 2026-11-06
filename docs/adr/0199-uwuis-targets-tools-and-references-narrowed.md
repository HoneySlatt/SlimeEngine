# 0199. uwui's targets, tools and references narrowed

- Status: Accepted; amends [0190](0190-uwuis-requirements.md) (the owner's desktop, its shell,
  its login and lock screens and what its compositor draws, no longer targets; `uwui bundle`
  delegates to the platforms' tools; updates installed by an application to be evaluated, outside
  the core) and [0198](0198-screens-of-reference-and-the-rivals-measured.md) (M6d.1's references
  and M6d.0b's measures widened)
- Date: 2026-11-06

## Context

[ADR-0190](0190-uwuis-requirements.md) set what uwui runs and what tools it brings, among them the
owner's desktop, its shell Amane, its login and lock screens and what the compositor Umbriel draws
itself; `uwui new`, `run`, `dev`, `bundle` and `doctor`; and updates signed and installed by an
application. The owner no longer wants uwui's interfaces for the shell, the login or the lock.

A comparison of Rune, Iced, MyGo, GPUI, Makepad, Floem and Electron, read from their documentation
and sources, found where each gains or loses in performance. None of it is measured here.

- Each framework's costs fall in distinct phases: the state, the view and its reconciliation, the
  layout, the text's shaping and rasterizing, the preparation of the GPU's work and its uploads,
  the GPU, the presentation. A model's name, immediate, retained, signals or messages, says little
  of which phase costs.
- Floem updates through fine-grained signals; Rune animates properties without running views
  again; Makepad reloads a format of interface of its own, with a runtime and tools to keep.
- Frames held at the display's rate can hide their cost in the CPU, the compositor and the GPU.
  Memory may not come back after a list is scrolled and closed.
- Updates installed by an application commit uwui to signing keys, compatibility, interrupted
  installs and rollback, one channel a platform; on Linux, Flatpak and the distributions update
  applications.

## Decision

The owner's decisions of 2026-11-06, one at a time.

**uwui no longer targets the owner's desktop:** neither its shell, nor its login and lock screens,
nor what the compositor draws itself. Their hosts leave uwui's plan.

**M6d.1 compares Floem, Rune and Makepad too,** beside GPUI with gpui-kit, Iced, Slint, Dioxus,
Flutter, Jetpack Compose and SwiftUI: Floem for signals, Rune for motion that runs no view again,
Makepad for a format of interface reloaded at once. Each is compared on the same cases: lines,
concepts to learn, errors, the state kept across a reload, and the cost of each phase. A reload is
three questions, each answered apart: a style or a layout, a function's body, a type whose state
migrates. MyGo is read at M6d.10, for its contracts with the platforms and its delivery.

**M6d.0b's method also measures,** from outside: memory after a list is scrolled and after it is
closed; the 95th percentile of frames beside the 99th; presentations missed.

**uwui measures its own phases from M6d.4:** each milestone that touches a phase adds its counters
and its discriminating cases to uwui's benchmarks, checked in CI as the engine's are
([ADR-0186](0186-counters-and-ratios-in-ci.md)): a colour changed without the text shaped again, a
small change in a large panel, a cache full that evicts.

**A matrix of platforms and functions** in uwui's roadmap from M6d.10, each cell available,
experimental, planned or not checked: input methods, accessibility, the GPU and the CPU's
renderer, tools, delivery.

**Tools.** `uwui dev`, its reload, inspector, tests and MCP server, a priority, as ADR-0190 has it.
`uwui new` a command of uwui's own, as ADR-0190 has it. `uwui bundle` a command that delegates to
the tools that make each package, cargo-packager, flatpak-builder, Gradle, wasm-pack or trunk,
uwui bringing their common configuration only, built when the pilots are to be distributed.
Updates installed by an application are no longer promised: at most an optional crate, later, for
Windows and macOS, decided when a pilot needs it; Linux, Android and the web update through their
own channels.

## Alternatives considered

- **Umbriel's drawing kept as a target,** or kept as a track to evaluate: a host for later, but
  the core pulled towards the system for a use the owner no longer wants.
- **Floem and Rune alone at M6d.1,** without Makepad's runtime and tools; **no reference added:**
  less work, but signals and motion untried before the choice.
- **Fewer measures at M6d.0b:** memory after a close alone, the gap that the owner's study of GPUI
  found largest; or none.
- **uwui's phases measured in a milestone of their own** after M6d.1, on code that then moves; or
  after the move, in uwui's first stage.
- **Templates on Cargo alone,** after the API is stable, instead of `uwui new`; **a packager of
  uwui's own,** or none; **updates kept as promised,** or never uwui's.

## Consequences

- uwui's roadmap loses the hosts of a shell, of login and lock screens and of a compositor.
- M6d.1 grows by three references; M6d.0b by three measures.
- Whether games' interfaces need a framework apart is not decided: M6d.10 weighs it, from what
  *Hunna Wood*'s interface costs the game.
