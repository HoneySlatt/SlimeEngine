# 0025. Giving up a window's surface that is lost for good

- Status: Accepted
- Date: 2026-10-12

## Context

[ADR-0020](0020-rendering-in-the-main-loop.md) has `Surface::acquire` create the window's surface
again when the graphics API reports it lost, and skip the frame. [ADR-0022](0022-losing-the-gpu-device.md)
stops the run when the device is lost, since nothing can be drawn any more. A surface can be lost
for good as well: creating it again can fail, and a surface created again can be lost at once, at
every frame (on Vulkan, a lost surface does not come back by itself). The engine then logged an
error or a warning at every frame, sixty times per second, while the run went on without drawing,
and nothing stopped it. Neither case can be caused on purpose, so the audit of 0.0.2 established
this from the engine's code and wgpu's sources.

## Decision

- `Surface` remembers when it is lost for good: when creating it again fails, or after it was lost
  again more than three times in a row without an image acquired in between. It logs that once, as
  an error, and `acquire` then returns `None` without logging.
- `Surface::check` returns `GfxError::SurfaceLost`, with the reason, once the surface is lost for
  good, as `Gpu::check` does for the device.
- The windowed driver checks the surface before each frame, with the device and the other reasons
  to stop: the run stops like after an exit request, `shutdown` is called, and `run` returns
  `RunError::Gfx` with `GfxError::SurfaceLost`.

- A configuration of the surface that the graphics API refuses counts as a loss: the surface is
  created again, counted toward the same limit, and given up with wgpu's message when the limit is
  passed. The surface is configured inside wgpu error scopes for that, an exception to ADR-0016's
  rule that wgpu's errors panic: this one comes from the window's environment (a window or a
  display gone, a swap chain that cannot be created), not from a bug. Acquiring an image while the
  previous frame is still held stays a panic, checked by the surface itself, so that the scope does
  not turn that bug into a lost surface. (Added on 2026-10-13 by the owner's decision.)

This amends ADR-0020 (a lost surface is not created again without end) and ADR-0022 (the loss of
the surface stops the run as the loss of the device does).

## Alternatives considered

- **Log once and go on without drawing.** No flood, but a run that draws nothing for ever, which
  the game cannot detect: a window that stays blank until the user kills the process.
- **Create the surface again without limit.** A surface lost at every frame would be created sixty
  times per second, with a warning each time.
- **Stop at the first loss.** A loss can be transient (a display configuration change); creating the
  surface again is what ADR-0020 chose, and it stays.

## Consequences

- A window whose surface is gone for good ends the run cleanly, with an error that names the cause,
  instead of a blank window and a flooded log.
- `GfxError` gains a variant (`#[non_exhaustive]`, so callers keep compiling) and `Surface` a
  method.
- Like the loss of the device, the loss of the surface cannot be caused on purpose and is not
  tested. The limit of three losses in a row is a guess, to adjust if a driver reports transient
  losses in bursts.
