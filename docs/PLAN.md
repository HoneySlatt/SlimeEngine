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

- Stage 2 — GPU layer → 0.0.2: done and released (tag `v0.0.2`). Its milestones stay below until
  the next stage is planned.
- Next: planning Stage 3a with the owner (see [`ROADMAP.md`](ROADMAP.md)). No code is written
  for it before its plan is validated.
- A stabilization pass on 2026-10-12 corrected what an audit of 0.0.2 found: the corrections are
  listed in [`CHANGELOG.md`](../CHANGELOG.md) under *Unreleased*, and what was found without being
  corrected is in [Known debt](#known-debt).

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M2.0](#m20--windows-in-ci) | Windows in CI | Done |
| [M2.1](#m21--gpu-device-without-a-window) | GPU device without a window | Done |
| [M2.2](#m22--offscreen-triangle-and-reference-image) | Offscreen triangle and reference image | Done |
| [M2.3](#m23--the-triangle-in-the-window) | The triangle in the window | Done |
| [M2.4](#m24--frame-pacing) | Frame pacing | Done |
| [M2.5](#m25--debug-markers-robustness-release-002) | Debug markers, robustness, release 0.0.2 | Done |

Stages 0, 1 and 2 are done; see [Completed stages](#completed-stages).

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
  during an interactive resize (not during a move: see [Known debt](#known-debt)).
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
- **Measured** on 2026-10-12, with the default `AppConfig` (60 frames per second at most) and
  wgpu's FIFO present mode; frame times from the `frame` spans (`SLIME_LOG=slime_app=trace`), in a
  debug build unless stated:
  - niri (Wayland), RX 6900 XT (RADV): frames every 16.66 ms on average (standard deviation
    1.5 ms; one interval in 340 above 20 ms). In a release build, 5 % of one CPU core, about 55
    system calls and 4 wake-ups of the event loop per frame; the GPU load of the triangle cannot be
    told apart from the desktop's.
  - niri, window moved to an empty workspace (hidden): `present` blocks for one second in each
    frame, so frames run once per second, each with the maximum of 8 fixed steps: the simulation
    runs 8 steps per second instead of 60. CPU use falls to 0.2–0.4 %.
  - X11 through Xwayland (xwayland-satellite on niri), RADV: frames every 16.66 ms (standard
    deviation 0.7 ms, longest 19 ms).
  - Weston (headless) with lavapipe: the triangle appears in a screenshot of the compositor. One
    image acquisition in three waits about 0.8 s, so 120 frames take about 25 s; with the Mailbox
    or Immediate present mode, 60 frames take 1.06 s.
  - Present modes offered: Mailbox and Fifo on niri; Immediate, Mailbox, Fifo and FifoRelaxed on
    X11; Mailbox, Fifo and Immediate with lavapipe in Weston. Every surface offered
    `Bgra8UnormSrgb` first.
  - The owner saw the triangle on niri and through Xwayland, and reports that `window.exe` works
    in the Windows virtual machine.

### M2.4 — Frame pacing

- **Goal.** Frames are paced without stalling the engine, on each platform.
- **Crates.** `slime-app`, `slime-gfx`.
- **Delivers.** The pacing policy chosen from the measurements of M2.3, the present mode in
  `AppConfig`, the frame scheduler kept or removed per platform, a profiling span for submitting
  the work of a frame (acquiring, recording and presenting have theirs since M2.3). ADR: frame
  pacing.
- **Done when.** A hidden window does not stall the loop beyond what the ADR states (Ctrl+C is
  still handled); idle CPU and GPU use and the regularity of frames are measured on niri; Windows
  is checked in the virtual machine, whose GPU says nothing about timing.
- **How to test.** Run the `window` example, hide it, and read the frame spans with
  `SLIME_LOG=trace`.
- **Measured** on 2026-10-12 on niri, RX 6900 XT (RADV), displays at 240 Hz and at 180 Hz with
  variable refresh, 60 frames per second asked, release builds:
  - Before the decision, with frames timed inside the program (one window moved to an empty
    workspace to hide it):
    - FIFO and the schedule: regular frames while visible; hidden, each `present` waits one
      second, because niri sends a hidden window one frame callback per second.
    - Mailbox and the schedule: 60 frames per second visible and hidden, frame starts 0.02 to
      0.5 ms from the schedule (standard deviation).
    - FIFO without the schedule: one frame per refresh of the display (180 per second), each
      waiting in `present`; once per second while hidden.
    - Frames on the compositor's frame callbacks (winit's `pre_present_notify`), with FIFO or
      Mailbox: in step with the display, without waiting, while visible; once per second while
      hidden.
  - With the decision (ADR-0021, Mailbox by default): 4 % of one CPU core visible, 3 % hidden;
    the GPU load of the triangle cannot be told apart from the desktop's (11 to 24 % for the whole
    GPU). A termination signal sent while the window is hidden stops the run 17 ms later. With
    `SLIME_LOG=slime_app=trace`, frame ends are 3.1 ms from the schedule (standard deviation,
    part of it from writing the log), and `present` takes 1.4 ms on average.
  - Headless Weston with lavapipe: 120 frames take 2.1 s with Mailbox; with
    `--present-mode fifo`, 30 frames take 5.5 s.
  - Windows, in the virtual machine, with the RX 6900 XT through Direct3D 12 (AMD driver
    32.0.21030.2001), Mailbox: 600 frames take 16.4 s when the window is moved for about 6 s, so
    frames pause during the move; they take 10.2 s when the window is minimized for 6.6 s, so
    frames go on while minimized, without drawing.

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
- **Checked** on 2026-10-12:
  - Frames are named through the window's render target (`window, frame <index>`) rather than
    by a debug group: a group cannot span the several submissions a frame can make, and the
    target's label names both the pass and its command buffer.
  - A capture of the `window` example (debug build) in RenderDoc 1.46, on Wayland in a headless
    Weston with lavapipe, the capture triggered through RenderDoc's target control: the pass
    appears as a region named `window, frame 76` (RenderDoc's frame 76 too), containing the
    render pass, inside which a region named `triangle` holds the pipeline and vertex buffer
    bindings and the draw; the pipeline and the vertex buffer carry their labels (`triangle`,
    `triangle vertices`). Xwayland was not needed.
  - A test destroys the device: `Gpu::check` reports the loss, and rendering and reading back
    with the lost device do not panic; without the engine's error handler, the same test panics.
    It passes on lavapipe and on WARP. A real loss and the loss of the surface cannot be caused
    on purpose and are not tested.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml`, rename the `[Unreleased]` section of
   `CHANGELOG.md` to `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2".

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |

The details of each completed milestone (what it delivered, how it was checked) are in the history
of this file and in the commits that implemented it.

## Known debt

- **Frames are not synchronized with the display's refresh** (ADR-0021). With Mailbox, the
  default, the schedule alone paces frames: on a 60 Hz display, an image is now and then shown
  twice while the next one is dropped. Aligning frames on the compositor's frame callbacks is to
  reconsider when a renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.** Windows that do not offer it, and
  applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a
  hidden window on niri. In a headless Weston with lavapipe, `acquire` waits for Weston's frame
  callbacks for a time that depends on the machine's load: 120 frames of the `window` example took
  15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other
  measurements the same day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the
  development machine): the engine cannot tell on Wayland whether its window is visible. winit
  reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland
  included, the X server reports windows as visible anyway.
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory panics as well on the paths where wgpu does not treat it as
  the loss of the device (ADR-0022). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error.
- **A lost GPU device is not replaced** (ADR-0022): the run stops with an error, and since winit
  allows one event loop per process, carrying on means starting the process again.
- **Captures in RenderDoc were not tried on Windows** (Direct3D 12).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**: the system then runs a loop of its own, in which no redraw arrives and
  the engine's loop does not wake, so frames and the check for termination signals pause until it
  ends (ADR-0020; the move was measured in M2.4). Keeping frames going would mean asking for
  redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine
  then uses another one: seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **Only `App::init` can return an error** (ADR-0020). An application that hits an error in
  another method logs it and requests an exit.
- **Input not reported yet:** the state of the modifier keys as a whole, input-method text,
  touch, touchpad gestures, gamepads, dropped files.
- **The interpolation factor is not passed to applications.** `FixedStep::alpha` computes it, but
  `run` keeps its `FixedStep` private, so an application cannot interpolate between fixed steps
  yet. To expose when a renderer needs it.
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when
  a second one ends the process; after `shutdown`, releasing the window or the surface can block
  too (seen with a Weston stopped by SIGSTOP).
- **A lost surface that cannot be created again** makes `Surface::acquire` log an error at every
  frame while the run goes on without drawing; only a lost device stops the run (ADR-0022). Neither
  loss can be caused on purpose.
- **`GfxError::Wait` is never returned**: wgpu's indefinite wait reports no error that the engine
  could map to it, and a loss that the wait is the first to find panics inside wgpu instead.
- **On Direct3D 12, a covered window or a locked session was not measured.** In `acquire`, wgpu
  waits on the swap chain's frame-latency object, up to one second, if the compositor stops taking
  images; whether Windows does so for such windows is unknown.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), and log an `ERROR` from
  `sctk_adwaita::config` under the default log filter when no session bus or settings portal
  answers, although nothing timed out.

## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further.
