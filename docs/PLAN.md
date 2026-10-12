# Plan

This file says what is being worked on now. It lists the milestones of the current stage, their
status, how each one is checked, and the known debt. Direction beyond this stage is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. From Stage 2 on, every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 2 — GPU layer → 0.0.2
- Milestone M2.2 — Offscreen triangle and reference image
- Left over from Stage 1: the `v0.0.1` tag, which waits for a check by hand of keyboard and mouse
  input (see [Releasing 0.0.1](#releasing-001)).

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M2.0](#m20--windows-in-ci) | Windows in CI | Done, except the check by hand in the Windows virtual machine (see [Known debt](#known-debt)) |
| [M2.1](#m21--gpu-device-without-a-window) | GPU device without a window | Done |
| [M2.2](#m22--offscreen-triangle-and-reference-image) | Offscreen triangle and reference image | In progress |
| [M2.3](#m23--the-triangle-in-the-window) | The triangle in the window | Planned |
| [M2.4](#m24--frame-pacing) | Frame pacing | Planned |
| [M2.5](#m25--debug-markers-robustness-release-002) | Debug markers, robustness, release 0.0.2 | Planned |

Stages 0 and 1 are done; see [Completed stages](#completed-stages).

## Stage 2 — GPU layer → 0.0.2

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-2--gpu-layer--002)): a triangle renders and
survives resize, on Linux and on Windows; an offscreen render is compared to a reference image in
CI, rendered with lavapipe on Linux and WARP on Windows, with a tolerance so that the same test
passes on hardware drivers.

Decisions taken when this stage was planned, each recorded as an ADR in the milestone that applies
it:

- GPU resources are engine-owned types that hold their wgpu object privately (no table of
  identifiers until something needs one).
- The engine drives rendering: windowed runs create the GPU device and the surface, the engine
  acquires each frame's image, calls a new `App::render` after `update`, then presents. Headless
  runs create no GPU device.
- Frames are driven by redraw requests, which keep arriving while Windows runs its modal loop
  during an interactive resize or move.
- Frame pacing is decided by measurement, possibly differently on each platform.
- GPU object creation reports errors (shader and pipeline errors with the shader compiler's
  message); `App::init` can fail and `run` returns a `RunError`.
- Image tests: reference PNG images in the repository, produced with lavapipe, compared with a
  tolerance; CI uses lavapipe on Linux and WARP on Windows.
- Dependencies: `wgpu` 30 (Vulkan on Linux, Direct3D 12 on Windows, WGSL shaders), `pollster`,
  `png`. `softbuffer` is removed. Windows builds use the MSVC toolchain. (`bytemuck`, also planned,
  proved unnecessary in M2.2.)

### M2.0 — Windows in CI

- **Goal.** Everything that exists builds, passes its tests and runs headless on Windows, checked on
  every push.
- **Crates.** `slime-app` (tests, documentation), `examples`; CI.
- **Delivers.** A Windows job in CI (toolchain from `rust-toolchain.toml` through rustup, MSVC
  target): clippy, build, tests, headless samples. The examples' Windows build published as a CI
  artifact. The Windows target of the standard library in `rust-toolchain.toml`, so that
  `scripts/check.sh` also runs clippy for Windows on Linux. Signal tests limited to Unix. The
  Windows development setup in `CONTRIBUTING.md`. ADR: platforms and CI.
- **Done when.** The Windows job is green; on Linux, `scripts/check.sh` fails on a deliberate
  Windows-only compile error; the owner ran the `window` example from the artifact in a Windows
  virtual machine (window, resize, close, keyboard and mouse).
- **How to test.** Push and read the CI run; download the artifact in the Windows virtual machine
  and run `window.exe`.

### M2.1 — GPU device without a window

- **Goal.** Open a GPU device without a window, on Linux and Windows, on hardware and software
  drivers.
- **Crates.** New `slime-gfx`.
- **Delivers.** Creation of the GPU device and queue (Vulkan on Linux, Direct3D 12 on Windows,
  high-performance adapter), the adapter described in the log, a device-loss callback, error types.
  The Nix shell puts the Vulkan loader on the library path and gives the path of the lavapipe
  driver; `scripts/check.sh` and CI run GPU tests on lavapipe (Linux) and WARP (Windows). A build
  cache in CI if build times call for it. ADR: the `gfx` crate and wgpu's configuration, with the
  GPU error policy. Dependencies: `wgpu`, `pollster`.
- **Done when.** A test opens a device on lavapipe (Linux CI), on WARP (Windows CI) and on the
  owner's GPU (by hand); headless runs open none.
- **How to test.** `cargo test -p slime-gfx`, and again with lavapipe forced through
  `VK_DRIVER_FILES`.

### M2.2 — Offscreen triangle and reference image

- **Goal.** Render without a window and compare the result with a reference image, on Linux and
  Windows.
- **Crates.** `slime-gfx`; `examples` gains `offscreen`.
- **Delivers.** The resource types (buffer, texture, shader module, render pipeline), WGSL compiled
  from source text with readable errors, a minimal pipeline and render pass, reading the image back
  to the CPU, a comparison with tolerance, a reference image produced with lavapipe, a label
  required on every resource. The `offscreen` example writes the triangle to a PNG file. ADRs: GPU
  resource types; image tests. Dependency: `png`. `bytemuck` was planned too, but vertex data is
  turned into bytes with the standard library (`f32::to_le_bytes`) as long as there are no vertex
  structures (ADR-0017).
- **Done when.** The image test passes on lavapipe, WARP and the owner's GPU (or uses one
  reference per driver if no tolerance covers all three, as the ADR then records); a deliberate
  colour change makes it fail; a WGSL error gives a readable message; a resource's label appears in
  a validation error.
- **How to test.** `cargo test -p slime-gfx`; `cargo run --example offscreen` and open the PNG.

### M2.3 — The triangle in the window

- **Goal.** The engine presents the triangle in the window, on Linux and Windows.
- **Crates.** `slime-gfx` (surface), `slime-app` (render callback, fallible `init`, `RunError`,
  frames driven by redraw requests), `slime-platform` (redraw requests), `examples` (`window` draws
  the triangle; `softbuffer` removed).
- **Delivers.** A surface created from the window through `raw-window-handle`, an sRGB format,
  resize, handling of every state of the surface, `App::render`. Measurements: a hidden window on
  niri, idle CPU and GPU, frame times, X11, Weston with lavapipe, the Windows virtual machine. ADR:
  rendering in the main loop and application errors (replacing parts of ADR-0010 and ADR-0011).
- **Done when.** The triangle shows on niri (Wayland), through Xwayland and on Windows; it survives
  resize, including an interactive resize on Windows; the Weston check passes with lavapipe; the
  measurements are recorded.
- **How to test.** `cargo run --example window` on Linux; `window.exe` from the CI artifact on
  Windows.

### M2.4 — Frame pacing

- **Goal.** Frames are paced without stalling the engine, on each platform.
- **Crates.** `slime-app`, `slime-gfx`.
- **Delivers.** The pacing policy chosen from the measurements of M2.3, the present mode in
  `AppConfig`, the frame scheduler kept or removed per platform, profiling spans for acquiring,
  recording, submitting and presenting a frame. ADR: frame pacing.
- **Done when.** A hidden window does not stall the loop beyond what the ADR states (Ctrl+C is
  still handled); idle CPU and GPU use and the regularity of frames are measured on niri; Windows
  is checked in the virtual machine, whose GPU says nothing about timing.
- **How to test.** Run the `window` example, hide it, and read the frame spans with
  `SLIME_LOG=trace`.

### M2.5 — Debug markers, robustness, release 0.0.2

- **Goal.** GPU work can be inspected in a capture tool, failures of the GPU do not crash the
  engine, and 0.0.2 is released.
- **Crates.** `slime-gfx`, `slime-app`.
- **Delivers.** Debug groups for frames and render passes, checked in RenderDoc (Linux, through
  Xwayland); loss of the surface or of the device handled without a panic; the documentation
  reread by a reviewing agent; `CHANGELOG.md`; the `v0.0.2` tag on a commit whose CI is green on
  both platforms.
- **Done when.** A RenderDoc capture shows the labels and groups; the checks are green on both
  platforms; the tag is pushed.
- **How to test.** Capture a frame of the `window` example in RenderDoc.

## Releasing 0.0.1

The release is a git tag on a commit whose CI run is green
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. Push the repository to GitHub and check that the CI workflow passes (done: the first push, on
   2026-10-12, ran green).
2. Check keyboard and mouse input by hand (see [Known debt](#known-debt)).
3. In one commit: set `version = "0.0.1"` in the root `Cargo.toml`, rename the `[Unreleased]`
   section of `CHANGELOG.md` to `[0.0.1] - <date>`, and update this plan (done: the commit
   "Release 0.0.1").
4. Once step 2 is done, tag that commit, not a later one:
   `git tag -a v0.0.1 -m "SlimeEngine 0.0.1" <commit>`, then push the tag.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done, except the check by hand of keyboard and mouse input. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |

The details of each completed milestone (what it delivered, how it was checked) are in the history
of this file and in the commits that implemented it.

## Known debt

- **Temporary frame pacing and display (Stage 2).** The windowed driver limits the frame rate by
  sleeping (`AppConfig::max_frame_rate`), and the `window` example fills its window from the CPU
  with `softbuffer`. M2.3 replaces the CPU fill; M2.4 decides the pacing.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **`App` methods cannot return errors** (ADR-0010). `App::init` becomes fallible in M2.3.
- **Keyboard and mouse input not checked by hand.** The translation is unit-tested, and cursor
  events were seen in a real run, but key presses, mouse buttons and scrolling were not checked in
  a real window: an automated check would have to type into the owner's session. To do: run
  `cargo run --example window`, type, click and scroll, and read the log. On a French AZERTY
  layout, the first letter key of the top row (labelled A) must log `physical: KeyQ` and
  `logical: Character("a")`.
- **The examples were not run by hand on Windows yet.** CI builds and tests them on Windows, but
  only a person can check a window there. To do: download the artifact `slime-examples-windows`
  of the latest CI run (on GitHub, or with `gh run download -n slime-examples-windows`), then in
  the Windows virtual machine run `window.exe` from a terminal: the window appears, resizes,
  closes cleanly with Escape or its close button, and logs keys (including on an AZERTY layout),
  clicks and scrolling; Ctrl+C in the terminal also ends it cleanly.
- **Input not reported yet:** the state of the modifier keys as a whole, input-method text,
  touch, touchpad gestures, gamepads, dropped files.
- **The interpolation factor is not passed to applications.** `FixedStep::alpha` computes it, but
  `run` keeps its `FixedStep` private, so an application cannot interpolate between fixed steps
  yet. To expose when a renderer needs it.

## Notes

- CI build cache: not added in M2.1. With wgpu, each CI job takes about 5 minutes, which is
  acceptable; a cache is to add if it grows much further.
