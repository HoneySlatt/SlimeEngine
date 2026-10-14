# slime-render

The renderer: draws meshes with their materials, seen from a camera, into render targets, through
the GPU layer (`slime-gfx`).

## Status

Partial: the renderer of Stage 3b, physically based by the end of the stage (`docs/PLAN.md`). It
draws instances of meshes from a perspective camera, with a depth test, each with a physically based
material, glTF's metallic-roughness model (a base color, metallic and roughness, normal, occlusion
and emissive textures; opaque or masked; single- or double-sided), lit by up to 16 directional,
point and spot lights in physical units, lux and candelas, as glTF's `KHR_lights_punctual` defines
them (`docs/adr/0035-physically-based-materials.md`,
`docs/adr/0037-lights-in-physical-units-and-pre-exposure.md`), and by their surroundings: a
procedural daytime sky with a sun, computed on the CPU into an environment that surfaces reflect as
their material does (image-based lighting) and that is drawn behind them, with the sun's disc
(`docs/adr/0039-image-based-lighting-computed-on-the-cpu.md`). The first directional light that
casts shadows casts them, through a shadow map that covers the instances, its edges blended over
three of the map's texels (`docs/adr/0038-shadow-mapping.md`); the sky's light is not shadowed. It
draws the scene in high dynamic range, where light may exceed 1, multiplied by an exposure before
it is written, then maps it to the output with a tone-mapping operator, Khronos PBR Neutral
(`docs/adr/0034-hdr-rendering-and-tone-mapping.md`,
`docs/adr/0037-lights-in-physical-units-and-pre-exposure.md`), after mixing in a bloom: a share of
the light spread around where it lands, as a lens spreads it, which makes the sun's disc glow
(`docs/adr/0040-bloom.md`). It reports what each frame cost on the CPU and, on adapters that can
time their work, what each of its passes cost on the GPU, the bloom's counted together.
Blended transparency is not drawn, and a surface leaves an alpha of 1 in the target. Instances are
drawn in the order of the scene, neither sorted nor batched. An instance whose transform mirrors it
(a negative determinant, such as a scale of −1 along one axis) keeps its front faces, as glTF
does.

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
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, MaterialData, MeshData, RenderError, Renderer, Scene,
};

use std::f32::consts::PI;

fn main() -> Result<(), RenderError> {
    let gpu = Gpu::new()?;
    let target = gpu.create_render_target("image", 64, 64, TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format())?;
    let triangle = renderer.create_mesh(&gpu, "triangle", &MeshData {
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]],
        // Along +X, where u grows.
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
    })?;
    // The default material is a white matte dielectric.
    let red = renderer.create_material(
        &gpu,
        "red",
        &MaterialData { base_color: [1.0, 0.0, 0.0, 1.0], ..MaterialData::default() },
    );

    let camera = Camera { eye: Vec3::new(0.0, 0.0, 3.0), ..Camera::default() };
    // The sun of a clear day, 100 000 lux, shining straight at the triangle, which faces +Z, and no
    // environment (a black sky): the matte red surface reflects 0.97 / π of it in red and
    // 0.01 / π in green and blue, about 30 900 and 320 nits, which an exposure of π / 100 000
    // brings to 0.97 and 0.01.
    let sun = Light::Directional(DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        illuminance: 100_000.0,
        casts_shadows: true,
    });
    let instances = [Instance { mesh: &triangle, material: &red, transform: Mat4::IDENTITY }];
    let scene = Scene {
        camera,
        lights: &[sun],
        environment: None,
        exposure: PI / 100_000.0,
        // No bloom, which would spread a little of the triangle's light around its edges.
        bloom: 0.0,
        instances: &instances,
    };
    let stats = renderer.render(&gpu, &target, &scene);
    assert_eq!((stats.draw_calls, stats.triangles), (1, 1));

    let image = gpu.read_render_target(&target)?;
    let center = (32 * 64 + 32) * 4;
    // Tone mapping compresses light towards 1 and turns it towards white: (0.97, 0.01, 0.01)
    // shows as about (240, 29, 29), not (255, 0, 0).
    let [red, green, blue] = [0, 1, 2].map(|channel| image.pixels[center + channel]);
    assert!(red.abs_diff(240) <= 1 && green.abs_diff(29) <= 1 && blue.abs_diff(29) <= 1);
    Ok(())
}
```

- `Renderer::new(gpu, format)` creates a renderer for output targets of one `TextureFormat`, the
  window's for a windowed application (`Context::surface_format` in `slime-app`). An sRGB format
  encodes the tone-mapped light for display; `Rgba16Float` holds the tone-mapped values themselves;
  `Rgba8Unorm` stores them unencoded, which displays too dark. It compiles the three shaders, builds
  the ten pipelines (two for the shadow pass, the second for masked materials, three for the
  scene, which differ by the faces they cull, the sky's, the bloom's two and the tone-mapping
  one), uploads the table of the BRDF's scale and bias, which the first renderer of a process
  computes on the CPU, and creates the shadow map, 2048 × 2048 depths (16 MiB); the
  high-dynamic-range target, the depth target and the bloom's levels (4.7 MiB at 1280 × 720) are
  created at the size of the first output drawn into, and again when the size changes.
- `Renderer::create_environment(gpu, &Sky)` computes a sky's `Environment` on the CPU, on every
  core, and uploads it: a cube map of 128 × 128 texels per face and four levels prefiltered for
  roughnesses 0.25 to 1, in half-precision floats (1.0 MiB), the last also the light a matte
  surface diffuses. It takes tens of milliseconds (a sky of one radiance without a sun almost
  none): create one per sky, not per frame. A `Sky` holds the radiances of the zenith, the horizon
  and the ground, in nits, at most 65 504 each (more panics), and an optional sun, a
  `DirectionalLight`, whose glow the sky holds and whose disc the sky pass draws; the sun's light
  itself reaches surfaces through the same light in the scene's `lights`, which casts shadows. An
  environment is shared by its clones and not tied to the renderer that made it.
- `Renderer::create_mesh(gpu, label, &MeshData)` uploads positions, normals, texture coordinates,
  tangents and indices as a `Mesh`, 48 bytes per vertex; data that cannot be drawn (arrays of
  different lengths, no vertex, no triangle, a number of indices that is not a multiple of three, an
  index that names no vertex, more vertices or indices than a GPU buffer holds) is returned as
  `RenderError::InvalidMesh`. `Renderer::create_material(gpu, label, &MaterialData)` creates a
  `Material` from glTF's metallic-roughness model: a base color, metallic and roughness factors, a
  normal scale, an occlusion strength, an emission and its strength, an `AlphaMode` (`Opaque`, or
  `Mask` with a cutoff below which nothing is drawn) and double-sidedness, and five texture slots,
  each an optional `SampledTexture` (a `slime_gfx::Texture` and the `Sampler` that reads it, created
  with `Gpu::create_texture` and `Gpu::create_sampler`) read at the texture coordinates of the mesh,
  whose (0, 0) is the texture's top left. The base color and emissive textures hold colors in an
  sRGB format; the metallic-roughness (green: roughness, blue: metallic), normal and occlusion (red)
  textures hold data in `Rgba8Unorm`; a texture in the other kind of format panics. A slot left
  empty reads one white pixel, so that its factor counts alone. `MaterialData::default()` is an
  opaque, single-sided, white matte dielectric. Meshes and materials are cheap to clone and shared
  by their clones.
- `Renderer::render(gpu, target, &Scene)` draws a frame in passes. The shadow pass draws the depths
  of every instance, all their faces but what masked materials discard, as the first directional
  light that casts shadows sees them, into the shadow map, through an orthographic projection that
  covers the boxes around the instances' meshes; it is skipped when no directional light casts
  shadows. The scene pass draws its instances (`Instance`: a mesh, a material, a transform from the
  mesh's space to world space) into a target of half-precision floats from its `Camera`, lit by its
  `lights` and its `environment` (none is a black sky), with the depth test, so that nearer surfaces
  hide farther ones whatever the order of the instances, the shadowed light only where the shadow
  map holds nothing nearer to it, compared at 3 × 3 points one texel apart, so that a shadow's edge
  fades over three texels; it panics on a scene of more than `MAX_LIGHTS` lights. The sky pass then
  draws the environment, and its sun's disc, where no surface was drawn. The light of both is
  multiplied by the scene's `exposure` (1 leaves it as it is, 2 is one photographic stop brighter)
  before it is written, divided by 64, so that the target's half-precision floats, whose largest
  value is 65 504, hold exposed light up to 64 × 65 504; brighter light is written as that largest
  value. Unless the scene's `bloom` is 0, the bloom's passes then filter that light down into a
  chain of levels, each half the size of the one before, to a side of 8 texels (six levels at 1280
  × 720, seven at 1920 × 1080, none below 15 × 15), and average them back up into a blurred copy,
  every level with the same weight. The tone-mapping pass mixes the blurred copy in by `bloom`, the
  share of each pixel's light spread around it (0.04 in the samples; a uniform image is unchanged,
  light is moved and never added), then maps that light to the output's range with Khronos PBR
  Neutral: colors whose channels lie between 0.08 and 0.8 come out lowered by 0.04, darker ones
  less, and brighter ones are compressed towards 1 and turned towards white, so that a surface
  reflecting a light of 1 shows at 0.87 (sRGB 240). It returns `RenderStats`: the scene pass's draw
  calls and triangles, the shadow pass's draw calls (`shadow_draw_calls`), the CPU time of the call,
  and the GPU time of each pass (`shadow_gpu_time`, `scene_gpu_time`, `sky_gpu_time`,
  `tone_mapping_gpu_time`, and `bloom_gpu_time` for the bloom's passes together), measured with a
  `slime_gfx::GpuTimer` per pass on adapters that can time their work and read back a few frames
  later (`None` otherwise, and until the first time comes back); they can come from different
  frames.
- `Camera`: an eye, a target, an up direction, a vertical field of view in radians, and the near
  and far distances; `view`, `projection(aspect)` and `view_projection(aspect)` give its matrices.
- `Light`: one of the punctual lights of glTF's `KHR_lights_punctual`, each with a linear color and
  a strength in its physical unit, and glTF's defaults. A `DirectionalLight` comes from one
  direction everywhere with an illuminance in lux; it casts shadows unless its `casts_shadows` is
  `false`, and only the first of the scene's directional lights that casts shadows casts them. Point
  and spot lights cast none. A `PointLight` shines from a position with an intensity in candelas: a
  surface facing it d metres away receives intensity / d² lux, brought smoothly to zero at its
  optional range, and nearer than 1 cm what it would receive at 1 cm. A `SpotLight` is a point light
  with a direction, full within its inner cone angle, dark beyond its outer one, and fading in
  between with the square of a ramp linear in the cosine. Surfaces reflect each light with glTF's
  metallic-roughness model, so that a white matte surface facing a light and the camera reflects
  0.97/π of the illuminance it receives, and reflect their environment as the Khronos glTF Sample
  Viewer does (the split sum, with the light that rough surfaces scatter more than once), times
  their occlusion: a white surface of any roughness, metal or not, inside a uniform sky shows the
  sky's radiance. A scene holds at most `MAX_LIGHTS` (16) lights, every one of which every
  fragment evaluates. Normals follow the rotation of their instance and stay perpendicular to
  surfaces scaled unevenly, even flattened along one axis.
- Conventions: world space is right-handed with +Y up, as in glTF; distances are in metres and light
  is in physical units, as in glTF: lux, candelas, and nits (candelas per square metre) for the
  light that surfaces reflect and emit and for the sky; the camera looks down its −Z axis; front
  faces wind counter-clockwise; depth runs from 0 at the near plane to 1 at the far plane; colors in
  materials and skies are linear; light is not negative and may exceed 1, and once exposed, the
  high-dynamic-range target holds it, divided by 64, up to 64 × 65 504; an sRGB output encodes the
  tone-mapped values; tangents and normal textures follow glTF: x along increasing u, y up the image
  (the bitangent is cross(normal, tangent) × w), z along the normal; a mirrored instance keeps its
  front faces.
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
