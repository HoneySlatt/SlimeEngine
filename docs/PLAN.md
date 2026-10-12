# Plan

This file says what is being worked on now. It lists the milestones of the current stages, their
status, how each one is checked, and the known debt. Direction beyond these stages is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread.

## Current

- Stage 1 — Core and platform
- Milestone M1.6 — Clean shutdown, hardening, release 0.0.1

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M0.1](#m01--repository-license-and-adr-process) | Repository, license and ADR process | Done |
| [M0.2](#m02--nix-development-environment) | Nix development environment | Done |
| [M0.3](#m03--workspace-lints-and-crate-skeletons) | Workspace, lints and crate skeletons | Done |
| [M0.4](#m04--dependency-policy-and-ci) | Dependency policy and CI | Done, CI not run yet (see [Known debt](#known-debt)) |
| [M1.1](#m11--logging-and-diagnostics) | Logging and diagnostics | Done |
| [M1.2](#m12--frame-timing) | Frame timing | Done |
| [M1.3](#m13--lifecycle-and-headless-mode) | Lifecycle and headless mode | Done |
| [M1.4](#m14--window) | Window | Done |
| [M1.5](#m15--input) | Input | Done, except the check by hand of keys and mouse buttons (see [Known debt](#known-debt)) |
| [M1.6](#m16--clean-shutdown-hardening-release-001) | Clean shutdown, hardening, release 0.0.1 | In progress |

## Stage 0 — Foundations

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-0--foundations)): CI is green on a workspace
that contains the skeletons of the Stage 1 crates; the policies exist as ADRs; the documentation
baseline exists and CI checks it.

### M0.1 — Repository, license and ADR process

- **Goal.** The repository exists and its rules are written down.
- **Crates.** None.
- **Delivers.** `LICENSE` (MPL-2.0), `README.md`, `CONTRIBUTING.md` (skeleton), this plan,
  [`docs/adr/README.md`](adr/README.md) (process and template), ADR-0001 (decisions are recorded
  as ADRs), ADR-0002 (roadmap amendments) with the amended roadmap,
  [`docs/glossary.md`](glossary.md).
- **Done when.** The files exist and every relative link resolves (checked by hand; the link
  checker arrives in M0.4).
- **How to test.** Read `README.md`, then `CONTRIBUTING.md`, then this plan; every link leads
  somewhere.

### M0.2 — Nix development environment

- **Goal.** `nix develop` provides exactly the pinned toolchain and the project's tools.
- **Delivers.** `flake.nix`, `flake.lock`, `rust-toolchain.toml`, ADR-0003 (toolchain, edition,
  minimum supported Rust version). The development shell provides the Rust toolchain (rustc,
  cargo, clippy, rustfmt, rust-src, rust-analyzer), `cargo-deny` and `lychee`. The system
  libraries that the windowing code loads at run time are added in M1.4, with the code that loads
  them, because they cannot be checked before.
- **Done when.** `nix develop -c rustc --version` prints the version pinned in
  `rust-toolchain.toml`, and `nix flake check` passes.
- **How to test.** Run both commands.

### M0.3 — Workspace, lints and crate skeletons

- **Goal.** A Cargo workspace that builds with every policy active.
- **Crates.** `slime-platform` and `slime-app`, whose `lib.rs` contains only crate
  documentation.
- **Delivers.** The workspace `Cargo.toml` (edition, `rust-version`, version, license, lints),
  `.gitignore`, `Cargo.lock`, one `README.md` per crate, `docs/ARCHITECTURE.md` (crate map,
  dependency graph, layering rules), ADR-0004 (workspace layout and crate naming), ADR-0005 (lints
  and `unsafe` policy).
- **Done when.** The five commands of `AGENTS.md` pass. A public function without documentation
  and an `unsafe` block are each shown to fail the checks, then removed.
- **How to test.** Run the five commands listed in `AGENTS.md`.

### M0.4 — Dependency policy and CI

- **Goal.** Every check of the definition of done runs from one script, locally and in CI.
- **Delivers.** `deny.toml`, `docs/dependencies.md` (dependency register), `scripts/check.sh`,
  the GitHub Actions workflow, ADR-0006 (dependency policy), ADR-0007 (documentation tooling and
  CI).
- **Done when.** `scripts/check.sh` passes locally; a deliberately broken Markdown link makes it
  fail; CI is green on GitHub. The repository has no remote yet: until it is pushed, the CI run is
  recorded below as not verified.
- **How to test.** `nix develop -c scripts/check.sh`.

## Stage 1 — Core and platform → 0.0.1

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-1--core-and-platform--001)): a window opens on
Wayland, input events are logged, shutdown is clean. The same binary runs N frames headless in
CI. X11 is checked through Xwayland.

### M1.1 — Logging and diagnostics

- **Goal.** Every engine application logs correctly from its first line.
- **Crates.** `slime-app`; the `examples` package with a first example, `hello`.
- **Delivers.** Installation of the log subscriber (`tracing-subscriber`: output to stderr,
  timestamps, filter read from `SLIME_LOG`), a panic hook that logs the panic before the default
  hook runs, a startup log line with the engine version, OS, architecture and number of logical CPUs.
  ADR-0008 (errors, logging, assertions). Dependencies: `tracing`, `tracing-subscriber`.
- **Done when.** The `hello` example logs; `SLIME_LOG=debug` changes the level; `hello --panic`
  logs the panic and exits with a non-zero code; an automated test checks the panic output.
- **How to test.** Run `hello` with and without `SLIME_LOG` and `--panic`.

### M1.2 — Frame timing

- **Goal.** Deterministic frame timing, testable without sleeping.
- **Crates.** `slime-app` (module `time`).
- **Delivers.** A frame clock (delta clamped to a maximum, elapsed time, frame index) and a
  fixed-step accumulator (maximum number of steps per frame, interpolation factor). Both are fed
  durations, so a real clock and a simulated clock drive them the same way. ADR-0009 (frame
  timing).
- **Done when.** Unit tests cover the clamp, the step count, the absence of a "spiral of death"
  and the range of the interpolation factor, without any sleep.
- **How to test.** `cargo test -p slime-app`.

### M1.3 — Lifecycle and headless mode

- **Goal.** An application runs frames without a window, deterministically.
- **Crates.** `slime-app`; `examples` gains `headless`.
- **Delivers.** The `App` trait (`init`, `fixed_update`, `update`, `shutdown`), the application
  configuration, `run()`, the headless driver with simulated time, exit requests from the
  application, a frame limit, profiling spans (`frame`, `fixed_update`, `update`). ADR-0010 (main
  loop ownership). The lifecycle flow in `docs/ARCHITECTURE.md`.
- **Done when.** An integration test checks the order and number of callbacks, that `shutdown`
  runs exactly once, and the exit code; the example runs with `WAYLAND_DISPLAY` and `DISPLAY`
  unset; `SLIME_LOG=trace` shows the spans.
- **How to test.** `cargo run --example headless -- --frames 600`.

### M1.4 — Window

- **Goal.** A window opens, shows its content, resizes and closes cleanly.
- **Crates.** `slime-platform` (winit 0.30, never exposed), `slime-app` (windowed driver);
  `examples` gains `window`.
- **Delivers.** The window and its events (close requested, resized, scale factor, focus), the
  system libraries it loads at run time (added to the Nix shell), the windowed driver calling the
  same frame function as the headless one, window handles exposed through `raw-window-handle`,
  the frame limiter. The `window` example fills its window with a
  solid color through `softbuffer` (checked first without it, to confirm the problem described in
  ADR-0002). ADR-0011 (windowing with winit). Dependencies: `winit`, `raw-window-handle`,
  `thiserror`; `softbuffer` for the examples only.
- **Done when.** On niri the window appears, resizes are logged, closing it from the compositor
  shuts down cleanly (`shutdown` runs, exit code 0), CPU use stays low while idle, and the same
  holds on X11 through Xwayland.
- **How to test.** `cargo run --example window`, close the window; then again
  with `WAYLAND_DISPLAY` unset to use X11.

### M1.5 — Input

- **Goal.** Keyboard and mouse input reaches the application as engine-owned events.
- **Crates.** `slime-platform`; `examples` (`window` logs every event, Escape quits).
- **Delivers.** Engine-owned input events: keyboard (physical key, logical key, text, pressed or
  released, repeat), mouse (buttons, position, wheel), cursor entering or leaving the window; their
  translation from winit.
- **Done when.** Unit tests cover the translation; by hand, every kind of event appears in the
  log, and with a French AZERTY layout the logical key is right (the physical `KeyQ` key produces
  `a`).
- **How to test.** Type and use the mouse in the `window` example and read the log.

### M1.6 — Clean shutdown, hardening, release 0.0.1

- **Goal.** The application always ends cleanly, and 0.0.1 is released.
- **Crates.** `slime-app`, `examples`.
- **Delivers.** SIGINT and SIGTERM trigger a clean shutdown in both modes (dependency `ctrlc`);
  without a display, a clear error and a non-zero exit code instead of a panic; the `window`
  example also runs headless (`--headless --frames N`): the same binary in both modes; optionally,
  an automated windowed smoke test under a headless Weston compositor; `CHANGELOG.md`; a full
  reread of the documentation as a newcomer; the `v0.0.1` git tag.
- **Done when.** Ctrl+C in both modes logs the shutdown and exits with code 0; CI is green.
- **How to test.** Press Ctrl+C in `window` and in `headless`; run
  `cargo run --example window` with `WAYLAND_DISPLAY` and `DISPLAY` unset.

## Known debt

- **CI has never run.** The repository has no remote yet, so the GitHub Actions workflow has not
  been executed. `scripts/check.sh` passes locally in the same Nix shell that CI uses. Stage 0 is
  complete except for this: the first push to GitHub must show a green run.
- **Temporary frame pacing and display (Stage 2).** The windowed driver limits the frame rate by
  sleeping (`AppConfig::max_frame_rate`), and the `window` example fills its window from the CPU
  with `softbuffer`. Both go away when the GPU layer presents frames.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **`App` methods cannot return errors** (ADR-0010). Revisit before games depend on the API.
- **Keyboard and mouse input not checked by hand.** The translation is unit-tested, and cursor
  events were seen in a real run, but key presses, mouse buttons and scrolling were not checked in
  a real window: an automated check would have to type into the owner's session. To do: run
  `cargo run --example window`, type, click and scroll, and read the log. On a French AZERTY
  layout, the first letter key of the top row (labelled A) must log `physical: KeyQ` and
  `logical: Character("a")`.
- **Input not reported yet:** the state of the modifier keys as a whole, input-method text,
  touch, touchpad gestures, gamepads, dropped files.

## Notes

None.
