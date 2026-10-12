# 0020. Rendering in the main loop, and application errors

- Status: Accepted; amended by [0021](0021-frame-pacing.md) (present mode) and [0022](0022-losing-the-gpu-device.md) (device loss)
- Date: 2026-10-12

## Context

Stage 2 draws a triangle in the window, on Linux and Windows. Three parts of the engine have to
change for it.

- **Who renders.** [ADR-0010](0010-main-loop-ownership.md) gave applications `init`,
  `fixed_update`, `update` and `shutdown`, none of which draws. Drawing into a window needs the GPU,
  a *surface* (the images the window shows) and, for each frame, an image to draw into, handed back
  to the window once drawn (*presented*).
- **When frames run.** [ADR-0011](0011-windowing-with-winit.md) ran frames when winit's event loop
  was about to wait (`about_to_wait`). On Windows, while the user resizes or moves a window, the
  system runs a loop of its own in which winit only reports window events and redraw requests
  (`RedrawRequested`, from `WM_PAINT`): `about_to_wait` is not called until the user releases the
  window, so frames would stop, and a resized window would show a stretched or blank image. winit
  recommends drawing in response to redraw requests.
- **Errors.** ADR-0010 let no application method return an error. Creating GPU objects can fail
  for reasons an application should report (a shader that does not compile, for example), and
  `init` is where they are created.

The Stage 2 plan decided the direction: the engine drives rendering, frames follow redraw
requests, `init` can fail and `run` returns a `RunError`. This ADR records how.

## Decision

**GPU and surface.**

- A windowed run opens the GPU for its window before `init`: `slime_gfx::Gpu::new_for_window`
  creates the window's surface, asks for an adapter that can present to it, and configures it. A
  headless run opens no GPU.
- The surface uses the first sRGB format the window offers among the engine's formats
  (`Bgra8UnormSrgb` on every driver measured, `Rgba8UnormSrgb` otherwise), and wgpu's FIFO present
  mode, the only one every platform supports, until frame pacing is decided by measurement in
  the next milestone.
- `Surface::acquire` handles every state wgpu reports: a size of zero (minimized window), an image
  that is not ready in time, an occluded window, an outdated configuration (configured again and
  retried once), a suboptimal image (used, then configured again), and a lost surface (created
  again). In each case where no image can be drawn, it returns `None` and the frame is not drawn.
- `Context::gpu()` gives the application the GPU, and `Context::surface_format()` the format of
  the window's images; both are `None` when headless.

**Frames.**

- Each frame runs its fixed steps and `update`, then, if the surface gives an image,
  `App::render(ctx, target)` draws into it and the engine presents it. Frames where no image is
  available still run their fixed steps and update.
- Frames run on redraw requests. When a frame is due, `about_to_wait` asks the window for a redraw
  (`Window::request_redraw`), and the frame runs when `slime-platform` reports it through
  `EventHandler::redraw_requested`. A redraw that the windowing system asks for on its own (a
  resize, a window uncovered) runs a frame as well. The frame scheduler of Stage 1 still decides
  when the engine asks: on average at most `AppConfig::max_frame_rate` frames per second.
- Profiling spans `acquire`, `render` and `present` join `fixed_update` and `update` inside each
  `frame` span, so that the cost of presentation can be measured.

**Errors.**

- `App::init` returns `Result<(), AppError>`, where `AppError` is any boxed error. When it fails,
  no frame runs, `shutdown` is not called, and `run` returns `RunError::Init` with the error as its
  source.
- `run` returns `RunError`: `Platform` (the window or its event loop failed), `Gfx` (no GPU can
  draw into the window) or `Init`. Its causes are reachable through `source()`.
- The other `App` methods still return nothing: no caller can handle their errors better than the
  application yet.

**Checks.**

- The windowed check in a headless Weston ([ADR-0013](0013-windowed-smoke-test-in-weston.md))
  now draws and presents GPU images, with lavapipe (`VK_DRIVER_FILES`), the driver of the other
  checks, so that it checks the whole surface path and does not depend on the developer's GPU.

This amends ADR-0010 (the `App` methods, `run`'s error type), ADR-0011 (frames run from redraw
requests, not from `about_to_wait`; the examples no longer use softbuffer) and ADR-0013 (what the
check presents, and how long it takes).

## Alternatives considered

- **Frames in `about_to_wait`, plus a redraw for the windowing system's own requests.** Works for
  resizes, but leaves two paths that run frames, and goes against how winit expects rendering to
  be driven.
- **The application owns the surface.** Every application would handle acquisition, resizes and
  the states of the surface itself, and the engine could not control presentation, which frame
  pacing needs.
- **`render` receiving the GPU instead of reading it from the context.** Two ways to reach the
  same GPU; `init` needs it from the context anyway.
- **`shutdown` called after a failed `init`.** An application that failed to start may be half
  initialized; like a constructor that fails, a failed `init` has nothing to undo through
  `shutdown`.
- **Every method returning a `Result`.** No error from `update` or `render` can be handled better
  by the engine than by the application yet.
- **A presentation mode other than FIFO now.** Choosing it is the subject of frame pacing, decided
  from the measurements made with this design.

## Consequences

- Games implement `render` and create their GPU objects in `init`; existing implementations of
  `init` must return `Ok(())`, and callers of `run` handle a `RunError`.
- `slime-app` depends on `slime-gfx`, and the examples no longer need softbuffer.
- On Windows, the triangle follows an interactive resize, because each step of it sends a redraw.
  While the user moves the window, no redraw arrives and frames pause; this is measured in the
  Windows virtual machine and handled, if needed, with frame pacing.
- With FIFO, presentation can block: on compositors that stop showing a hidden window, each frame
  waits for the compositor (measured: one second per frame on niri), and in a headless Weston with
  lavapipe, one image in three waits about 0.8 s, so the Weston check takes about 25 seconds
  instead of 2 (a measurement of that day; later ones the same day, with the same Weston and Mesa,
  gave 9 to 15 s for 120 frames with waits of 0.13 to 0.25 s: the stall depends on the machine's
  load). Frame pacing addresses both.
