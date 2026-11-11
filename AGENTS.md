# AGENTS.md

Rules for AI agents; read it fully at each session's start.

## Project

A general-purpose, modular game engine in Rust: runtime, asset pipeline, editor, SDK. Its source is public; its licence is its own, free under a threshold, a royalty above (ADR-0214); uwui stays MIT OR Apache-2.0.
AI agents do most of the work; the owner validates direction and architecture.
Others use it and contribute: documentation is part of every deliverable, held to the code's standard.

## Read before working

You remember no past session: the repository must hold what the next session needs.

- Each session: `docs/PLAN.md`'s "Current" and the current milestone.
- Before a crate: its `README.md` and `docs/ARCHITECTURE.md`.
- Only to plan, or when a task touches a decision: `docs/ROADMAP.md`, `docs/stage-6d.md`, `docs/adr/`.

## Scope

- Work on `docs/PLAN.md`'s current milestone, or on what the owner asks.
- Do not start later stages, nor abstractions for them.
- If a task needs something out of scope, stop and say so.
- The roadmap sets direction and order, not what is built: build what the milestone needs; stages may change or be dropped.
- When experience contradicts the roadmap, propose a change; once accepted, record it in an ADR.

## Environment and commands

- Builds and runs on NixOS under Wayland, and on Windows (x86_64, MSVC) (ADR-0015).
- CI checks Windows on each push and pull request; windowed behaviour, by hand in a VM.
- `scripts/check.sh` also compiles for Windows: most breakage shows on Linux.

```sh
nix develop
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
scripts/check.sh [lint|headless|windowed|game|docs]
cargo run -p slime-cook -- cook examples/assets target/cooked/examples
cargo run --example <name>
cargo run -p hunnawood -- --new --save <file>
cargo run --example editor -- --level <copy> --journal <file>
cargo run --quiet -p slime-mcp
```

- Every other command: `CONTRIBUTING.md`, "Commands", where new ones go; here only those every session needs.
- Tests run on lavapipe, a CPU Vulkan driver: `CONTRIBUTING.md`, "GPU".

Keep off the owner's files and desktop:

- Run the game with `--new --save <your file>`: without `--save`, it resumes and writes the user's save.
- Run the MCP server in headless Weston: windows stay off the owner's desktop.
- Run the editor on a level's copy, with your own journal: the user's default one keeps unwritten steps, recovered at start.
- Drive the editor through an MCP server with its own `XDG_STATE_HOME`: the editor writes keys, layout, speeds and recent levels there, not in the owner's.

## Definition of done

A milestone is done only when:

- Build, tests, clippy, fmt, doc pass (`scripts/check.sh`), before the report.
- The binary or example ran as expected.
- New behaviour has tests where practical.
- The quality bar's budgets (`docs/ROADMAP.md`) that it touches are measured; one missed is debt.
- `docs/PLAN.md` shows the new status.
- Every document the change affects is updated, then reread as a newcomer would.
- Each architecture decision is recorded in an ADR.

Never report as working what was not verified: say what was not, and why.

## Architecture rules

- Crate dependencies flow one way, per `docs/ARCHITECTURE.md`. No cycles, no shortcuts across layers.
- wgpu types stay in `gfx`, `render` and engine tests (via `slime-gfx`'s `wgpu` feature, ADR-0031); everything else uses engine-owned handles.
- The editor is a client of the engine: public API only.
- The engine runs headless (no window, renderer or audio); core systems never depend on them.
- Internal Rust APIs may change freely. Nothing is stable before 1.0.

## Code rules

- No abstraction without a second concrete use; no trait, generic or layer "just in case".
- Do not reimplement the standard library unless a measurement justifies it.
- Every `unsafe` block has a `// SAFETY:` comment saying why it is sound.
- Follow the ADRs' conventions (errors, logging, naming); if none applies: propose an ADR, never decide silently.
- Comments explain why, not what. No commented-out code.

## Consistency rules

- Before writing a helper, type or pattern, search the workspace for one to reuse or extend.
- One way to do a thing. Unify two you find, or report them.
- Prefer less code; delete dead code, unused parameters and features.
- Keep changes local to the milestone; unrelated cleanup goes in its own commit, or a note in `docs/debt.md`.
- Report drift from `docs/ARCHITECTURE.md` or an ADR. Never silently work around it.

## Documentation

Write for a competent Rust developer new to the project. From the repository alone they must grasp each part and why, then build, test and change it.

- Describe the code as it is now. Wrong documentation is worse than none.
- No reference to past sessions, conversations or context the reader cannot see.
- Never name a game as an inspiration here; describe it in its own terms. Its story, characters and world stay out of the repository.
  Games and engines are named only as benches of measure, in the quality bar and `docs/measurements.md`, with version, settings and date ([ADR-0215](docs/adr/0215-the-engine-measured-against-the-market.md)).
- Define each term the project coins where it first appears.
- State limits and known debt (`docs/debt.md`) plainly. Never present a partial feature as complete.
- What agents read each session: short sentences, one fact per line.

### One home per fact

A fact written twice drifts: one home each (ADR-0053); elsewhere state it without figures and link there.
Each file: what it holds; when to update.

- rustdoc: each public item's contract (behaviour, invariants, errors, panics, limits, figures),
  examples as doc tests; API changes.
- `examples/`: a runnable example per notable feature, kept working.
- `crates/*/README.md`: role, dependencies, what it must not do, entry points to rustdoc;
  created or role changed.
- `docs/ARCHITECTURE.md`: crates and roles, dependency graph, cross-crate flows without figures, where to start;
  structure or flows change.
- `docs/adr/NNNN-title.md`: context, decision, rejected alternatives, consequences; each decision;
  afterwards only its status changes.
- `docs/adr/README.md`: the ADRs' index and statuses; either changes.
- `docs/PLAN.md`: the current stage's status, the milestone in progress, the checks by hand; every milestone.
- `docs/stage-6d.md`: Stage 6d's whole plan while in progress; summed up in `docs/history.md` at its end.
- `docs/stage-6c.md`: Stage 6c's plan while paused; back into `docs/PLAN.md` when it resumes.
- `docs/uwui-needs.md`: what the engine asks of uwui, general needs beyond its own; a need found or met.
- `docs/compositors.md`: the series' compositors, what each gives; it changes.
- `docs/debt.md`: open known debt: each item what is missing or wrong, why, where followed; introduced or settled.
- `docs/history.md`: finished stages in brief, versions and ADRs, newest first; a stage finished.
- `docs/ROADMAP.md`: direction, stage order, exit criteria, quality bar budgets;
  a stage reshaped, merged or dropped; a budget changed (ADR).
- `CHANGELOG.md`: user-visible changes, once, per release; such a change, a release tagged.
- `docs/measurements.md`: build, benchmark, CI figures per milestone or machine;
  a speed-up claimed, benchmarks changed, a milestone done.
- `docs/glossary.md`: the project's terms; one coined or changing meaning.
- `docs/dependencies.md`: each dependency and tool: users, reason, alternative, license;
  added, removed or repurposed.
- `README.md`: the project, its real state, building and running it, what to read next; these change.
- `CONTRIBUTING.md`: environment, commands, rules, making and reviewing a change; process or tools change.
- `examples/README.md`, `benches/README.md`: each example (options, tests), each benchmark (what it measures,
  how to read it); they change.
- `games/*/README.md`: the game, playing and making it, its assets; it changes.

## Commits

- One logical change per commit, small enough to review, that passes the checks: its subject in the imperative, its body saying why unless obvious.
- Tags and releases wait for the owner's go.

## When to stop and ask

Stop and ask the owner when:

- A decision affects architecture and no ADR covers it.
- You want to add a dependency.
- The request contradicts `docs/ROADMAP.md` or an ADR.
- The plan is wrong, or a milestone larger than expected.
- You would delete or rewrite much existing code.

Propose options with trade-offs and a recommendation. Do not work around a problem to keep going.

## Reporting

At each milestone's end, report:

- What was done.
- Notable design choices, why, alternatives rejected.
- The budgets measured, and their figures.
- What to read first.
- Shortcuts taken and debt introduced (`docs/debt.md`).

## Working with several agents

- One session per milestone; after its report, the next starts from the repository.
- Hand research and reviews to subagents; keep their conclusions.
- Each agent takes one milestone or crate at a time, as `docs/PLAN.md` assigns.
- Edit only files in your scope. Report changes needed elsewhere.
- One agent at a time edits shared files: `docs/PLAN.md`, `docs/ARCHITECTURE.md`, workspace `Cargo.toml`.
- A reviewing agent on the strongest model checks each milestone against this file, the ADRs and the quality bar before reporting.

## Language

- Everything in the repository is in English: code, comments, commits, documentation.
- Talk to the owner in French.
