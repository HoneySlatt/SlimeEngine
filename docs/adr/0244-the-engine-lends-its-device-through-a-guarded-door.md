# 0244. The engine lends its device to uwui through a guarded door, and draws uwui from `slime-app`

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md) and
  [0031](0031-dev-overlay-with-egui.md) (wgpu reaches `uwui-render`, and `slime-app` through a
  door of `slime-gfx`), [0181](0181-uwuis-scope.md) (`uwui-render` records into its host's
  encoder) and [0242](0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md)
  (M6d.4b1 cut in two)
- Date: 2026-11-12

## Context

M6d.4b1 moves the interface's *canvas*, which draws boxes, shadows, gradients and images, and the
glyphs' atlas out of `slime-render` into `uwui-render`, on wgpu directly, its images unchanged
([ADR-0242](0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md)).
[ADR-0181](0181-uwuis-scope.md) has `uwui-render` take its host's device, its queue and a
target. Today no crate but `slime-gfx` reaches wgpu; its `wgpu` feature gives the device to the
engine's tests alone ([ADR-0031](0031-dev-overlay-with-egui.md)).

`slime-gfx` does more than open the device. Every creation runs in an error scope: memory that
runs out is `GfxError::OutOfMemory` ([ADR-0044](0044-running-out-of-gpu-memory.md)), a lost device
`GfxError::DeviceLost`, from which programs recover
([ADR-0022](0022-losing-the-gpu-device.md)), and any other error of wgpu panics, a bug
([ADR-0016](0016-gpu-layer-with-wgpu.md)). An object used with another device panics
([ADR-0157](0157-dialogs-the-cursor-held-and-the-gpus-identity.md)). Submissions take a lock,
and a window's image that nothing drew into gets an empty submission.

Read in wgpu 30.0.1's sources on 2026-11-12:

- A device has one callback for its loss and one handler of the errors that no scope catches;
  setting either again replaces it. wgpu cannot be asked whether a device is lost.
- On a lost device, creations return invalid objects without a word, and error scopes answer
  nothing. Some of wgpu's helpers panic then: `create_buffer_init`, and a `poll` that finds a lost
  Vulkan device.
- Error scopes belong to a thread; a scope's guard cannot leave it.
- Every object holds its device: the canvas's objects must be dropped before `Gpu::restore` opens a
  new device, which Direct3D 12 needs, as today's renderer must be.
- A texture does not say which other formats it can be viewed in; the adapter, not the device,
  gives the downlevel flags that M6d.4b2 reads
  ([ADR-0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md)).
- egui-wgpu takes a device and a queue, records into its caller's encoder and pass, and handles
  neither memory that runs out nor a lost device.

A `challenger` review found that moving the canvas and changing its GPU layer together would hide
which of the two changed an image or a time: each bind group pays a probe of the device today,
and wgpu directly would not.

## Decision

**M6d.4b1 is cut in two**, the owner's choice of 2026-11-12:

- **M6d.4b1a, the canvas moved.** The canvas, the atlas, their tests and benchmarks move into
  `uwui-render`, still on `slime-gfx`'s handles; `slime-render` holds no canvas; the callers reach
  it through `slime-app`'s bridge, in the form they keep after M6d.4b1b. Images and figures
  compare with today's.
- **M6d.4b1b, the canvas on wgpu.** `uwui-render` on wgpu, through the door below; a test draws a
  tree of uwui with a bare wgpu device.

**A guarded door.** `slime-gfx` lends its device through one method, behind its `wgpu` feature:
`Gpu::with_wgpu`, which runs a closure under the engine's policy and gives it the device, the
queue, the adapter's downlevel capabilities, an encoder, and the wgpu objects behind the engine's
targets and textures, each checked to be of this device. The door:

- checks the loss first: a lost device runs nothing and returns `GfxError::DeviceLost`;
- runs the closure inside scopes of memory, validation and internal errors, on the calling thread;
- submits the encoder under the engine's lock, and marks a window's image drawn into;
- then asks whether the device is still valid. A loss wins: `DeviceLost`. Then memory that ran out:
  `OutOfMemory`. Any other error caught is a bug, and panics with wgpu's message, as everywhere in
  the engine.

**`uwui-render` knows no engine crate.** It takes a `wgpu::Device`, a `wgpu::Queue`, an encoder
that it records its passes into and that its host submits, as egui-wgpu does, a target's texture,
whose view without sRGB encoding it makes (what a target must allow,
[ADR-0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md)), and images as
texture views. It never sets a callback or a handler on a device that it is given. It uses none of
wgpu's helpers that panic on a lost device, and polls no device. A shader that it makes, its
canvas's shader reloaded included, is checked in scopes of its own: a broken one is an error that
it returns, and it keeps the pipeline that it had. It reports memory that runs out, as its own
error, which the bridge carries with `slime-gfx`'s. It keeps no clone of a device, nor a view of a
target, from one draw to the next.

**The bridge is a module of `slime-app`**, the engine's host of uwui, which
[M6d.5](../stage-6d.md#m6d5--the-hosts-api) shapes: it takes a `Gpu`, a target and the engine's
images, the textures and the targets that views drew, and draws a display list with `uwui-render`
inside the door. `slime-devui`, the editor, the examples and the benchmarks draw through it. Only
`slime-app` and the engine's tests open the door; `scripts/check.sh` checks it.

## Alternatives considered

- **M6d.4b1 whole**: one review fewer, but the move and the change of GPU layer in one diff, as
  ADR-0242 refused for the move and the change of layout.
- **The device, the queue and the textures given as they are**, as egui-wgpu takes them: simplest,
  but every caller then handles a lost device and memory that runs out, wgpu's helpers panic on
  them, and an object of another device is no longer caught.
- **`uwui-render` submitting its own passes**, as today's canvas does: simpler now, but a change of
  uwui's public API after it leaves the repository, and a submission that the door would not see.
- **The bridge in a crate of its own**: a clear role, `slime-app` unchanged, but a crate that M6d.5
  would likely fold into `slime-app`.
- **Each caller through the door**: no bridge, but the same code six times, and wgpu in six crates.
- **`uwui-render` kept on `slime-gfx`**, or a GPU trait in `uwui-render` that the engine
  implements: the first keeps uwui from leaving; the second is an abstraction with one implementer,
  which ADR-0181 refused.
- **The bridge in `slime-render`**: the renderer would depend on uwui.

## Consequences

- [`stage-6d.md`](../stage-6d.md) holds M6d.4b1a and M6d.4b1b in place of M6d.4b1's card.
- `uwui-render` depends on `slime-color` (M6d.4b1a), then on wgpu without `slime-gfx`
  (M6d.4b1b); `slime-app` on `uwui-render` and, at M6d.4b1b, on `slime-gfx`'s `wgpu` feature;
  `slime-devui` on `slime-app`. Every game compiles `uwui-render`, which `slime-app`'s bridge
  needs. The owner accepted these links on 2026-11-12.
- The rule on wgpu's types names `slime-gfx`, `uwui-render`, `slime-app`'s bridge and the engine's
  tests ([`AGENTS.md`](../../AGENTS.md)); a game no longer controls whether `slime-gfx`'s `wgpu`
  feature is on, so the check of `scripts/check.sh`, not the feature, keeps the door to
  `slime-app`.
- `uwui-render`'s figures change at M6d.4b1b alone: one probe of the device for each draw, where
  each bind group pays one today.
- The probe of what the renderer costs an interface, against the canvas alone
  ([ADR-0180](0180-one-canvas-and-the-frameworks-foundations.md)), moves to the benchmarks, the one
  client that holds both. The canvas's shader leaves the renderer's shader sources: a game that
  watches its shaders no longer reloads it; `uwui-render` reloads it on its own call.
- On uwui's own desktop host ([M6d.7](../stage-6d.md#m6d7--the-desktop-host-its-window-and-its-frames)),
  which opens a bare device, a creation on a lost device seems to succeed: that host handles the
  loss.
