// The shader of the scene pass (ADR-0034, ADR-0035, ADR-0037): vertices transformed by their
// instance and the camera; surfaces shaded with glTF's metallic-roughness model, with the formulas
// of the Khronos glTF Sample Viewer (glTF 2.0 specification, Appendix B), under the scene's
// directional light, plus its constant ambient light and the material's emission; multiplied by
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

struct FrameUniform {
    // From world space to clip space.
    view_proj: mat4x4<f32>,
    // Towards the light, of length one, in world space; w unused.
    to_light: vec4<f32>,
    // The illuminance the directional light gives a surface facing it; w unused.
    light_color: vec4<f32>,
    // The radiance of the ambient light, the same from every direction; w unused.
    ambient: vec4<f32>,
    // Where the camera is, in world space.
    camera_position: vec3<f32>,
    // What the light is multiplied by before it is written into the high-dynamic-range target
    // (pre-exposure, ADR-0037).
    exposure: f32,
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
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world = instance.model * vec4<f32>(input.position, 1.0);
    output.clip_position = frame.view_proj * world;
    output.world_position = world.xyz;
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
    let direct = frame.light_color.rgb
        * reflected(base.rgb, metallic, roughness, normal, frame.to_light.xyz, to_camera);
    // Until image-based lighting, the ambient light stands for even surroundings, which every
    // surface reflects in its base color, metal or not; occlusion darkens it, and it alone
    // (glTF: indirect light).
    let ambient = frame.ambient.rgb * base.rgb
        * (1.0 + material.occlusion_strength * (occlusion - 1.0));
    // Exposed here, in 32-bit floats, before the half-precision target: light in physical units
    // reaches hundreds of thousands, which half precision does not hold (ADR-0037). Surfaces are
    // opaque: the target's alpha is 1 wherever one is drawn.
    let exposed = (direct + ambient + emitted) * frame.exposure;
    return vec4<f32>(min(exposed, vec3<f32>(LARGEST_HALF)), 1.0);
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

// What a surface reflects towards v from a directional light of illuminance 1 coming from l
// (unit vectors), glTF 2.0 Appendix B: a dielectric diffuses its base color (Lambert) under a
// specular layer, the two mixed by Fresnel; a metal reflects specularly, tinted by its base
// color; the two are blended by metallic, and multiplied by the cosine of incidence.
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
