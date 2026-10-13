# 0037. Lights in physical units, and pre-exposure

- Status: Accepted; amends [0027](0027-the-render-crate.md),
  [0034](0034-hdr-rendering-and-tone-mapping.md) and [0035](0035-physically-based-materials.md);
  amended by [0038](0038-shadow-mapping.md) (the frame's values; `DirectionalLight::casts_shadows`)
- Date: 2026-10-13

## Context

A scene had one directional light, which also carried a constant ambient light
([ADR-0027](0027-the-render-crate.md)); since M3b.3 its color is an illuminance, which surfaces
reflect through glTF's metallic-roughness BRDF, so that a white matte surface facing it reflects
0.97 / π of it ([ADR-0035](0035-physically-based-materials.md)). The plan of Stage 3b asks for
directional, point and spot lights in glTF's units, read from `KHR_lights_punctual` through the
`gltf` crate's feature of that name, a fixed maximum per scene in a uniform buffer, and storage
buffers only if a measurement asks for them; the ambient light stays until image-based lighting
(M3b.6).

`KHR_lights_punctual`, ratified by the Khronos Group, defines lights without size. A directional
light's intensity is an illuminance in lux, a point or spot light's a luminous intensity in
candela; a linear color, within 0 and 1, multiplies the intensity per channel. A light sits at its
node's world position and shines down the node's −Z axis, which the node's rotation turns; the
node's scale changes none of its properties. An optional range, positive, ends a point or spot
light, and the extension recommends the attenuation max(min(1 − (d / range)⁴, 1), 0) / d². A spot
light is full within an inner angle from its axis and gives nothing beyond an outer one,
0 ≤ inner < outer ≤ π/2; its reference code interpolates the cosines, saturate(cos θ × s + o)² with
s = 1 / max(0.001, cos inner − cos outer) and o = −cos outer × s. The Khronos glTF Sample Viewer
computes exactly these, with no lower bound on d. glTF's emission is in candela per square metre
(nits), `KHR_materials_emissive_strength` a unitless multiplier of it, and lengths are metres.

In these units, a sunlit matte surface reflects about 30 000 nits, and highlights far more, while
the HDR target holds at most 65 504 ([ADR-0034](0034-hdr-rendering-and-tone-mapping.md),
ADR-0035). ADR-0034 listed pre-exposure, the exposure applied before the target is written, as
an alternative to propose to the owner when lights get their units. Engines that use these units,
Frostbite and Filament, derive the exposure from a camera's exposure value at ISO 100: the
luminance that saturates the sensor is 1.2 × 2^EV100 nits, and the exposure is its inverse.

Constraints: no wgpu in `slime-render` (ADR-0027); no new crate; uniform bindings within the 16 KiB
of wgpu's downlevel limits; the glTF loader temporary, refusing what it cannot draw right
([ADR-0029](0029-temporary-gltf-loading.md)); the asset under CC0 and 500 kB; image tests strict on
lavapipe ([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)).

To decide: the lights' API and units; where the ambient light goes; the maximum and how a scene
with more is refused; the values the shader reads; the falloff, the cone, and a light at the shaded
point; where the exposure is applied, to the background too, and what the tone-mapping pass keeps;
what the loader reads and refuses; the sample's values. The owner decided on 2026-10-13 how a
light's strength is written, the maximum and its refusal, pre-exposure, the sample's sun and its
other lights, and validated the rest.

## Decision

### Lights and units

- `Light` is an enum of `DirectionalLight { direction, color, illuminance }`,
  `PointLight { position, color, intensity, range }` and `SpotLight { position, direction, color,
  intensity, range, inner_cone_angle, outer_cone_angle }`: glTF's three types. Color and strength
  are separate, as in glTF: the color is linear and not negative, and the strength carries the
  unit, an illuminance in lux or an intensity in candela. Positions and ranges are in metres,
  angles in radians from the axis; `range: Option<f32>`, `None` reaching every distance. The
  defaults are glTF's: white, 1 lux or 1 candela, no range, cone angles 0 and π/4, a spot light at
  the origin shining down −Z. The directional light keeps the direction of its former default, from
  above and in front of the default camera.
- `Scene` holds `lights: &[Light]`, at most `MAX_LIGHTS` = 16, and `ambient: Vec3`, the radiance of
  the ambient light, which leaves `DirectionalLight`. The light that surfaces reflect and emit, the
  ambient light and the background are in nits.
- `Renderer::render` panics on a scene with more than `MAX_LIGHTS` lights, a bug in the caller as a
  target of another format is (ADR-0027); the examples' loader refuses files that place more.
- `Scene::exposure` stays a linear factor. Its documentation gives the exposure of a camera of
  exposure value EV100, 1 / (1.2 × 2^EV100), the convention of Frostbite and Filament, which the
  `scene` example uses for its slider.

### The values the shader reads

| Uniform | Bytes | Members (offset) |
|---|---|---|
| Frame, group 0 | 1 120 | view_proj (0), camera_position (64), exposure (76), ambient (80), light_count (92, `u32`), lights (96, 16 × 64 bytes) |
| Light, in the frame | 64 | color × strength (0), kind (12, `u32`: 0 directional, 1 point, 2 spot), position (16), 1 / range² (28, 0 without range), direction (32, of length one, the way the light travels), cone scale (44), cone offset (48) |

The CPU normalizes the directions, multiplies color by strength, inverts the squared range and
computes the cone's scale and offset as the extension's reference code does; point and directional
lights get a scale of 0 and an offset of 1, which give 1. The slots of the lights a scene does not
have are zero, and the buffer is written at its full size, as ADR-0026 requires. A unit test checks
that the shader's `MAX_LIGHTS` is the renderer's.

### Shading

- The fragment shader loops over the scene's lights, `light_count` times, after reading every
  texture. A directional light gives its illuminance from −direction. A point or spot light gives
  intensity × window / max(d², 10⁻⁴ m²), times the spot's cone squared, with the extension's
  window, not squared, as the Sample Viewer draws it: within 1 cm of the light, a surface receives
  what it would at 1 cm, as Filament and Bevy take a punctual light to have a size; within 1 µm,
  where positions in 32-bit floats are rounding noise, a fragment has no direction to the light
  and receives nothing. Each light's illuminance multiplies the BRDF times n·l (ADR-0035).
- The sum of the lights, the ambient light and the emission is multiplied by the exposure, then
  written as at most 65 504 per channel.

### Pre-exposure

- The scene pass applies the exposure, as above. Its clear writes the background without a shader,
  so `render` multiplies the background by the exposure on the CPU, in f64, and bounds each channel
  by 65 504.
- The tone-mapping pass reads the exposed light and applies the curve; it has no uniform.

### The examples' loader

The `gltf` crate is built with its `KHR_lights_punctual` feature, which adds no crate. The lists of
ADR-0035, which the module comment of `examples/common/gltf.rs` holds too, gain:

- **Reads:** the lights that the nodes of the drawn scene place, with their color, intensity, range
  and cone angles, each at its node's world origin and along its node's world −Z axis, whatever the
  node's scale; a light that two nodes place gives two lights.
- **Refuses:** a light whose color or intensity is negative or not a finite number, whose range is
  not positive and finite, or whose cone angles are not 0 ≤ inner < outer ≤ π/2, which refuses the
  equal angles that Blender writes for a spot whose blend is 0; a light that its node places at no
  finite point, or whose −Z axis its node flattens; more lights than `MAX_LIGHTS`; and, through the
  `gltf` crate's validation, a light of an unknown type, a spot light without its `spot` object, or
  a node naming a light the file does not have, the cases on which the crate's readers would panic.
  A file that requires an extension other than `KHR_lights_punctual` and
  `KHR_materials_emissive_strength`.

A file without lights gets none; the sample lights it with its sun.

### The sample

The asset, exported in Blender's standard lighting mode (683 lm/W) with the EEVEE render engine,
under which the exporter reads the lights' own strengths, holds three lights: a sun of 100 000 lux
along (−0.5, −1, −0.6); a teal point light of 37 700 cd with a range of 4 m at the center of its
glowing ring, a child of the ring's node, which stands for the ring's glow (the flux of its glowing
segments); and a warm spot light of 80 000 cd on the floor, aimed up at the side of the wall that
the sun leaves in the shade. The ring emits 127 324 nits. The sample adds an ambient light of
(5 730, 6 048, 7 321) nits and a background of (1 592, 1 910, 2 546) nits, and draws at an
exposure of π / 100 000, EV100 14.7, near the "sunny 16" rule's 15, under which every surface but
those near the new lights shows as before the units. The `scene` example's slider moves in exposure
values, from 18 on its left to 0 on its right. The shapes of the image test and the renderer's
tests keep unit-scale lights at an exposure of 1, so that their references and expected values
prove the change.

This amends:
- **ADR-0027:** a scene is a camera, lights, an ambient light, a clear color, an exposure and
  instances; light is in physical units, lengths in metres.
- **ADR-0034:** the exposure is applied by the scene pass, and to the background by `render`; the
  tone-mapping pass reads no exposure and has no uniform; light must stay within 65 504 after the
  exposure, not before; the clear color is in nits.
- **ADR-0035:** the frame's values are those above; `DirectionalLight::color` becomes a color, and
  its strength `illuminance` in lux; the ambient light moves to `Scene`; the loader's lists gain the
  items above.

## Alternatives considered

- **One per-channel strength**, as `DirectionalLight::color` was: one field fewer, but color and
  strength mixed, a unit on a vector, and every tool that writes lights (glTF, Blender, Unity,
  Unreal, Godot) keeps them apart, as `MaterialData` keeps `emissive` and `emissive_strength`.
- **Three slices in `Scene`**, one per type: three counts and three loops, and the order that
  "the first directional light" of M3b.5 refers to lost. **One struct with a kind field**, glTF's
  JSON: fields that do nothing for some kinds. **An enum with struct variants:** no `Default` per
  kind, and no type to name.
- **A sentinel for "no range"**: the Sample Viewer's −1, or infinity, which WGSL lets
  implementations assume never occurs.
- **Full cone angles, degrees, or cosines** for spot lights: not glTF's half-angles in radians.
- **The ambient light as a `Light`**, or in an `Environment` with the background: not punctual,
  counted against the maximum; an abstraction for M3b.6, which replaces both.
- **A `Result` from `render`**, an error path in every frame for a count the caller controls, or
  **the first 16 lights drawn with a warning**, a plausible wrong image that depends on the
  order of the list.
- **A maximum of 8, or 32 and more** (16 KiB hold 254 lights): fewer than some glTF scenes hold, or
  a worst case per fragment two to four times higher, which forward shading without light culling
  cannot afford on modest GPUs.
- **The exposure as an EV100 in `Scene`**, or a conversion function in the renderer: scenes at
  unit scale would write log2(1 / 1.2) for 1, a game computing its exposure otherwise would
  convert back, and only the example's slider needs it.
- **Lights in a binding of their own, or a storage buffer**: a buffer and a write more for 1 KiB
  that the frame's uniform holds; storage needs a measurement.
- **48-byte lights with a homogeneous position** (w = 0 for a directional light) instead of a kind:
  256 bytes saved, and a convention that every reader of the shader must decode.
- **The squared window of Frostbite, Filament and Unreal Engine 4**, smooth at the range: not the
  extension's recommendation nor the Sample Viewer's image (0.64 against 0.80 at two thirds of the
  range).
- **No lower bound on the distance**, as the Sample Viewer: an infinity at the light, NaN at an
  exposure of 0. **Unreal Engine 4's 1 / (d² + 1)**: below the inverse square law at every
  distance.
- **`smoothstep` between the cone's cosines** (older Sample Viewers, three.js): not the reference
  code.
- **The lights multiplied by the exposure on the CPU**, Filament's pre-exposed lights: emission and
  the ambient light need it in the shader anyway, two ways for one thing (ADR-0034).
- **The background exposed in a shader**: a pass that M3b.6's sky brings.
- **A uniform kept in the tone-mapping pass for M3b.7's bloom**: a binding for a later milestone.
- **Refusing light colors above 1**, which glTF does not allow: the renderer draws them right.
- **Equal cone angles drawn**, with the hard edge that the reference code's 0.001 gives: one
  invalid case accepted among others refused.
- **A sun of 10 000 lux**, under which lamps of familiar strengths show: the same image, but the
  sun is no longer the sun, and M3b.5's sharp shadows and M3b.6's sky expect a clear day's.
  **EV100 15 exactly**: the sample 0.3 stop darker than before.
- **A point light or a spot light alone in the asset**: each leaves a kind of punctual light
  unchecked from Blender's export to the image.
- **Blender's unitless or raw lighting modes**: the Blender scene would no longer be physical, and
  raw is deprecated.

## Consequences

- API changes: `Scene::light` gives way to `lights` and `ambient`; `DirectionalLight` has `color`
  and `illuminance` and glTF's default strength; `Light`, `PointLight`, `SpotLight` and
  `MAX_LIGHTS` are new. A caller that drew with a light of π at an exposure of 1 draws the same
  image with an illuminance of π.
- Physical scenes fit half precision at a camera's exposure. The clamp at 65 504 now catches only
  exposed light above it: at the sample's exposure, the sun's highlights on metals smoother than
  about 0.125 seen near grazing incidence, and emission or a background above 65 504 / exposure.
- Every fragment evaluates every light of the scene: the scene pass's cost grows with the number of
  lights, about 20 µs per light for a screen of 1920 × 1080 fragments on an RX 6900 XT, and a
  light's range saves no work.
- The frame's uniform is 1 120 bytes, written every frame.
- Point and spot lights cast no shadows, and light what objects hide.
- Within 1 cm of a point or spot light, the image differs from the Sample Viewer's, which grows
  without bound there; the window reaches zero at the range with a slope, as the extension's
  formula does.
- glTF files that Blender exports with lights require the extension, which the loader reads; a
  Blender spot light whose blend is 0 is refused.
- The sample's images change near its new lights; the shapes' do not.
- M3b.5 can shadow the first directional light of `Scene::lights`; M3b.6 replaces `Scene::ambient`
  and the cleared background, and its sky must be exposed as the scene pass exposes surfaces;
  M3b.7's bloom reads exposed light.
