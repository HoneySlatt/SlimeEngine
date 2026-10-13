// The shader of the scene pass (ADR-0034, ADR-0035, ADR-0037): vertices transformed by their
// instance and the camera; surfaces shaded with glTF's metallic-roughness model, with the formulas
// of the Khronos glTF Sample Viewer (glTF 2.0 specification, Appendix B), under the scene's lights
// (KHR_lights_punctual), plus its constant ambient light and the material's emission; multiplied by
// the exposure and written into the high-dynamic-range target, which the tone-mapping pass then
// maps to the output.

const PI: f32 = 3.141592653589793;
// What a dielectric reflects at normal incidence: ((1.5 - 1) / (1.5 + 1))^2, glTF's index of
// refraction being 1.5.
const DIELECTRIC_F0: f32 = 0.04;
// The smoothest surface shaded, Filament's minimum for 32-bit floats: at a roughness of 0, the
// distribution divides 0 by 0 at the center of a highlight.
const MIN_ROUGHNESS: f32 = 0.045;
// The largest value of the high-dynamic-range target's half-precision floats. Brighter exposed
// light is written as it rather than left to the driver, which may store an infinity that the tone
// curve turns into NaN.
const LARGEST_HALF: f32 = 65504.0;
// The most lights a scene holds: MAX_LIGHTS in light.rs, which a unit test compares with it.
const MAX_LIGHTS: u32 = 16u;
// Light::kind of a directional light. Point lights are 1u and spot lights 2u, which the shader
// treats alike: a point light's cone values give 1 in every direction.
const DIRECTIONAL: u32 = 0u;
// The square of the distance, in square metres, within which a point or spot light gives what it
// gives 1 cm away, as Filament and Bevy do: the inverse square of the distance would grow without
// bound at the light.
const MIN_DISTANCE_SQUARED: f32 = 1e-4;
// The square of the distance, 1 µm squared, within which a fragment has no direction to a point or
// spot light: positions in 32-bit floats a few metres from the origin are about that precise.
const DIRECTIONLESS_SQUARED: f32 = 1e-12;

// One light, in world space, as Light::push_values in light.rs packs it.
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
}

struct FrameUniform {
    // From world space to clip space.
    view_proj: mat4x4<f32>,
    // Where the camera is, in world space.
    camera_position: vec3<f32>,
    // What the light is multiplied by before it is written into the high-dynamic-range target
    // (pre-exposure, ADR-0037).
    exposure: f32,
    // The radiance of the ambient light, in nits, the same from every direction.
    ambient: vec3<f32>,
    // How many entries of `lights` hold the scene's lights, from 0 to MAX_LIGHTS.
    light_count: u32,
    // From world space to the shadow map's clip space (ADR-0038): x and y from -1 to 1 across the
    // map, depth from 0 to 1 away from the light. Orthographic: w stays 1.
    light_view_proj: mat4x4<f32>,
    // The index in `lights` of the light that the shadow map shadows, the scene's first
    // directional light that casts shadows; light_count or more when none is.
    shadowed_light: u32,
    lights: array<Light, MAX_LIGHTS>,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;

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
// Red: the share of the ambient light that reaches the surface.
@group(1) @binding(7) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(8) var occlusion_sampler: sampler;
@group(1) @binding(9) var emissive_texture: texture_2d<f32>;
@group(1) @binding(10) var emissive_sampler: sampler;

struct InstanceUniform {
    // From the mesh's space to world space.
    model: mat4x4<f32>,
    // For normals: proportional to the inverse transpose of the model's rotation and scale, so
    // that normals stay perpendicular to surfaces scaled unevenly. It gives directions only.
    normal: mat3x3<f32>,
    // 1, or -1 for an instance whose transform mirrors it: its front faces wind clockwise on
    // screen, and the cross product that gives its bitangents turns the other way.
    handedness: f32,
}
@group(2) @binding(0) var<uniform> instance: InstanceUniform;

// The depths of what the shadowed light reaches first, drawn by the shadow pass, and the sampler
// that compares a depth with them: 1 where the depth is at most the stored one (lit), blending the
// 2 x 2 nearest texels.
@group(3) @binding(0) var shadow_map: texture_depth_2d;
@group(3) @binding(1) var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // xyz: along increasing u; w: 1 or -1, the sign of the bitangent (glTF).
    @location(3) tangent: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // In world space, of any length: the normal matrix gives directions only, and interpolation
    // across the triangle shortens it.
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) world_position: vec3<f32>,
    // xyz in world space, of any length; w: the bitangent's sign times the handedness.
    @location(3) tangent: vec4<f32>,
    // Where the shadowed light sees the vertex, in the shadow map's clip space, computed as the
    // shadow pass computes it, so that both round alike.
    @location(4) light_position: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world = instance.model * vec4<f32>(input.position, 1.0);
    output.clip_position = frame.view_proj * world;
    output.world_position = world.xyz;
    output.light_position = (frame.light_view_proj * world).xyz;
    output.normal = instance.normal * input.normal;
    output.uv = input.uv;
    // A tangent lies in the surface: the model matrix carries it as it carries the surface. A
    // mirror reverses the cross product that gives the bitangent, which the handedness undoes.
    output.tangent = vec4<f32>(
        (instance.model * vec4<f32>(input.tangent.xyz, 0.0)).xyz,
        input.tangent.w * instance.handedness,
    );
    return output;
}

// The shadow pass (ADR-0038): each vertex where the shadowed light sees it. The pipeline of opaque
// materials has no fragment stage: it writes depth only; masked materials' runs `fs_shadow`.
struct ShadowVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_shadow(@location(0) position: vec3<f32>, @location(2) uv: vec2<f32>) -> ShadowVertex {
    return ShadowVertex(frame.light_view_proj * (instance.model * vec4<f32>(position, 1.0)), uv);
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
    // Every texture is read first, in uniform control flow, before a fragment may be discarded.
    let base = material.base_color
        * textureSample(base_color_texture, base_color_sampler, input.uv);
    let metallic_roughness =
        textureSample(metallic_roughness_texture, metallic_roughness_sampler, input.uv);
    let encoded_normal = textureSample(normal_texture, normal_sampler, input.uv).xyz;
    let occlusion = textureSample(occlusion_texture, occlusion_sampler, input.uv).r;
    let emitted = material.emissive
        * textureSample(emissive_texture, emissive_sampler, input.uv).rgb;
    // glTF's MASK alpha mode: below the cutoff, nothing is drawn, not even depth.
    if base.a < material.alpha_cutoff {
        discard;
    }
    let metallic = clamp(material.metallic * metallic_roughness.b, 0.0, 1.0);
    let roughness = clamp(material.roughness * metallic_roughness.g, MIN_ROUGHNESS, 1.0);
    let normal = shading_normal(input.normal, input.tangent, encoded_normal, front_facing);
    let to_camera = normalize(frame.camera_position - input.world_position);
    // Read once, outside the loop over the lights, whose bound the shader compiler of Direct3D 12
    // cannot unroll.
    var shadow = 1.0;
    if frame.shadowed_light < frame.light_count {
        shadow = light_reaching(input.light_position);
    }
    // The loop's bound and each light are the same for every fragment: control flow stays
    // uniform, and every light is evaluated whatever lies around it.
    var direct = vec3<f32>(0.0);
    for (var i = 0u; i < frame.light_count; i += 1u) {
        let light = frame.lights[i];
        var l = -light.direction;
        // The shadowed light is directional: nothing else attenuates it.
        var attenuation = select(1.0, shadow, i == frame.shadowed_light);
        if light.kind != DIRECTIONAL {
            let to_light = light.position - input.world_position;
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
        }
        direct += light.color * attenuation
            * reflected(base.rgb, metallic, roughness, normal, l, to_camera);
    }
    // Until image-based lighting, the ambient light stands for even surroundings, which every
    // surface reflects in its base color, metal or not; occlusion darkens it, and it alone
    // (glTF: indirect light).
    let ambient = frame.ambient * base.rgb
        * (1.0 + material.occlusion_strength * (occlusion - 1.0));
    // Exposed here, in 32-bit floats, before the half-precision target: light in physical units
    // reaches hundreds of thousands, which half precision does not hold (ADR-0037). Surfaces are
    // opaque: the target's alpha is 1 wherever one is drawn.
    let exposed = (direct + ambient + emitted) * frame.exposure;
    return vec4<f32>(min(exposed, vec3<f32>(LARGEST_HALF)), 1.0);
}

// The share of the shadowed light that reaches `position`, from 0 in shadow to 1: its depth seen
// from the light compared with the shadow map's at 3 x 3 points one texel apart, each comparison
// blending the 2 x 2 texels nearest to it (percentage-closer filtering), so that a shadow's edge
// fades over three texels. Sampled at level 0, without derivatives: allowed outside uniform
// control flow.
fn light_reaching(seen: vec3<f32>) -> f32 {
    // Clip space has y up, texture coordinates y down.
    let uv = seen.xy * vec2<f32>(0.5, -0.5) + 0.5;
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    var lit = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let at = uv + vec2<f32>(f32(x), f32(y)) * texel;
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, at, seen.z);
        }
    }
    return lit / 9.0;
}

// The unit normal a fragment is shaded with: the mesh's interpolated normal, tilted by the normal
// texture in tangent space (x along the tangent, y along the bitangent, up the texture's image,
// z along the normal), and turned around on a face seen from behind, which only double-sided
// materials draw (glTF).
fn shading_normal(
    normal: vec3<f32>,
    tangent: vec4<f32>,
    encoded: vec3<f32>,
    front_facing: bool,
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
        let scale = vec3<f32>(material.normal_scale, material.normal_scale, 1.0);
        n = normalize(mat3x3<f32>(t, b, n) * normalize((encoded * 2.0 - 1.0) * scale));
    }
    // A mirrored instance's front faces wind clockwise on screen: the rasterizer calls them back
    // faces.
    let seen_from_the_front = front_facing == (instance.handedness > 0.0);
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
