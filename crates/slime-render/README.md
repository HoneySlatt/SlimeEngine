# slime-render

The renderer: draws meshes with their materials, seen from a camera, into render targets, through
the GPU layer (`slime-gfx`).

## Status

Partial: the first renderer of Stage 3a (`docs/PLAN.md`). It draws instances of meshes from a
perspective camera, with a depth test and back-face culling, each with a material of one color,
lit by one directional light and an ambient light. Textures and samplers, and statistics beyond
counts, come with the later milestones of the stage.

## Role

- Owns what drawing a scene needs beyond the GPU layer: the vertex layout and the shader, the
  camera's matrices, the values the shaders read (per frame, per material, per instance) and
  their layout, the depth target, and the buffer of the instances' values.
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
    let red = renderer.create_material(&gpu, "red", &MaterialData { base_color: [1.0, 0.0, 0.0, 1.0] });

    let camera = Camera { eye: Vec3::new(0.0, 0.0, 3.0), ..Camera::default() };
    // A light shining straight at the triangle, which faces +Z, and no ambient light.
    let light = DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        ambient: Vec3::ZERO,
    };
    let instances = [Instance { mesh: &triangle, material: &red, transform: Mat4::IDENTITY }];
    let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let scene = Scene { camera, light, clear: black, instances: &instances };
    let stats = renderer.render(&gpu, &target, &scene);
    assert_eq!((stats.draw_calls, stats.triangles), (1, 1));

    let image = gpu.read_render_target(&target)?;
    let center = (32 * 64 + 32) * 4;
    assert_eq!(&image.pixels[center..center + 3], [255, 0, 0]);
    Ok(())
}
```

- `Renderer::new(gpu, format)` creates a renderer for targets of one `TextureFormat`, the window's
  for a windowed application (`Context::surface_format` in `slime-app`). It compiles the shader
  and builds the pipeline; the depth target is created at the size of the first target drawn into,
  and again when the size changes.
- `Renderer::create_mesh(gpu, label, &MeshData)` uploads positions, normals, texture coordinates
  and indices as a `Mesh`; data that cannot be drawn (arrays of different lengths, no triangle, an
  index that names no vertex) is returned as `RenderError::InvalidMesh`.
  `Renderer::create_material(gpu, label, &MaterialData)` creates a `Material` of one base color.
  Both are cheap to clone and shared by their clones.
- `Renderer::render(gpu, target, &Scene)` clears the target to the scene's color and draws its
  instances (`Instance`: a mesh, a material, a transform from the mesh's space to world space)
  from its `Camera`, lit by its `DirectionalLight`, with the depth test, so that nearer surfaces
  hide farther ones whatever the order of the instances. It returns `RenderStats`: draw calls and
  triangles.
- `Camera`: an eye, a target, an up direction, a vertical field of view in radians, and the near
  and far distances; `view`, `projection(aspect)` and `view_projection(aspect)` give its matrices.
- `DirectionalLight`: the direction the light travels in, its color and the ambient light's
  color, all in world space and linear. A surface is lit by the cosine of the angle between its
  normal and the direction to the light (Lambert), plus the ambient light, times its material's
  base color. Normals follow the rotation of their instance and stay perpendicular to surfaces
  scaled unevenly.
- Conventions: world space is right-handed with +Y up, as in glTF; the camera looks down its −Z
  axis; front faces wind counter-clockwise; depth runs from 0 at the near plane to 1 at the far
  plane; colors in materials and clear colors are linear, and the sRGB target encodes them.
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
