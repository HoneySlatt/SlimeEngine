# 0221. Compiling within its budgets

- Status: Accepted; amends [0104](0104-the-build-ci-and-measurements.md) (dependencies optimised in
  debug builds) and [0220](0220-the-roadmap-reshaped-by-the-market.md) (one set of features where a
  program and the game that its Play builds meet, not for the whole workspace)
- Date: 2026-11-08

## Context

[ADR-0215](0215-the-engine-measured-against-the-market.md) budgets compiling on the Ryzen 7 5800X:
a change of a game's code to the game running again, 2 s in a debug build; the editor built again
after a change of an engine crate, 10 s; the whole engine from nothing, 3 min; a game's build for
shipping, 10 min. [ADR-0220](0220-the-roadmap-reshaped-by-the-market.md) asks, now: one set of
features for the whole workspace, dependencies optimised in debug builds, a faster linker chosen by
measure, and a profile for shipping with optimisation across crates.

`scripts/compile-times.sh` timed each step before any change
([`measurements.md`](../measurements.md#c1-before-ryzen-7-5800x)). A game's change built again in
1.42 s, and Play gave control in 0.97 s after cargo's own look: about 2.4 s, over the budget. The
other budgets were met, and there was no profile for shipping.

The features, read unit by unit from cargo's messages:

- *Hunna Wood*'s editor, `-p hunnawood-editor`, and the game that its Play builds,
  `-p hunnawood --bin hunnawood`, already share every engine crate.
- The examples' editor, `cargo run --example editor` from the root, resolves the features of every
  member and of their dev-dependencies; its Play built `world` with `-p slime-examples`, the
  examples' alone. The two shared no engine crate: each engine change was compiled twice.
- The examples' editor shares every engine crate with the workspace's tests and clippy, which also
  resolve every member's features and their dev-dependencies.
- `cargo build --workspace`, without tests, and the game and its editor, each a set of its own,
  share no engine crate with the others: the first build of one after another compiles the engine
  again, once.
- cargo's `resolver.feature-unification = "workspace"`, which would unify features across members
  whatever is built, is unstable still: cargo 1.99 ignores it without `-Z feature-unification`.

## Decision

The owner's decisions of 2026-11-08.

**Dependencies are optimised at the first level in debug builds**
(`[profile.dev.package."*"] opt-level = 1`). On *Hunna Wood*'s zone, Play's median time to control,
five runs each: 1.11 s unoptimised, 0.90 s at level 1, 0.94 s at 2, 0.95 s at 3; the workspace from
nothing, 33 s, 64 s, 75 s and 80 s. Level 1 gives what the others give for less time to build. The
engine's own crates stay unoptimised, to be stepped through in a debugger.

**mold links what is built for Linux, in the Nix development shell.** The shell gives mold and sets
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS` to `-C link-arg=-fuse-ld=mold`. A game's change
built again, median of 7: 1.44 s with rustc's own lld, 0.97 s with mold 2.42.1, 0.99 s with wild
0.10.0. Linux's CI runs in the shell and uses it too. A contributor without Nix builds with rustc's
lld, as before. Windows keeps MSVC's linker.

**One set of features where a program and its Play's game meet.** The examples' editor's Play
builds `world` with `--workspace`, as `cargo run --example editor` builds the editor from the root:
the two share every engine crate. A game and its editor, packages of their own, share them already.
`cargo build --workspace`, the game with its editor, and the examples with the tests keep a set
each: written in [`debt.md`](../debt.md), until cargo's unification by workspace is stable, when it
becomes one line of cargo's configuration.

**A profile for shipping**, `shipping`: release's, with optimisation across crates (`lto = "fat"`)
and one unit of code generation per crate (`codegen-units = 1`). A game ships with
`cargo build --profile shipping -p <game> --no-default-features`. CI's published programs stay
release builds.

## Alternatives considered

- **Dependencies at level 3**, as many engines advise: no faster Play here, 16 s more from nothing.
- **Dependencies unoptimised**: 0.2 s more on each Play.
- **wild**: as fast as mold here, but at 0.10, before its 1.0, and less proven.
- **rustc's lld, kept**: 0.47 s more on each change, and the budget of 2 s missed.
- **The linker in `.cargo/config.toml`**: every Linux build would need mold, contributors without
  Nix included.
- **A workspace-hack crate, made by cargo-hakari**, which every member depends on: third-party
  features unified everywhere, but every program on uwui would compile wgpu and the rest, which
  Stage 6d's exit forbids; one more line in every crate's manifest and a tool to run; the engine's
  own features, `slime-gfx`'s `wgpu` in tests and accessibility in the examples, would still differ.
- **cargo's unification by workspace, on a nightly toolchain**: ADR-0003 pins a stable one, and the
  option may change.
- **Thin optimisation across crates for shipping**: faster to build, slower to run; the fat one
  keeps within its budget.

## Consequences

- Building the workspace from nothing takes about 1.75 times as long, within its budget.
- A build outside the Nix shell, without mold, gives rustc other flags: cargo builds the workspace
  once more, beside the shell's build, and keeps both. Without mold, a game's change misses its
  budget.
- A `RUSTFLAGS` of one's own replaces the shell's flags for Linux, mold's among them.
- `scripts/compile-times.sh` times the steps at each milestone
  ([`measurements.md`](../measurements.md)).
- The shipping profile is built and run by hand on Linux, and waits for a check by hand on
  Windows: CI builds no program with it.
- A game outside the workspace writes these profiles in its own manifest, as the
  [`README`](../../README.md#using-slimeengine-in-a-game) shows; there, the engine's crates are not
  members, and are optimised with the dependencies.
