# AGENTS.md

Rules for AI agents in this repository. Read it fully at each session's start.

## Project

A general-purpose, modular game engine in Rust: runtime, asset pipeline, editor, SDK. Its source is public; its licence is its own, free under a threshold, a royalty above (ADR-0214); uwui stays MIT OR Apache-2.0.
AI agents do most of the work. The owner validates direction and architecture.
Others will use it and contribute: documentation is part of every deliverable, held to the code's standard.

## Read before working

You remember no past session: what the repository does not hold, the next session will not know.

- Each session: `docs/PLAN.md`'s "Current" section and the current milestone. They say what to do now.
- Before touching a crate: its `README.md`, and `docs/ARCHITECTURE.md`.
- Only to plan, or when the task touches a decision: `docs/ROADMAP.md` and `docs/adr/`.
- Do not re-decide what an ADR decided.

## Scope

- Work on `docs/PLAN.md`'s current milestone, or on what the owner asks.
- Do not start later stages, nor add abstractions for them.
- If a task needs something out of scope, stop and say so.
- The roadmap sets direction and order, not what is built: build what the milestone needs. Stages may change or be dropped.
- When experience contradicts the roadmap, propose a change. Once accepted, record it in an ADR.

## Environment and commands

- Everything builds and runs on NixOS under Wayland, and on Windows (x86_64, MSVC) (ADR-0015).
- CI checks Windows on each push and pull request; windowed behaviour, by hand in a VM.
- `scripts/check.sh` compiles for Windows too: most Windows breakage shows on Linux.

```sh
nix develop
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
scripts/check.sh [headless|windowed|game|docs]         # all checks, or one CI job
cargo run -p slime-cook -- cook examples/assets target/cooked/examples  # the examples' data
cargo run --example <name>                             # examples/README.md
cargo run --release -p slime-benches -- [<name>...]    # benches/README.md
cargo run -p slime-benches -- --write-counters [<name>...]  # ADR-0186
scripts/measure.sh "<title>" >> docs/measurements.md   # ADR-0104
scripts/compile-times.sh                               # compiling's budgets, ADR-0221
scripts/start-times.sh [--gpu] [--cold] [--runs <n>]   # starts, ADR-0189
scripts/input-to-present.sh [--gpu]                    # input to present, ADR-0189
scripts/resources.sh [--gpu] [--heaptrack]             # memory and power, ADR-0193
scripts/canvas-cost.sh [--gpu]                         # renderer against canvas, ADR-0180
scripts/uwui-program.sh [--features <list>]            # a program on uwui built, ADR-0180
cargo run -p hunnawood -- --new --save <file>          # games/hunnawood/README.md
cargo run --example editor -- --level <copy> --journal <file>  # examples/README.md
cargo run --quiet -p slime-mcp                         # crates/slime-mcp/README.md
```

- All commands: `CONTRIBUTING.md`, "Commands". Update both lists when adding commands, examples or tools.
- Run `scripts/check.sh` before reporting a milestone done. What it checks: `CONTRIBUTING.md`, "Checks".
- Tests run on lavapipe, a CPU Vulkan driver. For one test: `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"`,
  with `VK_LOADER_DRIVERS_SELECT` and `VK_LOADER_DRIVERS_DISABLE` unset.

Keep off the owner's files and desktop:

- Run the game with `--new --save <your file>`. Without `--save`, it resumes and writes the user's save.
- Run the MCP server in a headless Weston's environment: windows stay off the owner's desktop.
- Run the editor on a copy of a level, with your own journal: the user's default one keeps unwritten steps, recovered at start.
- Drive the editor through an MCP server with its own state directory (`XDG_STATE_HOME`):
  the editor writes the owner's keys, layout, speeds and recent levels in theirs.

## Definition of done

A milestone is done only when:

- Build, tests, clippy, fmt and doc pass with the commands above.
- The binary or example was actually run and behaved as expected.
- New behaviour has tests where a test is practical.
- The budgets of the quality bar (`docs/ROADMAP.md`) that it touches are measured; one missed is debt.
- `docs/PLAN.md` shows the new status.
- Every document the change affects is updated, and reread as a newcomer would.
- Each architecture decision is recorded in an ADR.

Never report that something works without verifying it.
If you could not, say exactly what was not verified and why.

## Architecture rules

- Crate dependencies flow one way, per `docs/ARCHITECTURE.md`. No cycles, no shortcuts across layers.
- wgpu types stay in `gfx`, `render` and the engine's tests (via `slime-gfx`'s `wgpu` feature, ADR-0031).
  Everything else uses engine-owned handles.
- The editor is a client of the engine: public API only.
- The engine runs headless (no window, renderer or audio). Core systems never depend on them.
- Internal Rust APIs may change freely. Nothing is stable before 1.0.

## Code rules

- No abstraction without a second concrete use. No trait, generic or layer "just in case".
- Do not reimplement the standard library unless a measurement justifies it.
- Justify each new dependency: why, the alternative considered, license compatibility.
- Every `unsafe` block has a `// SAFETY:` comment saying why it is sound.
- Follow the ADRs' conventions (errors, logging, naming). If none applies, propose an ADR; never decide silently.
- Comments explain why, not what. No commented-out code.

## Consistency rules

- Before writing a helper, type or pattern, search the workspace for one to reuse or extend.
- One way to do a thing. Unify two you find, or report them.
- Prefer less code. Delete dead code, unused parameters and features.
- Keep changes local to the milestone. Unrelated cleanup goes in its own commit, or a note in `docs/debt.md`.
- Report drift from `docs/ARCHITECTURE.md` or an ADR. Never silently work around it.

## Documentation

Write for a competent Rust developer new to the project, at any point in its life.
From the repository alone, they must grasp each part and why, then build, test and change it.

- Describe the code as it is now. Wrong documentation is worse than none.
- No reference to past sessions, conversations or context the reader cannot see.
- Never name a game as an inspiration of a game here; describe a game in its own terms. Its story, characters and world stay out of the repository.
  Games and engines are named only as benches of measure, in the quality bar and `docs/measurements.md`, with version, settings and date ([ADR-0215](docs/adr/0215-the-engine-measured-against-the-market.md)).
- Define each term the project coins where it first appears.
- State limits and known debt (`docs/debt.md`) plainly. Never present a partial feature as complete.
- In what agents read each session: short sentences, one fact per line.
- A change that leaves a document wrong is not finished.

### One home per fact

A fact written twice drifts: each has one home (ADR-0053).
Elsewhere, state it in a sentence, without figures, and link there.
Each file: what it holds; when to update it.

- rustdoc: each public item's contract (what it does, invariants, errors, panics, limits, figures),
  examples run as doc tests; the API changes.
- `examples/`: a runnable example per notable feature, kept working.
- `crates/*/README.md`: role, dependencies, what it must not do, entry points linking to rustdoc;
  crate created or role changed.
- `docs/ARCHITECTURE.md`: crates and roles, dependency graph, cross-crate flows without figures, where to start;
  structure or flows change.
- `docs/adr/NNNN-title.md`: context, decision, rejected alternatives, consequences; each decision.
  Afterwards only its status changes.
- `docs/adr/README.md`: the ADRs' index and statuses; either changes.
- `docs/PLAN.md`: current stage, milestones, status, linking to facts; every milestone.
- `docs/stage-6c.md`: Stage 6c's plan while it is paused; back into `docs/PLAN.md` when it resumes.
- `docs/uwui-needs.md`: what the engine asks of uwui, general needs apart from its own; a need found or met.
- `docs/debt.md`: open known debt, each item: what is missing or wrong, why, where followed; debt introduced or settled.
- `docs/history.md`: finished stages in brief, versions and ADRs, newest first; a stage finished.
- `docs/ROADMAP.md`: direction, stage order, exit criteria, the quality bar's budgets;
  a stage reshaped, merged or dropped, a budget changed (with an ADR).
- `CHANGELOG.md`: user-visible changes, once, per release; such a change, a release tagged.
- `docs/measurements.md`: build, benchmark and CI figures per milestone or machine;
  a speed-up claimed, benchmarks changed, a milestone done.
- `docs/glossary.md`: the project's terms; one coined or changing meaning.
- `docs/dependencies.md`: each dependency and tool: users, reason, alternative, license;
  one added, removed or repurposed.
- `README.md`: the project, its real state, building and running it, what to read next; these change.
- `CONTRIBUTING.md`: environment, commands, rules, making and reviewing a change; process or tools change.
- `examples/README.md`, `benches/README.md`: each example (options, tests), each benchmark (what it measures,
  how to read it); they change.
- `games/*/README.md`: the game, playing and making it, its assets; it changes.

## Commits

- One logical change per commit, small enough to review.
- Subject in the imperative, in English. The body says why, unless obvious.
- Do not commit code that fails the checks.
- Tags and releases wait for the owner's go.

## When to stop and ask

Stop and ask the owner when:

- A decision affects architecture and no ADR covers it.
- You want to add a dependency.
- The request contradicts `docs/ROADMAP.md` or an ADR.
- The plan is wrong, or a milestone is larger than expected.
- You would delete or rewrite much existing code.

Propose options with trade-offs and a recommendation. Do not work around a problem to keep going.

## Reporting

At the end of each milestone, report:

- What was done.
- Notable design choices, why, and alternatives rejected.
- The budgets measured, and their figures.
- What to read first.
- Shortcuts taken and debt introduced (`docs/debt.md`).

## Working with several agents

- One session per milestone; after its report, the next starts anew from the repository.
- Hand research and reviews to subagents. Keep their conclusions.
- Each agent takes one milestone or crate at a time, as `docs/PLAN.md` assigns.
- Edit only files in your scope. Report changes needed elsewhere.
- One agent at a time edits shared files: `docs/PLAN.md`, `docs/ARCHITECTURE.md`, workspace `Cargo.toml`.
- A reviewing agent on the strongest model checks each milestone against this file, the ADRs and the quality bar before its report.

## Language

- Everything in the repository is in English: code, comments, commits, documentation.
- Talk to the owner in French.
