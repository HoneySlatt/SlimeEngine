# AGENTS.md

Instructions for AI agents working in this repository. Read this file fully at the start of every session.

## Project

A general-purpose, modular, open-source (MPL-2.0) game engine written in Rust: runtime, asset pipeline, editor and SDK. It is developed primarily by AI agents; the human owner validates direction and architecture decisions.

The engine is meant to be used and contributed to by other people. Anyone arriving in the repository, at any point in its life, must be able to understand all of it from the repository alone. Documentation is part of every deliverable, held to the same standard as the code (see "Documentation").

## Read before working

1. `docs/PLAN.md` — current stage, milestones and status. This is the source of truth for what to do now.
2. `docs/ROADMAP.md` — direction, stage order, exit criteria. An overview, not a commitment (see "Scope").
3. `docs/ARCHITECTURE.md` — crates, their roles and their dependencies.
4. `docs/adr/` — decisions already made. Do not re-decide them.
5. The `README.md` of each crate you are about to touch.

You have no memory of previous sessions. These files are the project's memory: if something is not written there, the next session will not know it.

## Scope

- Work only on the stage and milestone marked as current in `docs/PLAN.md`.
- Do not start later stages, and do not add abstractions that prepare for them.
- If a task needs something out of scope, stop and say so.

The roadmap is an overview. It sets direction and dependency order; it does not say what will be built, nor how. Not all of it will happen: stages will be merged, cut down, reordered or dropped as the project teaches us. Build something because the current milestone needs it, never because the roadmap mentions it. When experience contradicts the roadmap, propose a change to it; once accepted, record the change in an ADR.

## Environment and commands

Development happens on NixOS, under Wayland. Everything must build and run there, and on Windows (x86_64, MSVC toolchain): CI checks Windows on every push to `main` and every pull request, and windowed behaviour is checked by hand in a Windows virtual machine (ADR-0015). `scripts/check.sh` also compiles the code for Windows, so most Windows breakage shows up on Linux.

```sh
nix develop                                               # enter the dev shell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
scripts/check.sh                                          # everything above, plus the checks below
cargo run -p slime-cook -- cook examples/assets target/cooked/examples  # cook what the scene and world examples draw
cargo run --release -p slime-cook -- watch <sources> <output>  # cook again each change (ADR-0076)
cargo run --example <name>                                # run an example (list in examples/README.md)
cargo run --release -p slime-benches -- [<name>...]       # run the benchmarks (benches/README.md)
scripts/measure.sh "<title>" >> docs/measurements.md      # time the build, run the benchmarks (ADR-0104)
scripts/fetch-assets.sh                                   # fetch the game's source assets (games/hunnawood/README.md)
blender --background --factory-startup --python games/hunnawood/zone/build_zone.py  # rebuild the game's zone (same README)
cargo run -p hunnawood                                    # play the game (games/hunnawood/README.md)
cargo run -p hunnawood -- --watch                         # load its zone and the shaders again as they change
cargo run -p hunnawood -- --new --save <file>             # play a new run, saved to <file>, not to the user's save
```

The game resumes and writes the user's save, `hunnawood/save.toml` in their data directory, when it runs
without `--save`: an agent runs it with `--new --save <a file of its own>`.

`scripts/check.sh` is what CI runs. Besides the commands above, it runs the renderer's and the world's tests once more with glam's assertions on (`--features glam/debug-glam-assert`), checks dependency licenses, security advisories and sources with `cargo deny --locked check` (needs network access), checks the list of the sample game's source assets with `scripts/fetch-assets.sh`, checks that every source asset has a metadata file of its own with `slime-cook check`, cooks the sample twice from empty directories and compares the bytes, fetches and cooks the sample game's zone and runs the tests that read it (`cargo test -p hunnawood -- --ignored`), runs the samples without a display server, the windowed samples, the `scene` example on a copy of the cooked sample with one byte changed (it must end with an error, not a panic), the game (its new runs, and one resumed from a copy of its save fixture, its saves kept in a private directory) and the windowed lifecycle test inside a headless Weston compositor (and that test's resize scenario in a second one, at scale 2), and checks links between Markdown files with `lychee`. Run it before reporting a milestone done. It runs the tests on lavapipe, a Vulkan driver that renders on the CPU; to run one test the same way, prefix it with `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"`.

Keep this section up to date when commands, examples or tools are added.

## Definition of done

A milestone is done only when all of the following hold:

- Build, tests, clippy, fmt and doc pass with the commands above.
- The binary or example was actually run and behaved as expected.
- New behaviour is covered by tests where a test is practical.
- `docs/PLAN.md` reflects the new status.
- Every document affected by the change is updated (see "Documentation"), and was reread as a newcomer would read it.
- Any architecture decision taken is recorded as an ADR.

Never report that something works without having verified it. If you could not verify, say exactly what was not verified and why.

## Architecture rules

- Dependencies between crates flow in one direction, as described in `docs/ARCHITECTURE.md`. No cycles, no shortcuts across layers.
- wgpu types stay inside the `gfx` and `render` crates, and the dev overlay's `slime-devui`, tooling, which reaches them through `slime-gfx`'s `wgpu` feature (ADR-0031). Everything else uses engine-owned handles.
- The editor is a client of the engine. It uses public API only.
- The engine must run headless (no window, renderer or audio). Do not make core systems depend on them.
- Internal Rust APIs may change freely. Nothing is promised stable before 1.0.

## Code rules

- No abstraction without a second concrete use. No trait, generic or layer added "just in case".
- Do not reimplement what the standard library provides without a measurement that justifies it.
- Every new dependency needs a justification: why, which alternative was considered, license compatibility with MPL-2.0. Ask before adding one.
- Every `unsafe` block carries a `// SAFETY:` comment explaining why it is sound.
- Follow the conventions set by ADRs (errors, logging, naming). If no ADR covers a case, propose one instead of deciding silently.
- Comments explain why, not what. No commented-out code.

## Consistency rules

These keep the codebase understandable as it grows.

- Before writing a helper, type or pattern, search the workspace for an existing one. Reuse or extend it.
- One way to do a given thing. If you find two, unify them or report it.
- Prefer less code. Delete dead code, unused parameters and unused features instead of keeping them.
- Keep changes local to the milestone. Unrelated cleanup goes in a separate commit, or in a note in `docs/PLAN.md`.
- If you notice the code has drifted from `docs/ARCHITECTURE.md` or an ADR, report it. Do not silently work around it.

## Documentation

### Standard

The reader is a competent Rust developer who has never seen this project, arriving at any point in its life. From the repository alone, they must be able to understand what every part does and why it is built that way, then build, test and change it.

- Documentation describes the code as it is now, not as it is intended to be. Wrong documentation is worse than missing documentation.
- Write for that reader: no reference to past sessions, conversations or context they cannot see; no undefined project jargon; every term the project coins is defined where it first appears.
- State limits and known debt plainly. Do not present a partial feature as complete.
- Every public item has rustdoc that says what it does, its invariants, its errors and panics. Examples in rustdoc compile and run as doc tests.
- Each notable feature has a runnable example in `examples/`, kept working.
- A change that leaves a document wrong is not finished.

### One home per fact

A fact written in two places drifts: one copy is updated, the other is not. Each kind of fact has one home, and other documents link to it instead of copying it (ADR-0053):

- **rustdoc**: the API's contracts, what an item does, its invariants, errors, panics and limits, with their figures (sizes, costs, defaults).
- **A crate's README**: its role, what it depends on, what it must not do, and one or two sentences per entry point, linking to the rustdoc.
- **`docs/ARCHITECTURE.md`**: the flows across crates, without the figures the rustdoc holds.
- **ADRs**: why a decision was taken and what was rejected. Only their status changes afterwards.
- **`docs/PLAN.md`**: status and known debt, linking to where the facts live.
- **`CHANGELOG.md`**: what changed for users, once, when it changes.
- **`docs/measurements.md`**: what the build and the benchmarks measured, one section per milestone
  or machine.

A document that needs a fact held elsewhere states it in a sentence, without its figures, and links to its home.

### Files

| File | Content | Update when |
|---|---|---|
| `README.md` | What the project is, its real current state, how to build and run it, where to read next | State or entry points change |
| `CONTRIBUTING.md` | Environment, commands, rules, how a change is made and reviewed | Process or tools change |
| `docs/ROADMAP.md` | Direction, stage order, exit criteria | A stage is reshaped, merged or dropped (with an ADR) |
| `docs/PLAN.md` | Current stage, milestones, status, known debt | Every milestone |
| `docs/ARCHITECTURE.md` | Crate map, responsibilities, dependency graph, main data flows, where to start reading | Structure or a flow changes |
| `docs/adr/NNNN-title.md` | Context, decision, alternatives rejected, consequences | Each architecture decision |
| `docs/adr/README.md` | The index of the ADRs and their statuses | An ADR is added, or its status changes |
| `docs/glossary.md` | Terms the project defines | A term is coined or changes meaning |
| `docs/dependencies.md` | Every third-party dependency and tool: who uses it, why, the alternative considered, its license | A dependency is added, removed or repurposed |
| `docs/measurements.md` | The build's and the benchmarks' figures, one section per milestone or machine | A milestone measures what it claims to make faster, or changes what the benchmarks measure |
| `CHANGELOG.md` | Notable changes, per release | A change users would notice; a release is tagged |
| `crates/*/README.md` | Role, what it depends on, entry points, what it must not do | Crate created or its role changes |
| `examples/README.md` | Each example, its options, and the examples' tests | An example or one of their tests changes |
| `benches/README.md` | Each benchmark, what it measures, and how its figures are read | A benchmark changes |
| `games/*/README.md` | What the game is, how to play it, how it is made, its assets | The game changes |
| rustdoc | Public API | Public API changes |

## Commits

- One logical change per commit, small enough to review.
- Subject in the imperative, in English. The body explains why when it is not obvious.
- Do not commit code that fails the checks.

## When to stop and ask

Stop and ask the owner before proceeding when:

- A decision affects architecture and no ADR covers it.
- You want to add a dependency.
- The request contradicts `docs/ROADMAP.md` or an ADR.
- The plan turns out to be wrong or a milestone is larger than expected.
- You are about to delete or rewrite a significant part of existing code.

Propose options with trade-offs and a recommendation. Do not work around a problem to keep going.

## Reporting

At the end of each milestone, report:

- What was done.
- Notable design choices, why, and which alternatives were rejected.
- What deserves to be read first.
- Shortcuts taken and debt introduced.

## Working with several agents

- Each agent works on one milestone or one crate at a time, as assigned in `docs/PLAN.md`.
- Do not edit files outside your assigned scope. If you need a change elsewhere, report it.
- Shared files (`docs/PLAN.md`, `docs/ARCHITECTURE.md`, workspace `Cargo.toml`) are edited by one agent at a time.
- A reviewing agent checks a milestone against this file and the ADRs before it is reported as done.

## Language

- Everything in the repository is in English: code, comments, commits, documentation.
- Talk to the owner in French.
