# 0022. Losing the GPU device

- Status: Accepted; amended by [0025](0025-giving-up-a-lost-surface.md) (surface lost for good), [0044](0044-running-out-of-gpu-memory.md) (a lost device creates nothing; memory that runs out unseen gives the device up), [0075](0075-recovering-from-a-lost-gpu-device.md) (a run may go on with a new device) and [0103](0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md) (a loss is an error wherever the engine finds it, a wait included)
- Date: 2026-10-12

## Context

A GPU device can be lost while the engine runs: the driver crashes or is reset (on Windows, after
a GPU takes too long, the system resets it), the GPU is removed, or it runs out of memory. Every
object created from the device then becomes unusable.

[ADR-0016](0016-gpu-layer-with-wgpu.md) logs the loss as an error, leaves "stopping the run on it"
to rendering, and treats every error that wgpu reports outside an error scope as a bug that panics,
through wgpu's default handler. After a loss, though, every use of the device makes wgpu report an
error, so the first frame after a driver reset would crash the engine. The Stage 2 plan requires
the loss of the device or of the surface to be handled without a panic.

In wgpu 30, when the driver reports a lost device, or runs out of memory on most paths, wgpu marks
the device lost and calls the device-lost callback before it reports the error of the operation
that found the loss. The loss of the window's surface is already handled: the surface is created
again (ADR-0020).

## Decision

- `slime-gfx` records the loss, with the reason the driver gives, in the device-lost callback.
  `Gpu::check()` returns `GfxError::DeviceLost` once the device is lost.
- `slime-gfx` replaces wgpu's error handler with its own. Before the loss, an error panics, as
  with wgpu's default handler: it is a bug. After the loss, errors are logged at the `debug` level
  and ignored, since they only repeat that the device is gone.
- The windowed run checks the GPU before each frame, with the other reasons to stop. Once the
  device is lost, the run stops like after an exit request: the frame in progress finishes, without
  drawing, `shutdown` is called, and `run` returns `RunError::Gfx` with `GfxError::DeviceLost`.
- The engine does not open a new device to carry on.

This amends ADR-0016: wgpu's errors still panic, except after the device was lost.

## Alternatives considered

- **Open a new device and carry on.** Every GPU object of the application would have to be
  created again, through a new callback, and the engine has no use for that yet. Games that
  recover from a lost device are rare; the error lets the caller report the problem.
- **Keep panicking.** A driver reset would crash the game instead of letting it close cleanly and
  report what happened.
- **Log every wgpu error instead of panicking.** Bugs in the use of wgpu would go unnoticed, and a
  frame would go on with half of its work rejected.

## Consequences

- After a driver reset or the removal of the GPU, the engine stops with an error instead of
  crashing. Since winit allows one event loop per process, carrying on means starting the process
  again.
- Errors after the loss appear only in the `debug` log; the loss itself is logged as an error.
- The handling is tested by destroying the device in a test (`Gpu::check` and the absence of
  panics); a real loss, and the loss of the surface, cannot be caused on purpose and are not
  tested.
- Running out of GPU memory on the paths where wgpu does not treat it as a loss still panics, like
  every other error that wgpu reports outside an error scope.
- A loss that a wait for the GPU is the first to find, in `Gpu::wait_idle` or
  `Gpu::read_render_target`, still panics inside wgpu, whose `Device::poll` treats it as fatal; the
  windowed loop never polls the device (`GpuTimer::latest`, called at every frame, leaves the
  times of finished passes to be collected when work is submitted, which reports a loss without
  panicking), so only applications that read back or wait are exposed.
