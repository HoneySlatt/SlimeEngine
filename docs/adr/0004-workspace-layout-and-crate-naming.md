# 0004. Workspace layout and crate naming

- Status: Accepted; amended by [0014](0014-amend-roadmap-for-stage-2-and-windows.md) (no `core` crate in Stage 2)
  and [0055](0055-games-in-the-workspace.md) (a `games/` directory)
- Date: 2026-10-12

## Context

The engine is split into crates so that layers stay separate, can be compiled and tested on
their own, and can be replaced. The roadmap lists the layers it expects (`core`, `platform`,
`gfx`, `render`, ...), but a crate created before it has code designs a boundary without knowing
what crosses it. Stage 1 needs two things: something that talks to the windowing system, and
something that runs the application's main loop, with or without a window.

A crate cannot simply be called `core`: Rust's own `core` crate is in every crate's extern
prelude, and a package named `core` breaks macros that refer to `::core`.

## Decision

- The repository root is a virtual Cargo workspace (no root package). Engine crates live in
  `crates/<package-name>/`. Runnable examples live in an `examples/` package. Other top-level
  directories mentioned in the roadmap (`editor/`, `sdk/`, `tools/`, `tests/`) are created when
  their first content exists.
- Package names are `slime-<role>`; the matching Rust library name is `slime_<role>`. The
  directory of a crate is named after its package.
- All crates share one version, set in `[workspace.package]`, and are released together by git
  tags. No crate is published on crates.io (`publish = false`) until the owner decides otherwise.
- Shared metadata (edition, `rust-version`, license, version) and lints are declared once in the
  workspace and inherited. Third-party dependencies are declared once in
  `[workspace.dependencies]` and inherited with `<name>.workspace = true`, so the whole workspace
  uses one version of each.
- `Cargo.lock` is committed.
- Stage 1 has two engine crates:
  - `slime-platform`: windows and input. It is the only crate that uses the windowing library,
    and that library's types never appear in its public API.
  - `slime-app`: application lifecycle, main loop, frame timing, diagnostics setup, headless
    mode. It depends on `slime-platform` for windows, never the reverse.
- There is no `slime-core` crate yet. It is created when two crates need to share code that
  belongs to neither, which the roadmap expects in Stage 2.
- A new crate is created only when code needs it, with its `README.md` in the same change.

## Alternatives considered

- **Follow the roadmap's layer list literally** (`slime-core` + `slime-platform` + `slime-app`).
  In Stage 1, `slime-core` would hold only frame timing, which only `slime-app` uses.
- **One `slime` crate with modules**, split later. Simplest to start, but responsibilities mix
  quickly and the split costs more the longer it waits.
- **Finer crates** (log, time, window, input, ...). More boundaries to maintain than there is
  code to separate.
- **Unprefixed package names** (`platform`, `app`). They collide with Rust's `core` and with
  existing crates.io names, and are ambiguous in other projects' dependency lists.

## Consequences

- Moving code between crates later is cheap while internal APIs are unstable: a module moves and
  its `use` paths change.
- The `slime-` prefix becomes expensive to change once the project is public, because other
  projects and documentation refer to it.
- Games depend on both `slime-app` and `slime-platform`; there is no facade crate re-exporting
  them until there is a second use for one.
