# 0176. Fuzzing and crash reports

- Status: Accepted; amends [0096](0096-the-files-the-engine-reads.md) (the readers are fuzzed,
  besides their damaged-input tests) and [0102](0102-a-damaged-file-gives-an-error.md) (fuzzing,
  which it rejected, enters, outside the workspace)
- Date: 2026-11-05

## Context

[ADR-0096](0096-the-files-the-engine-reads.md) asks that no file the engine reads from outside,
cooked data and its manifest, saves, scene files, asset sources and their metadata, make the
program panic, hang or exhaust its memory. [ADR-0102](0102-a-damaged-file-gives-an-error.md)
tests each reader with damaged copies of valid files: cut at every byte, each byte changed three
ways, and 2 000 copies mutated at random from a fixed seed. It rejected a crate of fuzzing, as
ADR-0096 asked for no dependency.

Those tests explore only what lies close to a valid file. A fuzzer guided by coverage, which keeps
the inputs that reach new code and mutates them again, finds the paths that random bytes reach
rarely: a length that overflows, a depth that recurses, a count that allocates before it checks.

When the program does crash, the user has nothing to attach to an issue: a panic prints to a
terminal that a program started from a desktop does not show.

The owner decided both on 2026-11-05, with the quality bar
([ADR-0175](0175-the-quality-bar.md)).

## Decision

**Fuzzing with `cargo-fuzz`, outside the workspace.** A `fuzz/` directory at the repository's root
holds a package of its own, outside the workspace, with one target for each reader of untrusted
files, on `libfuzzer-sys`. The workspace, its build and its checks do not change: the fuzz package
needs a nightly toolchain, which only it uses.

- **The toolchain.** A nightly, pinned to a date, from the `rust-overlay` that the flake already
  uses, in a development shell of its own, `nix develop .#fuzz`, beside `cargo-fuzz`. The default
  shell keeps the stable toolchain.
- **When it runs.** CI's weekly run fuzzes each target for a few minutes, within the 20 minutes of a
  job. Contributors run longer sessions by hand.
- **What it keeps.** An input that crashes is kept in the repository, minimized, and replayed by
  `cargo test` on the stable toolchain, so that the bug, once fixed, cannot come back unseen. The
  corpora that the fuzzer grows are not tracked: CI keeps them in its cache.
- The damaged-input tests of ADR-0102 stay: they run at each push, on stable, in seconds.

**A crash leaves a report on the user's machine.** A panic hook of the engine's, in the programs it
runs, the editor's and games', writes a text file in the user's state directory: the panic's
message and its backtrace, the program's version, the system and the machine, the GPU and its
driver, and the last lines of the log. The user's home directory is written `~` in it. At the next
start, the program says where the report is, so that the user can attach it to an issue. Nothing
leaves the machine; no dependency is added: the standard library's backtrace and the engine's own
logging write it.

**When it is built.** Both at M6c.Q4. `cargo-fuzz`, `libfuzzer-sys` and the nightly enter
[`dependencies.md`](../dependencies.md) and [`CONTRIBUTING.md`](../../CONTRIBUTING.md) then, their
licenses checked.

## Alternatives considered

- **The damaged-input tests alone**, as ADR-0102 decided: no nightly and no dependency, but no
  guidance by coverage.
- **A fuzzer on stable**, as `afl.rs` or a crate of property testing: `afl.rs` builds AFL++ and
  needs its own runtime; property tests explore what their generators describe, not the bytes of a
  file. `cargo-fuzz` is the tool that Rust's projects share, and its targets also run under
  `cargo test` once their inputs are kept.
- **Fuzzing at each push**: minutes that the jobs do not have; a week's delay finds the same bugs
  before a release.
- **A crash report sent to a server**, as many programs do: faster to learn of crashes, but data
  leaving the machine, a service to run, and the user's consent to ask; never without it.
- **A crate that reports crashes**, as `human-panic`: a dependency for a hook of some lines, which
  would not know the GPU nor the engine's log.

## Consequences

- The repository gains a package outside the workspace, its own toolchain and its own shell.
- Bugs that fuzzing finds are fixed under the debugging rules: each with a test that failed first.
- A program that crashes points to its report at the next start, which CI's tests check.
