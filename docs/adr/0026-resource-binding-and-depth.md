# 0026. Resource binding and depth in `slime-gfx`

- Status: Accepted; amended by [0032](0032-mipmaps-computed-at-texture-creation.md) (mipmaps), by [0033](0033-gpu-layer-for-multi-pass-rendering.md) (binding kinds, sampled depth targets, the pipeline descriptor, depth bias) by [0047](0047-reversed-depth-with-an-infinite-far-plane.md) (depth targets cleared to 0: depth is reversed) and by [0106](0106-the-measured-quick-wins.md) (a uniform buffer's start written alone)
- Date: 2026-10-12

## Context

[ADR-0017](0017-gpu-resource-types.md) gave `slime-gfx` the resource types that the samples'
triangle needs, as engine-owned types holding their wgpu object: render targets, vertex buffers,
shader modules and pipelines. It left the rest to the renderer that would need it, and two
questions open: a generic buffer type was to be considered with the second kind of buffer, and
`bytemuck` with the first vertex structure.

The renderer of Stage 3a (`docs/PLAN.md`) needs, from the GPU layer: indices, so that the
triangles of a mesh share vertices; values that a whole draw reads (a camera's matrices, a
transform, a material), which shaders declare as uniform bindings and read through bind groups; a
depth test, so that nearer surfaces hide farther ones whatever the order they are drawn in; and
face culling. With wgpu, a pipeline is created either with an explicit layout of its bind groups
or with one inferred from its shaders, and bind groups are created from a layout either way. The
crate above `slime-gfx` must create all of this without seeing wgpu's types
([ADR-0016](0016-gpu-layer-with-wgpu.md)).

## Decision

- **Typed buffers.** Each use of a buffer is its own type, created filled by a method of `Gpu`:
  `VertexBuffer` (bytes, as before), `IndexBuffer` (32-bit indices, which knows their count) and
  `UniformBuffer` (bytes laid out as the shader's uniform declares, written again with
  `Gpu::write_uniform_buffer`, always with contents of the size given at creation). There is no
  generic buffer: a type per use keeps a pipeline from binding a uniform buffer as vertices, and
  each type carries only the operations its use has.
- **Bind groups as engine-owned types.** `BindGroupLayout` is created from entries (a binding
  number, the shader stages that read it, the kind of resource: a uniform buffer for now; textures
  and samplers join when the renderer's materials need them, dynamic offsets when its instances
  do), `BindGroup` from a layout and the resources, and `RenderPass::set_bind_group` binds one at
  a `@group` index. A pipeline lists the layouts of the groups its shaders read
  (`RenderPipelineDescriptor::bind_group_layouts`), and its layout is always explicit: a binding
  that the shaders declare and that no layout holds is rejected as `GfxError::Pipeline`, never
  inferred.
- **Depth.** `DepthTarget` is a texture of 32-bit floating-point depth, created at the size of
  the render targets it is used with. `Gpu::render` takes a `RenderPassDescriptor`: the render
  target, whether to clear it (to a color, and the depth target to the farthest depth) or keep
  what both hold, and the optional depth target. A pipeline declares its depth test
  (`DepthState`: the comparison and whether it writes) and its culling (`CullMode`; front faces
  wind counter-clockwise, as in glTF). A pipeline with a depth test draws only in passes that
  have a depth target, and one without only in passes without.
- **Bytes stay bytes.** Vertex and uniform data are given as bytes, which the caller produces
  with `to_le_bytes`: `bytemuck` is still not needed (ADR-0017), and the renderer packs its few
  structures by hand.

This amends ADR-0017: the second kind of buffer brought a second type rather than a generic one,
and pipelines read bind groups, test depth and cull faces.

## Alternatives considered

- **One generic `Buffer` with usage flags.** Fewer types, but a buffer of the wrong usage would be
  caught by wgpu at draw time, as a panic, instead of by the compiler.
- **Pipeline layouts inferred from the shaders** (wgpu's `layout: None`, which the triangle used).
  Nothing to declare, but the caller still needs a layout to create bind groups, which it would
  have to get back from the pipeline, and a typo in a shader would silently change the layout.
- **Dynamic offsets, textures and samplers now.** They have no user yet; each joins the entry
  kinds with its first use.
- **`Depth24Plus`.** Supported everywhere too, but its precision depends on the driver, while
  `Depth32Float` is the same on every driver, which image tests prefer.
- **A stencil buffer.** Nothing needs one.
- **Separate clearing of the color and depth targets.** A pass that keeps the image keeps the
  depth with it; a pass that needs one without the other has no use yet.

## Consequences

- The renderer of Stage 3a is written against engine-owned types only. wgpu's binding model shows
  through the shapes of the types (layouts, groups, `@group` and `@binding` numbers); this is
  deliberate: the layer stays thin.
- Every pipeline declares its bind group layouts, its depth test and its culling; the samples'
  triangle declares none, no depth test and no culling.
- Each new kind of binding is a variant added to `BindingType` and `BindingResource`, which are
  `#[non_exhaustive]`.
- `Gpu::render` takes a descriptor instead of a target and a clear color: every caller changed.
