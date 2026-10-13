# slime-render

The renderer: draws meshes with their materials, seen from a camera, into render targets, through
the GPU layer (`slime-gfx`).

## Status

Partial: the renderer of Stage 3b, physically based by the end of the stage (`docs/PLAN.md`). It
draws instances of meshes from a perspective camera, with a depth test and back-face culling,
each with a material of one base color, which a texture may modulate, lit by one directional
light and an ambient light. It draws the scene in high dynamic range, where light may exceed 1,
then maps it to the output with an exposure and a tone-mapping operator, Khronos PBR Neutral
(`docs/adr/0034-hdr-rendering-and-tone-mapping.md`). It reports what each frame cost on the CPU
and, on adapters that can time their work, what each of its two passes cost on the GPU.
Surfaces are opaque: the alpha of a base color and of its texture is not used, and a surface
leaves an alpha of 1 in the target. Instances are drawn in the order of the scene, neither sorted
nor batched. An instance whose transform mirrors it (a negative determinant, such as a scale of
−1 along one axis) is drawn inside out: its front faces are culled and its back faces drawn,
where glTF keeps the front faces of a mirrored node.

## Role

- Owns what drawing a scene needs beyond the GPU layer: the vertex layout and the shaders, the
  camera's matrices, the values the shaders read (per frame, per material, per instance) and
  their layout, the high-dynamic-range and depth targets, and the buffer of the instances'
  values.
- Is built on `slime-gfx`'s engine-owned types only: it does not use wgpu
  (`docs/adr/0027-the-render-crate.md`).
- Draws what it is given: a flat list of instances that the application builds for each frame.
  It keeps no scene between frames and has no scene graph; the world model of Stage 5 will produce
  such lists.

## Entry points

Drawing a red triangle into an offscreen image:

```rust
use slime_gfx::{Color, Gpu, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, MaterialData, MeshData, RenderError, Renderer, Scene,
};

fn main() -> Result<(), RenderError> {
    let gpu = Gpu::new()?;
    let target = gpu.create_render_target("image", 64, 64, TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format())?;
    let triangle = renderer.create_mesh(&gpu, "triangle", &MeshData {
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]],
        indices: vec![0, 1, 2],
    })?;
    let red = renderer.create_material(
        &gpu,
        "red",
        &MaterialData { base_color: [1.0, 0.0, 0.0, 1.0], ..MaterialData::default() },
    );

    let camera = Camera { eye: Vec3::new(0.0, 0.0, 3.0), ..Camera::default() };
    // A light shining straight at the triangle, which faces +Z, and no ambient light.
    let light = DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        ambient: Vec3::ZERO,
    };
    let instances = [Instance { mesh: &triangle, material: &red, transform: Mat4::IDENTITY }];
    let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let scene = Scene { camera, light, clear: black, exposure: 1.0, instances: &instances };
    let stats = renderer.render(&gpu, &target, &scene);
    assert_eq!((stats.draw_calls, stats.triangles), (1, 1));

    let image = gpu.read_render_target(&target)?;
    let center = (32 * 64 + 32) * 4;
    // Tone mapping compresses light towards 1 and turns it towards white: a red of 1 shows as
    // about (241, 33, 33), not (255, 0, 0).
    let [red, green, blue] = [0, 1, 2].map(|channel| image.pixels[center + channel]);
    assert!(red.abs_diff(241) <= 1 && green.abs_diff(33) <= 1 && blue.abs_diff(33) <= 1);
    Ok(())
}
```

- `Renderer::new(gpu, format)` creates a renderer for output targets of one `TextureFormat`, the
  window's for a windowed application (`Context::surface_format` in `slime-app`). An sRGB format
  encodes the tone-mapped light for display; `Rgba16Float` holds the tone-mapped values
  themselves; `Rgba8Unorm` stores them unencoded, which displays too dark. It compiles the two
  shaders and builds the two pipelines; the high-dynamic-range target and the depth target are
  created at the size of the first output drawn into, and again when the size changes.
- `Renderer::create_mesh(gpu, label, &MeshData)` uploads positions, normals, texture coordinates
  and indices as a `Mesh`; data that cannot be drawn (arrays of different lengths, no vertex, no
  triangle, a number of indices that is not a multiple of three, an index that names no vertex,
  more vertices or indices than a GPU buffer holds) is returned as `RenderError::InvalidMesh`.
  `Renderer::create_material(gpu, label, &MaterialData)` creates a `Material`: a base color, and
  a `SampledTexture` (a `slime_gfx::Texture` and the `Sampler` that reads it, created with
  `Gpu::create_texture` and `Gpu::create_sampler`) multiplied with it where the material has one,
  read at the texture coordinates of the mesh, whose (0, 0) is the texture's top left. Both are
  cheap to clone and shared by their clones.
- `Renderer::render(gpu, target, &Scene)` draws a frame in two passes. The scene pass clears a
  target of half-precision floats to the scene's background and draws its instances (`Instance`:
  a mesh, a material, a transform from the mesh's space to world space) from its `Camera`, lit by
  its `DirectionalLight`, with the depth test, so that nearer surfaces hide farther ones whatever
  the order of the instances. The tone-mapping pass multiplies that light by the scene's
  `exposure` (1 leaves it as it is, 2 is one photographic stop brighter) and maps it to the
  output's range with Khronos PBR Neutral: colors whose channels lie between 0.08 and 0.8 come out
  lowered by 0.04, darker ones less, and brighter ones are compressed towards 1 and turned towards
  white, so that a white surface under a light of 1 shows at 0.87 (sRGB 240). It returns
  `RenderStats`: the scene pass's draw calls and triangles, the CPU time of the call, and the GPU
  time of each pass (`scene_gpu_time`, `tone_mapping_gpu_time`), measured with a
  `slime_gfx::GpuTimer` each on adapters that can time their work and read back a few frames
  later (`None` otherwise, and until the first time comes back); the two can come from different
  frames.
- `Camera`: an eye, a target, an up direction, a vertical field of view in radians, and the near
  and far distances; `view`, `projection(aspect)` and `view_projection(aspect)` give its matrices.
- `DirectionalLight`: the direction the light travels in, its color and the ambient light's
  color, all in world space and linear. A surface is lit by the cosine of the angle between its
  normal and the direction to the light (Lambert), plus the ambient light, times its material's
  base color. Normals follow the rotation of their instance and stay perpendicular to surfaces
  scaled unevenly, even flattened along one axis.
- Conventions: world space is right-handed with +Y up, as in glTF; the camera looks down its −Z
  axis; front faces wind counter-clockwise; depth runs from 0 at the near plane to 1 at the far
  plane; colors in materials and clear colors are linear; light before tone mapping is not
  negative and may exceed 1, up to 65 504, the largest value of the high-dynamic-range target; an
  sRGB output encodes the tone-mapped values.
- `slime_render::glam` re-exports the math crate the API uses (`Mat4`, `Vec3`, `Quat`), so that
  callers use the same version.
- `RenderError`: an invalid mesh, or a failure of the GPU layer (`GfxError` as its source).

## Dependencies

- `slime-gfx`: the GPU objects and the render passes.
- `glam`: vectors, matrices and quaternions.
- `thiserror`: derives `RenderError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Use wgpu: it draws through `slime-gfx`'s types only.
- Depend on `slime-platform` or `slime-app`: it draws into any render target, with or without a
  window.
- Keep a scene between frames: it draws what it is given.
