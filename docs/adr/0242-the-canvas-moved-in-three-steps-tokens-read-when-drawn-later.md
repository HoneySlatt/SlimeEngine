# 0242. The canvas moved in three steps, its prototype on wgpu's GL backend; tokens read when drawn at uwui's first stage

- Status: Accepted; amends [0220](0220-the-roadmap-reshaped-by-the-market.md) (M6d.4 cut in
  three) and [0238](0238-tokens-typed-named-in-code-declared-by-each-crate.md) (a token and less
  motion read when a component is drawn come at uwui's first stage, with live values, not at M6d.4)
- Date: 2026-11-11

## Context

M6d.4's card ([`stage-6d.md`](../stage-6d.md#m6d4--the-canvas-in-uwui-on-wgpu)) holds a prototype
thrown away, which chooses how the canvas hands its items to its shaders on a path that WebGL2
takes; the canvas moved out of `slime-render` into `uwui-render`, on wgpu directly; image tests
on wgpu's GL backend; and a component drawn on the canvas, a knob or a meter, that reads its tokens
and less motion when it draws ([ADR-0238](0238-tokens-typed-named-in-code-declared-by-each-crate.md)).
Stage 6d's rule lets a milestone be cut at its start.

The facts, read in the repository and in wgpu 30.0.1's sources on 2026-11-11:

- The canvas reads each item, nine `vec4`s, from a storage buffer in both of its shaders
  ([ADR-0137](0137-the-canvas.md)). wgpu's limits for WebGL2 allow no storage buffer, 15 values
  between the shader's stages, 16 vertex attributes, a vertex buffer's stride of at most 255 bytes,
  textures of 2048 texels a side and 16 KiB of uniforms.
- Some of what WebGL2 lacks is not a limit but a capacity that wgpu's GL backend does not report:
  a texture viewed in another format, which the canvas uses to blend in sRGB's encoding into an
  sRGB target; and a window's image that can be read or copied, which a blur of what lies behind
  an element needs. A device made with WebGL2's limits on Vulkan does not show these.
- Handing the items to the fragment shader as values of the vertices was measured slower than
  reading them by their index: 101.2 µs against 76.3 µs at 1280 × 800 on the RX 6900 XT
  ([`measurements.md`](../measurements.md)).
- The canvas, its atlas, its GPU timer, its shader's reload and its probe of cost are some
  3,300 lines on `slime-gfx`'s handles. No crate but `slime-gfx` reaches wgpu
  ([`ARCHITECTURE.md`](../ARCHITECTURE.md)): handing the engine's device to uwui is a decision of
  its own.
- The canvas receives colours already resolved, and paint draws again only the elements marked
  as changed. No element draws itself yet; live values, which an element reads as it is drawn
  ([ADR-0205](0205-signals-change-uwuis-interface.md)), are not built. A token read when drawn
  touches the registry of names that M6d.3c removes.

## Decision

**M6d.4 is cut in three**, each a milestone of track B, each closed by its review:

- **M6d.4a, the canvas's prototype and its ADR.** A crate thrown away, on a branch of its own, on
  wgpu alone. Three layouts draw the same screens: the storage buffers of today, the control; small
  instances of a vertex buffer, one format for each kind of item; items in a texture read by their
  index. Each holds a blur of what lies behind, a group drawn apart, a transform per item and a
  path, blends in sRGB's encoding without a second view of its target, and draws one element
  animated over a still screen that grows. Each runs on Vulkan, on a device made with WebGL2's
  limits, and on wgpu's GL backend; each is counted against WebGL2's limits for the drawing that
  uwui's first stage plans, and timed. Its ADR chooses the layout, records the canvas's passes and
  what a target must allow. How a path is drawn stays open for uwui's first stage.
- **M6d.4b1, the canvas on wgpu, as it is.** The decision of how the engine hands its device to
  uwui, in an ADR; the canvas and the atlas moved into `uwui-render`, drawing with a wgpu device,
  its queue and a target, on today's layout, their images unchanged; their tests, probe of cost and
  benchmarks moved; `AGENTS.md`'s rule on wgpu's types; a test that draws a tree of uwui with a bare
  wgpu device; `render.canvas` and `ui.draw.menu` before, at 3840 × 2160 on the RX 6900 XT.
- **M6d.4b2, the layout chosen.** The canvas on M6d.4a's layout; image tests on wgpu's GL
  backend, if the owner accepts the `gles` feature in the workspace, in a job other than
  `check (headless)`; uwui's counters in CI; `render.canvas` and `ui.draw.menu` after.

**The prototype runs on wgpu's GL backend.** Its branch enables wgpu's `gles` feature, the owner's
choice; the workspace's own use of it is put to the owner at M6d.4b2.

**A token, and less motion, read when a component is drawn come at uwui's first stage**, with live
values and the elements that draw themselves, as one way of reading a value when drawing, designed
once with a component as its first reader. M6d.4 does not build it.

## Alternatives considered

- **M6d.4 whole**: one session and one review for a prototype, a move of 3,300 lines and a change
  of layout; the prototype's verdict changes the shape of what follows.
- **In two, the prototype then the rest**: one review fewer, but the move and the change of layout
  together, so that an image that changes is hard to trace.
- **The prototype on Vulkan alone, with WebGL2's limits**: no new feature, but what GL lacks
  counted from wgpu's sources, and a verdict provisional until the web's trial.
- **Tokens read when drawn in a milestone M6d.4c**, on track A after M6d.3c, a meter of boxes in an
  example its first reader: the promise to the owner's audio application kept within Stage 6d, but
  the way of reading a value when drawing designed before live values, and likely again with them.

## Consequences

- [`stage-6d.md`](../stage-6d.md) holds M6d.4a, M6d.4b1 and M6d.4b2 in place of M6d.4's single
  card; [`PLAN.md`](../PLAN.md) holds M6d.4a while it is in progress.
- uwui's roadmap gains, at its first stage, a token and less motion read when a component is
  drawn; the item of [`debt.md`](../debt.md) on it is followed there.
- The item of [`debt.md`](../debt.md) on the device's first allocations is followed at M6d.7,
  where uwui's desktop host opens a device of its own; M6d.4 opens one in tests only.
- The prototype's branch, `m6d4a-prototype`, stays while the measurements link to its code, as
  `m6d1b-bench` does; its deletion waits for the owner's go.
