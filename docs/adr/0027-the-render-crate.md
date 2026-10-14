# 0027. The render crate, what it draws, and glam

- Status: Accepted; amended by [0034](0034-hdr-rendering-and-tone-mapping.md) (two passes, the scene in HDR, the exposure, tone mapping), by [0035](0035-physically-based-materials.md) (three scene pipelines chosen per draw, physically based materials, tangents), by [0037](0037-lights-in-physical-units-and-pre-exposure.md) (a scene's lights and ambient light, light in physical units, lengths in metres), by [0038](0038-shadow-mapping.md) (a shadow pass before the scene pass, the shadow map at a fourth bind group, meshes keep their box), by [0039](0039-image-based-lighting-computed-on-the-cpu.md) (a scene's environment instead of its ambient light and clear color; a sky pass after the scene pass) and by [0040](0040-bloom.md) (the bloom's passes; `Scene::bloom`)
- Date: 2026-10-13

## Context

Stage 3a builds the first renderer (`docs/PLAN.md`): meshes, materials, a camera, a light,
textures and statistics, drawn into a window or offscreen. The roadmap lists a `render` crate
above `gfx` and allows it to use wgpu's types; [ADR-0016](0016-gpu-layer-with-wgpu.md) confines
them to `slime-gfx`, and [ADR-0026](0026-resource-binding-and-depth.md) gave `slime-gfx` the
resources a renderer needs, so that none has to leak. The renderer needs vectors, matrices and
quaternions, which the standard library does not provide, and conventions for axes, winding,
depth and colors. It also draws a scene before the engine has a world model (Stage 5), a question
that [ADR-0002](0002-amend-roadmap-stages-0-to-3.md) left to this stage.

## Decision

- **Crate.** `slime-render` holds the renderer: the vertex layout and the shader, the camera's
  matrices, the values the shaders read and their layout, the depth target, the buffer of the
  instances' values and, as the stage goes on, lighting, textures and statistics. It depends on
  `slime-gfx` and on no other engine crate, so that it draws into any render target, with or
  without a window; `slime-app` does not depend on it: an application creates its renderer in
  `init` from the GPU of its context. It does not use wgpu: it is written against `slime-gfx`'s
  engine-owned types, which makes it their second concrete use and keeps ADR-0016 whole. The dev
  overlay of the stage will be the one exception, through a feature of `slime-gfx`, by a later ADR.
- **What it draws.** A `Scene` is a camera, a clear color and a flat list of `Instance`s, each a
  `Mesh`, a `Material` and a transform, built by the application for each frame and forgotten
  after it: the renderer keeps no scene between frames and has no scene graph. The world model of
  Stage 5 will produce such lists; until then the samples and the temporary glTF loader build
  them, the loader flattening the file's hierarchy.
- **How it draws.** One pipeline, with the depth test and back-face culling. The values the
  shaders read sit in three bind groups: the frame (the camera's view-projection matrix) at group
  0, the material at group 1, and the instance (its transform) at group 2, in one uniform buffer
  with a slot per instance, bound through a dynamic offset. One draw call per instance, in the
  order given; `RenderStats` counts draw calls and triangles.
- **Conventions.** World space is right-handed with +Y up, as in glTF; the camera looks down its
  −Z axis; front faces wind counter-clockwise; depth runs from 0 at the near plane to 1 at the
  far plane; the colors of materials and clear colors are linear, and the sRGB render target
  encodes them.
- **Math.** `glam` provides vectors, matrices and quaternions. Its types appear in
  `slime-render`'s API (`Camera`, `Instance::transform`), the exception that ADR-0006 allows for a
  dependency whose purpose it is, and the crate re-exports it so that callers use the same
  version. Its camera module gives the view and projection matrices in WebGPU's convention, which
  wgpu follows on every graphics API. Bytes for the GPU are packed from its arrays with
  `to_le_bytes`, as ADR-0017 and ADR-0026 chose: no `bytemuck`.
- **Errors.** Mesh data that cannot be drawn is returned as `RenderError::InvalidMesh`, since it
  may come from a file; a failure of the GPU layer is returned as `RenderError::Gfx`; a misuse of
  the renderer, such as a target of another format, is a bug and panics, as in `slime-gfx`.

## Alternatives considered

- **The renderer inside `slime-gfx`.** One crate fewer, but the GPU layer would own meshes,
  materials and cameras, which are not its boundary, and egui, the tooling of the stage, would
  end up in the GPU layer too.
- **wgpu in `slime-render`**, as the roadmap allows. Nothing in a first renderer needs it, and
  using `slime-gfx`'s types instead tests that boundary: whatever the renderer lacks is added to
  `slime-gfx` as a type of its own.
- **A scene kept by the renderer** (objects added and removed, transforms updated). A second
  scene model next to the world of Stage 5, to delete then.
- **One uniform buffer per instance.** Simple, but a bind group per instance, created again at
  every change; dynamic offsets cost one buffer and one group.
- **Instancing through a storage buffer and the instance index.** Fewer draw calls when one mesh
  is drawn many times; no scene draws one mesh many times yet.
- **`nalgebra`.** More general (any dimension), heavier to compile, and less used by Rust
  graphics code, where `glam` is the usual choice.
- **Own vectors and matrices.** Hundreds of lines outside the engine's purpose (roadmap,
  principle 7).

## Consequences

- Games and the examples build their `Scene` each frame from their own data; nothing persists in
  the renderer between frames but GPU objects.
- `glam` becomes part of the API that games see; replacing it would change every signature that
  takes a matrix or a vector.
- Draw calls are neither sorted nor batched; the statistics of the stage will say when that
  matters.
- The rule that `slime-render` uses no wgpu is checked by review, since Cargo cannot forbid a
  dependency.
