// The shader of the scene pass (ADR-0034, ADR-0035, ADR-0037): vertices transformed by their
// instance and the camera; surfaces shaded with glTF's metallic-roughness model, with the formulas
// of the Khronos glTF Sample Viewer (glTF 2.0 specification, Appendix B), under the scene's lights
// (KHR_lights_punctual), plus the light of its environment (image-based lighting, ADR-0039) and the
// material's emission; multiplied by the exposure and written into the high-dynamic-range target,
// which the tone-mapping pass then maps to the output. Also the shadow pass's stages (ADR-0038),
// and the sky pass's, which draw the environment and its sun's disc where no surface is
// (ADR-0039).

const PI: f32 = 3.141592653589793;
// What a dielectric reflects at normal incidence: ((1.5 - 1) / (1.5 + 1))^2, glTF's index of
// refraction being 1.5.
const DIELECTRIC_F0: f32 = 0.04;
// The smoothest surface shaded, Filament's minimum for 32-bit floats: at a roughness of 0, the
// distribution divides 0 by 0 at the center of a highlight.
const MIN_ROUGHNESS: f32 = 0.045;
// The largest value of the high-dynamic-range target's half-precision floats. Brighter light is
// written as it rather than left to the driver, which may store an infinity that the tone curve
// turns into NaN.
const LARGEST_HALF: f32 = 65504.0;
// What exposed light is divided by before it is written into the high-dynamic-range target, and
// multiplied by again in the tone-mapping pass (ADR-0040): light up to 64 × 65 504 keeps its energy
// for the bloom, as a sun's disc brighter than half precision does when the exposure is raised. A
// power of two, which half precision divides exactly but below 64 × 2^-14 (about 0.004 once
// exposed, black on an 8-bit display), where light is stored in steps of 64 × 2^-24.
// HEADROOM in tone_mapping.wgsl is the same, which a unit test checks.
const HEADROOM: f32 = 64.0;
// The most lights a scene holds: MAX_LIGHTS in light.rs, which a unit test compares with it.
const MAX_LIGHTS: u32 = 16u;
// The most light that one light gives a point before the surface reflects it: LARGEST_LIGHT in
// light.rs, which a unit test compares with it. Bounded before it is multiplied by what the
// surface reflects, which may be 0: an infinity times 0 is not a number.
const LARGEST_LIGHT: f32 = 1e30;
// How many cascades the shadowed light's shadows have: CASCADES in shadow.rs, which a unit test
// compares with it.
const CASCADES: u32 = 4u;
// Light::kind of a directional light. Point lights are 1u and spot lights 2u, which the shader
// treats alike: a point light's cone values give 1 in every direction.
const DIRECTIONAL: u32 = 0u;
// Light::tile of a light without a tile of the spot lights' shadow atlas: NO_TILE in light.rs, which
// a unit test compares with it.
const NO_TILE: u32 = 0xffffffffu;
// The square of the distance, in square metres, within which a point or spot light gives what it
// gives 1 cm away, as Filament and Bevy do: the inverse square of the distance would grow without
// bound at the light.
const MIN_DISTANCE_SQUARED: f32 = 1e-4;
// The square of the distance, 1 µm squared, within which a fragment has no direction to a point or
// spot light: positions in 32-bit floats a few metres from the camera, the origin of the frame's
// space, are about that precise.
const DIRECTIONLESS_SQUARED: f32 = 1e-12;
// The environment's last level, prefiltered for a roughness of 1, which is also the irradiance
// divided by pi: ENVIRONMENT_LEVELS - 1 in environment.rs, which a unit test compares with it.
const ROUGHEST_LEVEL: f32 = 4.0;
// The BRDF table's size along each axis: BRDF_TABLE_SIZE in environment.rs, which a unit test
// compares with it. Its texels' centres lie on n.v and roughness 0 and 1.
const BRDF_TABLE_SIZE: f32 = 32.0;
// The chord from the centre of the sun's disc to its edge on the unit sphere, 2 sin(r / 2) for its
// angular radius r, 0.2665 degrees (SUN_ANGULAR_RADIUS in environment.rs, which a unit test
// compares with it).
const SUN_CHORD: f32 = 0.0046512983;
// The sine of the depression below the horizon over which the ground hides the horizon's light by
// a factor e (GROUND_HAZE in environment.rs, which a unit test compares with it): it hides the
// sun's disc as the environment hides its glow.
const GROUND_HAZE: f32 = 0.03;

// One light, in the frame's space, as Light::push_values in light.rs packs it.
struct Light {
    // The light's color times its illuminance (lux) for a directional light, times its intensity
    // (candelas) for the others.
    color: vec3<f32>,
    // DIRECTIONAL, or 1u for a point light, 2u for a spot light.
    kind: u32,
    // Point and spot lights: where the light is.
    position: vec3<f32>,
    // Point and spot lights: 1 / range^2, or 0 for a light without range.
    inverse_range_squared: f32,
    // Directional and spot lights: the direction the light travels in, of length one.
    direction: vec3<f32>,
    // Spot lights: the cone's attenuation is saturate(cos * scale + offset)^2, where cos is the
    // cosine of the angle from the axis (KHR_lights_punctual's reference code). 0 and 1 for the
    // other lights: no cone.
    cone_scale: f32,
    cone_offset: f32,
    // Spot lights that cast shadows: the index of their tile in FrameUniform::spot_tiles; NO_TILE
    // for the other lights.
    tile: u32,
}

// A tile of a shadow atlas, a cascade's (ADR-0064) or a spot light's (ADR-0058, ADR-0061), as the
// renderer packs it.
struct ShadowTile {
    // From the frame's space to the tile's clip space: x and y from -1 to 1 across the tile, and
    // depth, reversed (ADR-0047): from 1 to 0 along the light for a cascade, orthographic, so
    // that w stays 1; 0.05 m / d at a distance d along the light's axis for a spot light.
    view_proj: mat4x4<f32>,
    // The tile in the atlas's texture coordinates: its top left corner (x, y) and its side (z);
    // and the width of its texels (w): in metres for a cascade; for a spot light, 1 m from the
    // light along its axis, 2 tan(h) / N, h the angle from the axis to the tile's sides and N its
    // side in texels.
    place: vec4<f32>,
}

// Positions are in the frame's space: world space moved so that the camera's eye is at its origin
// (ADR-0048), computed on the CPU, so that what is near the camera keeps its precision however far
// from the world's origin it is.
struct FrameUniform {
    // From the frame's space to clip space.
    view_proj: mat4x4<f32>,
    // From clip space to directions from the camera, with world space's axes: the inverse of
    // view_proj. A point (x, y, 0, 1) at infinity gives (d, 0), d the direction it is seen in.
    sky_from_clip: mat4x4<f32>,
    // What the light is multiplied by before it is written into the high-dynamic-range target
    // (pre-exposure, ADR-0037).
    exposure: f32,
    // How many entries of `lights` hold the scene's lights, from 0 to MAX_LIGHTS.
    light_count: u32,
    // The index in `lights` of the light that the cascades shadow, the scene's first directional
    // light that casts shadows; light_count or more when none is.
    shadowed_light: u32,
    // The unit direction towards the centre of the environment's sun's disc, and the disc's
    // radiance, in nits: 0 without a sun (ADR-0039).
    to_sun: vec3<f32>,
    sun_radiance: vec3<f32>,
    lights: array<Light, MAX_LIGHTS>,
    // The tiles of the spot lights that cast shadows, in the order of their lights; those beyond
    // the last such light are not read.
    spot_tiles: array<ShadowTile, MAX_LIGHTS>,
    // The cascades of the shadowed light, from the nearest to the camera to the farthest; not read
    // without a shadowed light.
    cascades: array<ShadowTile, CASCADES>,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;
// The scene's environment, in nits (ADR-0039): level 0 the sky, level i prefiltered for a
// roughness of i / 4, the last also the irradiance divided by pi. One black texel without an
// environment.
@group(0) @binding(1) var environment: texture_cube<f32>;
// The BRDF's scale (red) and bias (green) for image-based lighting, n.v across, roughness down.
@group(0) @binding(2) var brdf_table: texture_2d<f32>;
// Linear, between levels too, clamped to the edge.
@group(0) @binding(3) var environment_sampler: sampler;

struct MaterialUniform {
    // Linear; alpha is compared with the cutoff.
    base_color: vec4<f32>,
    // Linear: the emissive factor times its strength.
    emissive: vec3<f32>,
    metallic: f32,
    roughness: f32,
    // 0 without a normal texture: the mesh's normal is then used as it is.
    normal_scale: f32,
    occlusion_strength: f32,
    // 0 for opaque materials, which keeps every fragment.
    alpha_cutoff: f32,
}
@group(1) @binding(0) var<uniform> material: MaterialUniform;
// A slot that the material leaves empty holds one white pixel: 1 in every channel.
@group(1) @binding(1) var base_color_texture: texture_2d<f32>;
@group(1) @binding(2) var base_color_sampler: sampler;
// Green: roughness; blue: metallic.
@group(1) @binding(3) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(4) var metallic_roughness_sampler: sampler;
// Tangent-space normals, each component from -1 to 1 stored as 0 to 1.
@group(1) @binding(5) var normal_texture: texture_2d<f32>;
@group(1) @binding(6) var normal_sampler: sampler;
// Red: the share of the environment's light that reaches the surface.
@group(1) @binding(7) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(8) var occlusion_sampler: sampler;
@group(1) @binding(9) var emissive_texture: texture_2d<f32>;
@group(1) @binding(10) var emissive_sampler: sampler;

struct Instance {
    // From the mesh's space to the frame's space.
    model: mat4x4<f32>,
    // For normals: proportional to the inverse transpose of the model's rotation and scale, so
    // that normals stay perpendicular to surfaces scaled unevenly. It gives directions only.
    normal: mat3x3<f32>,
    // 1, or -1 for an instance whose transform mirrors it: its front faces wind clockwise on
    // screen, and the cross product that gives its bitangents turns the other way.
    handedness: f32,
}
// The values of the instances that the frame's passes draw, and, for each draw, where the values
// of the instances it draws are: the instance of number n, `@builtin(instance_index)`, has its
// values at instances[drawn[n]].
@group(2) @binding(0) var<storage, read> instances: array<Instance>;
@group(2) @binding(1) var<storage, read> drawn: array<u32>;

// The cascades' atlas: each tile the depths of what the shadowed light reaches first across its
// square, drawn by the shadow pass (ADR-0064); one texel, never read, until a directional light
// casts shadows. And the sampler that compares a depth with them: 1 where the depth is at least
// the stored one (lit: depth is reversed, nearer is greater), blending the 2 x 2 nearest texels.
@group(3) @binding(0) var cascade_atlas: texture_depth_2d;
@group(3) @binding(1) var shadow_sampler: sampler_comparison;
// The spot lights' shadow atlas: each tile the depths that its light reaches first, drawn by the spot
// lights' shadow pass, compared through the same sampler. One texel, never read, when no spot light
// has cast shadows yet.
@group(3) @binding(2) var spot_atlas: texture_depth_2d;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // xyz: along increasing u; w: 1 or -1, the sign of the bitangent (glTF).
    @location(3) tangent: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // With world space's axes, of any length: the normal matrix gives directions only, and
    // interpolation across the triangle shortens it.
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // In the frame's space: from the camera.
    @location(2) position: vec3<f32>,
    // xyz with world space's axes, of any length; w: the bitangent's sign times the handedness.
    @location(3) tangent: vec4<f32>,
    // The instance's handedness, the same at every vertex.
    @location(4) @interpolate(flat) handedness: f32,
}

@vertex
fn vs_main(input: VertexInput, @builtin(instance_index) index: u32) -> VertexOutput {
    let instance = instances[drawn[index]];
    var output: VertexOutput;
    let position = instance.model * vec4<f32>(input.position, 1.0);
    output.clip_position = frame.view_proj * position;
    output.position = position.xyz;
    output.normal = instance.normal * input.normal;
    output.uv = input.uv;
    // A tangent lies in the surface: the model matrix carries it as it carries the surface. A
    // mirror reverses the cross product that gives the bitangent, which the handedness undoes.
    output.tangent = vec4<f32>(
        (instance.model * vec4<f32>(input.tangent.xyz, 0.0)).xyz,
        input.tangent.w * instance.handedness,
    );
    output.handedness = instance.handedness;
    return output;
}

// How many of a shadow map's texels a surface is looked up away from itself, along its triangle's
// normal, times the sine of the light's incidence (ADR-0050, ADR-0062, ADR-0063, ADR-0065). The
// filter compares a point with texels up to three texels away along each axis of the map, 4.24
// diagonally, where a surface tilted by an angle t from facing the light lies up to 4.24 tan(t)
// texels' depth nearer to the light; moved by 4.5 sin(t) texels along its normal, the point lies
// 4.5 tan(t) texels' depth nearer than its own surface, whatever t: no surface shadows itself, flat
// or curved, while a shadow moves by at most 4.5 texels.
const NORMAL_OFFSET_TEXELS: f32 = 4.5;

// How far inside its edges a cascade holds a point, in its texels: the filter reads texels up to
// three away from the point (`compared`), which then all lie in the cascade's tile (ADR-0065).
const CASCADE_MARGIN_TEXELS: f32 = 4.0;

// The width of the band, inside the edges of a cascade's square, over which its shadows fade into
// the next cascade's, or out beyond the last: a tenth of its width, 2 in its clip space (ADR-0064).
const CASCADE_BAND: f32 = 0.2;

// The shadow passes (ADR-0038, ADR-0061, ADR-0064): each vertex where a light sees it, into a tile
// of the cascades' atlas or of the spot lights'. The pipeline of opaque materials has no
// fragment stage: it writes depth only; masked materials' runs `fs_shadow`.
struct ShadowVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// From the frame's space to the clip space of the tile the shadow pass draws: one view per tile,
// bound at its offset for each. Apart from group 3 of the scene pass, which holds the atlases it
// reads.
struct ShadowView {
    view_proj: mat4x4<f32>,
}
@group(3) @binding(3) var<uniform> shadow_view: ShadowView;

@vertex
fn vs_shadow(
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @builtin(instance_index) index: u32,
) -> ShadowVertex {
    let model = instances[drawn[index]].model;
    return ShadowVertex(shadow_view.view_proj * (model * vec4<f32>(position, 1.0)), uv);
}

// The shadow pass of masked materials: below the cutoff, no depth is written, as in the scene pass.
@fragment
fn fs_shadow(@location(0) uv: vec2<f32>) {
    let alpha = material.base_color.a * textureSample(base_color_texture, base_color_sampler, uv).a;
    if alpha < material.alpha_cutoff {
        discard;
    }
}

@fragment
fn fs_main(
    input: VertexOutput,
    @builtin(front_facing) front_facing: bool,
) -> @location(0) vec4<f32> {
    // Every texture is read first, and the derivatives taken, in uniform control flow, before a
    // fragment may be discarded.
    let base = material.base_color
        * textureSample(base_color_texture, base_color_sampler, input.uv);
    let metallic_roughness =
        textureSample(metallic_roughness_texture, metallic_roughness_sampler, input.uv);
    let encoded_normal = textureSample(normal_texture, normal_sampler, input.uv).xy;
    let occlusion = textureSample(occlusion_texture, occlusion_sampler, input.uv).r;
    let emitted = material.emissive
        * textureSample(emissive_texture, emissive_sampler, input.uv).rgb;
    // The normal of the triangle itself, flat, which shadows are looked up off: the surface that
    // shadow maps hold, where a smooth normal leans across a facet. A pixel that sees the
    // triangle edge on has none, and moves the point nowhere.
    let face = cross(dpdx(input.position), dpdy(input.position));
    let face_squared = dot(face, face);
    let geometric = select(vec3<f32>(0.0), face * inverseSqrt(face_squared), face_squared > 0.0);
    // glTF's MASK alpha mode: below the cutoff, nothing is drawn, not even depth.
    if base.a < material.alpha_cutoff {
        discard;
    }
    let metallic = clamp(material.metallic * metallic_roughness.b, 0.0, 1.0);
    let roughness = clamp(material.roughness * metallic_roughness.g, MIN_ROUGHNESS, 1.0);
    let normal = shading_normal(
        input.normal,
        input.tangent,
        encoded_normal,
        front_facing,
        input.handedness,
    );
    // The camera is at the origin of the frame's space.
    let to_camera = normalize(-input.position);
    // Read once, outside the loop over the lights, whose bound the shader compiler of Direct3D 12
    // cannot unroll.
    var shadow = 1.0;
    if frame.shadowed_light < frame.light_count {
        shadow = light_reaching(input.position, geometric);
    }
    // The loop's bound and each light are the same for every fragment, and every light is
    // evaluated whatever lies around it; a spot light's shadow is read only where its light
    // reaches, at level 0, which control flow that differs between fragments allows.
    var direct = vec3<f32>(0.0);
    for (var i = 0u; i < frame.light_count; i += 1u) {
        let light = frame.lights[i];
        var l = -light.direction;
        // The shadowed light is directional: nothing else attenuates it.
        var attenuation = select(1.0, shadow, i == frame.shadowed_light);
        if light.kind != DIRECTIONAL {
            let to_light = light.position - input.position;
            let distance_squared = dot(to_light, to_light);
            // A fragment on the light has no direction to it, and gets no light rather than NaN.
            // The divisor is bounded so that no infinity is computed where it is not used either,
            // which WGSL lets implementations assume never happens.
            l = select(
                vec3<f32>(0.0),
                to_light * inverseSqrt(max(distance_squared, DIRECTIONLESS_SQUARED)),
                distance_squared > DIRECTIONLESS_SQUARED,
            );
            // KHR_lights_punctual: the inverse square of the distance, brought to 0 at the range
            // by 1 - (d / range)^4, not squared.
            let ratio = distance_squared * light.inverse_range_squared;
            let window = saturate(1.0 - ratio * ratio);
            let cone = saturate(dot(light.direction, -l) * light.cone_scale + light.cone_offset);
            attenuation = window / max(distance_squared, MIN_DISTANCE_SQUARED) * cone * cone;
            // A spot light's shadow, where its light reaches (ADR-0061).
            if light.tile != NO_TILE && attenuation > 0.0 {
                let tile = frame.spot_tiles[light.tile];
                attenuation *= spot_light_reaching(light, tile, input.position, geometric, l);
            }
        }
        direct += min(light.color * attenuation, vec3<f32>(LARGEST_LIGHT))
            * reflected(base.rgb, metallic, roughness, normal, l, to_camera);
    }
    // The environment's light, which occlusion darkens, and it alone (glTF: indirect light). Its
    // level is read from the material's roughness itself: the shading's lower bound avoids 0 / 0
    // in the distribution, which a lookup does not compute, and a mirror reads level 0.
    let surroundings = clamp(material.roughness * metallic_roughness.g, 0.0, 1.0);
    let indirect = environment_light(base.rgb, metallic, surroundings, normal, to_camera)
        * (1.0 + material.occlusion_strength * (occlusion - 1.0));
    // Exposed here, in 32-bit floats, before the half-precision target: light in physical units
    // reaches hundreds of thousands, which half precision does not hold (ADR-0037). Surfaces are
    // opaque: the target's alpha is 1 wherever one is drawn.
    let exposed = (direct + indirect + emitted) * frame.exposure;
    return vec4<f32>(min(exposed / HEADROOM, vec3<f32>(LARGEST_HALF)), 1.0);
}

// What a surface reflects towards v of its environment (image-based lighting, ADR-0039), as the
// Khronos glTF Sample Viewer computes it: the split sum of Karis, the diffuse light read along the
// normal from the roughest level, the specular light along the reflection from the level of the
// roughness, each weighted by a Fresnel term from the BRDF table that accounts for multiple
// scattering (Fdez-Aguera), and mixed as the direct light is (glTF 2.0, Appendix B).
fn environment_light(
    base: vec3<f32>,
    metallic: f32,
    roughness: f32,
    n: vec3<f32>,
    v: vec3<f32>,
) -> vec3<f32> {
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    let diffuse = textureSampleLevel(environment, environment_sampler, n, ROUGHEST_LEVEL).rgb;
    let specular = textureSampleLevel(
        environment,
        environment_sampler,
        reflect(-v, n),
        roughness * ROUGHEST_LEVEL,
    ).rgb;
    // Texel centres on 0 and 1: the end points are read exactly.
    let at = (vec2<f32>(n_dot_v, roughness) * (BRDF_TABLE_SIZE - 1.0) + 0.5) / BRDF_TABLE_SIZE;
    let scale_bias = textureSampleLevel(brdf_table, environment_sampler, at, 0.0).rg;
    let dielectric = mix(
        diffuse * base,
        specular,
        environment_fresnel(vec3<f32>(DIELECTRIC_F0), n_dot_v, roughness, scale_bias),
    );
    let metal = environment_fresnel(base, n_dot_v, roughness, scale_bias) * specular;
    return mix(dielectric, metal, metallic);
}

// The share of a uniform environment that a surface reflecting f0 at normal incidence reflects
// specularly: Fdez-Aguera's roughness-dependent Fresnel times the BRDF table's scale and bias
// (single scattering), plus the light that scatters more than once between microfacets. For f0 = 1
// it is exactly 1: a white metal loses no light.
fn environment_fresnel(
    f0: vec3<f32>,
    n_dot_v: f32,
    roughness: f32,
    scale_bias: vec2<f32>,
) -> vec3<f32> {
    let x = 1.0 - n_dot_v;
    let x2 = x * x;
    let rough_fresnel = f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * (x2 * x2 * x);
    let single = rough_fresnel * scale_bias.x + scale_bias.y;
    let lost = 1.0 - (scale_bias.x + scale_bias.y);
    let average = f0 + (1.0 - f0) / 21.0;
    return single + lost * single * average / (1.0 - average * lost);
}

// The sky pass (ADR-0039): one triangle covering the target at depth 0, infinitely far (ADR-0047),
// which the depth test keeps where no surface was drawn.
struct SkyVertex {
    @builtin(position) clip_position: vec4<f32>,
    // The direction of the pixel: interpolated linearly, since w is 1 at every corner. A point at
    // infinity, its w is 0 and its xyz the direction.
    @location(0) direction: vec4<f32>,
}

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyVertex {
    // The corners (-1, -1), (3, -1) and (-1, 3), as the tone-mapping pass's.
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return SkyVertex(
        vec4<f32>(corner, 0.0, 1.0),
        frame.sky_from_clip * vec4<f32>(corner, 0.0, 1.0),
    );
}

@fragment
fn fs_sky(input: SkyVertex) -> @location(0) vec4<f32> {
    let direction = normalize(input.direction.xyz);
    var light = textureSampleLevel(environment, environment_sampler, direction, 0.0).rgb;
    // The sun's disc, which is in no level of the environment, so that surfaces receive the sun's
    // light once, through the scene's light, which casts shadows. Its edge is blended over a
    // pixel; the distance from its centre is the chord, which keeps its precision where the
    // cosine of so small an angle would not. Below the horizon, the ground's haze hides it.
    let from_centre = length(direction - frame.to_sun);
    let covered = saturate((SUN_CHORD - from_centre) / max(fwidth(from_centre), 1e-7) + 0.5);
    let seen = min(exp(direction.y / GROUND_HAZE), 1.0);
    light += frame.sun_radiance * (covered * seen);
    return vec4<f32>(min(light * frame.exposure / HEADROOM, vec3<f32>(LARGEST_HALF)), 1.0);
}

// The share of the shadowed light that reaches `position`, from 0 in shadow to 1, through its
// cascades (ADR-0064, ADR-0065), the finest first. Each cascade whose square holds the point,
// CASCADE_MARGIN_TEXELS inside its edges, compares it through `compared`, whose filter fades a
// shadow's edge smoothly over about six texels (ADR-0063), moved off its triangle, whose unit
// normal is `normal` (0 for a pixel that sees it edge on), towards the light by
// NORMAL_OFFSET_TEXELS of the cascade's texels times the sine of the light's incidence. A point
// deeper inside a cascade than its band takes that cascade's share of light alone; within the
// band, it takes it in part, from all of it at the band's inner edge to none at its outer edge,
// and the cascades after it, or beyond the last the light itself, give the rest: shadows fade from
// one cascade into the next, and out at the edge of the last, beyond which nothing is shadowed
// (ADR-0050).
fn light_reaching(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let cosine = dot(normal, -frame.lights[frame.shadowed_light].direction);
    let sine = sqrt(max(1.0 - cosine * cosine, 0.0));
    // In texels of the cascade it is read from, on the side of the surface the light reaches.
    let offset = normal * (sign(cosine) * NORMAL_OFFSET_TEXELS * sine);
    let texel = 1.0 / vec2<f32>(textureDimensions(cascade_atlas));
    var lit = 0.0;
    // The share of the light that no cascade has given yet.
    var left = 1.0;
    for (var i = 0u; i < CASCADES && left > 0.0; i += 1u) {
        let cascade = frame.cascades[i];
        let seen = (cascade.view_proj * vec4<f32>(position + offset * cascade.place.w, 1.0)).xyz;
        // How far inside its square the point lies, in its clip space, 2 wide, where a texel is
        // 2 / its tile's side in texels.
        let margin = CASCADE_MARGIN_TEXELS * 2.0 * texel.x / cascade.place.z;
        let held = saturate((1.0 - margin - max(abs(seen.x), abs(seen.y))) / CASCADE_BAND);
        if held > 0.0 {
            // Beyond the far end of the cascade's depths, at depths below 0, a point lies behind
            // every caster that reaches the cascade, and is compared as if it were at that end
            // (ADR-0049).
            let tile = in_tile(cascade.place, seen.xy, texel);
            let share = compared(
                cascade_atlas,
                tile.uv,
                max(seen.z, 0.0),
                texel,
                tile.lowest,
                tile.highest,
            );
            lit += left * held * share;
            left *= 1.0 - held;
        }
    }
    return lit + left;
}

// Where a point seen at `seen`, from -1 to 1 across a tile at `place` of an atlas whose texels are
// `texel` wide, lies in the atlas's texture coordinates, and the corners between which the
// filter's comparisons are kept, half a texel inside the tile, so that it reads none of another
// tile's texels.
struct InTile {
    uv: vec2<f32>,
    lowest: vec2<f32>,
    highest: vec2<f32>,
}

fn in_tile(place: vec4<f32>, seen: vec2<f32>, texel: vec2<f32>) -> InTile {
    // Clip space has y up, texture coordinates y down.
    let across = seen * vec2<f32>(0.5, -0.5) + 0.5;
    return InTile(
        place.xy + across * place.z,
        place.xy + 0.5 * texel,
        place.xy + place.z - 0.5 * texel,
    );
}

// The share of `map` that is lit for a point at `depth` seen at `uv`, through Castaño's filter over
// 5 x 5 texels (ADR-0063): nine comparisons of the depth with the map's, each blending the 2 x 2
// texels nearest to it (percentage-closer filtering), placed and weighed so that together they
// weigh each texel by a smooth bump around the point, three texels wide on each side. A shadow's
// edge then fades smoothly over about six texels, where comparisons one texel apart, equally
// weighed, left the staircase of its texels showing. Each comparison is kept between `lowest` and
// `highest`, so that a tile of the atlas reads none of its neighbours' texels. Sampled at level 0,
// without derivatives: allowed outside uniform control flow.
fn compared(
    map: texture_depth_2d,
    uv: vec2<f32>,
    depth: f32,
    texel: vec2<f32>,
    lowest: vec2<f32>,
    highest: vec2<f32>,
) -> f32 {
    // In texels: the corner between texels nearest to the point, and where the point lies from
    // the texel centre before it, 0 to 1 along each axis.
    let at = uv / texel;
    let corner = floor(at + 0.5);
    let s = at + 0.5 - corner;
    let first = corner - 0.5;
    // Along each axis, three comparisons' weights, which add up to 12, and their places from the
    // texel centre before the corner (Castaño, "Shadow Mapping Summary", 2013).
    var weights = array<vec2<f32>, 3>(4.0 - 3.0 * s, vec2<f32>(7.0), 1.0 + 3.0 * s);
    var places = array<vec2<f32>, 3>(
        (3.0 - 2.0 * s) / weights[0] - 2.0,
        (3.0 + s) / weights[1],
        s / weights[2] + 2.0,
    );
    var lit = 0.0;
    for (var y = 0; y < 3; y += 1) {
        for (var x = 0; x < 3; x += 1) {
            let place = vec2<f32>(places[x].x, places[y].y);
            let tap = clamp((first + place) * texel, lowest, highest);
            let weight = weights[x].x * weights[y].y;
            lit += weight * textureSampleCompareLevel(map, shadow_sampler, tap, depth);
        }
    }
    return lit / 144.0;
}

// The share of the spot light `light` that reaches `position`, from 0 in shadow to 1, through its
// tile of the atlas (ADR-0061): the point moved off its surface, whose unit normal is `normal`,
// along it towards the light `l`, by NORMAL_OFFSET_TEXELS of the tile's texels at the point's
// distance along the light's axis, times the sine of the light's incidence; then compared as the
// directional light's cascades are, within the tile. A point behind the light, or beyond the
// tile's sides, outside what the tile sees, is lit.
fn spot_light_reaching(
    light: Light,
    tile: ShadowTile,
    position: vec3<f32>,
    normal: vec3<f32>,
    l: vec3<f32>,
) -> f32 {
    // The tile's depths are distances along the light's axis, not along its rays: a surface
    // tilted from facing the axis by an angle a lies up to 4.24 sin(a) / cos(i) texels' depth
    // nearer at the filter's farthest points, i its incidence, and moved off itself by 4.5 sin(a)
    // texels it lies 4.5 sin(a) / cos(i) nearer than its own surface along the ray: lit, as the
    // directional light's normal offset keeps it, where the axis and the rays are one.
    let facing_axis = dot(normal, -light.direction);
    let sine = sqrt(max(1.0 - facing_axis * facing_axis, 0.0));
    let along = dot(position - light.position, light.direction);
    let texel_width = tile.place.w * max(along, 0.0);
    let side = sign(dot(normal, l));
    let offset = normal * (side * NORMAL_OFFSET_TEXELS * texel_width * sine);
    let clip = tile.view_proj * vec4<f32>(position + offset, 1.0);
    if clip.w <= 0.0 {
        return 1.0;
    }
    let seen = clip.xyz / clip.w;
    if any(abs(seen.xy) > vec2<f32>(1.0)) {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(spot_atlas));
    let at = in_tile(tile.place, seen.xy, texel);
    return compared(spot_atlas, at.uv, seen.z, texel, at.lowest, at.highest);
}

// The unit normal a fragment is shaded with: the mesh's interpolated normal, tilted by the normal
// texture in tangent space (x along the tangent, y along the bitangent, up the texture's image,
// z along the normal, rebuilt from x and y), and turned around on a face seen from behind, which
// only double-sided materials draw (glTF); `handedness` is the instance's.
fn shading_normal(
    normal: vec3<f32>,
    tangent: vec4<f32>,
    encoded: vec2<f32>,
    front_facing: bool,
    handedness: f32,
) -> vec3<f32> {
    var n = normalize(normal);
    // Without a normal texture the scale is 0, and the mesh's normal is kept exactly.
    if material.normal_scale != 0.0 {
        let tangent_length = length(tangent.xyz);
        // A tangent that its instance flattens to nothing tilts nothing.
        let t = select(vec3<f32>(0.0), tangent.xyz / tangent_length, tangent_length > 0.0);
        // glTF's bitangent, cross(normal, tangent) times w, built per fragment from the
        // interpolated normal, as MikkTSpace recommends.
        let b = cross(n, t) * tangent.w;
        // The texture's x and y; its z is rebuilt from them, the normal being of length one, so
        // that a texture of two channels (BC5) serves as well as one of three: blue is not read.
        let xy = encoded * 2.0 - 1.0;
        let z = sqrt(max(1.0 - dot(xy, xy), 0.0));
        let tilted = vec3<f32>(xy * material.normal_scale, z);
        n = normalize(mat3x3<f32>(t, b, n) * normalize(tilted));
    }
    // A mirrored instance's front faces wind clockwise on screen: the rasterizer calls them back
    // faces.
    let seen_from_the_front = front_facing == (handedness > 0.0);
    return select(-n, n, seen_from_the_front);
}

// What a surface reflects towards v from light of illuminance 1 arriving from l (unit vectors, but
// for an l of zero, which reflects nothing), glTF 2.0 Appendix B: a dielectric diffuses its base
// color (Lambert) under a specular layer, the two mixed by Fresnel; a metal reflects specularly,
// tinted by its base color; the two are blended by metallic, and multiplied by the cosine of
// incidence.
fn reflected(
    base: vec3<f32>,
    metallic: f32,
    roughness: f32,
    n: vec3<f32>,
    l: vec3<f32>,
    v: vec3<f32>,
) -> vec3<f32> {
    let sum = l + v;
    // l + v is zero where the light comes from exactly opposite the camera: n.l or n.v is then 0,
    // and any half vector does.
    let h = select(n, normalize(sum), dot(sum, sum) > 0.0);
    let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
    let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
    let alpha = roughness * roughness;
    let specular = distribution_ggx(n_dot_h, alpha) * visibility_ggx(n_dot_l, n_dot_v, alpha);
    let dielectric = mix(
        base / PI,
        vec3<f32>(specular),
        fresnel_schlick(vec3<f32>(DIELECTRIC_F0), v_dot_h),
    );
    let metal = fresnel_schlick(base, v_dot_h) * specular;
    return mix(dielectric, metal, metallic) * n_dot_l;
}

// Schlick's approximation of Fresnel's reflectance: f0 at normal incidence, rising to 1 at
// grazing incidence. The fifth power by multiplication, exact at v.h = 1 on every driver.
fn fresnel_schlick(f0: vec3<f32>, v_dot_h: f32) -> vec3<f32> {
    let x = 1.0 - v_dot_h;
    let x2 = x * x;
    return f0 + (1.0 - f0) * (x2 * x2 * x);
}

// The GGX (Trowbridge-Reitz) distribution of microfacet normals; alpha = roughness^2.
fn distribution_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let alpha2 = alpha * alpha;
    let f = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * f * f);
}

// Smith's height-correlated masking-shadowing for GGX, divided by 4 (n.l)(n.v).
fn visibility_ggx(n_dot_l: f32, n_dot_v: f32, alpha: f32) -> f32 {
    let alpha2 = alpha * alpha;
    let ggx_v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - alpha2) + alpha2);
    let ggx_l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - alpha2) + alpha2);
    let ggx = ggx_v + ggx_l;
    return select(0.0, 0.5 / ggx, ggx > 0.0);
}
