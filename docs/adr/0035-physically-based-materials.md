# 0035. Physically based materials

- Status: Accepted; amends [0027](0027-the-render-crate.md), [0029](0029-temporary-gltf-loading.md)
  and [0034](0034-hdr-rendering-and-tone-mapping.md)
- Date: 2026-10-13

## Context

The renderer of Stage 3a shaded every surface with Lambert's law: its base color, which one
texture could modulate, times the directional light's color and the cosine of incidence, plus the
ambient light ([ADR-0027](0027-the-render-crate.md)). Stage 3b makes it physically based
(`docs/PLAN.md`), and its plan fixed the outline of this milestone, M3b.3: glTF's
metallic-roughness model with the formulas of the Khronos glTF Sample Viewer (the GGX
distribution, Smith's visibility term, Schlick's Fresnel approximation); base color,
metallic-roughness, normal, occlusion and emissive textures, each created in the format of its
slot (sRGB for colors, linear for data); the `MASK` alpha mode by discarding fragments, `BLEND`
being outside the stage; double-sided materials drawn without culling; tangents in `MeshData`,
read from glTF files or computed per triangle by the examples' loader, MikkTSpace being left to
the importer of Stage 4; `KHR_materials_emissive_strength` read through the `gltf` crate's
feature of that name; no new dependency. The constant ambient light stays until image-based
lighting (M3b.6), and the exposure stays in the tone-mapping pass (ADR-0034).

Two known debts meet here. The renderer had one pipeline, which culled back faces, so an instance
whose transform mirrors it (a negative determinant) was drawn inside out, and the examples' loader
drew mirrored nodes with a copy of their mesh wound the other way. And a mesh flattened between two
rotations has a determinant of rounding noise of either sign, which the renderer read as a mirror
for about two such meshes in five, lighting them from the wrong side, while the loader decided from
a determinant computed another way. Double-sided materials need a pipeline chosen per draw anyway.

glTF 2.0 says:
- Appendix B: the BRDF is a dielectric's Lambert diffuse under a specular layer, mixed by Fresnel
  with F0 = 0.04, blended by `metallic` with a metal's specular reflection tinted by its base
  color;
- a material's defaults are a white base color, metallic 1 and roughness 1;
- a normal texture is in tangent space, red along the tangent (where u grows), green along the
  bitangent, cross(normal, tangent) × w, up the image (where v shrinks), blue along the normal,
  each component from −1 to 1 stored as 0 to 1;
- when tangents are missing, MikkTSpace computes them; when normals are missing too, flat normals
  are computed and the file's tangents ignored;
- occlusion darkens indirect light only;
- a double-sided material's back faces are lit with their normals reversed;
- a node whose transform has a negative determinant keeps its front faces.

Constraints:
- no wgpu in `slime-render` (ADR-0027);
- `slime-gfx` already has what this needs: `Rgba8Unorm` textures, `Float32x4` vertex attributes,
  front-face culling, and up to 16 sampled textures and 16 samplers per shader stage with the
  default limits;
- image tests are strict on lavapipe ([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md));
- `Rgba16Float` outputs can be read back exactly ([ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md)).

To decide:
- the material's API and defaults;
- how textures are given and checked;
- where tangents come from;
- the values the shaders read;
- the shading formulas and the light's unit;
- normal mapping and facing;
- the pipelines and their choice;
- masks;
- what the loader reads, refuses and warns about;
- how the shading is tested.

## Decision

### Materials

- `MaterialData` holds glTF's metallic-roughness model field for field:
  - `base_color` and `base_color_texture`;
  - `metallic`, `roughness` and `metallic_roughness_texture`, whose green channel multiplies the
    roughness and whose blue channel the metallic;
  - `normal_texture` and `normal_scale`;
  - `occlusion_texture`, whose red channel is read, and `occlusion_strength`;
  - `emissive`, `emissive_strength` and `emissive_texture`;
  - `alpha_mode`, an `AlphaMode`: `Opaque`, or `Mask { cutoff }`;
  - `double_sided`.
- `MaterialData::default()` is an opaque, single-sided, white matte dielectric, without textures:
  glTF's default material but for `metallic`, which glTF sets to 1. glTF's own default material
  is built by the loader as `MaterialData { metallic: 1.0, ..MaterialData::default() }`.
- Each texture is a `SampledTexture`, a texture and its own sampler, since glTF gives each texture
  its wrap modes and filters.
- The base color and emissive slots hold colors and need an sRGB format; the metallic-roughness,
  normal and occlusion slots hold data and need a format without sRGB encoding, `Rgba8Unorm`.
  `Renderer::create_material` asserts it and panics otherwise: a texture in the other kind of format
  is read wrongly, a bug in the caller (ADR-0027).
- One white sRGB pixel, the renderer's, stands in for every slot left empty: it reads exactly 1 in
  every channel, so that a factor counts alone. A material without a normal texture is packed
  with a normal scale of 0, which the shader takes to mean the mesh's normal, exactly.
- Bind group 1 holds the material's uniform at binding 0, then each slot's texture and sampler at
  bindings 1 and 2 (base color), 3 and 4 (metallic-roughness), 5 and 6 (normal), 7 and 8
  (occlusion), 9 and 10 (emissive): five sampled textures and five samplers per fragment stage.

### Tangents and the vertex

- `MeshData::tangents: Vec<[f32; 4]>` is required, one per vertex, in glTF's convention: xyz along
  increasing u, w = ±1 such that cross(normal, xyz) × w points up the image. The renderer never
  computes tangents. Callers that know theirs write them; the examples' loader reads a file's, or
  computes them per triangle (Lengyel's method: each triangle's ∂P/∂u and ∂P/∂v summed at its
  vertices, the first made perpendicular to the normal, w from the second).
- A vertex is twelve 32-bit floats, 48 bytes: position, normal, texture coordinates, tangent at
  location 3. The largest mesh holds 5 592 405 vertices with the default limits.

### The values the shaders read

| Uniform | Bytes | Members (offset) |
|---|---|---|
| Frame, group 0 | 128 | view_proj (0), to_light (64), light_color (80), ambient (96), camera_position (112) |
| Material, group 1 | 48 | base_color (0), emissive × strength (16, vec3), metallic (28), roughness (32), normal_scale (36, 0 without a normal texture), occlusion_strength (40), alpha_cutoff (44, 0 when opaque) |
| Instance, group 2 | 128 | model (0), normal matrix (64), handedness (112: −1 when mirrored, else 1) |

### Shading

- The scene shader evaluates glTF 2.0 Appendix B as the Sample Viewer does:
  - D, the GGX distribution, α²/(π((n·h)²(α² − 1) + 1)²) with α = roughness²;
  - V, Smith's height-correlated visibility,
    0.5 / (n·l √((n·v)²(1 − α²) + α²) + n·v √((n·l)²(1 − α²) + α²));
  - F, Schlick's approximation, f0 + (1 − f0)(1 − v·h)⁵, the fifth power by multiplication;
  - a dielectric is mix(base / π, D·V, F(0.04)); a metal is F(base)·D·V; the two are mixed by
    `metallic`, then multiplied by n·l.
- `DirectionalLight::color` is the illuminance on a surface facing the light: the reflected light
  is the BRDF times it times n·l, so a white matte dielectric facing the light and the camera
  reflects 0.97/π of it. Its default becomes π, under which such a surface reflects 0.97, sRGB 239
  after the tone curve; the samples' lights are multiplied by π.
- Roughness is shaded no smoother than 0.045, Filament's minimum for 32-bit floats: at 0, D
  divides 0 by 0 at the center of a highlight. The half vector is guarded where the light comes
  from exactly opposite the camera.
- The ambient light, a stand-in for even surroundings until image-based lighting, is reflected as
  ambient × base color × occlusion, whatever the metallic and roughness. Occlusion,
  1 + strength × (texel − 1), darkens it and it alone.
- The emission, factor × strength × texture, is added unoccluded.
- The shader writes at most 65 504 per channel, the largest half-precision value, rather than
  leave a brighter one to the driver, which may store an infinity that the tone curve turns into
  NaN. A metal smoother than about 0.05 under a light of π reaches it at its highlight.

### Normals and faces

- The vertex stage carries the tangent with the model matrix, and multiplies its w by the
  instance's handedness: a mirror reverses the cross product that gives the bitangent.
- The fragment stage builds the bitangent as cross(n, t) × w from the interpolated normal and
  tangent, as MikkTSpace recommends, and tilts the normal by the normal texture in that frame,
  with x and y scaled by `normal_scale`. A tangent of zero length tilts nothing.
- A fragment is seen from behind when `front_facing` differs from the instance not being mirrored
  (a mirrored instance's front faces wind clockwise on screen). Its normal is then reversed, which
  only double-sided materials show.

### Pipelines

- The scene pass has three pipelines, created by `Renderer::new`, which differ only by their
  culling: back faces, front faces, or none.
- Each draw takes one from `cull_mode(double_sided, mirrored)`: none for a double-sided material,
  front faces for a single-sided material on a mirrored instance, back faces otherwise. The
  pipeline is set only when it changes, and draws keep the scene's order.
- An instance is mirrored when the determinant of its transform's normalized columns is below
  −10⁻⁵. The same test decides the culling, the handedness and the sign of the normal matrix: a
  mesh flattened between two rotations, whose determinant is rounding noise of either sign (at
  most about 10⁻⁷), is not mirrored. The examples' loader no longer reverses the winding of
  mirrored nodes.

### Masks

One fragment shader serves every material. It reads every texture first, then discards a fragment
whose alpha is below the material's cutoff; opaque materials have a cutoff of 0, which keeps every
fragment. A discarded fragment writes neither color nor depth.

### The examples' loader

- The `gltf` crate is built with its `KHR_materials_emissive_strength` feature. The loader's own
  check of required extensions is deleted, since the crate's validation now accepts exactly the
  extensions the loader reads.
- Each texture is created in its slot's format: only the images that a material reads, each
  decoded once and uploaded once per format, with one sampler per glTF texture.
- `BLEND` materials are drawn opaque with a warning.

What the loader reads, refuses and warns about, replacing the lists of ADR-0029 (the module
comment of `examples/common/gltf.rs` holds the same lists):

- **Reads:**
  - triangle-list primitives: positions; normals (flat per triangle when missing); tangents
    (computed per triangle when missing, and ignored when normals are missing); the first set of
    texture coordinates (zero when missing); indices (the vertices in order when missing);
  - materials: the base color, metallic, roughness and emissive factors,
    `KHR_materials_emissive_strength`, the normal scale and occlusion strength, `OPAQUE` and
    `MASK` with its cutoff (0.5 when missing), and double-sidedness; glTF's default material for
    primitives without one;
  - the five textures, each in its slot's format, with their samplers. Of glTF's minification
    filters, only `NEAREST_MIPMAP_NEAREST` and `LINEAR_MIPMAP_LINEAR` are drawn exactly;
  - the node hierarchy of the default scene, or of the first, flattened into instances.
- **Refuses:**
  - a file that is not valid glTF, including one requiring another extension;
  - a file without a binary chunk; buffers and images in external files or in data URIs;
  - images that are not PNG, larger than the GPU accepts, lying outside the binary chunk, without
    a MIME type, without data, or undecodable;
  - primitives that are not triangle lists, or have no positions;
  - accessors of positions, normals, tangents, texture coordinates or indices stored in types glTF
    does not allow for them, sparse, empty, closer together than their size, or outside their
    buffer view or the binary chunk;
  - primitives whose vertices or indices need a buffer larger than the GPU accepts; mesh data
    that `Renderer::create_mesh` refuses;
  - textures read through another coordinate set than the first, in any slot;
  - node hierarchies that are not trees; a file without a scene, or whose scene draws nothing.
- **Warns about, and draws without:** animations, skins, cameras, vertex colors, extensions used
  but not read (`KHR_texture_transform` among them), and the blending of `BLEND` materials.

### Tests

- Tests that read pixels back render into an `Rgba16Float` output of odd size, whose center
  pixel's center lies on the camera's axis, and compare it within 2 × 10⁻³ with the tone curve of
  what a CPU copy of the BRDF predicts (`crates/slime-render/tests/common/brdf.rs`, in f64).
- That copy writes the visibility term from Heitz's Λ, a different form of the same function, and
  a test checks it against closed forms.
- The references of the image tests are rendered again on lavapipe; the tolerances of ADR-0030
  stand.

This amends:
- **ADR-0027:** "one pipeline, with the depth test and back-face culling" becomes three scene
  pipelines chosen per draw. The material group holds a uniform and five textures with their
  samplers. A vertex carries a tangent. Surfaces are shaded with glTF's metallic-roughness model,
  not Lambert's law. The conventions gain "a mirrored instance keeps its front faces" and the
  tangent space above.
- **ADR-0029:** its lists of what the loader reads, refuses and warns about are replaced by those
  above. The loader reads every metallic-roughness property, tangents and one extension, and no
  longer copies mirrored meshes.
- **ADR-0034:** "light above 65 504 overflows the HDR target and becomes NaN" becomes "light above
  65 504 is written as 65 504 by the scene shader". The pre-exposure proposal for M3b.4 stays
  open.

## Alternatives considered

- **glTF's default material (metallic 1) for `MaterialData::default()`.** Every material written
  with `..MaterialData::default()` would be a metal.
- **One `emissive` field holding the strength already.** The plan and glTF keep them apart, and
  the product costs one multiplication when the material is created.
- **`Option<f32>` for the cutoff.** One type fewer, but `None` meaning opaque is implicit;
  `AlphaMode` says glTF's mode.
- **Structs holding the normal texture with its scale and the occlusion texture with its
  strength.** Two more types for two numbers.
- **One sampler for every slot.** It loses glTF's per-texture wrap and filters, and saves
  nothing measurable.
- **Accepting any format, converting it, or returning an error.** A texture read in the other kind
  of format gives wrong values silently. Since the formats are known when the caller writes the
  code, this is a bug, not data.
- **Default textures per slot (linear white, a flat normal).** The sRGB white pixel already reads
  1 in every slot, and (128, 128, 255) is not quite flat (0.0039).
- **Tangents computed by the renderer when missing.** That puts an approximation, which Stage 4
  replaces with MikkTSpace, into the engine's API, and gives two ways to provide tangents.
- **MikkTSpace now.** A port of its reference code or a new dependency, both outside the stage.
- **A bitangent attribute.** Twelve more bytes per vertex for what w gives.
- **The bitangent taken in the mesh's space and carried by the model matrix.** Correct for
  mirrors, but under uneven scales it differs from the per-fragment cross product that MikkTSpace
  bakes against, and it costs a second interpolant.
- **The Sample Viewer's world-space bitangent without the handedness.** It turns a mirrored
  instance's normal texture upside down.
- **The formulation with F0 mixed by metallic and a (1 − F)(1 − metallic) diffuse** (UE4, Filament,
  older Sample Viewers). It differs from glTF's appendix between metallic 0 and 1.
- **Burley's diffuse, energy compensation for multiple scattering.** Not in glTF's appendix.
- **A light color without the 1/π of Lambert's diffuse.** Lights in lux (M3b.4) and
  `KHR_lights_punctual` expect the BRDF times the illuminance times n·l.
- **No roughness clamp.** NaN at roughness 0.
- **Filament's mobile minimum of 0.089.** That is for 16-bit shader arithmetic, which this shader
  does not use, and smoother materials would lose their sharpness.
- **A diffuse-only ambient term.** Metals would be black in the shade.
- **A Fresnel-weighted ambient term, or the split-sum approximation.** A formula that M3b.6
  replaces anyway, or a table that M3b.6 brings.
- **Occlusion on the direct light.** glTF applies it to indirect light only.
- **Six pipelines, opaque and masked times three cullings**, to keep early depth tests for opaque
  draws. Not measured; recorded as debt with how to measure.
- **A front-face winding per pipeline in `slime-gfx`.** A change to the GPU layer for the same
  number of pipelines, and the shader still needs the handedness.
- **Reversed indices by the caller**, the loader's former workaround. Every caller would have to
  do it, with a copy of each mirrored mesh.
- **The sign of the determinant without a threshold.** Flattened meshes lit from the wrong side
  (about 40 % of them, measured).
- **The determinant's sign recomputed in the shader.** The CPU decides the culling, and one test
  must decide both.
- **Pipelines created lazily or kept in a map.** An error path in `render` for three
  combinations.
- **Light above 65 504 left to the driver.** An infinity on some drivers, then NaN through the
  curve.
- **The loader's required-extension check, kept or narrowed.** It repeats the crate's validation,
  which refuses the same files.
- **Refusing `BLEND` materials.** Plain exports often mark materials so; drawn opaque with a
  warning, they still show.

## Consequences

- Every image changes again, and the references of the image tests are rendered again.
- API changes:
  - every `MeshData` literal needs tangents;
  - `MaterialData` has eleven more fields and `AlphaMode` is new;
  - `DirectionalLight::color` is an illuminance, π by default;
  - a vertex takes 48 bytes, and the largest mesh is 5 592 405 vertices;
  - `create_material` panics on a texture in the wrong format.
- `Renderer::new` compiles four pipelines instead of two.
- Every draw of the scene pass runs a shader that can discard, which may cost early depth tests on
  some GPUs (debt).
- Smooth metals clamp at 65 504 under strong lights: such a highlight loses its energy, which
  lower exposures and the bloom of M3b.7 would show (ADR-0034's pre-exposure question, M3b.4).
- The shading differs from the Sample Viewer's in three places:
  - for files without tangents, whose tangents are not MikkTSpace's;
  - in the ambient term;
  - for mirrored instances with normal textures, which the Sample Viewer turns upside down.
- Masks thin out in distant mip levels, whose alpha is averaged.
- Masked materials will cast solid shadows unless M3b.5 gives depth-only pipelines a fragment
  stage (ADR-0033).
- The loader no longer decodes images that no material reads: a broken image that nothing uses no
  longer refuses the file.
