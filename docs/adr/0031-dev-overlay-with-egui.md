# 0031. The dev overlay with egui, and wgpu for the engine's tooling

- Status: Accepted; its rule on the `wgpu` feature is widened to the engine's own tests
  (2026-10-13); amended by [0052](0052-amend-roadmap-before-0-1.md) (when the overlay is removed)
- Date: 2026-10-13

## Context

Stage 3a shows render statistics in a dev overlay (`docs/ROADMAP.md`: "egui, tooling only,
temporary"), and measures the GPU time of a frame. Three rules stand in the way:

- [ADR-0016](0016-gpu-layer-with-wgpu.md) confines wgpu's types to `slime-gfx`, and egui's
  renderer (`egui-wgpu`) draws with wgpu's device, queue and render pass.
- [ADR-0011](0011-windowing-with-winit.md) confines winit's types to `slime-platform`, and egui's
  window integration (`egui-winit`) reads winit's events.
- ADR-0016 asks for no optional GPU feature, and timing the GPU needs timestamp queries, which
  not every adapter offers.

When the stage was planned, the owner validated egui with `egui-wgpu` and no `egui-winit`, a
temporary crate for the overlay, and timestamp queries written by the engine.

## Decision

- **A temporary crate, `slime-devui`,** draws egui over the engine's frames with `egui-wgpu`,
  built without its default features, which would enable every wgpu backend. It is a client of
  the engine, like the examples: no engine crate depends on it. It is deleted when the editor's
  UI framework of Stage 6 replaces it.
- **wgpu for the engine's tooling.** `slime-gfx` gains a feature, `wgpu`, that re-exports wgpu
  and gives the wgpu objects behind the engine's types: the device and the queue of a `Gpu`, the
  pass of a `RenderPass`, the format of a `TextureFormat`. `slime-devui` is its only user; games
  must not enable it, and ADR-0016 stands for everything else: no wgpu type in the engine's
  public API, and `slime-render` written against the engine's types only. The lifetime of the
  engine's render pass is freed from the encoder's (wgpu's `forget_lifetime`), which egui's
  renderer requires; the pass still ends inside the closure that records it.
- **The engine's own tests may enable it too** (added on 2026-10-13 by the owner's decision), as a
  dev-dependency, to do what no engine API offers: the windowed lifecycle test of `slime-app`
  destroys the GPU device in the middle of a run, to check that the run stops as ADR-0022 says.
  Engine crates still never enable it in their dependencies.
- **No `egui-winit`.** `slime-devui` translates the engine's events into egui's: the cursor and
  the mouse buttons in logical points, the wheel, keys and the text they produce, the window's
  focus, the scale factor. What the engine does not report (the state of the modifier keys, the
  clipboard, cursor icons, input methods) does not reach egui either.
- **Optional GPU features, when offered.** The device asks for the optional features the adapter
  offers among those the engine uses, for now timestamp queries, and everything works without
  them: `Gpu::create_timer` gives a `GpuTimer` only when the adapter can time its work, and a
  pass timed with one gets its GPU time back a few frames later, without waiting. The renderer's
  statistics carry it, or `None`. This amends ADR-0016's device configuration.
- **Timestamp queries written by the engine,** not taken from the `wgpu-profiler` crate: one
  query set, a few slots of resolve and readback buffers, and a channel through which wgpu's
  callbacks report the slots that are mapped.
- **One license exception.** egui's default fonts come in the crate `epaint_default_fonts`,
  under the SIL Open Font License 1.1, which the OSI approves, and the Ubuntu Font Licence 1.0,
  which it does not, and which [ADR-0006](0006-dependency-policy.md) therefore refuses.
  `deny.toml` allows both for that one crate: the fonts are tooling, drawn by the overlay, and
  ship with no game. Everything else in ADR-0006 stands.

This amends ADR-0016 (wgpu reaches one more crate, through a feature; optional features are
requested when offered) and ADR-0006 (a license exception limited to one crate).

## Alternatives considered

- **No overlay: statistics in the log.** Enough to read numbers, but the roadmap wants them
  while the scene runs and the camera moves, and the overlay is where the stage's tooling will
  grow until the editor exists.
- **egui's renderer inside `slime-gfx`,** behind a feature, with no wgpu exposed at all. The
  GPU layer would carry a UI library; a feature that gives tooling what it needs keeps the layer
  what it is.
- **`egui-winit` inside `slime-platform`.** winit's types would reach a UI library through the
  platform crate, against ADR-0011, for input that the engine already reports.
- **The `wgpu-profiler` crate.** A dependency for what a hundred lines do; and the renderer
  needs one time per frame, not a hierarchy of scopes.
- **Always asking for timestamp queries.** The device request would fail on adapters without
  them, where the engine should run without GPU times.

## Consequences

- Upgrading wgpu waits for an egui release built against it, as the roadmap says (egui 0.36
  against wgpu 30 today).
- `slime-devui` carries egui's fonts; the exception in `deny.toml` is the first, and names the
  crate it covers, so that a new license anywhere else still fails the checks.
- egui blends its colors in sRGB space and warns once that the engine's framebuffer is sRGB;
  the overlay looks slightly lighter than egui's own demos. Harmless for tooling.
- GPU times are measured on lavapipe, RADV and WARP; the adapters that cannot time their work
  show none.
