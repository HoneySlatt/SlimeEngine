# 0021. Frame pacing

- Status: Accepted; amended by [0155](0155-the-editors-design.md) (a program may run its frames
  only when something asks for one) and [0156](0156-the-editors-window-and-its-operations.md) (how:
  `AppConfig::on_demand`, which replaces the paced run without a window)
- Date: 2026-10-12

## Context

[ADR-0014](0014-amend-roadmap-for-stage-2-and-windows.md) left frame pacing to measurement on each
platform, allowing the frame scheduler of Stage 1 to stay.
[ADR-0020](0020-rendering-in-the-main-loop.md) presented images with wgpu's FIFO present mode
until this decision: every image is shown, one per refresh of the display, and presenting waits
while earlier images are still to be shown.

Measured on niri with an RX 6900 XT (RADV), on displays at 240 Hz and at 180 Hz with variable
refresh, through Xwayland, and in a headless Weston with lavapipe, the engine asking for 60 frames
per second:

- **FIFO and the timer** (the state after ADR-0020): regular frames while the window is visible.
  Hidden on another workspace, each `present` waits one second: niri then sends the window a frame
  callback once per second, and Mesa's FIFO waits for it, since niri does not offer the protocol
  that lets FIFO work without callbacks. The simulation slows to 8 fixed steps per second, the most
  one frame can run. In a headless Weston with lavapipe, one image acquisition in three waits about
  0.8 s, so the windowed check takes about 25 s instead of 2 (a measurement of that day; later ones
  the same day gave 9 to 15 s for 120 frames: the stall depends on the machine's load).
- **Frames on the compositor's frame callbacks** (winit's `pre_present_notify`): aligned with the
  display and without waiting while visible, but once per second while hidden, for the same reason.
- **FIFO without the timer**, paced by the display: one frame per refresh (180 frames per second
  here), each waiting in `present`; one frame per second while hidden.
- **Mailbox and the timer**: each image presented replaces the one waiting to be shown, so
  presenting never waits. 60 frames per second while visible and while hidden, with frame starts
  0.02 to 0.5 ms from the schedule (standard deviation); 4 % of one CPU core visible and 3 %
  hidden in a release build; 60 frames in 1.06 s in the headless Weston.

Every surface measured offers Mailbox: RADV on Wayland and X11, lavapipe, and, according to wgpu,
Direct3D 12 on every Windows machine.

## Decision

- `AppConfig::present_mode` chooses how the window shows its images (`slime_gfx::PresentMode`):
  - `Mailbox`, the default: presenting never waits. On a window that does not offer it, `Fifo` is
    used instead, and the log says so.
  - `Fifo`: every image shown, one per refresh, presenting waiting when needed.
- The frame scheduler of Stage 1 stays on every platform: with Mailbox, it alone paces the frames,
  at most `AppConfig::max_frame_rate` per second (60 by default). Frames still run on redraw
  requests (ADR-0020).
- Profiling spans show where a frame spends its time: `acquire`, `render` (containing `submit`,
  recorded by `slime-gfx` when it hands the work to the GPU) and `present`.
- Windows follows the same policy. What happens there while the user moves or minimizes the window
  is measured in the Windows virtual machine.

This amends ADR-0020, whose surfaces used FIFO.

## Alternatives considered

- **FIFO by default.** Every platform offers it, and it shows every frame, but it stalls a hidden
  window on niri and slows the windowed check four- to fourteen-fold, depending on the load.
- **Frames aligned on the compositor while visible, and frames without drawing on the timer while
  hidden.** No stall, even with FIFO, and images in step with the display, but two kinds of frames,
  and only Wayland would benefit. To reconsider when a renderer draws real scenes, whose
  smoothness can be judged.
- **FIFO paced by the display, switching to the timer once a hidden window is detected.** The
  smoothest while visible, but it renders at the display's rate by default (240 frames per second
  on the development machine), still stalls until the detection, and needs a heuristic to detect.
- **Immediate.** Never waits either, but images can tear.

## Consequences

- A hidden window keeps running at the scheduled rate on niri, and a termination signal stops it
  within a frame (measured: 17 ms). The windowed check in Weston takes about 2 s again.
- Images are not synchronized with the display's refresh. At 60 frames per second on a 180 or
  240 Hz display, each image stays for 3 or 4 refreshes; on a 60 Hz display, the schedule and the
  display drift apart, and now and then an image is shown twice while the next one is dropped.
  Applications that prefer every image shown in step with the display choose `Fifo`.
- With Mailbox, the GPU draws images that the display may never show, and a hidden window keeps
  drawing.
- Drivers or compositors without Mailbox fall back to FIFO and keep its stalls.
