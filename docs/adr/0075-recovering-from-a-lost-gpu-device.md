# 0075. Recovering from a lost GPU device

- Status: Accepted; amends [0022](0022-losing-the-gpu-device.md) (the run stops after a loss,
  unless the application chose to go on)
- Date: 2026-10-15

## Context

[ADR-0022](0022-losing-the-gpu-device.md) stops a run once its GPU device is lost: the driver
crashed or was reset, the GPU was removed, or memory ran out where no caller could be told
([ADR-0044](0044-running-out-of-gpu-memory.md)). Since the windowing library allows one event loop
per process, going on meant starting the process again.

[ADR-0072](0072-stage-4b-live-assets.md) decides that an application may choose to go on: the
engine opens a new device through the same wgpu instance, creates the window's surface again, and
lets the application create its GPU objects again; it refuses a CPU adapter after losing a hardware
one, and gives up after three recoveries in a row without a presented frame, when the device ran
out of memory, or when no adapter opens. It leaves how to M4b.3.

Where the code stood: each `Gpu::new` and `Gpu::new_for_window` opened a wgpu instance of its own,
which the `Surface` kept, but not the `Gpu`; the loss was recorded as a message, without why;
destroying a device on purpose, for the tests, needed wgpu itself, through `slime-gfx`'s `wgpu`
feature.

Direct3D 12 constrains the order: a device is unique per adapter in a process, and while a removed
one still exists, creating a device on that adapter fails, so that wgpu would offer only another
adapter, a CPU one if there is no other. Every reference to the lost device, the application's
objects and the engine's own, must go before the new one opens. Microsoft's own templates do it in
two steps, `OnDeviceLost` then `OnDeviceRestored`. Vulkan has no such rule.

## Decision

In `slime-gfx`:

- **A loss says why** (`LossKind`): the driver's (`Driver`), the program's own (`Destroyed`), or
  memory that ran out where no caller could be told (`OutOfMemory`). `GfxError::DeviceLost` carries
  it.
- **A `Gpu` keeps the wgpu instance that opened it**, and opens the device that replaces it
  through the same one: objects of the lost device are then known to wgpu as another device's, and
  their use with the new one is refused, as the bug it is, where a new instance would number its
  objects from zero again and could take an old object for a new one.
- **`Gpu::restore` takes the lost `Gpu`** and opens a device to replace it, without a window.
  **`Surface::restore` takes the lost `Gpu` and the window's surface**, and opens one that can draw
  into the window, creating the window's surface again, of the same size and present mode. Both
  release what they take before the new device opens; the caller drops everything else that holds
  the lost device first. Both refuse with `GfxError::CannotRestore` when the device ran out of
  memory, before anything is released, or when the only adapter left renders on the CPU where the
  lost one did not.
- **`Gpu::lose`** loses a device on purpose, as a driver reset would, and records the loss at
  once: for the tests, and for the tools that check how a program goes on (the game's
  `--lose-device-at` and a button of its dev overlay). The engine's tests no longer reach wgpu to
  destroy a device.

In `slime-app`:

- **`AppConfig::restore_lost_device`**, `false` by default, chooses whether a windowed run replaces
  a lost device. An application that does not choose it stops as ADR-0022 says.
- **Between frames**, before deciding whether the run stops, a run that chose it and finds its
  device lost calls **`App::gpu_lost`**, where the application drops every object of the lost
  device and every clone of its `Gpu`, its loads included, whatever their state; then it takes the
  lost device out of the `Context` and hands it, with the surface, to `Surface::restore`; then it
  puts the new device in the `Context`, with the format of the new surface, and calls
  **`App::gpu_restored`**, where the application creates its objects again.
- **It does not restore** when the run is stopping anyway (an exit request, a termination signal),
  or when the window's surface was lost for good. While it can still restore, a lost device is not
  recorded as a reason to stop.
- **It gives up** once it has opened three devices in a row without a frame shown in between: the
  loss then stops the run, which returns `GfxError::DeviceLost`. When no device replaces the lost
  one, the run has neither device nor surface left: it stops and returns the error of the restore,
  and `App::shutdown` sees no device. An error from `gpu_restored` stops the run as
  `Context::exit_with_error` does.
- Headless runs open no GPU, and have nothing to restore.

## Alternatives considered

- **Keeping the lost device until the new one opens**, in one step, `gpu_restored` alone: simpler
  for applications, and `App::shutdown` would always see a device, but on Direct3D 12 the new
  device could not open on the lost one's adapter: recovery would work on Vulkan only.
- **A new wgpu instance for each new device**: what `Gpu::new` does, but an object of the lost
  device could then pass for one of the new device's that has the same number.
- **Keeping the lost device's surface, configured for the new device**: a surface belongs to the
  device it was configured with on Direct3D 12, and Vulkan refuses a second swap chain for a window
  whose first is not destroyed.
- **Restoring inside the frame where the loss is found**: the application is in the middle of
  `render`, holding the window's image and objects of the lost device; between frames, nothing is
  held.
- **A restore that the application asks for itself**, from a method of the `Context`: every
  application would have to look for the loss at every frame; the run already does, and calls
  the application once.
- **Recovery that the engine does alone, recreating every object**: ADR-0072 rejects it.

## Consequences

- An application that chooses to go on releases its GPU objects in `gpu_lost` and creates them in
  `init` and `gpu_restored`, usually one function that both call. A load that finished with the
  lost device holds objects of it as much as one in progress: both are dropped.
- The windowed lifecycle test checks a device replaced once, a device lost at every other frame and
  replaced each time, and a device lost at every frame until the run gives up, in a headless
  Weston with lavapipe. A real driver reset is checked by hand: on Windows with a forced timeout
  detection and recovery, and on Linux, if the owner chooses, with the GPU's recovery, which needs
  root (M4b.4). A restore that fails in a windowed run is not tested: nothing can make one fail on
  purpose there.
- `Surface::restore` consumes the surface: when no device opens, the window has none left, and the
  run stops.
- Reversing it means removing the setting and the two methods; applications that did not choose it
  see no difference.
