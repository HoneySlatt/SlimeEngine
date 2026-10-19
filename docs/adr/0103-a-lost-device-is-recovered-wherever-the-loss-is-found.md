# 0103. A lost GPU device is recovered wherever the loss is found

- Status: Accepted; amends [0022](0022-losing-the-gpu-device.md) (where a loss panics),
  [0072](0072-stage-4b-live-assets.md) and [0075](0075-recovering-from-a-lost-gpu-device.md)
  (when the run gives up replacing a lost device, and a loss that the application meets)
- Date: 2026-10-19

## Context

[ADR-0075](0075-recovering-from-a-lost-gpu-device.md) lets a windowed run replace a lost GPU device
between two frames. The audit of 0.4
([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) found that the recovery
covered one case, a loss that the main thread finds between two frames, and that two paths of a
frame crashed the program. The audit pass's third milestone, P3, fixes them;
[`docs/PLAN.md`](../PLAN.md) describes each finding by its number:

- **G14.** A window's image acquired and never presented crashed the program on Vulkan, when the
  window's surface was dropped. After a panic in `App::render` that drew, the second panic, during
  unwinding, aborted the process. A loss that another thread finds between the engine's check and
  the present made `Surface::restore` panic.
- **G15.** Found elsewhere, a loss panicked: in a loading thread, in wgpu's unmap of a texture's
  uploads; in `Device::poll`, which `Gpu::wait_idle` and the readbacks call. It came back as a
  failed readback. Found during a frame or during `App::gpu_restored`, it stopped the run as an error
  of the application. The dev overlay panicked on a lost device. A loss that came back a few frames
  after each new device was restored without end. After a restore that failed, the error that the
  application gave in `shutdown` was lost.

Two facts of wgpu 30 cause most of them:

- **wgpu holds a device invalid before it reports the loss.** A driver's loss marks the device
  invalid, then calls the device-lost callback on the thread that found it, which records the loss
  in the engine's `Gpu`; a device destroyed is reported only when the device is next polled. In
  between, every creation makes objects that cannot be used, and wgpu reports their use as a
  misuse, a validation error, which the engine's error handler panics on (ADR-0022). Using the
  device itself, submitting, presenting, gives errors that wgpu does not report. wgpu does not say
  whether a device is valid.
- **On Vulkan, wgpu panics when it destroys a swap chain whose synchronization an image still
  holds.** An image keeps it while it is acquired: when the device refused to present it, and while
  work in flight uses an image that a panic left unpresented. wgpu releases the image that a panic
  drops, but not the work's hold on it.

## Decision

In `slime-gfx`:

- **The surface releases the last image before the swap chain goes.** `Surface` keeps the texture
  of the last image it acquired, and destroys it when it is dropped, restored, created again or
  configured again: that releases the image's synchronization, and does nothing to an image
  already presented. Dropping the texture before a configuration lets Direct3D 12 resize the swap
  chain's images, which it refuses while one is referred to.
  `SurfaceFrame::present` presents the image whether the device is lost or not; once it is, wgpu
  refuses, silently, and the surface releases the image later.
- **A loss is recorded before it is logged**, so that other threads see it as soon as possible.
- **A creation returns only objects that can be used.** After each creation of a GPU object,
  `slime-gfx` asks wgpu whether the device is still valid: a buffer of 4 bytes created mapped can
  be written only on a valid device. When it is not, the loss is recorded as the driver's, if wgpu has not reported it yet,
  and the creation returns `GfxError::DeviceLost`. An upload or a readback whose mapping fails on a
  lost device returns the same.
- **The engine never polls the device.** `Gpu::wait_idle` and the readbacks wait for wgpu's
  callbacks by submitting empty work, sleeping in between, from 50 µs to 1 ms, as `GpuTimer` already
  left its callbacks to the frames' submissions: a submission reports a loss as an error, where
  `Device::poll` panics on it. A wait on a
  device lost, before or while it waits, returns `GfxError::DeviceLost`.

In `slime-devui`: **`Overlay::new` returns `GfxError::DeviceLost`** on a lost device, where egui's
renderer panicked.

In `slime-app`:

- **A loss that the application meets is the run's.** An error passed to
  `Context::exit_with_error`, or returned by `App::gpu_restored`, that is or is caused by
  `GfxError::DeviceLost`, once the device of the `Context` is lost, does not stop the run: the run
  handles the loss as one that it found itself, and opens a new device, or stops with
  `RunError::Gfx`. The error is logged at the `debug` level. *Hunna Wood* and the examples, which
  pass a frame's errors to `exit_with_error`, skip that frame.
- **The run gives up at a loss that comes after three new devices within five minutes**, where it
  gave up after three in a row without a frame shown: a device lost again soon after each new one
  will not stay, whether frames are shown in between or not. A device lost at every frame is given
  up as before.
- **The application's error comes first.** When the run ends, the error that the application passed
  to `exit_with_error`, in `shutdown` too, is returned rather than the error of a restore that
  failed; the error of `init` stays first, as `run` says.

The windowed lifecycle test reaches wgpu through `slime-gfx`'s `wgpu` feature, for one scenario: it
makes the device invalid without telling the engine, as wgpu does before it reports a driver's
loss, which no API of the engine does. ADR-0075 had the engine's tests lose devices through
`Gpu::lose` only.

## Alternatives considered

- **A `Drop` of `SurfaceFrame` that presents the image**: it would show a black or half-drawn frame
  instead of skipping it, and present during unwinding. **A `Drop` that destroys the image when it
  was not presented**: it covers the panic, not the image that a lost device refused to present
  after `present` returned, which only the surface still knows.
- **Catching wgpu's panic in `Device::poll`**: a panic is a bug under
  [ADR-0008](0008-errors-logging-and-assertions.md), and the panic hook would print it, or a game's crash
  reporter keep it; in a build that aborts on panics, nothing catches it. **Polling after checking
  the loss**, as before: the wait itself is where a driver's reset is found.
- **Telling an error that a lost device causes by its message**, `… is invalid`, in the error
  handler: it relies on wgpu's wording. **Asking whether the device is valid before a creation**:
  the device can be lost between the question and the creation.
- **Keeping the rule of three new devices in a row without a frame shown, beside the window of
  time**: two rules for one decision; the window gives up on a device lost at every frame as fast.
- **Each application skipping a frame that a lost device failed**: every application would repeat
  the same test, and one that forgot would stop where the run could have gone on.

## Consequences

- A panic in `App::render` once the frame drew ends the process with exit status 101, as a panic
  does, and the panic hook's report is the application's. A panic inside a pass is not tested. A loss found on any thread, during a frame or during
  `gpu_restored`, is restored as any other.
- Behaviour that changes for users: `Overlay::new` returns a `Result`; an error of a lost device
  passed to `exit_with_error` no longer stops the run; a device lost now and then, a frame shown in
  between, is given up at the fourth loss within five minutes; `Gpu::wait_idle` and the readbacks
  wake up to 1 ms after the GPU finishes; every creation costs a buffer of 4 bytes more, a few
  microseconds on lavapipe.
- What stays: egui's renderer still panics when its own upload of the panels is the first to find
  a loss, and `Overlay::new` when the device is lost between its check and egui's creation. A
  device destroyed through wgpu itself, which the engine never does, is recorded as the driver's
  loss. A driver's real loss cannot be caused on lavapipe: the tests make the device invalid as
  wgpu does, and the owner checks a real one by hand, in the Windows virtual machine. A restore
  that fails cannot be caused in a windowed run (ADR-0075): the order of the errors when it does is
  not tested.
- The two facts of wgpu above, and the panic of `Device::poll` on a lost device, are worth reporting
  to wgpu's authors, with the owner's agreement.
