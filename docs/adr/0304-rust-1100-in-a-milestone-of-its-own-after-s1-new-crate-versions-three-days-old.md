# 0304. Rust 1.100 in a milestone of its own after S1; new crate versions three days old

- Status: Accepted
- Date: 2026-11-18

## Context

The toolchain is pinned at 1.99.0 and bumped on purpose, in its own commit
([ADR-0003](0003-toolchain-edition-and-msrv.md)). Rust 1.100.0 came out on 2026-11-12. A read-only
analysis of its notes, made without a build, found for this repository:

- No gain shown for the work in progress: the allocator API, `DropGuard`, `Box::map` and
  `-Cprofile-sample-use` are leads, none measured here.
- One concrete risk: Cargo 1.100 changes the layout of its build directory, and tests that find
  their binaries from `std::env::current_exe()` assume `target/<profile>/deps`
  (`crates/uwui-desktop/tests/rest.rs`, among about ten such tests). No failure was reproduced.
- Cargo's new lint of unused dependencies, which skips dev-dependencies and depends on the targets
  built.
- Cargo's `registry.global-min-publish-age`, which does not choose a version published more
  recently than its age, versions already in `Cargo.lock` kept.
- Six `strip_prefix(…).unwrap_or(original)` that `trim_prefix` writes plainly (`uwui-kdl`, `uwui`,
  `slime-reflect-derive`).

## Decision

The owner's decisions of 2026-11-18.

**R1, on track A after S1c and before M6d.9a2c,** a milestone of its own, arranged with track B
between two of its parts since it touches track B's tests, bumps the toolchain to 1.100 and
`rust-version` with it. It holds:

- the new warnings fixed, and every test or script that assumes where Cargo puts its binaries
  checked, those that break finding them through Cargo rather than a guessed path;
- the six `trim_prefix`, behaviour unchanged;
- Cargo's lint of unused dependencies run once on Linux and Windows, what it finds removed, and made
  an error in `scripts/check.sh` if its results hold across features and targets; otherwise its
  reason written in [`debt.md`](../debt.md);
- `registry.global-min-publish-age` at three days, in the repository's `.cargo/config.toml`, with
  how to make an exception for an urgent fix written in `CONTRIBUTING.md`.

`DropGuard` and the rest of 1.100's API are taken up by a milestone that already works in their
code, where they make it simpler. The allocator API and profile-guided optimisation wait for a
measure (`PLAN.md`, *Ideas waiting for their milestone*).

## Alternatives considered

- **The bump right after S1a, or inside it:** current sooner, but S1 split by a migration, while
  track B works in `uwui-desktop`.
- **The bump before uwui's move, or only when a need comes:** one pass, perhaps straight to a later
  version, but a gap that grows, and work at the stage's busiest point.
- **`DropGuard` in the canvas's slot of timestamps (`uwui-render`):** a type of its own fewer, but a
  delicate GPU contract touched for maintenance alone.
- **Seven days before a new version, or none:** more margin, or ordinary fixes sooner; the few
  dependencies and the reviewed lock file stay in every case.

## Consequences

- The workspace stays on 1.99 for about three more sessions.
- A version of a crate published less than three days ago is not chosen by `cargo update` without
  an exception.
- The dependency graph may gain a machine's check beside review.

## Validation and reconsideration

Read: Rust 1.100.0's notes, Cargo's documentation of its build directory, its lints and its
configuration. Assumed, not checked: that rust-overlay gives 1.100.0, and which tests the new layout
breaks. Proposing another decision is justified if R1 finds the bump costlier than a milestone, or
the delay holds back a fix that matters.
