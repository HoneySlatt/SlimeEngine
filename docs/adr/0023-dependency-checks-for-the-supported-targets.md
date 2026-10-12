# 0023. Check the dependencies of the supported targets only

- Status: Accepted
- Date: 2026-10-12

## Context

[ADR-0006](0006-dependency-policy.md) has cargo-deny check the licenses, advisories and sources of
every dependency, and warn when several versions of one crate are in use. Without a list of targets,
cargo-deny evaluates `Cargo.lock` for every platform. Measured on 0.0.2: the lock holds 265
packages, of which 154 are compiled for Linux and Windows together (137 for Linux, 105 for
Windows); the rest serves Android, macOS, iOS and the web through winit and wgpu, and the engine
never builds it. The check printed 15 duplicate-version warnings, 8 of them about crates never
built here, over about a thousand lines in which a new duplicate goes unnoticed; and a RustSec
advisory against one of those crates (tokio, wasm-bindgen, jni and objc2 are in the lock) would
have failed the checks for code the engine never ships.

## Decision

`deny.toml` lists the supported targets of [ADR-0015](0015-platforms-and-ci.md) under
`[graph] targets`, so that cargo-deny checks the crates compiled for `x86_64-unknown-linux-gnu`
and `x86_64-pc-windows-msvc` only. The list grows with each platform the engine adds. Everything
else in ADR-0006 stands.

This amends ADR-0006: its checks cover the dependencies of the supported targets, not of every
platform in `Cargo.lock`.

## Alternatives considered

- **Keep checking every platform.** Advisories against crates the engine never compiles fail the
  checks for nothing, and the noise hides new duplicates.
- **Skip the remaining duplicate pairs one by one** (`[bans] skip`). ADR-0006 keeps duplicates as
  warnings on purpose, and a skip list has to be tuned at every upgrade.

## Consequences

- With the two targets, the same `Cargo.lock` gives 7 duplicate warnings and the same verdicts
  (measured: licenses, advisories and sources all pass).
- An advisory against a crate that only another platform compiles is no longer reported; it is
  reported again as soon as that platform joins the list.
- Adding a platform means adding its target here as well as in `rust-toolchain.toml` and CI.
