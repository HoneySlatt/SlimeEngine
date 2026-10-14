//! The renderer: draws instances of meshes, seen from a camera and shaded with glTF's
//! metallic-roughness materials under a scene's lights and its environment, into a render target,
//! in passes: the shadow map of the first directional light that casts shadows, the scene in high
//! dynamic range, exposed, the sky behind it, the bloom's chain of blurred copies, then tone
//! mapping, which mixes the bloom in.

use std::time::{Duration, Instant};

use glam::{Mat3, Mat4, Vec3};
use slime_gfx::{
    AddressMode, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
    BindingType, Color, CullMode, DepthBias, DepthCompare, DepthPassDescriptor, DepthState,
    DepthTarget, Filter, FragmentState, Gpu, GpuTimer, RenderPass, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RenderTarget, Sampler, SamplerDescriptor,
    ShaderStages, Texture, TextureFormat, UniformBuffer,
};

use crate::bytes::{push_f32s, push_mat3, push_mat4, push_u32s};
use crate::camera::Camera;
use crate::environment::{Environment, Sky, brdf_table};
use crate::error::RenderError;
use crate::light::{LIGHT_VALUES_SIZE, Light, MAX_LIGHTS};
use crate::material::{Material, MaterialData};
use crate::mesh::{Mesh, MeshData, VERTEX_LAYOUT};
use crate::shadow::{SHADOW_BIAS, SHADOW_MAP_SIZE, light_view_projection};

/// The shader of the scene pass.
const SCENE_SHADER: &str = include_str!("shader.wgsl");
/// The shader of the tone-mapping pass.
const TONE_MAPPING_SHADER: &str = include_str!("tone_mapping.wgsl");
/// The shader of the bloom's passes.
const BLOOM_SHADER: &str = include_str!("bloom.wgsl");
/// The bloom's chain stops before a level with a side shorter than this, in texels: its coarsest
/// texel is then an eighth to a sixteenth of the output's shorter side, whatever its resolution, so
/// that the halo reaches the same share of the screen (ADR-0040).
const BLOOM_SMALLEST_SIDE: u32 = 8;
/// The format the scene pass draws into: half-precision floats, which keep light above 1
/// (ADR-0034).
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
/// What the tone-mapping pass clears the output to; its triangle then covers every pixel.
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// The size of the values of a frame, as `FrameUniform` in `shader.wgsl` lays them out: the
/// camera's two `mat4x4<f32>`, its position and the exposure (16 bytes), the shadow map's
/// `mat4x4<f32>`, the number of lights and the index of the light the map shadows (16 bytes), the
/// direction towards the sun's disc and its radiance (16 bytes each), then room for
/// [`MAX_LIGHTS`] lights: 1 280 bytes.
const FRAME_VALUES_SIZE: usize = 2 * 64 + 16 + 64 + 16 + 2 * 16 + MAX_LIGHTS * LIGHT_VALUES_SIZE;
/// The index of the shadowed light when the shadow map shadows none: no light has it.
const NO_SHADOWED_LIGHT: u32 = u32::MAX;
/// The size of the values each instance has in the instance buffer: its `mat4x4<f32>` transform,
/// the `mat3x3<f32>` that transforms its normals (three columns padded to 16 bytes), and its
/// handedness, padded to 16 bytes.
const INSTANCE_VALUES_SIZE: usize = 64 + 3 * 16 + 16;
/// The texture slots of a material, each a texture and its sampler: base color,
/// metallic-roughness, normal, occlusion, emissive (ADR-0035).
const MATERIAL_TEXTURE_SLOTS: u32 = 5;
/// How far below zero the determinant of a transform's normalized columns must be for the
/// transform to mirror (see `is_mirrored`).
const MIRROR_THRESHOLD: f32 = 1e-5;
/// How many instances the instance buffer holds when the renderer is created; it doubles whenever
/// a scene has more.
const INITIAL_INSTANCE_CAPACITY: usize = 64;

/// One thing to draw: a mesh, its material, and where it is.
#[derive(Clone, Copy, Debug)]
pub struct Instance<'a> {
    /// What to draw.
    pub mesh: &'a Mesh,
    /// How it looks.
    pub material: &'a Material,
    /// From the mesh's space to world space. A transform that mirrors the mesh (a negative
    /// determinant, beyond rounding) keeps the mesh's front faces, as glTF does: the renderer
    /// culls its other faces.
    pub transform: Mat4,
}

/// What the renderer draws in one frame.
///
/// Light is in physical units (ADR-0037): lights in lux and candelas, the light that surfaces
/// reflect and emit and the environment's in nits (candelas per square metre),
/// distances in metres. The scene pass multiplies that light by the exposure before it writes it
/// into the high-dynamic-range target (pre-exposure), divided by 64 so that its half-precision
/// floats, which reach 65 504, hold exposed light up to 64 × 65 504 (ADR-0040): exposed light above
/// it is clipped to it.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// Where the scene is seen from.
    pub camera: Camera,
    /// What lights the surfaces, at most [`MAX_LIGHTS`]: [`Renderer::render`] panics on more.
    pub lights: &'a [Light],
    /// What surrounds the scene (ADR-0039): its light reaches every surface, reflected as glTF's
    /// materials reflect it (image-based lighting), and it shows where no surface is drawn, with
    /// its sun's disc. `None` is a black sky: only the lights and emission light surfaces, and the
    /// background is black. Created by [`Renderer::create_environment`].
    pub environment: Option<&'a Environment>,
    /// What the scene's light, the sky's included, is multiplied by before it is written
    /// into the high-dynamic-range target, then tone mapped (ADR-0034, ADR-0037): a linear factor,
    /// the inverse of the luminance, in nits, that shows as about 0.87 (sRGB 240). 2 is one
    /// photographic stop brighter; 0 gives black. A white matte surface facing a light of E lux
    /// reflects 0.97 E / π nits, which an exposure of π / E shows at sRGB 239. A camera of exposure
    /// value EV100 (at ISO 100) has an exposure of 1 / (1.2 × 2^EV100) in the convention that
    /// Frostbite and Filament follow: about 1 / 39 000 for EV100 15, a photographer's exposure for
    /// direct sunlight. Finite and not negative.
    pub exposure: f32,
    /// The share of each pixel's exposed light that the bloom spreads around where it lands, as a
    /// camera's lens and the eye spread light (ADR-0040): from 0, no bloom, to 1, where the image
    /// is replaced by its blurred copy. The blurred copy averages the image over every scale from a
    /// few pixels to about an eighth of the screen, with as much light at each scale, so that a
    /// pixel's halo falls with the square of the distance and looks the same at any resolution.
    /// What is spread is taken from where it lands, before tone mapping: a uniform image is
    /// unchanged, and light is moved, never added. Every pixel blooms, whatever its brightness:
    /// only the contrast makes a halo visible. 0.04, the samples' strength, gives the sun's disc a
    /// glow over about a third of the screen, softens edges between light and shade by about 4 % of
    /// their contrast, and makes surfaces a few times brighter than their surroundings glow
    /// faintly. 0 skips the bloom's passes and draws what a renderer without bloom draws; outputs
    /// smaller than 15 × 15 pixels have no bloom. Finite, from 0 to 1.
    pub bloom: f32,
    /// What to draw, in this order.
    pub instances: &'a [Instance<'a>],
}

/// What drawing a scene took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderStats {
    /// Draw calls of the scene pass: one per instance. The single draws of the sky and
    /// tone-mapping passes are not counted.
    pub draw_calls: u32,
    /// Triangles the scene pass submitted to the GPU, before culling.
    pub triangles: u64,
    /// How long [`Renderer::render`] took on the CPU: packing the values, recording the passes and
    /// submitting them.
    pub cpu_time: Duration,
    /// How long the GPU took to draw the scene into the high-dynamic-range target: the latest time
    /// that has come back, a few frames old. `None` on adapters that cannot time their work
    /// ([`Gpu::can_time`](slime_gfx::Gpu::can_time)), and until the first time comes back.
    pub scene_gpu_time: Option<Duration>,
    /// How long the GPU took for the tone-mapping pass, under the same rules. It may come from
    /// another frame than `scene_gpu_time`, and their sum is not the GPU time of a frame
    /// (ADR-0033).
    pub tone_mapping_gpu_time: Option<Duration>,
    /// Draw calls of the shadow pass, which draws the shadows of the scene's first directional
    /// light that casts shadows (ADR-0038): one per instance. None when no directional light of
    /// the scene casts shadows, or when every instance is flattened to a point, which casts
    /// nothing: the pass is then skipped.
    pub shadow_draw_calls: u32,
    /// How long the GPU took to draw the shadow map, under the same rules as `scene_gpu_time`;
    /// `None` too for a frame that skipped the shadow pass.
    pub shadow_gpu_time: Option<Duration>,
    /// How long the GPU took to draw the sky behind the scene, under the same rules as
    /// `scene_gpu_time`.
    pub sky_gpu_time: Option<Duration>,
    /// How long the GPU took for the bloom's passes (ADR-0040): the sum of the latest time of each,
    /// measured by a timer of its own, under the same rules as `scene_gpu_time`; the passes' times
    /// may come from different frames. `None` too for a frame that skipped the bloom (a strength
    /// of 0, or an output smaller than 15 × 15), and until every pass's time has come back.
    pub bloom_gpu_time: Option<Duration>,
}

/// Draws scenes into output targets of one format.
///
/// Each frame takes these passes (ADR-0034, ADR-0037, ADR-0038, ADR-0039, ADR-0040): the shadow
/// pass draws the depths of the instances, as the scene's first directional light that casts
/// shadows sees them, into the shadow map; the scene pass draws the instances into a target of
/// half-precision floats, where light may exceed 1, with a depth target, their light multiplied by
/// the scene's exposure, that light's only where the shadow map shows nothing nearer to it, and the
/// environment's; the sky pass draws the environment where no surface is; the bloom's passes filter
/// that light down into a chain of levels, each half the size of the one before, then average them
/// back up into a blurred copy; the tone-mapping pass then mixes the blurred copy in by the scene's
/// bloom strength and maps the light to the output's range with Khronos PBR Neutral. The renderer
/// owns ten pipelines: two for the shadow pass (the second discards what masked materials
/// discard), three for the scene pass, which differ by the faces they cull and among which each
/// draw chooses (ADR-0035), the sky pass's, the bloom's two (down and up) and the tone-mapping
/// pass's; the table of the BRDF's scale and bias and a black environment for scenes without one;
/// the shadow map, 2048 × 2048 depths created with the renderer; the high-dynamic-range and depth
/// targets and the bloom's levels, which it creates at the size of the output it draws into and
/// again when that size changes; the buffer of the instances' values; and a GPU timer per pass.
pub struct Renderer {
    /// The output's format.
    format: TextureFormat,
    scene_pipelines: ScenePipelines,
    tone_mapping_pipeline: RenderPipeline,
    material_layout: BindGroupLayout,
    instance_layout: BindGroupLayout,
    /// The layout of the group through which the tone-mapping pass reads the scene's target.
    tone_mapping_layout: BindGroupLayout,
    frame_uniform: UniformBuffer,
    /// The layout of the frame's group: the frame's values, the environment, the BRDF table and
    /// their sampler.
    frame_layout: BindGroupLayout,
    /// The environment the frame's group was last built with, kept alive by it, and that group.
    frame_group: (Environment, BindGroup),
    /// One black texel, the environment of scenes without one.
    no_environment: Environment,
    brdf_table: Texture,
    /// Reads the environment and the BRDF table.
    environment_sampler: Sampler,
    /// Draws the environment where no surface is.
    sky_pipeline: RenderPipeline,
    /// One white pixel, bound in every texture slot that a material leaves empty: every slot reads
    /// it as 1 in each channel, since sRGB's 255 decodes to exactly 1.
    white: Texture,
    /// The sampler of that pixel.
    default_sampler: Sampler,
    instances: InstanceSlots,
    /// Created by the first `render`, and again when the output's size changes.
    targets: Option<SizedTargets>,
    /// Draws the instances' depths as the shadowed light sees them, culling no face.
    shadow_pipeline: RenderPipeline,
    /// The same, discarding what masked materials discard.
    masked_shadow_pipeline: RenderPipeline,
    /// The depths the shadow pass draws, which the scene pass compares with: created with the
    /// renderer, at a size of its own.
    shadow_map: DepthTarget,
    /// The shadow map and its comparison sampler, bound at group 3 of the scene pass.
    shadow_group: BindGroup,
    /// Time each pass on the GPU, when the adapter can.
    scene_timer: Option<GpuTimer>,
    tone_mapping_timer: Option<GpuTimer>,
    shadow_timer: Option<GpuTimer>,
    sky_timer: Option<GpuTimer>,
    /// One per pass of the bloom, as many as the longest chain drawn so far needed: a resize keeps
    /// them, and their times.
    bloom_timers: Vec<GpuTimer>,
    /// Filters a level down from the one above, the scene's light for the first.
    bloom_down_pipeline: RenderPipeline,
    /// Averages a level with the coarser ones.
    bloom_up_pipeline: RenderPipeline,
    /// The layouts of the groups that the bloom's passes read: down, the finer level and the
    /// sampler; up, the coarser average, the sampler, the level's own light and its weight.
    bloom_down_layout: BindGroupLayout,
    bloom_up_layout: BindGroupLayout,
    /// Linear, mirrored at the edges: reads the levels between their texels, and beyond them their
    /// texels mirrored, so that a pixel near an edge keeps its light and none wraps around.
    bloom_sampler: Sampler,
    /// The strength the tone-mapping pass mixes the blurred copy in by, written every frame.
    bloom_strength: UniformBuffer,
}

/// The targets that follow the output's size: what the scene pass draws into, the bloom's levels,
/// and the groups through which the bloom's passes and the tone-mapping pass read them. They are
/// created together, and again together when the size changes, since a group keeps the target it
/// was created with (ADR-0033).
struct SizedTargets {
    /// The scene's light, in high dynamic range. Labelled "scene", which names the scene pass in
    /// capture tools.
    hdr: RenderTarget,
    depth: DepthTarget,
    tone_mapping_group: BindGroup,
    /// From the finest, half the output's size, to the coarsest; empty for outputs smaller than
    /// 15 × 15.
    bloom: Vec<BloomLevel>,
}

/// One level of the bloom's chain (ADR-0040).
struct BloomLevel {
    /// The level above filtered down to this one's size: the scene's light for the first level.
    down: RenderTarget,
    /// Reads the level above.
    down_group: BindGroup,
    /// The average of this level's light and the coarser levels', and the group that reads both
    /// and the level's weight; the coarsest level has none, its average being its own light.
    up: Option<(RenderTarget, BindGroup)>,
}

/// The sizes of the bloom's levels for an output of `width` × `height` pixels: level i (from 1) is
/// the output divided by 2^i, rounded to the nearest, while both sides keep at least
/// [`BLOOM_SMALLEST_SIDE`] texels. Empty below 15 × 15. Each side is then half the one before,
/// rounded down or up, so that a pixel's position doubled or halved falls where it should; and each
/// level's edge stays within half a texel of the output's, which keeps the light of pixels near the
/// right and bottom edges within about 20 % where the output's sides are not multiples of 2^i
/// (ADR-0040), against twice or ten times their light with other roundings.
fn bloom_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let level = |side: u32, index: u32| (side + (1 << (index - 1))) >> index;
    (1..)
        .map(|index| (level(width, index), level(height, index)))
        .take_while(|&(level_width, level_height)| {
            level_width >= BLOOM_SMALLEST_SIDE && level_height >= BLOOM_SMALLEST_SIDE
        })
        .collect()
}

impl Renderer {
    /// Creates a renderer that draws into output targets of `format`: the window's, for a
    /// windowed application (`Context::surface_format` in `slime-app`). An sRGB format encodes the
    /// tone-mapped light for display; `Rgba16Float` holds the tone-mapped values themselves;
    /// `Rgba8Unorm` stores them unencoded, which displays too dark. The first renderer of a process
    /// computes the table of the BRDF's scale and bias for image-based lighting on the CPU, on
    /// every core (ADR-0039): about 15 ms in a release build, more in a debug build; the others
    /// reuse it.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] if the graphics driver rejects one of the renderer's three shaders or
    /// ten pipelines, or if the device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let shader = gpu.create_shader_module("scene", SCENE_SHADER)?;
        let uniform = |label: &str, dynamic_offset: bool| {
            gpu.create_bind_group_layout(
                label,
                &[BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::VertexAndFragment,
                    ty: BindingType::UniformBuffer { dynamic_offset },
                }],
            )
        };
        // The frame's values, then the environment, the BRDF table and the sampler that reads both.
        let frame_layout = gpu.create_bind_group_layout(
            "frame",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::VertexAndFragment,
                    ty: BindingType::UniformBuffer {
                        dynamic_offset: false,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::CubeTexture,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Sampler,
                },
            ],
        );
        // The material's values, then each texture slot: a texture and its sampler.
        let mut material_entries = vec![BindGroupLayoutEntry {
            binding: 0,
            visible_to: ShaderStages::Fragment,
            ty: BindingType::UniformBuffer {
                dynamic_offset: false,
            },
        }];
        for slot in 0..MATERIAL_TEXTURE_SLOTS {
            for (binding, ty) in [
                (1 + 2 * slot, BindingType::Texture),
                (2 + 2 * slot, BindingType::Sampler),
            ] {
                material_entries.push(BindGroupLayoutEntry {
                    binding,
                    visible_to: ShaderStages::Fragment,
                    ty,
                });
            }
        }
        let material_layout = gpu.create_bind_group_layout("material", &material_entries);
        let instance_layout = uniform("instance", true);
        // Apart from the frame's group, which the shadow pass binds: a pass cannot bind what it
        // draws into.
        let shadow_layout = gpu.create_bind_group_layout(
            "shadow map",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::DepthTexture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::ComparisonSampler,
                },
            ],
        );
        let scene_pipeline = |label: &str, cull: CullMode| {
            gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label,
                shader: &shader,
                vertex_entry_point: "vs_main",
                vertex_layout: Some(VERTEX_LAYOUT),
                fragment: Some(FragmentState {
                    entry_point: "fs_main",
                    target_format: Some(HDR_FORMAT),
                }),
                bind_group_layouts: &[
                    &frame_layout,
                    &material_layout,
                    &instance_layout,
                    &shadow_layout,
                ],
                depth: Some(DepthState {
                    compare: DepthCompare::Less,
                    write: true,
                    bias: DepthBias::default(),
                }),
                cull,
            })
        };
        // Every face casts, whichever way it faces the light: a single-sided surface stops light
        // from behind too, and mirrored instances need no other pipeline.
        let shadow_pipeline = |label: &str, fragment: Option<FragmentState<'_>>| {
            gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label,
                shader: &shader,
                vertex_entry_point: "vs_shadow",
                vertex_layout: Some(VERTEX_LAYOUT),
                fragment,
                bind_group_layouts: &[&frame_layout, &material_layout, &instance_layout],
                depth: Some(DepthState {
                    compare: DepthCompare::Less,
                    write: true,
                    bias: SHADOW_BIAS,
                }),
                cull: CullMode::None,
            })
        };
        let masked_shadow_pipeline = shadow_pipeline(
            "shadow, masked",
            Some(FragmentState {
                entry_point: "fs_shadow",
                target_format: None,
            }),
        )?;
        let shadow_pipeline = shadow_pipeline("shadow", None)?;
        let shadow_map = gpu.create_depth_target("shadow map", SHADOW_MAP_SIZE, SHADOW_MAP_SIZE);
        let shadow_sampler =
            gpu.create_comparison_sampler("shadow map", DepthCompare::LessEqual, Filter::Linear);
        let shadow_group = gpu.create_bind_group(
            "shadow map",
            &shadow_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::DepthTarget(&shadow_map),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::ComparisonSampler(&shadow_sampler),
                },
            ],
        );
        let scene_pipelines = ScenePipelines {
            back_culled: scene_pipeline("scene, back faces culled", CullMode::Back)?,
            front_culled: scene_pipeline("scene, front faces culled", CullMode::Front)?,
            unculled: scene_pipeline("scene, no face culled", CullMode::None)?,
        };
        // Where the depth target still holds the farthest depth, which the sky's triangle has.
        let sky_pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "sky",
            shader: &shader,
            vertex_entry_point: "vs_sky",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_sky",
                target_format: Some(HDR_FORMAT),
            }),
            bind_group_layouts: &[&frame_layout],
            depth: Some(DepthState {
                compare: DepthCompare::LessEqual,
                write: false,
                bias: DepthBias::default(),
            }),
            cull: CullMode::None,
        })?;

        let tone_mapping_shader = gpu.create_shader_module("tone mapping", TONE_MAPPING_SHADER)?;
        let fragment_entries = |types: &[BindingType]| -> Vec<BindGroupLayoutEntry> {
            (0..)
                .zip(types)
                .map(|(binding, &ty)| BindGroupLayoutEntry {
                    binding,
                    visible_to: ShaderStages::Fragment,
                    ty,
                })
                .collect()
        };
        let values = BindingType::UniformBuffer {
            dynamic_offset: false,
        };
        // The scene's light, the bloom's blurred copy, the sampler that magnifies it, the strength.
        let tone_mapping_layout = gpu.create_bind_group_layout(
            "tone mapping",
            &fragment_entries(&[
                BindingType::Texture,
                BindingType::Texture,
                BindingType::Sampler,
                values,
            ]),
        );
        let bloom_shader = gpu.create_shader_module("bloom", BLOOM_SHADER)?;
        let bloom_down_layout = gpu.create_bind_group_layout(
            "bloom down",
            &fragment_entries(&[BindingType::Texture, BindingType::Sampler]),
        );
        let bloom_up_layout = gpu.create_bind_group_layout(
            "bloom up",
            &fragment_entries(&[
                BindingType::Texture,
                BindingType::Sampler,
                BindingType::Texture,
                values,
            ]),
        );
        let bloom_pipeline = |label: &str, entry_point: &str, layout: &BindGroupLayout| {
            gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label,
                shader: &bloom_shader,
                vertex_entry_point: "vs_main",
                vertex_layout: None,
                fragment: Some(FragmentState {
                    entry_point,
                    target_format: Some(HDR_FORMAT),
                }),
                bind_group_layouts: &[layout],
                depth: None,
                cull: CullMode::None,
            })
        };
        let bloom_down_pipeline =
            bloom_pipeline("bloom down", "fs_downsample", &bloom_down_layout)?;
        let bloom_up_pipeline = bloom_pipeline("bloom up", "fs_upsample", &bloom_up_layout)?;
        let bloom_sampler = gpu.create_sampler(
            "bloom",
            &SamplerDescriptor {
                address_u: AddressMode::MirrorRepeat,
                address_v: AddressMode::MirrorRepeat,
                ..SamplerDescriptor::default()
            },
        );
        let bloom_strength = gpu.create_uniform_buffer("bloom strength", &[0; 16]);
        let tone_mapping_pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "tone mapping",
            shader: &tone_mapping_shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(format),
            }),
            bind_group_layouts: &[&tone_mapping_layout],
            depth: None,
            cull: CullMode::None,
        })?;
        let frame_uniform = gpu.create_uniform_buffer("frame", &[0; FRAME_VALUES_SIZE]);
        let no_environment = Environment::black(gpu);
        let brdf_table = brdf_table(gpu);
        let environment_sampler = gpu.create_sampler(
            "environment",
            &SamplerDescriptor {
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
                ..SamplerDescriptor::default()
            },
        );
        let frame_group = frame_group(
            gpu,
            &frame_layout,
            &frame_uniform,
            &no_environment,
            &brdf_table,
            &environment_sampler,
        );
        let white = gpu.create_texture(
            "white",
            1,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 255, 255, 255],
        );
        let default_sampler = gpu.create_sampler("default", &SamplerDescriptor::default());
        let instances = InstanceSlots::new(gpu, &instance_layout, INITIAL_INSTANCE_CAPACITY);
        let scene_timer = gpu.create_timer("scene");
        let tone_mapping_timer = gpu.create_timer("tone mapping");
        let shadow_timer = gpu.create_timer("shadow");
        let sky_timer = gpu.create_timer("sky");
        if scene_timer.is_none() {
            tracing::info!("the adapter cannot time its work: the statistics have no GPU time");
        }
        Ok(Self {
            format,
            scene_pipelines,
            tone_mapping_pipeline,
            material_layout,
            instance_layout,
            tone_mapping_layout,
            frame_uniform,
            frame_layout,
            frame_group: (no_environment.clone(), frame_group),
            no_environment,
            brdf_table,
            environment_sampler,
            sky_pipeline,
            white,
            default_sampler,
            instances,
            targets: None,
            shadow_pipeline,
            masked_shadow_pipeline,
            shadow_map,
            shadow_group,
            scene_timer,
            tone_mapping_timer,
            shadow_timer,
            sky_timer,
            bloom_timers: Vec::new(),
            bloom_down_pipeline,
            bloom_up_pipeline,
            bloom_down_layout,
            bloom_up_layout,
            bloom_sampler,
            bloom_strength,
        })
    }

    /// Computes the environment of `sky` on the CPU and uploads it (ADR-0039): the sky drawn into
    /// a cube map of 128 × 128 texels per face, each the average of 2 × 2 points of it, then
    /// prefiltered for roughnesses 0.25, 0.5, 0.75 and 1 into faces of 64, 32, 16 and 8 texels.
    /// This takes tens of milliseconds, on every core: create an environment once per sky, not per
    /// frame. A sky of one radiance in every direction, without a sun, takes almost none. The
    /// environment is not tied to this renderer.
    ///
    /// # Panics
    ///
    /// If the zenith's, the horizon's or the ground's radiance is negative, not finite, or above
    /// 65 504 nits, the largest value of the environment's half-precision texels: this is a bug in
    /// the caller. A clear sky stays below about 30 000 nits away from the sun.
    pub fn create_environment(&self, gpu: &Gpu, sky: &Sky) -> Environment {
        Environment::new(gpu, sky)
    }

    /// Uploads a mesh. `label` names its buffers in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] if `data` cannot be drawn: a vertex array (normals, uvs,
    /// tangents) of another length than the positions, no vertex, no triangle, a number of indices
    /// that is not a multiple of three, an index that names no vertex, or vertices or indices that
    /// need a buffer larger than the device accepts ([`Gpu::max_buffer_size`], 256 MiB with the
    /// default limits: 5 592 405 vertices, or 67 108 864 indices).
    pub fn create_mesh(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &MeshData,
    ) -> Result<Mesh, RenderError> {
        Mesh::new(gpu, label, data)
    }

    /// Creates a material. `label` names it in error messages and in GPU capture tools. The
    /// material keeps its textures and samplers alive.
    ///
    /// # Panics
    ///
    /// If a texture's format does not suit its slot: the base color and emissive textures hold
    /// colors and need an sRGB format; the metallic-roughness, normal and occlusion textures hold
    /// data and need a format without sRGB encoding, `Rgba8Unorm`. Read in the other kind of
    /// format, their values would be wrong: this is a bug in the caller.
    pub fn create_material(&self, gpu: &Gpu, label: &str, data: &MaterialData<'_>) -> Material {
        let uniform = gpu.create_uniform_buffer(label, &data.values());
        // Each slot: its name, its texture, and whether it holds colors (sRGB) rather than data.
        let slots = [
            ("base color", data.base_color_texture, true),
            ("metallic-roughness", data.metallic_roughness_texture, false),
            ("normal", data.normal_texture, false),
            ("occlusion", data.occlusion_texture, false),
            ("emissive", data.emissive_texture, true),
        ];
        let mut entries = vec![BindGroupEntry {
            binding: 0,
            resource: BindingResource::UniformBuffer(&uniform),
        }];
        for (binding, (slot, sampled, colors)) in (1u32..).step_by(2).zip(slots) {
            let (texture, sampler) = match sampled {
                Some(sampled) => {
                    let format = sampled.texture.format();
                    assert!(
                        format.is_srgb() == colors,
                        "material `{label}`: its {slot} texture is {format:?}, but the {slot} \
                         slot reads {}",
                        if colors {
                            "colors, from an sRGB format"
                        } else {
                            "data, from Rgba8Unorm"
                        }
                    );
                    (sampled.texture, sampled.sampler)
                }
                None => (&self.white, &self.default_sampler),
            };
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::Texture(texture),
            });
            entries.push(BindGroupEntry {
                binding: binding + 1,
                resource: BindingResource::Sampler(sampler),
            });
        }
        let group = gpu.create_bind_group(label, &self.material_layout, &entries);
        Material {
            group,
            double_sided: data.double_sided,
            masked: matches!(data.alpha_mode, crate::material::AlphaMode::Mask { .. }),
        }
    }

    /// Draws `scene` into `target`, in passes. The shadow pass draws into the shadow map the
    /// depths of every instance, all their faces (the parts that masked materials discard
    /// excepted), as the first directional light of `scene.lights` that casts shadows sees them,
    /// through an orthographic projection that covers the boxes around the instances' meshes
    /// (ADR-0038); it is skipped when no directional light casts shadows. The scene pass draws
    /// every instance into a high-dynamic-range target with the depth test, so that nearer surfaces
    /// hide farther ones whatever their order, each surface shaded by the scene's light with glTF's
    /// metallic-roughness model and its material's textures, the shadowed light's only where the
    /// shadow map holds nothing nearer to it, its edges blended over three of the map's texels, and
    /// culled or not as its material and its transform ask (ADR-0035), the scene's environment
    /// reflected as its material reflects it (ADR-0039), its light multiplied by the scene's
    /// exposure (ADR-0037). Point and spot lights cast no shadows. The sky pass draws the
    /// environment, exposed, where no surface was drawn, with its sun's disc. Unless `scene.bloom`
    /// is 0 or `target` is smaller than 15 × 15, the bloom's passes filter that light down into
    /// levels of half the size of the one before, to a side of at least 8 texels, with Jimenez's
    /// 13-tap filter, then average them back up through 3 × 3 tents, every level with the same
    /// weight, into a blurred copy (ADR-0040): 11 passes at 1280 × 720, 13 at 1920 × 1080. The
    /// tone-mapping pass then mixes the blurred copy in by `scene.bloom`, maps that light to the
    /// range a display shows, and writes it into `target`, opaque. The work runs on the GPU after
    /// this method returns, as with [`Gpu::render`](slime_gfx::Gpu::render).
    ///
    /// The high-dynamic-range and depth targets and the bloom's levels (4.7 MiB at 1280 × 720,
    /// 10.5 MiB at 1920 × 1080) are created at the size of `target` the first time, and again
    /// whenever that size changes; the instance buffer grows when a scene has more
    /// instances than before. The shadow map keeps its size: its texels are the extent of the
    /// instances across the light divided by 2048, so that a scene that spreads wider gets coarser
    /// shadows.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the renderer was created for, if the scene has more
    /// than [`MAX_LIGHTS`] lights, or if the instances need a buffer larger than the device allows
    /// (256 MiB with the default limits: about a million instances): these are bugs in the caller.
    ///
    /// With glam's assertions on (see [`Camera`]), also for a camera or a light that would
    /// otherwise draw nothing usable: as [`Camera::view_projection`] does, and for a zero
    /// [`DirectionalLight::direction`](crate::DirectionalLight::direction) or
    /// [`SpotLight::direction`](crate::SpotLight::direction).
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, scene: &Scene<'_>) -> RenderStats {
        let started = Instant::now();
        assert!(
            scene.lights.len() <= MAX_LIGHTS,
            "the scene has {} lights; the renderer draws at most {MAX_LIGHTS}",
            scene.lights.len()
        );
        assert!(
            target.format() == self.format,
            "the renderer draws into {:?} targets, not into `{}`, which is {:?}",
            self.format,
            target.label(),
            target.format()
        );
        let (width, height) = (target.width(), target.height());
        if self
            .targets
            .as_ref()
            .is_none_or(|targets| (targets.hdr.width(), targets.hdr.height()) != (width, height))
        {
            tracing::debug!(
                width,
                height,
                "creating the high-dynamic-range, depth and bloom targets"
            );
            self.targets = Some(self.create_sized_targets(gpu, width, height));
        }
        let targets = self.targets.as_ref().expect("created above");
        // The bloom's passes: one per level down, one per level but the coarsest up.
        let bloom_passes = (2 * targets.bloom.len()).saturating_sub(1);
        let bloom = scene.bloom > 0.0 && bloom_passes > 0;
        if bloom {
            while self.bloom_timers.len() < bloom_passes {
                match gpu.create_timer(&format!("bloom {}", self.bloom_timers.len())) {
                    Some(timer) => self.bloom_timers.push(timer),
                    None => break,
                }
            }
        }
        let mut strength = Vec::with_capacity(16);
        push_f32s(
            &mut strength,
            &[if bloom { scene.bloom } else { 0.0 }, 0.0, 0.0, 0.0],
        );
        gpu.write_uniform_buffer(&self.bloom_strength, &strength);

        // The first directional light that casts shadows casts them, when an instance has a box
        // to cast them (ADR-0038).
        let shadow = scene
            .lights
            .iter()
            .enumerate()
            .find_map(|(index, light)| match light {
                Light::Directional(light) if light.casts_shadows => Some((index, light.direction)),
                _ => None,
            })
            .and_then(|(index, direction)| {
                let boxes = scene.instances.iter().map(|instance| {
                    (
                        instance.transform,
                        instance.mesh.center,
                        instance.mesh.half_size,
                    )
                });
                Some((index, light_view_projection(direction, boxes)?))
            });

        let aspect = width as f32 / height as f32;
        let mut frame_bytes = Vec::with_capacity(FRAME_VALUES_SIZE);
        push_mat4(&mut frame_bytes, scene.camera.view_projection(aspect));
        // Directions only: the view without the camera's position, which also keeps the inverse
        // away from the rounding of large coordinates.
        let rotation = Mat4::from_mat3(Mat3::from_mat4(scene.camera.view()));
        push_mat4(
            &mut frame_bytes,
            (scene.camera.projection(aspect) * rotation).inverse(),
        );
        let eye = scene.camera.eye;
        push_f32s(&mut frame_bytes, &[eye.x, eye.y, eye.z, scene.exposure]);
        let (shadowed, light_view_proj) = shadow
            .map_or((NO_SHADOWED_LIGHT, Mat4::IDENTITY), |(index, matrix)| {
                (u32::try_from(index).expect("checked above"), matrix)
            });
        push_mat4(&mut frame_bytes, light_view_proj);
        push_u32s(
            &mut frame_bytes,
            &[
                u32::try_from(scene.lights.len()).expect("checked above"),
                shadowed,
                0,
                0,
            ],
        );
        let environment = scene.environment.unwrap_or(&self.no_environment);
        let (to_sun, sun) = (environment.to_sun, environment.sun_radiance);
        push_f32s(&mut frame_bytes, &[to_sun.x, to_sun.y, to_sun.z, 0.0]);
        push_f32s(&mut frame_bytes, &[sun.x, sun.y, sun.z, 0.0]);
        for light in scene.lights {
            light.push_values(&mut frame_bytes);
        }
        // The slots of the lights the scene does not have: the shader does not read them.
        frame_bytes.resize(FRAME_VALUES_SIZE, 0);
        gpu.write_uniform_buffer(&self.frame_uniform, &frame_bytes);
        if !self.frame_group.0.is(environment) {
            self.frame_group = (
                environment.clone(),
                frame_group(
                    gpu,
                    &self.frame_layout,
                    &self.frame_uniform,
                    environment,
                    &self.brdf_table,
                    &self.environment_sampler,
                ),
            );
        }
        let frame_group = &self.frame_group.1;

        if self.instances.capacity < scene.instances.len() {
            let mut capacity = self.instances.capacity.max(1);
            while capacity < scene.instances.len() {
                capacity *= 2;
            }
            tracing::debug!(capacity, "growing the instance buffer");
            self.instances = InstanceSlots::new(gpu, &self.instance_layout, capacity);
        }
        let stride = self.instances.stride;
        let mut instance_bytes = vec![0; self.instances.capacity * stride];
        for (slot, instance) in scene.instances.iter().enumerate() {
            let mut bytes = Vec::with_capacity(INSTANCE_VALUES_SIZE);
            push_mat4(&mut bytes, instance.transform);
            push_mat3(&mut bytes, normal_matrix(instance.transform));
            let handedness = if is_mirrored(instance.transform) {
                -1.0
            } else {
                1.0
            };
            push_f32s(&mut bytes, &[handedness, 0.0, 0.0, 0.0]);
            instance_bytes[slot * stride..slot * stride + INSTANCE_VALUES_SIZE]
                .copy_from_slice(&bytes);
        }
        gpu.write_uniform_buffer(&self.instances.buffer, &instance_bytes);

        let mut stats = RenderStats::default();
        if shadow.is_some() {
            let shadow_pass = DepthPassDescriptor {
                depth: &self.shadow_map,
                timer: self.shadow_timer.as_ref(),
            };
            gpu.render_depth(&shadow_pass, |pass| {
                pass.set_bind_group(0, frame_group, &[]);
                let mut bound = None;
                for (slot, instance) in scene.instances.iter().enumerate() {
                    let masked = instance.material.masked;
                    if bound != Some(masked) {
                        pass.set_pipeline(if masked {
                            &self.masked_shadow_pipeline
                        } else {
                            &self.shadow_pipeline
                        });
                        bound = Some(masked);
                    }
                    self.instances.draw(pass, slot, instance);
                    stats.shadow_draw_calls += 1;
                }
            });
            stats.shadow_gpu_time = self.shadow_timer.as_ref().and_then(GpuTimer::latest);
        }
        let scene_pass = RenderPassDescriptor {
            target: &targets.hdr,
            // The sky pass draws every pixel that no surface covers.
            clear: Some(BLACK),
            depth: Some(&targets.depth),
            timer: self.scene_timer.as_ref(),
        };
        gpu.render(&scene_pass, |pass| {
            pass.set_bind_group(0, frame_group, &[]);
            pass.set_bind_group(3, &self.shadow_group, &[]);
            // The three pipelines share their layouts, so the bound groups stay bound when one
            // replaces another; it is set only when a draw needs another.
            let mut bound = None;
            for (slot, instance) in scene.instances.iter().enumerate() {
                let cull = cull_mode(
                    instance.material.double_sided,
                    is_mirrored(instance.transform),
                );
                if bound != Some(cull) {
                    pass.set_pipeline(self.scene_pipelines.get(cull));
                    bound = Some(cull);
                }
                self.instances.draw(pass, slot, instance);
                stats.draw_calls += 1;
                stats.triangles += u64::from(instance.mesh.triangle_count());
            }
        });
        // Over the scene, keeping its light and its depths: the sky is drawn only where the depth
        // target still holds the farthest depth, where no surface was drawn.
        let sky_pass = RenderPassDescriptor {
            target: &targets.hdr,
            clear: None,
            depth: Some(&targets.depth),
            timer: self.sky_timer.as_ref(),
        };
        gpu.render(&sky_pass, |pass| {
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, frame_group, &[]);
            pass.draw(0..3);
        });
        if bloom {
            self.draw_bloom(gpu, targets, &mut stats, bloom_passes);
        }
        // The tone-mapping pass clears rather than keeps the output: its triangle covers every
        // pixel, and a clear does not read what the window's image held before.
        let tone_mapping_pass = RenderPassDescriptor {
            target,
            clear: Some(BLACK),
            depth: None,
            timer: self.tone_mapping_timer.as_ref(),
        };
        gpu.render(&tone_mapping_pass, |pass| {
            pass.debug_group("tone mapping", |pass| {
                pass.set_pipeline(&self.tone_mapping_pipeline);
                pass.set_bind_group(0, &targets.tone_mapping_group, &[]);
                pass.draw(0..3);
            });
        });
        stats.scene_gpu_time = self.scene_timer.as_ref().and_then(GpuTimer::latest);
        stats.tone_mapping_gpu_time = self.tone_mapping_timer.as_ref().and_then(GpuTimer::latest);
        stats.sky_gpu_time = self.sky_timer.as_ref().and_then(GpuTimer::latest);
        stats.cpu_time = started.elapsed();
        stats
    }

    /// The targets and groups for an output of `width` × `height` pixels (see [`SizedTargets`]).
    fn create_sized_targets(&self, gpu: &Gpu, width: u32, height: u32) -> SizedTargets {
        let hdr = gpu.create_render_target("scene", width, height, HDR_FORMAT);
        let depth = gpu.create_depth_target("scene depth", width, height);
        let sizes = bloom_sizes(width, height);
        let downs: Vec<RenderTarget> = (0..)
            .zip(&sizes)
            .map(|(index, &(level_width, level_height))| {
                let label = format!("bloom down {index}");
                gpu.create_render_target(&label, level_width, level_height, HDR_FORMAT)
            })
            .collect();
        // From the coarsest up: each average reads the coarser one, or the coarsest level's own
        // light, and mixes in its level by 1 / (the levels from it to the coarsest), so that each
        // level ends with the same share of the blurred copy, and no average exceeds the light it
        // averages, which a sum would beyond half precision near the sun.
        let mut ups: Vec<Option<(RenderTarget, BindGroup)>> = sizes.iter().map(|_| None).collect();
        for index in (0..sizes.len().saturating_sub(1)).rev() {
            let (level_width, level_height) = sizes[index];
            let label = format!("bloom up {index}");
            let average = gpu.create_render_target(&label, level_width, level_height, HDR_FORMAT);
            let coarser = ups[index + 1]
                .as_ref()
                .map_or(&downs[index + 1], |(average, _)| average);
            let mut weight = Vec::with_capacity(16);
            push_f32s(
                &mut weight,
                &[1.0 / (sizes.len() - index) as f32, 0.0, 0.0, 0.0],
            );
            let weight = gpu.create_uniform_buffer(&label, &weight);
            let group = gpu.create_bind_group(
                &label,
                &self.bloom_up_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::RenderTarget(coarser),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&self.bloom_sampler),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::RenderTarget(&downs[index]),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::UniformBuffer(&weight),
                    },
                ],
            );
            ups[index] = Some((average, group));
        }
        let bloom: Vec<BloomLevel> = (0..downs.len())
            .zip(ups)
            .map(|(index, up)| {
                let above = if index == 0 { &hdr } else { &downs[index - 1] };
                let down_group = gpu.create_bind_group(
                    &format!("bloom down {index}"),
                    &self.bloom_down_layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::RenderTarget(above),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&self.bloom_sampler),
                        },
                    ],
                );
                BloomLevel {
                    down: downs[index].clone(),
                    down_group,
                    up,
                }
            })
            .collect();
        // The blurred copy, or the scene's light itself when there is no chain: any texture does,
        // since the tone-mapping pass then mixes it in by 0.
        let blurred = match bloom.first() {
            Some(BloomLevel {
                up: Some((average, _)),
                ..
            }) => average,
            Some(BloomLevel { down, .. }) => down,
            None => &hdr,
        };
        let tone_mapping_group = gpu.create_bind_group(
            "tone mapping",
            &self.tone_mapping_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::RenderTarget(&hdr),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::RenderTarget(blurred),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.bloom_sampler),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::UniformBuffer(&self.bloom_strength),
                },
            ],
        );
        SizedTargets {
            hdr,
            depth,
            tone_mapping_group,
            bloom,
        }
    }

    /// Draws the bloom's chain (ADR-0040): each level filtered down from the one above, the first
    /// from the scene's light, then each average from the coarser one, from the coarsest up; each
    /// pass draws one triangle over its target, timed by its own timer when the adapter can time,
    /// and the sum of their latest times goes into `stats`. `passes` is the number of the chain's
    /// passes.
    fn draw_bloom(
        &self,
        gpu: &Gpu,
        targets: &SizedTargets,
        stats: &mut RenderStats,
        passes: usize,
    ) {
        let mut timers = self.bloom_timers.iter();
        let mut draw = |target: &RenderTarget, pipeline: &RenderPipeline, group: &BindGroup| {
            let pass = RenderPassDescriptor {
                target,
                // The triangle covers every texel; a clear reads nothing back on GPUs that keep
                // targets in tiles.
                clear: Some(BLACK),
                depth: None,
                timer: timers.next(),
            };
            gpu.render(&pass, |pass| {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..3);
            });
        };
        for level in &targets.bloom {
            draw(&level.down, &self.bloom_down_pipeline, &level.down_group);
        }
        for level in targets.bloom.iter().rev() {
            if let Some((average, group)) = &level.up {
                draw(average, &self.bloom_up_pipeline, group);
            }
        }
        if self.bloom_timers.len() >= passes {
            stats.bloom_gpu_time = self.bloom_timers[..passes]
                .iter()
                .map(GpuTimer::latest)
                .sum();
        }
    }
}

/// The frame's group: the frame's values, `environment`, the BRDF table and their sampler.
fn frame_group(
    gpu: &Gpu,
    layout: &BindGroupLayout,
    uniform: &UniformBuffer,
    environment: &Environment,
    brdf_table: &Texture,
    sampler: &Sampler,
) -> BindGroup {
    gpu.create_bind_group(
        "frame",
        layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::UniformBuffer(uniform),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Texture(&environment.texture),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Texture(brdf_table),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    )
}

/// The matrix that transforms the normals of an instance placed by `transform`. It gives
/// directions only: the shader normalizes the normals.
///
/// It is the cofactor matrix of the transform's 3 × 3 part: the inverse transpose, which keeps
/// normals perpendicular to surfaces scaled unevenly, times the determinant. It turns normals the
/// same way where the inverse exists, and stays finite where it does not: a mesh flattened along
/// one axis stays visible, and keeps the normal of its plane. It is negated when the determinant
/// is negative, so that the normals of a mirrored instance still point out of its surfaces (the
/// sign is decided by `is_mirrored`, which also decides the instance's culling and handedness, so
/// that a determinant of rounding noise counts as positive), and divided by its largest value, so
/// that its values stay within ±1, which the shader normalizes without overflow or underflow. Its
/// cofactors are products of two scales in f32, which bounds the uniform scales it handles to
/// about 1e-19 to 1e19, far beyond those of the inverse (about 1e-13 to 7e12). A mesh flattened
/// along two axes or three gets zero, but has no triangle left to draw.
fn normal_matrix(transform: Mat4) -> Mat3 {
    let linear = Mat3::from_mat4(transform);
    let cofactor = Mat3::from_cols(
        linear.y_axis.cross(linear.z_axis),
        linear.z_axis.cross(linear.x_axis),
        linear.x_axis.cross(linear.y_axis),
    );
    let largest = cofactor
        .abs()
        .to_cols_array()
        .into_iter()
        .fold(0.0, f32::max);
    if largest == 0.0 {
        return cofactor;
    }
    cofactor
        / if is_mirrored(transform) {
            -largest
        } else {
            largest
        }
}

/// The scene pass's pipelines, which differ only by the faces they cull (ADR-0035).
struct ScenePipelines {
    /// Back faces culled: single-sided materials.
    back_culled: RenderPipeline,
    /// Front faces culled: single-sided materials on mirrored instances, whose front faces wind
    /// clockwise on screen.
    front_culled: RenderPipeline,
    /// No face culled: double-sided materials.
    unculled: RenderPipeline,
}

impl ScenePipelines {
    fn get(&self, cull: CullMode) -> &RenderPipeline {
        match cull {
            CullMode::Back => &self.back_culled,
            CullMode::Front => &self.front_culled,
            CullMode::None => &self.unculled,
        }
    }
}

/// The faces a draw culls (ADR-0035): none for a double-sided material; otherwise the instance's
/// back faces, which wind clockwise on screen, unless its transform mirrors it and turns its front
/// faces clockwise: its counter-clockwise faces are then the back ones.
fn cull_mode(double_sided: bool, mirrored: bool) -> CullMode {
    match (double_sided, mirrored) {
        (true, _) => CullMode::None,
        (false, false) => CullMode::Back,
        (false, true) => CullMode::Front,
    }
}

/// Whether `transform` mirrors what it places, turning its front faces clockwise on screen: the
/// determinant of its 3 × 3 part is negative beyond rounding. The determinant of the columns
/// normalized is compared with −10⁻⁵. A mesh flattened between two rotations has columns in one
/// plane up to rounding, a determinant of noise of either sign, about 10⁻⁷ at most, which does
/// not count as mirrored, nor does a column of zero length. Uniform scales between about 10⁻¹⁹ and
/// 10¹⁹ are told apart, as by `normal_matrix`.
fn is_mirrored(transform: Mat4) -> bool {
    let linear = Mat3::from_mat4(transform);
    let [x, y, z] = [linear.x_axis, linear.y_axis, linear.z_axis].map(Vec3::normalize_or_zero);
    x.dot(y.cross(z)) < -MIRROR_THRESHOLD
}

/// The uniform buffer holding the values of every instance of a frame, one slot per instance at
/// a multiple of the device's alignment, and the group that binds one slot at a time.
struct InstanceSlots {
    buffer: UniformBuffer,
    group: BindGroup,
    /// How many slots the buffer has.
    capacity: usize,
    /// The distance between slots, in bytes.
    stride: usize,
}

impl InstanceSlots {
    fn new(gpu: &Gpu, layout: &BindGroupLayout, capacity: usize) -> Self {
        let stride = INSTANCE_VALUES_SIZE.next_multiple_of(gpu.uniform_offset_alignment());
        let buffer = gpu.create_uniform_buffer("instances", &vec![0; capacity * stride]);
        let group = gpu.create_bind_group(
            "instances",
            layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::UniformBufferRange {
                    buffer: &buffer,
                    size: INSTANCE_VALUES_SIZE,
                },
            }],
        );
        Self {
            buffer,
            group,
            capacity,
            stride,
        }
    }

    /// Draws `instance`, whose values are in slot `slot`, with the pipeline that `pass` has set:
    /// binds its material at group 1 and its slot at group 2, then its mesh.
    fn draw(&self, pass: &mut RenderPass<'_>, slot: usize, instance: &Instance<'_>) {
        let offset = u32::try_from(slot * self.stride)
            .expect("the instance buffer is smaller than the device allows");
        pass.set_bind_group(1, &instance.material.group, &[]);
        pass.set_bind_group(2, &self.group, &[offset]);
        pass.set_vertex_buffer(0, &instance.mesh.vertices);
        pass.set_index_buffer(&instance.mesh.indices);
        pass.draw_indexed(0..instance.mesh.indices.count());
    }
}

#[cfg(test)]
mod tests {
    use glam::{EulerRot, Quat, Vec4};

    use super::*;
    use crate::environment::{
        BRDF_TABLE_SIZE, ENVIRONMENT_LEVELS, GROUND_HAZE, SUN_ANGULAR_RADIUS,
    };

    /// Where `matrix` sends `normal`, normalized as the shader does.
    fn direction(matrix: Mat3, normal: Vec3) -> Vec3 {
        (matrix * normal).normalize()
    }

    fn assert_close(actual: Vec3, expected: Vec3, case: &str) {
        assert!(
            actual.abs_diff_eq(expected, 1e-5),
            "{case}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn normals_turn_as_the_inverse_transpose_turns_them_where_it_exists() {
        let rotation = Quat::from_euler(EulerRot::XYZ, 0.3, -1.1, 2.0);
        for (transform, case) in [
            (Mat4::IDENTITY, "identity"),
            (
                Mat4::from_rotation_translation(rotation, Vec3::new(4.0, -2.0, 7.0)),
                "turned and moved",
            ),
            (
                Mat4::from_scale_rotation_translation(
                    Vec3::new(4.0, 0.5, 2.0),
                    rotation,
                    Vec3::ONE,
                ),
                "scaled unevenly",
            ),
            (Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)), "mirrored"),
            (
                Mat4::from_scale(Vec3::splat(1e15)) * Mat4::from_quat(rotation),
                "scaled up a lot",
            ),
            (
                Mat4::from_scale(Vec3::splat(1e-15)) * Mat4::from_quat(rotation),
                "scaled down a lot",
            ),
        ] {
            let matrix = normal_matrix(transform);
            let largest = matrix.abs().to_cols_array().into_iter().fold(0.0, f32::max);
            assert!(
                (largest - 1.0).abs() < 1e-6,
                "{case}: the largest value is {largest}"
            );
            // The inverse transpose, in f64 so that the extreme scales do not overflow it.
            let inverse_transpose = Mat3::from_mat4(transform).as_dmat3().inverse().transpose();
            for normal in [
                Vec3::X,
                Vec3::Y,
                Vec3::Z,
                Vec3::new(1.0, -2.0, 3.0).normalize(),
            ] {
                let expected = (inverse_transpose * normal.as_dvec3())
                    .normalize()
                    .as_vec3();
                assert_close(direction(matrix, normal), expected, case);
            }
        }
    }

    #[test]
    fn a_mirrored_instance_keeps_its_normals_pointing_out() {
        // Mirrored along X, a surface facing +Z still faces +Z, and one facing +X faces −X.
        let matrix = normal_matrix(Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
        assert_close(direction(matrix, Vec3::Z), Vec3::Z, "facing +Z");
        assert_close(direction(matrix, Vec3::X), -Vec3::X, "facing +X");
    }

    #[test]
    fn a_mesh_flattened_along_one_axis_keeps_the_normal_of_its_plane() {
        // A surface in the plane z = 0, flattened along Z, is unchanged: it still faces +Z.
        let matrix = normal_matrix(Mat4::from_scale(Vec3::new(1.0, 1.0, 0.0)));
        assert!(matrix.is_finite(), "{matrix}");
        assert_close(direction(matrix, Vec3::Z), Vec3::Z, "flattened along Z");
        // Turned, then flattened along the world's Y axis: the surface now lies in the plane
        // y = 0, so its normal is along Y.
        let turned = Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)) * Mat4::from_rotation_x(0.5);
        let matrix = normal_matrix(turned);
        assert!(matrix.is_finite(), "{matrix}");
        let normal = direction(matrix, Vec3::Z);
        assert!(normal.x.abs() < 1e-6 && normal.z.abs() < 1e-6, "{normal}");
    }

    #[test]
    fn the_scene_shader_holds_as_many_lights_as_the_renderer_packs() {
        // WGSL sizes the array of lights with a constant of its own, which cannot come from Rust.
        let declared = format!("const MAX_LIGHTS: u32 = {MAX_LIGHTS}u;");
        assert!(
            SCENE_SHADER.contains(&declared),
            "shader.wgsl lacks `{declared}`"
        );
    }

    /// The value of the WGSL constant `name` in the scene shader.
    fn shader_constant(name: &str) -> f64 {
        constant_of(SCENE_SHADER, name)
    }

    /// The value of the WGSL constant `name` in `shader`.
    fn constant_of(shader: &str, name: &str) -> f64 {
        let start = format!("const {name}: f32 = ");
        let line = shader
            .lines()
            .find_map(|line| line.strip_prefix(&start))
            .unwrap_or_else(|| panic!("the shader lacks `{start}`"));
        line.trim_end_matches(';').parse().expect("a number")
    }

    #[test]
    fn the_tone_mapping_pass_restores_the_light_the_scene_pass_divided() {
        // Both shaders scale by the same power of two, which half precision multiplies exactly.
        let headroom = shader_constant("HEADROOM");
        assert_eq!(constant_of(TONE_MAPPING_SHADER, "HEADROOM"), headroom);
        assert_eq!(headroom, 64.0);
    }

    #[test]
    fn the_scene_shader_reads_the_environment_and_the_table_the_renderer_computes() {
        assert_eq!(
            shader_constant("ROUGHEST_LEVEL"),
            (ENVIRONMENT_LEVELS - 1) as f64
        );
        assert_eq!(
            shader_constant("BRDF_TABLE_SIZE"),
            f64::from(BRDF_TABLE_SIZE)
        );
    }

    #[test]
    fn the_sky_shader_draws_the_sun_s_disc_at_the_size_the_environment_computes_it_for() {
        // The chord from the disc's centre to its edge on the unit sphere, 2 sin(r / 2), for the
        // sun's mean apparent diameter, 0.533°.
        let chord = 2.0 * (SUN_ANGULAR_RADIUS / 2.0).sin();
        assert!((SUN_ANGULAR_RADIUS - 0.533_f64.to_radians() / 2.0).abs() < 1e-12);
        assert!((shader_constant("SUN_CHORD") / chord - 1.0).abs() < 1e-6);
        // The ground hides the disc as the environment hides the sun's glow.
        assert_eq!(shader_constant("GROUND_HAZE") as f32, GROUND_HAZE);
    }

    #[test]
    fn the_bloom_s_levels_halve_down_to_a_side_of_8_texels() {
        assert_eq!(bloom_sizes(1, 1), []);
        assert_eq!(bloom_sizes(3, 2), []);
        assert_eq!(bloom_sizes(14, 14), []);
        assert_eq!(bloom_sizes(15, 15), [(8, 8)]);
        assert_eq!(bloom_sizes(16, 16), [(8, 8)]);
        assert_eq!(bloom_sizes(33, 17), [(17, 9)]);
        // Each level is the output divided by 2^i, rounded to the nearest: 720 / 32 = 22.5 gives
        // 23, 720 / 64 = 11.25 gives 11, though 23 / 2 would round to 12.
        assert_eq!(
            bloom_sizes(1280, 720),
            [
                (640, 360),
                (320, 180),
                (160, 90),
                (80, 45),
                (40, 23),
                (20, 11)
            ]
        );
        assert_eq!(
            bloom_sizes(1920, 1080),
            [
                (960, 540),
                (480, 270),
                (240, 135),
                (120, 68),
                (60, 34),
                (30, 17),
                (15, 8),
            ]
        );
        // An odd output's first level rounds 960.5 up, and the next ones meet the even output's.
        assert_eq!(bloom_sizes(1921, 1081)[..2], [(961, 541), (480, 270)]);
        // Every level is half the one before, rounded down or up, which the passes' doubled and
        // halved positions assume.
        for (width, height) in [(1921, 1081), (637, 696), (1023, 767), (257, 131)] {
            let mut finer = (width, height);
            for (level_width, level_height) in bloom_sizes(width, height) {
                assert!(
                    [finer.0 / 2, finer.0.div_ceil(2)].contains(&level_width)
                        && [finer.1 / 2, finer.1.div_ceil(2)].contains(&level_height),
                    "{width}x{height}: {level_width}x{level_height} after {finer:?}"
                );
                finer = (level_width, level_height);
            }
        }
        // The coarsest level has 8 to 15 texels across the shorter side: each spans an eighth to a
        // sixteenth of the screen, whatever its resolution.
        for (width, height) in [
            (256, 256),
            (1280, 720),
            (2560, 1440),
            (3840, 2160),
            (8192, 8192),
        ] {
            let (level_width, level_height) = *bloom_sizes(width, height).last().expect("a chain");
            let across = level_width.min(level_height);
            assert!(
                (8..16).contains(&across),
                "{width}x{height}: {across} texels"
            );
        }
    }

    #[test]
    fn each_draw_culls_the_faces_its_material_and_mirroring_ask_for() {
        assert_eq!(cull_mode(false, false), CullMode::Back);
        assert_eq!(cull_mode(false, true), CullMode::Front);
        assert_eq!(cull_mode(true, false), CullMode::None);
        assert_eq!(cull_mode(true, true), CullMode::None);
    }

    #[test]
    fn a_transform_mirrors_when_its_determinant_is_negative_beyond_rounding() {
        let rotation = Quat::from_euler(EulerRot::XYZ, 0.3, -1.1, 2.0);
        let mirror = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        for (transform, mirrored, case) in [
            (Mat4::IDENTITY, false, "identity"),
            (mirror, true, "mirrored along X"),
            (
                Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)),
                false,
                "turned by a half turn",
            ),
            (
                Mat4::from_scale(Vec3::new(-1e-3, 1.0, 1.0)),
                true,
                "mirrored and almost flattened",
            ),
            (
                Mat4::from_scale_rotation_translation(
                    Vec3::new(4.0, 0.5, 2.0),
                    rotation,
                    Vec3::ONE,
                ),
                false,
                "scaled unevenly",
            ),
            (
                Mat4::from_scale(Vec3::splat(1e15)) * mirror,
                true,
                "mirrored and scaled up a lot",
            ),
            (
                Mat4::from_scale(Vec3::splat(1e-15)) * mirror,
                true,
                "mirrored and scaled down a lot",
            ),
            (
                Mat4::from_scale(Vec3::new(1.0, 1.0, 0.0)),
                false,
                "flattened",
            ),
            (
                Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W),
                false,
                "a point",
            ),
        ] {
            assert_eq!(is_mirrored(transform), mirrored, "{case}");
        }
    }

    #[test]
    fn a_mesh_flattened_between_two_rotations_is_not_mirrored() {
        // Its determinant is rounding noise, of either sign: neither the culling nor the normals
        // may follow that sign, or the mesh would be culled or lit from either side at random.
        for i in 0..60 {
            for j in 0..60 {
                let (i, j) = (i as f32, j as f32);
                let child = Mat4::from_euler(EulerRot::XYZ, 0.1 * i, 0.37 * i + 0.2, 0.23 * i);
                let parent =
                    Mat4::from_euler(EulerRot::XYZ, 0.11 * j + 0.3, 0.29 * j, 0.17 * j + 0.5);
                let transform = parent * Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)) * child;
                assert!(!is_mirrored(transform), "{transform}");
                assert!(normal_matrix(transform).is_finite(), "{transform}");
            }
        }
    }
}
