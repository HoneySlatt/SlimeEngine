//! The renderer: draws instances of meshes, seen from a camera and shaded with glTF's
//! metallic-roughness materials under a scene's lights and its environment, into a render target,
//! in passes: the shadows of the first directional light that casts shadows, in four cascades, and
//! those of the spot lights, the scene in high dynamic range, exposed, the sky behind it, the
//! resolve of its samples, the bloom's chain of blurred copies, then tone mapping, which mixes the
//! bloom in.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::{DAffine3, DMat4, DVec3, Mat3, Mat4, Vec3};
use slime_assets::CookedTexture;
use slime_gfx::{
    AddressMode, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
    BindingType, ComparisonSampler, CullMode, DepthBias, DepthCompare, DepthState, DepthTarget,
    Filter, FragmentState, GfxError, Gpu, RenderPass, RenderPipeline, RenderPipelineDescriptor,
    RenderTarget, Sampler, SamplerDescriptor, ShaderModule, ShaderStages, StorageBuffer, Texture,
    TextureFormat, UniformBuffer,
};

use crate::bytes::{push_f32s, push_mat3, push_mat4, push_u32s};
use crate::camera::{Camera, Frustum};
use crate::cooked::upload_texture;
use crate::environment::{Environment, Sky, brdf_table};
use crate::error::RenderError;
use crate::graph::{ColorId, Draws, Graph, Imported, PassDeclaration, scaled_side};
use crate::light::{LIGHT_VALUES_SIZE, Light, MAX_LIGHTS, NO_TILE, SpotLight};
use crate::material::{MATERIAL_VALUES_SIZE, Material, MaterialData, MaterialParts};
use crate::mesh::{Mesh, MeshData, MeshPages, Page, VERTEX_LAYOUT};
use crate::shaders::ShaderSources;
use crate::shadow::{
    CASCADE_ATLAS_SIZE, CASCADES, CascadeView, SHADOW_BIAS, SPOT_ATLAS_SIZE, ShadowTile, SpotView,
    cascade_slice, cascade_tile, spot_tiles,
};

/// The size of the buffer of the tone-mapping pass's settings: `Settings` in `tone_mapping.wgsl`,
/// three numbers, which the renderer writes followed by a fourth, unused.
const TONE_MAPPING_SETTINGS_SIZE: usize = 16;
/// The size of the buffer of a level's weight in the bloom's average up: `Weight` in `bloom.wgsl`,
/// one number, which the renderer writes followed by three, unused.
const BLOOM_WEIGHT_SIZE: usize = 16;
/// The bloom's chain stops before a level with a side shorter than this, in texels: its coarsest
/// texel is then an eighth to a sixteenth of the output's shorter side, whatever its resolution, so
/// that the halo reaches the same share of the screen (ADR-0040).
const BLOOM_SMALLEST_SIDE: u32 = 8;
/// The format the scene pass draws into: half-precision floats, which keep light above 1
/// (ADR-0034).
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
/// The size of the values of a frame, as `FrameUniform` in `shader.wgsl` lays them out: the
/// camera's two `mat4x4<f32>`; the exposure, the number of lights and the index of the light the
/// cascades shadow (16 bytes); the direction towards the sun's disc and its radiance (16 bytes
/// each); room for [`MAX_LIGHTS`] lights and as many tiles of the spot lights' atlas; then the
/// [`CASCADES`] tiles of the cascades' atlas: 2 800 bytes.
const FRAME_VALUES_SIZE: usize = 2 * 64
    + 16
    + 2 * 16
    + MAX_LIGHTS * (LIGHT_VALUES_SIZE + SHADOW_TILE_VALUES_SIZE)
    + CASCADES * SHADOW_TILE_VALUES_SIZE;
/// The size of the values of one tile of a shadow atlas, `ShadowTile` in `shader.wgsl`: its
/// `mat4x4<f32>` view and its place in the atlas (16 bytes).
const SHADOW_TILE_VALUES_SIZE: usize = 64 + 16;
/// The index of the shadowed light when the cascades shadow none: no light has it.
const NO_SHADOWED_LIGHT: u32 = u32::MAX;
/// The size of the values of an instance in the buffer of the instances' values, `Instance` in
/// `shader.wgsl`: its `mat4x4<f32>` transform, the `mat3x3<f32>` that transforms its normals (three
/// columns padded to 16 bytes), and its handedness, padded to 16 bytes.
const INSTANCE_VALUES_SIZE: usize = 64 + 3 * 16 + 16;
/// The bits of the passes that draw an instance in a frame: the scene pass's, then one for each
/// cascade, from the nearest, then one for each tile of the spot lights' atlas.
const SCENE_PASS: u32 = 1;
const fn cascade_pass(cascade: usize) -> u32 {
    1 << (1 + cascade)
}
const fn spot_pass(tile: usize) -> u32 {
    1 << (1 + CASCADES + tile)
}
/// The bits of every cascade.
const CASCADE_PASSES: u32 = ((1 << CASCADES) - 1) << 1;
/// Where the values of an instance that no pass draws are: nowhere.
const NOT_DRAWN: u32 = u32::MAX;
const _: () = assert!(
    1 + CASCADES + MAX_LIGHTS <= 32,
    "an instance's passes fit in a u32"
);
// Every pass draws an instance at most once, so that the list of the instances that each draw
// draws never needs more bytes than their values: it is never the larger buffer.
const _: () = assert!((1 + CASCADES + MAX_LIGHTS) * 4 <= INSTANCE_VALUES_SIZE);
/// The size of the values of one view of a shadow pass, `ShadowView` in `shader.wgsl`: a matrix.
const SHADOW_VIEW_SIZE: usize = 64;
/// The texture slots of a material, each a texture and its sampler: base color,
/// metallic-roughness, normal, occlusion, emissive (ADR-0035).
const MATERIAL_TEXTURE_SLOTS: u32 = 5;
/// How far below zero the determinant of a transform's normalized columns must be for the
/// transform to mirror (see `is_mirrored`).
const MIRROR_THRESHOLD: f32 = 1e-5;
/// How many instances the buffer of the instances' values holds when the renderer is created, and
/// how many entries the list of the instances that the draws draw does; each doubles whenever a
/// frame needs more, and comes back down once a frame uses a quarter of it or less
/// (`instance_capacity`).
const INITIAL_INSTANCE_CAPACITY: usize = 64;

/// One thing to draw: a mesh, its material, and where it is.
///
/// Made by [`Instance::new`], so that a field added later does not break the code that makes one
/// (ADR-0046); the fields can then be changed.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Instance<'a> {
    /// What to draw.
    pub mesh: &'a Mesh,
    /// How it looks.
    pub material: &'a Material,
    /// From the mesh's space to world space, in metres, its translation in `f64` as positions in
    /// the world are (ADR-0079). A transform that mirrors the mesh (a negative
    /// determinant, beyond rounding) keeps the mesh's front faces, as glTF does: the renderer
    /// culls its other faces. Normals follow its rotation and stay perpendicular to the surfaces it
    /// scales unevenly, even when it flattens them along one axis.
    pub transform: DAffine3,
    /// Whether it casts shadows (ADR-0038, ADR-0049); it receives them either way. An instance that
    /// casts none is left out of the shadow passes, which it would otherwise cost GPU time, and
    /// of the depths each cascade spans along the light (ADR-0050, ADR-0064): a vast floor or a sky
    /// dome that nothing needs to shade would otherwise stretch them, and coarsen the depths the
    /// cascade tells apart.
    pub casts_shadows: bool,
}

impl<'a> Instance<'a> {
    /// `mesh` drawn with `material`, placed by `transform`, casting shadows.
    #[must_use]
    pub fn new(mesh: &'a Mesh, material: &'a Material, transform: DAffine3) -> Self {
        Self {
            mesh,
            material,
            transform,
            casts_shadows: true,
        }
    }
}

/// What the renderer draws in one frame.
///
/// Light is in physical units (ADR-0037): lights in lux and candelas, the light that surfaces
/// reflect and emit and the environment's in nits (candelas per square metre),
/// distances in metres. The scene pass multiplies that light by the exposure before it writes it
/// into the high-dynamic-range target (pre-exposure), divided by 64 so that its half-precision
/// floats, which reach 65 504, hold exposed light up to 64 × 65 504 (ADR-0040): exposed light above
/// it is clipped to it.
///
/// Positions are in world space, in metres, as `f64`s (ADR-0079). The renderer moves them by
/// −`camera.eye` in `f64` before they reach the GPU as `f32`s (ADR-0048), so that what is near the
/// camera is drawn as precisely 10 000 km from the origin as at it.
///
/// Made by [`Scene::new`], so that a field added later does not break the code that makes one
/// (ADR-0046), then filled in by changing the fields:
///
/// ```
/// # use slime_render::{Camera, DirectionalLight, Light, Scene};
/// let sun = [Light::Directional(DirectionalLight::default())];
/// let mut scene = Scene::new(Camera::default(), &[]);
/// scene.lights = &sun;
/// scene.exposure = 0.5;
/// ```
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Scene<'a> {
    /// Where the scene is seen from.
    pub camera: Camera,
    /// What lights the surfaces. [`Renderer::render`] draws the first [`MAX_LIGHTS`] and leaves
    /// the others out, logging a warning the first time a renderer meets such a scene; it panics
    /// on a drawn light with a value outside the range its documentation gives.
    pub lights: &'a [Light],
    /// What surrounds the scene (ADR-0039): its light reaches every surface, reflected as glTF's
    /// materials reflect it (image-based lighting, with the split sum and the light that rough
    /// surfaces scatter more than once, as the Khronos glTF Sample Viewer does: a white surface of
    /// any roughness, metal or not, inside a uniform sky shows the sky's radiance), and it shows
    /// where no surface is drawn, with its sun's disc. `None` is a black sky: only the lights and
    /// emission light surfaces, and the background is black. Created by
    /// [`Renderer::create_environment`].
    pub environment: Option<&'a Environment>,
    /// What the scene's light, the sky's included, is multiplied by before it is written
    /// into the high-dynamic-range target, then tone mapped (ADR-0034, ADR-0037): a linear factor,
    /// the inverse of the luminance, in nits, that shows as about 0.87 (sRGB 240). 2 is one
    /// photographic stop brighter; 0 gives black. A white matte surface facing a light of E lux
    /// reflects 0.97 E / π nits, which an exposure of π / E shows at sRGB 239. A camera of exposure
    /// value EV100 (at ISO 100) has an exposure of 1 / (1.2 × 2^EV100) in the convention that
    /// Frostbite and Filament follow: about 1 / 39 000 for EV100 15, a photographer's exposure for
    /// direct sunlight. Finite and not negative: [`Renderer::render`] panics otherwise.
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
    /// smaller than 15 × 15 pixels have no bloom. From 0 to 1: [`Renderer::render`] panics
    /// otherwise.
    pub bloom: f32,
    /// What to draw. Each pass draws those it sees, grouped by pipeline, then by material, then by
    /// mesh, each material and mesh in the order it first appears here, a clone counting as the
    /// material or the mesh it was cloned from, then in this order, so that a draw binds only what
    /// changed since the one before (ADR-0106); the instances of one pipeline, material and mesh
    /// that follow each other in that order are drawn by one draw call (instancing), in that order
    /// still. Of two surfaces at the same depth, the one drawn
    /// first stays, which this order alone does not decide. The shadow passes draw the instances
    /// of one mesh together whatever their materials, except those of masked materials, whose
    /// shadows the material cuts.
    pub instances: &'a [Instance<'a>],
}

impl<'a> Scene<'a> {
    /// `instances` seen from `camera`, without lights or environment, at an exposure of 1 and
    /// without bloom: only the instances' emission lights the frame, as it is, on black.
    #[must_use]
    pub fn new(camera: Camera, instances: &'a [Instance<'a>]) -> Self {
        Self {
            camera,
            lights: &[],
            environment: None,
            exposure: 1.0,
            bloom: 0.0,
            instances,
        }
    }
}

/// What drawing a scene took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderStats {
    /// Draw calls of the scene pass: one per group of instances of one pipeline, material and mesh
    /// that it draws ([`Scene::instances`]). The draws of the sky, bloom and tone-mapping passes
    /// are not counted.
    pub draw_calls: u32,
    /// Instances the scene pass drew: those whose box the camera sees, frustum culling having left
    /// the others out.
    pub instances: u32,
    /// Triangles the scene pass submitted to the GPU: those of the instances it drew, before the
    /// GPU culls the faces turned away.
    pub triangles: u64,
    /// How long [`Renderer::render`] took on the CPU: packing the values, recording the passes and
    /// submitting them, four to a submission (ADR-0041). It includes any wait inside a submission:
    /// lavapipe runs a submission only once the previous one is done, so on lavapipe it holds the
    /// GPU work of the frame's earlier submissions, and in a windowed run the wait for the previous
    /// frame.
    pub cpu_time: Duration,
    /// How long the GPU took to draw the scene into the high-dynamic-range target: the latest time
    /// that has come back, a few frames old. The passes of a frame are timed four to a submission
    /// (ADR-0041): the times of the passes of one submission come from the same frame, those of
    /// different submissions can come from different frames, usually consecutive ones. `None` on
    /// adapters that cannot time their work ([`Gpu::can_time`](slime_gfx::Gpu::can_time)), until
    /// the first time comes back, and for a few frames after the frame's passes or targets change
    /// (the output's size, the shadow pass or the bloom turned on or off), until the submission
    /// that holds the pass has its first time. A renderer whose frame changes at every call, drawing
    /// into outputs of two sizes in turn for example, gets no tone-mapping or bloom time: use one
    /// renderer per output size.
    pub scene_gpu_time: Option<Duration>,
    /// How long the GPU took for the tone-mapping pass, under the same rules. On Vulkan, each time
    /// runs from the end of the work before the pass to its end, so the times of a frame do not
    /// add up to its GPU time (ADR-0033).
    pub tone_mapping_gpu_time: Option<Duration>,
    /// Draw calls of the shadow pass, which draws the shadows of the scene's first directional
    /// light that casts shadows into its four cascades (ADR-0038, ADR-0064): for each cascade, one
    /// per group of the instances that cast shadows whose boxes reach across its square, of one
    /// mesh, and of one material for masked ones ([`Scene::instances`]), so that an instance is
    /// drawn up to four times, whether the camera sees it or not. None when no directional light of
    /// the scene casts shadows, when no instance that casts shadows reaches any cascade or every
    /// one that does is flattened to a point, which casts nothing, or when the camera sees nothing:
    /// the pass is then skipped.
    pub shadow_draw_calls: u32,
    /// The draw calls the spot lights' shadow pass made: for each spot light that casts shadows,
    /// one per group of the instances that cast shadows whose boxes reach into its tile, grouped as
    /// in the cascades.
    pub spot_shadow_draw_calls: u32,
    /// How long the GPU took to draw the cascades, under the same rules as `scene_gpu_time`;
    /// `None` too for a frame that skipped the shadow pass.
    pub shadow_gpu_time: Option<Duration>,
    /// How long the GPU took to draw the spot lights' shadow atlas, under the same rules as
    /// `scene_gpu_time`; `None` too for a frame in which no spot light casts shadows.
    pub spot_shadow_gpu_time: Option<Duration>,
    /// How long the GPU took to draw the sky behind the scene, under the same rules as
    /// `scene_gpu_time`.
    pub sky_gpu_time: Option<Duration>,
    /// How long the GPU took to resolve the scene's samples into one pixel each, under the same
    /// rules as `scene_gpu_time`.
    pub resolve_gpu_time: Option<Duration>,
    /// How long the GPU took for the bloom's passes (ADR-0040): the sum of their times, under the
    /// same rules as `scene_gpu_time`. `None` too for a frame that skipped the bloom (a strength
    /// of 0, or an output smaller than 15 × 15), and until every pass's time has come back.
    pub bloom_gpu_time: Option<Duration>,
}

/// Draws scenes into output targets of one format.
///
/// Each frame takes these passes (ADR-0034, ADR-0037, ADR-0038, ADR-0039, ADR-0040): the shadow
/// pass draws the depths of the instances, as the scene's first directional light that casts
/// shadows sees them, into the tile of each of its four cascades (ADR-0064); the spot lights'
/// shadow pass draws them, as each spot light that casts shadows sees them, into its tile of the
/// atlas (ADR-0058, ADR-0061); the scene pass draws the instances into a target of half-precision
/// floats, where light may exceed 1, of four samples per pixel, with a depth target of four samples
/// too (ADR-0056), their light multiplied by the scene's exposure, the shadowed directional light's
/// only where the finest cascades that hold a point show nothing nearer to it, each spot light's
/// that casts shadows only where its tile does, and the environment's; the sky pass draws the
/// environment where no surface is, sample by sample; the resolve pass averages the samples of each
/// pixel into a target of one sample per pixel, each weighed by its brightness (ADR-0060), so that
/// the edges of what both passes drew are smooth (multisample anti-aliasing); the bloom's passes
/// filter that light down into a chain of levels, each half the size of the one before, then
/// average them back up into a blurred copy; the tone-mapping pass then mixes the blurred copy in
/// by the scene's bloom strength and maps the light to the output's range with Khronos PBR Neutral.
/// The renderer owns ten pipelines: two for the shadow pass (the second discards what masked
/// materials discard), three for the scene pass, which differ by the faces they cull and among
/// which each draw chooses (ADR-0035), the sky pass's, the resolve pass's, the bloom's two (down
/// and up) and the tone-mapping pass's; the table of the BRDF's scale and bias and a black
/// environment for scenes without one; the cascades' atlas, 4096 × 4096 depths (64 MiB) created by
/// the first frame in which a directional light casts shadows; the spot lights' shadow atlas, as
/// large, created by the first frame in which a spot light casts shadows; the buffer of the shadow
/// passes' views; the buffers of the instances' values and of the list of the instances that each
/// draw draws; the pages that hold its meshes' vertices and indices ([`Mesh`]); and the frame's
/// render graph (ADR-0041): its passes in order, the high-dynamic-range and depth targets and the
/// bloom's levels at the size of the output it draws into, the groups that read them and GPU
/// timers for its submissions, built again when the frame's shape changes (the output's size,
/// either shadow pass or the bloom turned on or off). A renderer draws best into outputs of one
/// size: drawing into outputs of two sizes in turn creates its targets again at every call, and
/// reports no tone-mapping or bloom time; use one renderer per output size. At 1920 × 1080 with the
/// bloom, a renderer holds about 122 MiB of GPU memory: 111 for the high-dynamic-range and depth
/// targets, 95 of them for their four samples per pixel, and 10.5 for the bloom's levels; then 64
/// more once a directional light has cast shadows, and 64 more once a spot light has; at 3840 ×
/// 2160, about 485 MiB, and as much more; and the pages of its meshes.
///
/// Each frame, the scene pass leaves out the instances whose boxes lie outside what the camera
/// sees (frustum culling), and the shadow passes those whose boxes lie outside each cascade's
/// square and each spot light's cone; each pass then draws the instances it keeps of one
/// pipeline, material and mesh with one draw call (instancing), each instance's values read from a
/// storage buffer by its number. Nothing is kept from one frame to the next: every instance of the
/// scene costs CPU time at each frame, to place its box and test it, whether a pass draws it or
/// not. A frame draws at most as many instances as the device's storage buffers hold values of,
/// 1 048 576 with the default limits: the instances beyond, in the order of the scene's, are left
/// out of every pass, with a warning the first time a renderer meets such a scene.
///
/// It does not yet smooth the edges that a masked material cuts inside a triangle, or blend
/// transparent surfaces.
pub struct Renderer {
    /// The output's format.
    format: TextureFormat,
    /// Its pipelines, made from its shaders' sources ([`Renderer::set_shaders`]).
    pipelines: Pipelines,
    /// The layouts of the groups that its pipelines read.
    layouts: Layouts,
    /// What creating a material takes besides its data.
    pub(crate) material_parts: MaterialParts,
    frame_uniform: UniformBuffer,
    /// The environment the frame's group was last built with, kept alive by it, and that group.
    frame_group: (Environment, BindGroup),
    /// One black texel, the environment of scenes without one.
    no_environment: Environment,
    brdf_table: Texture,
    /// Reads the environment and the BRDF table.
    environment_sampler: Sampler,
    /// The pages of the meshes it creates, which its loads share.
    pub(crate) meshes: MeshPages,
    /// The storage buffers of the instances that a frame draws, and the group that binds them.
    instances: InstanceBuffers,
    /// What the frame drawn last worked out about its instances: kept from one frame to the next,
    /// so that a frame writes into memory it has, and given back after a peak, as the buffers
    /// shrink (`reuse`).
    scratch: Scratch,
    /// The frame's graph and what reads its targets: created by the first `render`, and again
    /// when the frame's shape changes.
    frame: Option<Frame>,
    /// The cascades' atlas (ADR-0064): the depths that the shadow pass draws into the tile of each
    /// cascade, which the scene pass compares with; created by the first frame in which a
    /// directional light casts shadows, then kept.
    cascade_atlas: Option<DepthTarget>,
    /// One texel, bound in the place of either atlas until it is created.
    no_atlas: DepthTarget,
    /// The cascades' atlas, its comparison sampler and the spot lights' atlas, bound at group 3 of
    /// the scene pass, built again when either atlas is created.
    shadow_group: BindGroup,
    shadow_sampler: ComparisonSampler,
    /// The view of each tile that the shadow passes draw: the cascades' in the first [`CASCADES`]
    /// slots, then one per spot light that casts shadows, each slot `shadow_view_stride` bytes
    /// apart, written at every frame.
    shadow_views: UniformBuffer,
    shadow_view_stride: usize,
    /// Those views, bound at group 3 of the shadow passes, at the offset of each.
    shadow_views_group: BindGroup,
    /// The spot lights' shadow atlas (ADR-0058): created by the first frame in which a spot light
    /// casts shadows, then kept.
    spot_atlas: Option<DepthTarget>,
    /// Linear, mirrored at the edges: reads the levels between their texels, and beyond them their
    /// texels mirrored, so that a pixel near an edge keeps its light and none wraps around.
    bloom_sampler: Sampler,
    /// The tone-mapping pass's settings, `Settings` in `tone_mapping.wgsl`: the strength it mixes
    /// the blurred copy in by, written every frame, and how it dithers the output (ADR-0051).
    tone_mapping_settings: UniformBuffer,
    /// Whether a scene with more than [`MAX_LIGHTS`] lights was drawn: it is said once, not at
    /// every frame.
    warned_of_lights: bool,
    /// The indices of the lights that the last frame left out, which [`Light::check`] refused:
    /// each is said when it is first left out, not at every frame.
    left_out_lights: Vec<usize>,
    /// Whether a frame left out instances beyond what the device's storage buffers hold: it is
    /// said once, not at every frame.
    warned_of_instances: bool,
}

/// What a frame works out about the scene's instances, one entry per instance, or per instance
/// that a pass draws.
#[derive(Default)]
struct Scratch {
    /// Each instance's transform from its mesh's space to the frame's space.
    models: Vec<Mat4>,
    /// The passes that draw each instance: bits of [`SCENE_PASS`], [`cascade_pass`] and
    /// [`spot_pass`].
    passes: Vec<u32>,
    /// For each instance that a pass draws, where its values are in the buffer of values: the
    /// instances drawn, in the scene's order.
    values_of: Vec<u32>,
    /// The values of the instances drawn, as the buffer holds them.
    values: Vec<u8>,
    /// The keys of the instances that the scene pass draws, and of those that the shadow passes
    /// draw, sorted ([`draw_keys`]).
    scene_keys: Vec<u128>,
    shadow_keys: Vec<u128>,
    /// For each draw of each pass, in the order of the passes and of their draws, where the
    /// values of the instances it draws are, as the buffer of the list holds them: 32-bit
    /// numbers.
    list: Vec<u8>,
}
/// The passes of the renderer's frame (ADR-0041), as its graph names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    /// The depths of the instances as the shadowed light sees them, into the tile of each cascade.
    Shadow,
    /// The depths of the instances as each spot light that casts shadows sees them, into its tile
    /// of the atlas.
    SpotShadows,
    /// The instances, shaded, into the high-dynamic-range target.
    Scene,
    /// The environment where no surface is.
    Sky,
    /// The samples of the scene and the sky, weighed by their brightness, averaged into one pixel
    /// each, which the bloom and the tone mapping read.
    Resolve,
    /// The bloom's level of this index (from 0, the finest), filtered down from the one above.
    BloomDown(usize),
    /// The average of the bloom's level of this index and the coarser ones.
    BloomUp(usize),
    /// The light mapped to the output's range, the bloom mixed in.
    ToneMapping,
}

/// What decides the passes and targets of a frame: the output's size, whether the shadow pass
/// runs, whether the spot lights' shadow pass does, and whether the bloom's passes do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    width: u32,
    height: u32,
    shadow: bool,
    spot_shadows: bool,
    bloom: bool,
}

/// The frame's graph for one [`Shape`], with the groups that read its targets. Built again,
/// together, when the shape changes, since a group keeps the targets it was created with
/// (ADR-0033).
struct Frame {
    shape: Shape,
    graph: Graph,
    /// For each pass, in the graph's order, the group through which it reads the transient targets
    /// it declares, created from that declaration so that the two cannot differ; `None` for the
    /// passes that read none (shadow, scene and sky).
    groups: Vec<Option<BindGroup>>,
}

/// What the shadow pass draws into the tile of one cascade, and what the scene pass reads of it.
struct CascadeDraw {
    tile: ShadowTile,
    /// From the frame's space to the cascade's clip space.
    view_projection: Mat4,
    /// The width of its texels, in metres.
    texel: f32,
    /// The draws of the instances that cast shadows across its square.
    draws: Vec<Batch>,
}

/// What the spot lights' shadow pass draws into one tile of the atlas.
struct SpotDraw {
    tile: ShadowTile,
    /// The draws of the instances that cast shadows into the tile.
    draws: Vec<Batch>,
}

/// One draw call: the instances of one pipeline, material and mesh that a pass draws together.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Batch {
    /// The index in the scene's instances of the first instance it draws, whose pipeline,
    /// material and mesh are those of every one.
    first: usize,
    /// Where the instances it draws are in the list of the instances that the draws draw.
    instances: Range<u32>,
}

/// How many levels the bloom's chain has for an output of `width` × `height` pixels: the level
/// of index i, from 1, is the output divided by 2^i, rounded to the nearest (`scaled_side`), while
/// both sides keep at least [`BLOOM_SMALLEST_SIDE`] texels; none below 15 × 15. Each side is then
/// half the one before, rounded down or up, so that a pixel's position doubled or halved falls
/// where it should; and each level's edge stays within half a texel of the output's: where the
/// output's sides are not multiples of 2^i, a pixel's halo at the right and bottom edges holds
/// 0.83 to 1.37 of its light, up to 1.9 at the far corner (ADR-0040), against 0.2 or ten times its
/// light with levels rounded down or up.
fn bloom_levels(width: u32, height: u32) -> usize {
    (1..)
        .take_while(|&index| {
            scaled_side(width, index) >= BLOOM_SMALLEST_SIDE
                && scaled_side(height, index) >= BLOOM_SMALLEST_SIDE
        })
        .count()
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
    /// [`RenderError::Gfx`] if the graphics driver rejects one of the renderer's four shaders or
    /// ten pipelines, made from the embedded sources ([`ShaderSources::embedded`]); with
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold the shadow map or the renderer's
    /// other objects; or with [`GfxError::DeviceLost`] if the device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        // The frame's values, then the environment, the BRDF table and the sampler that reads both.
        let frame_layout = gpu.create_bind_group_layout(
            "frame",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::VertexAndFragment,
                    ty: BindingType::UniformBuffer {
                        dynamic_offset: false,
                        size: FRAME_VALUES_SIZE as u64,
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
        )?;
        // The material's values, then each texture slot: a texture and its sampler.
        let mut material_entries = vec![BindGroupLayoutEntry {
            binding: 0,
            visible_to: ShaderStages::Fragment,
            ty: BindingType::UniformBuffer {
                dynamic_offset: false,
                size: MATERIAL_VALUES_SIZE as u64,
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
        let material_layout = gpu.create_bind_group_layout("material", &material_entries)?;
        // The instances' values, then the list of the instances that each draw draws, by the index
        // of their values: the vertex shader passes on what the fragment shader needs.
        let instance_layout = gpu.create_bind_group_layout(
            "instances",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Vertex,
                    ty: BindingType::StorageBuffer {
                        size: INSTANCE_VALUES_SIZE as u64,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Vertex,
                    ty: BindingType::StorageBuffer { size: 4 },
                },
            ],
        )?;
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
                BindGroupLayoutEntry {
                    binding: 2,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::DepthTexture,
                },
            ],
        )?;
        // The view of the map or tile a shadow pass draws, one slot per view (`ShadowView` in
        // `shader.wgsl`).
        let shadow_views_layout = gpu.create_bind_group_layout(
            "shadow views",
            &[BindGroupLayoutEntry {
                binding: 3,
                visible_to: ShaderStages::Vertex,
                ty: BindingType::UniformBuffer {
                    dynamic_offset: true,
                    size: SHADOW_VIEW_SIZE as u64,
                },
            }],
        )?;
        let shadow_sampler = gpu.create_comparison_sampler(
            "shadow map",
            DepthCompare::GreaterEqual,
            Filter::Linear,
        )?;
        // Bound in the place of either atlas until a light casts shadows into it, the scene shader
        // reading no tile of it then.
        let no_atlas = gpu.create_depth_target("no shadow atlas", 1, 1)?;
        let shadow_group =
            shadow_group(gpu, &shadow_layout, &no_atlas, &shadow_sampler, &no_atlas)?;
        // The views of the cascades, then one per tile of the spot lights' atlas.
        let shadow_view_stride = SHADOW_VIEW_SIZE.next_multiple_of(gpu.uniform_offset_alignment());
        let shadow_views = gpu.create_uniform_buffer(
            "shadow views",
            &vec![0; (CASCADES + MAX_LIGHTS) * shadow_view_stride],
        )?;
        let shadow_views_group = gpu.create_bind_group(
            "shadow views",
            &shadow_views_layout,
            &[BindGroupEntry {
                binding: 3,
                resource: BindingResource::UniformBufferRange {
                    buffer: &shadow_views,
                    size: SHADOW_VIEW_SIZE,
                },
            }],
        )?;
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
        let values = |size: usize| BindingType::UniformBuffer {
            dynamic_offset: false,
            size: size as u64,
        };
        // The scene's light, the bloom's blurred copy, the sampler that magnifies it, the strength.
        let tone_mapping_layout = gpu.create_bind_group_layout(
            "tone mapping",
            &fragment_entries(&[
                BindingType::Texture,
                BindingType::Texture,
                BindingType::Sampler,
                values(TONE_MAPPING_SETTINGS_SIZE),
            ]),
        )?;
        let bloom_down_layout = gpu.create_bind_group_layout(
            "bloom down",
            &fragment_entries(&[BindingType::Texture, BindingType::Sampler]),
        )?;
        let bloom_up_layout = gpu.create_bind_group_layout(
            "bloom up",
            &fragment_entries(&[
                BindingType::Texture,
                BindingType::Sampler,
                BindingType::Texture,
                values(BLOOM_WEIGHT_SIZE),
            ]),
        )?;
        let bloom_sampler = gpu.create_sampler(
            "bloom",
            &SamplerDescriptor {
                address_u: AddressMode::MirrorRepeat,
                address_v: AddressMode::MirrorRepeat,
                ..SamplerDescriptor::default()
            },
        )?;
        // The scene's samples, read one by one.
        let resolve_layout = gpu.create_bind_group_layout(
            "resolve",
            &fragment_entries(&[BindingType::MultisampledTexture]),
        )?;
        let layouts = Layouts {
            frame: frame_layout,
            material: material_layout.clone(),
            instance: instance_layout,
            shadow: shadow_layout,
            shadow_views: shadow_views_layout,
            tone_mapping: tone_mapping_layout,
            bloom_down: bloom_down_layout,
            bloom_up: bloom_up_layout,
            resolve: resolve_layout,
        };
        let pipelines = Pipelines::new(gpu, &ShaderSources::embedded(), &layouts, format)?;
        let tone_mapping_settings =
            gpu.create_uniform_buffer("tone-mapping settings", &[0; TONE_MAPPING_SETTINGS_SIZE])?;
        let frame_uniform = gpu.create_uniform_buffer("frame", &[0; FRAME_VALUES_SIZE])?;
        let no_environment = Environment::black(gpu)?;
        let brdf_table = brdf_table(gpu)?;
        let environment_sampler = gpu.create_sampler(
            "environment",
            &SamplerDescriptor {
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
                ..SamplerDescriptor::default()
            },
        )?;
        let frame_group = frame_group(
            gpu,
            &layouts.frame,
            &frame_uniform,
            &no_environment,
            &brdf_table,
            &environment_sampler,
        )?;
        let white = gpu.create_texture(
            "white",
            1,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 255, 255, 255],
        )?;
        let default_sampler = gpu.create_sampler("default", &SamplerDescriptor::default())?;
        let instances = InstanceBuffers::new(
            gpu,
            &layouts.instance,
            INITIAL_INSTANCE_CAPACITY,
            INITIAL_INSTANCE_CAPACITY,
        )?;
        if !gpu.can_time() {
            tracing::info!("the adapter cannot time its work: the statistics have no GPU time");
        }
        Ok(Self {
            format,
            pipelines,
            layouts,
            material_parts: MaterialParts {
                layout: material_layout,
                white,
                sampler: default_sampler,
            },
            frame_uniform,
            frame_group: (no_environment.clone(), frame_group),
            no_environment,
            brdf_table,
            environment_sampler,
            meshes: MeshPages::default(),
            instances,
            scratch: Scratch::default(),
            frame: None,
            cascade_atlas: None,
            no_atlas,
            shadow_group,
            shadow_sampler,
            shadow_views,
            shadow_view_stride,
            shadow_views_group,
            spot_atlas: None,
            bloom_sampler,
            tone_mapping_settings,
            warned_of_lights: false,
            left_out_lights: Vec::new(),
            warned_of_instances: false,
        })
    }

    /// Makes the renderer's pipelines again from `sources`, all of them or none: when a shader
    /// does not compile or a pipeline is refused, the renderer keeps the pipelines it had, and
    /// draws on with them (ADR-0077). A game that hot-reloads gives it the sources of the shaders'
    /// files when they change ([`ShaderWatch`](crate::ShaderWatch)).
    ///
    /// It compiles the four shaders and makes the ten pipelines on the calling thread, which waits
    /// for the driver: the frame that calls it is late by that long. In a release build on an RX
    /// 6900 XT (RADV), 7 ms when the driver's cache holds every shader already, as when the
    /// sources come back to shaders that it compiled before, and 76 ms when it holds none; an
    /// edited shader is one that the cache does not hold (ADR-0104), which was not measured there.
    /// On lavapipe in a release build, 45 ms from unchanged sources on a machine of 16 threads; on
    /// one of 4 logical CPUs, 55 to 60 ms from unchanged sources and 57 to 71 ms after an edit of
    /// the scene's shader, over two runs. A debug build takes about twice as long, in wgpu's own
    /// work. Direct3D 12 compiles the
    /// shaders again with DXC for each pipeline. The benchmark `render.shaders` of `slime-benches`
    /// measures both cases on the machine that runs it.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with [`GfxError::Shader`] if a shader does not compile, whose message
    /// names the shader's file and gives the line and column of the error; with
    /// [`GfxError::Pipeline`] if a pipeline is refused, as when a shader reads more values than the
    /// renderer gives it at a uniform binding, or, on Direct3D 12, when the shader compiler
    /// rejects one; with [`GfxError::DeviceLost`] once the device is lost.
    pub fn set_shaders(&mut self, gpu: &Gpu, sources: &ShaderSources) -> Result<(), RenderError> {
        self.pipelines = Pipelines::new(gpu, sources, &self.layouts, self.format)?;
        Ok(())
    }

    /// Computes the environment of `sky` on the CPU and uploads it (ADR-0039): the sky drawn into a
    /// cube map of 128 × 128 texels per face, each the average of 2 × 2 points of it, then
    /// prefiltered for roughnesses 0.25, 0.5, 0.75 and 1 into faces of 64, 32, 16 and 8 texels, the
    /// last also the light that matte surfaces diffuse: 1.0 MiB of half-precision floats. This
    /// takes tens of milliseconds on every core in a release build, several times as long in a
    /// debug build: create an environment once per sky, not per frame. A sky of one radiance in
    /// every direction, without a sun, takes almost none. The environment is not tied to this
    /// renderer.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with [`GfxError::OutOfMemory`] when the GPU's memory cannot hold it,
    /// or [`GfxError::DeviceLost`] once the device is lost: nothing is created then.
    ///
    /// # Panics
    ///
    /// If the zenith's, the horizon's or the ground's radiance is negative, not finite, or above
    /// 65 504 nits, the largest value of the environment's half-precision texels, or if the sun is
    /// not a light that the renderer draws ([`Light::check`], its shadow distance aside): this is a
    /// bug in the caller. A clear sky stays below about 30 000 nits away from the sun. A sun whose
    /// disc would be brighter than 10³⁰ nits is drawn that bright.
    pub fn create_environment(&self, gpu: &Gpu, sky: &Sky) -> Result<Environment, RenderError> {
        Ok(Environment::new(gpu, sky)?)
    }

    /// Uploads a cooked texture, every level as the cooker made it, in the GPU format of its
    /// encoding: BC7 with or without sRGB, BC5, or 8-bit texels with or without sRGB. A game reads
    /// it from its cooked data with `slime_assets::CookedAssets::texture`, and gives it, with a
    /// sampler, to a material's slot of the same kind: colors (sRGB) or data. `label` names it in
    /// error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with [`GfxError::UnsupportedTexture`] for a texture of blocks on a GPU
    /// that cannot sample them, which no desktop GPU of Linux and Windows is; with
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, or with
    /// [`GfxError::DeviceLost`] once the device is lost.
    ///
    /// # Panics
    ///
    /// If `data`'s levels are not those of its size and encoding, or if it is larger than the
    /// device allows ([`Gpu::max_texture_dimension`]): a texture read from a cooked file has its
    /// levels, and is no larger than `slime_assets::MAX_SIDE`, which the device's default limits
    /// allow.
    pub fn create_texture(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &CookedTexture,
    ) -> Result<Texture, RenderError> {
        upload_texture(gpu, label, data)
    }

    /// Uploads a mesh: 48 bytes per vertex and 4 per index, into a page of the meshes that this
    /// renderer creates ([`Mesh`]), a new page when none has room. `label` names it in error
    /// messages.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] if `data` cannot be drawn: a vertex array (normals, uvs,
    /// tangents) of another length than the positions, no vertex, no triangle, a number of indices
    /// that is not a multiple of three, an index that names no vertex, a vertex value (position,
    /// normal, uv or tangent) that is not finite, or vertices or indices that need a buffer larger
    /// than the device accepts ([`Gpu::max_buffer_size`], 256 MiB with the
    /// default limits: 5 592 405 vertices, or 67 108 864 indices). [`RenderError::Gfx`] with
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold the new page that the mesh
    /// needs, or with [`GfxError::DeviceLost`] once the device is lost.
    pub fn create_mesh(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &MeshData,
    ) -> Result<Mesh, RenderError> {
        Mesh::new(gpu, &self.meshes, label, data)
    }

    /// Creates a material. `label` names it in error messages and in GPU capture tools. The
    /// material keeps its textures and samplers alive; its values can change afterwards
    /// ([`Material::set_values`]).
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMaterial`] if a value of `data` is out of the range that
    /// [`MaterialValues`](crate::MaterialValues) gives; [`RenderError::Gfx`] with
    /// [`GfxError::OutOfMemory`] when the GPU's memory cannot hold it, or
    /// [`GfxError::DeviceLost`] once the device is lost. Nothing is created then.
    ///
    /// # Panics
    ///
    /// If a texture's format does not suit its slot: the base color and emissive textures hold
    /// colors and need an sRGB format; the metallic-roughness, normal and occlusion textures hold
    /// data and need a format without sRGB encoding, such as `Rgba8Unorm`. Read in the other kind
    /// of format, their values would be wrong: this is a bug in the caller.
    pub fn create_material(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &MaterialData<'_>,
    ) -> Result<Material, RenderError> {
        self.material_parts.create(gpu, label, data)
    }

    /// Draws `scene` into `target`, in passes. The first directional light of `scene.lights` that
    /// casts shadows casts them through four cascades: what the camera sees up to the light's
    /// [`shadow_distance`](crate::DirectionalLight::shadow_distance) is split along its axis
    /// into four slices, each twice as deep as the one before, and each slice is covered by a
    /// square across the light, of 2048 × 2048 texels whatever its size (ADR-0038, ADR-0049,
    /// ADR-0050, ADR-0064). The shadow pass draws into the tile of each cascade the depths of
    /// the instances that cast shadows across its square, all their faces (the parts that masked
    /// materials discard excepted), as the light sees them; it is skipped when no directional
    /// light casts shadows, or when no instance that casts shadows reaches any cascade. The
    /// spot lights' shadow pass then draws, into the tile of the atlas of each spot light
    /// that casts shadows, the depths of the instances that cast shadows inside its cone and
    /// range, seen in perspective from the light (ADR-0058, ADR-0061); it is skipped when no
    /// spot light casts shadows. The scene pass draws the instances whose boxes the camera
    /// sees into a high-dynamic-range target of four samples per pixel with the depth test,
    /// so that nearer surfaces hide farther ones whatever their order, each surface shaded by
    /// the scene's light with glTF's metallic-roughness model and its material's textures,
    /// the shadowed light's only where its cascades hold nothing nearer to it: the finest
    /// cascade whose square holds a point shades it, and fades into the next across a band, a
    /// tenth of its width, at its edge, as the last one does into light, beyond which nothing
    /// is shadowed (ADR-0065); a point is looked up in each cascade at most 4.5 of its texels
    /// off its triangle, towards the light, and compared through a filter over 5 × 5 texels
    /// so that a shadow's edges fade smoothly over about six texels (ADR-0063); each surface is
    /// culled or not as its material and its transform ask (ADR-0035), the scene's environment
    /// reflected as its material reflects it (ADR-0039), its light multiplied by the scene's
    /// exposure (ADR-0037). Each spot light that casts shadows lights a point only where its
    /// tile holds nothing nearer to it, looked up off the triangle by up to 4.5 of the tile's
    /// texels and compared as the directional light's cascades are (ADR-0061, ADR-0062);
    /// point lights cast no shadows. The sky pass draws the environment, exposed, where no
    /// surface was drawn, with its sun's disc. The resolve pass then averages the samples of
    /// each pixel, each weighed by 1 / (1 + its brightest channel), which smooths the edges of
    /// what both passes drew, the bright ones too (ADR-0056, ADR-0060).  Unless `scene.bloom`
    /// is 0 or `target` is smaller than 15 × 15, the bloom's passes filter that light down
    /// into levels of half the size of the one before, to a side of at least 8 texels, with
    /// Jimenez's 13-tap filter, then average them back up through 3 × 3 tents, every level
    /// with the same weight, into a blurred copy (ADR-0040): 11 passes at 1280 × 720, 13 at
    /// 1920 × 1080. The tone-mapping pass then mixes the blurred copy in by `scene.bloom`,
    /// maps that light to the range a display shows with Khronos PBR Neutral, which lowers
    /// a color whose channels all lie between 0.08 and 0.8 by 0.04, a darker one less, and
    /// compresses a brighter one towards 1 and turns it towards white, dithers it when `target`
    /// has 8 bits per channel, with noise of up to one step that keeps its average and leaves
    /// black and white as they are (ADR-0051), and writes it into `target`, opaque. A render
    /// graph orders the passes, and they are recorded in that order, four to a submission
    /// named after `target` (ADR-0041); the work runs on the GPU after this method returns.
    ///
    /// The high-dynamic-range and depth targets are created at the size of `target` the first
    /// time, and again whenever that size changes; the bloom's levels (4.7 MiB at 1280 × 720,
    /// 10.5 MiB at 1920 × 1080) likewise, and only while the bloom runs: they are freed when it
    /// stops, and created again, in one frame, when it resumes. The buffers of the instances'
    /// values and of their list grow when a frame draws more than they hold, and shrink back once
    /// a frame uses a quarter of them or less. Each instance drawn takes 128 bytes of values, and
    /// 4 bytes in the list for each pass that draws it.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with [`GfxError::OutOfMemory`] when the GPU's memory cannot hold what
    /// the frame creates: the targets of a new size, the bloom's levels when it resumes, the
    /// instances' buffers when they grow or shrink, or the bindings of a new environment; or with
    /// [`GfxError::DeviceLost`] once the device is lost, whether the frame creates anything or not.
    /// Nothing is drawn then, and the renderer keeps what it had: the next call tries again, so
    /// that a game can free memory, or draw into a smaller target, in between.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the renderer was created for; if the scene's exposure
    /// or bloom is outside the range its documentation gives (the message names it): these are bugs
    /// in the caller. Instances beyond what the device's storage buffers hold are left out, with a
    /// warning ([`Renderer`]). A light with a value outside the range its documentation gives,
    /// which a game's data may hold ([`Light::check`]), or a point or spot light farther than
    /// 10¹⁸ m from the camera along an axis, where the shader's 32-bit floats no longer measure it,
    /// is left out, with a warning that names it by its index in the scene's lights when it is
    /// first left out; the [`MAX_LIGHTS`] drawn are the first of the others.
    ///
    /// With glam's assertions on (see [`Camera`]), also for a camera that would otherwise draw
    /// nothing usable, as [`Camera::view_projection`] does.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        scene: &Scene<'_>,
    ) -> Result<RenderStats, RenderError> {
        let started = Instant::now();
        assert!(
            scene.exposure.is_finite() && scene.exposure >= 0.0,
            "the scene's exposure is {}; it is finite and not negative",
            scene.exposure
        );
        assert!(
            (0.0..=1.0).contains(&scene.bloom),
            "the scene's bloom is {}; it is from 0 to 1",
            scene.bloom
        );
        // Lights come from a game's data, which a player should not see end the game: a light that
        // cannot be drawn is left out, and a scene with more lights than the renderer packs draws
        // its first ones.
        let mut left_out = Vec::new();
        let mut lights = Vec::with_capacity(scene.lights.len().min(MAX_LIGHTS));
        for (index, light) in scene.lights.iter().enumerate() {
            match light.check_from(scene.camera.eye) {
                Ok(()) => lights.push(*light),
                Err(reason) => {
                    // In the order of the indices, as `left_out` is.
                    if self.left_out_lights.binary_search(&index).is_err() {
                        tracing::warn!(
                            "light {index} of the scene is left out: {reason} (said again once it \
                             is drawn and left out again)"
                        );
                    }
                    left_out.push(index);
                }
            }
        }
        self.left_out_lights = left_out;
        if lights.len() > MAX_LIGHTS && !self.warned_of_lights {
            tracing::warn!(
                lights = lights.len(),
                "the scene has more lights than the renderer draws, {MAX_LIGHTS}: the first are \
                 drawn and the others left out (said once per renderer)"
            );
            self.warned_of_lights = true;
        }
        lights.truncate(MAX_LIGHTS);
        let lights = &lights[..];
        assert!(
            target.format() == self.format,
            "the renderer draws into {:?} targets, not into `{}`, which is {:?}",
            self.format,
            target.label(),
            target.format()
        );
        // A lost device ignores what is recorded on it: without this, a frame that creates
        // nothing would report itself drawn.
        gpu.check()?;
        let (width, height) = (target.width(), target.height());
        let bloom = scene.bloom > 0.0 && bloom_levels(width, height) > 0;
        // An 8-bit output is dithered by one of its 255 steps; a floating-point one is not.
        let step = if self.format == TextureFormat::Rgba16Float {
            0.0
        } else {
            1.0 / 255.0
        };
        let srgb = if self.format.is_srgb() { 1.0 } else { 0.0 };
        let mut settings = Vec::with_capacity(TONE_MAPPING_SETTINGS_SIZE);
        push_f32s(
            &mut settings,
            &[if bloom { scene.bloom } else { 0.0 }, step, srgb, 0.0],
        );
        gpu.write_uniform_buffer(&self.tone_mapping_settings, &settings);

        // Every position reaches the GPU relative to the camera (ADR-0048).
        let origin = scene.camera.eye;
        let aspect = width as f32 / height as f32;
        let view_projection = scene.camera.projection(aspect) * scene.camera.view_from_eye();
        let instances = scene.instances;
        let Scratch {
            models,
            passes,
            values_of,
            values,
            scene_keys,
            shadow_keys,
            list,
        } = &mut self.scratch;
        reuse(models, instances.len());
        models.extend(
            instances
                .iter()
                .map(|instance| relative_to(origin, instance.transform)),
        );
        let boxed = |slot: usize| {
            let mesh = instances[slot].mesh;
            (models[slot], mesh.center, mesh.half_size)
        };
        // The scene pass draws what the camera sees.
        let frustum = Frustum::new(view_projection);
        reuse(passes, instances.len());
        passes.extend((0..instances.len()).map(|slot| {
            if frustum.holds(boxed(slot)) {
                SCENE_PASS
            } else {
                0
            }
        }));
        // The first directional light that casts shadows casts them, through its cascades, each
        // drawing the instances that cast shadows across its square (ADR-0038, ADR-0064); none when
        // the camera sees nothing.
        let shadowed = lights
            .iter()
            .enumerate()
            .find_map(|(index, light)| match light {
                Light::Directional(light) if light.casts_shadows => Some((index, light)),
                _ => None,
            })
            .and_then(|(index, light)| {
                let views = (0..CASCADES)
                    .map(|cascade| {
                        let camera = &scene.camera;
                        let (near, far) = cascade_slice(camera, light.shadow_distance, cascade);
                        CascadeView::new(light.direction, camera, aspect, near, far)
                    })
                    .collect::<Option<Vec<CascadeView>>>()?;
                Some((index, views))
            });
        // Each spot light that casts shadows, among those drawn, gets a tile of the atlas, in the
        // scene's order, and draws there the instances that cast shadows into it (ADR-0058).
        let spot_lights: Vec<&SpotLight> = lights
            .iter()
            .filter_map(|light| match light {
                Light::Spot(light) if light.casts_shadows => Some(light),
                _ => None,
            })
            .collect();
        let tiles: Vec<ShadowTile> = spot_tiles(spot_lights.len()).collect();
        let spot_views: Vec<SpotView> = spot_lights
            .iter()
            .zip(&tiles)
            .map(|(light, tile)| SpotView::new(light, origin, tile.size))
            .collect();
        for (slot, instance) in instances.iter().enumerate() {
            if !instance.casts_shadows {
                continue;
            }
            let boxed = boxed(slot);
            for (cascade, view) in shadowed.iter().flat_map(|(_, views)| views).enumerate() {
                if view.reaches(boxed) {
                    passes[slot] |= cascade_pass(cascade);
                }
            }
            for (tile, (light, view)) in spot_lights.iter().zip(&spot_views).enumerate() {
                if view.reaches(light.range, boxed) {
                    passes[slot] |= spot_pass(tile);
                }
            }
        }
        // A frame draws at most as many instances as a storage buffer holds values of (G29): the
        // last ones that a pass draws are left out of every pass, rather than make a buffer that
        // the device refuses.
        let most = u32::try_from(gpu.max_storage_buffer_size() / INSTANCE_VALUES_SIZE as u64)
            .unwrap_or(u32::MAX);
        reuse(values_of, instances.len());
        let (drawn, left_out) = place_values(passes, most, values_of);
        if left_out > 0 && !self.warned_of_instances {
            tracing::warn!(
                drawn,
                left_out,
                "the passes draw more instances than the device's storage buffers hold the values \
                 of: the last ones, in the scene's order, are left out (said once per renderer)"
            );
            self.warned_of_instances = true;
        }
        // Each cascade's depths are fitted to the boxes that reach across its square; a cascade
        // that nothing reaches is cleared, and the points its square holds are lit.
        let mut shadow = shadowed.and_then(|(index, views)| {
            let reached = passes.iter().any(|&pass| pass & CASCADE_PASSES != 0);
            let cascades = views.iter().enumerate().map(|(cascade, view)| {
                let reaching = (0..instances.len())
                    .filter(|&slot| passes[slot] & cascade_pass(cascade) != 0)
                    .map(boxed);
                CascadeDraw {
                    tile: cascade_tile(cascade),
                    view_projection: view.view_projection(reaching),
                    texel: view.texel,
                    draws: Vec::new(),
                }
            });
            reached.then(|| (index, cascades.collect::<Vec<CascadeDraw>>()))
        });
        // The draws of each pass go by pipeline, then material, then mesh, so that each binds
        // only what changed since the one before, and draws together the instances of one.
        reuse(scene_keys, instances.len());
        reuse(shadow_keys, instances.len());
        draw_keys(instances, passes, scene_keys, shadow_keys);
        list.clear();
        let scene_draws = batches(scene_keys.iter().copied(), values_of, list);
        let (passes, shadow_keys): (&[u32], &[u128]) = (passes, shadow_keys);
        let in_pass = |pass: u32| {
            shadow_keys
                .iter()
                .copied()
                .filter(move |&key| passes[slot_of(key)] & pass != 0)
        };
        if let Some((_, cascades)) = &mut shadow {
            for (cascade, draw) in cascades.iter_mut().enumerate() {
                draw.draws = batches(in_pass(cascade_pass(cascade)), values_of, list);
            }
        }
        let spot_draws: Vec<SpotDraw> = tiles
            .iter()
            .enumerate()
            .map(|(index, &tile)| SpotDraw {
                tile,
                draws: batches(in_pass(spot_pass(index)), values_of, list),
            })
            .collect();
        // The values of the instances drawn, in the scene's order.
        values.clear();
        for (slot, instance) in instances.iter().enumerate() {
            if values_of[slot] == NOT_DRAWN {
                continue;
            }
            let linear = linear(instance.transform);
            push_mat4(values, models[slot]);
            push_mat3(values, normal_matrix(linear));
            let handedness = if is_mirrored(linear) { -1.0 } else { 1.0 };
            push_f32s(values, &[handedness, 0.0, 0.0, 0.0]);
        }
        let capacity = (
            instance_capacity(self.instances.capacity.0, drawn as usize).min(most as usize),
            instance_capacity(self.instances.capacity.1, list.len() / 4)
                .min((gpu.max_storage_buffer_size() / 4) as usize),
        );
        if capacity != self.instances.capacity {
            tracing::debug!(?capacity, "resizing the instances' buffers");
            self.instances =
                InstanceBuffers::new(gpu, &self.layouts.instance, capacity.0, capacity.1)?;
            // The memory of the values and of the list follows the buffers, back down after a
            // peak.
            values.shrink_to(capacity.0 * INSTANCE_VALUES_SIZE);
            list.shrink_to(capacity.1 * 4);
        }
        if !values.is_empty() {
            gpu.write_storage_buffer(&self.instances.values, 0, values);
        }
        if !list.is_empty() {
            gpu.write_storage_buffer(&self.instances.list, 0, list);
        }

        // Each atlas is created by the first frame that draws into it, and the scene pass reads it
        // from then on, where it read a texel of its own.
        let mut created = false;
        if shadow.is_some() && self.cascade_atlas.is_none() {
            self.cascade_atlas = Some(gpu.create_depth_target(
                "cascades' atlas",
                CASCADE_ATLAS_SIZE,
                CASCADE_ATLAS_SIZE,
            )?);
            created = true;
        }
        if !spot_draws.is_empty() && self.spot_atlas.is_none() {
            self.spot_atlas = Some(gpu.create_depth_target(
                "spot shadow atlas",
                SPOT_ATLAS_SIZE,
                SPOT_ATLAS_SIZE,
            )?);
            created = true;
        }
        if created {
            self.shadow_group = shadow_group(
                gpu,
                &self.layouts.shadow,
                self.cascade_atlas.as_ref().unwrap_or(&self.no_atlas),
                &self.shadow_sampler,
                self.spot_atlas.as_ref().unwrap_or(&self.no_atlas),
            )?;
        }

        let mut frame_bytes = Vec::with_capacity(FRAME_VALUES_SIZE);
        push_mat4(&mut frame_bytes, view_projection);
        // Without the camera's position, its inverse gives directions from the camera.
        push_mat4(&mut frame_bytes, view_projection.inverse());
        let (shadowed, cascades) = match &shadow {
            Some((index, cascades)) => (
                u32::try_from(*index).expect("below MAX_LIGHTS"),
                cascades.as_slice(),
            ),
            None => (NO_SHADOWED_LIGHT, [].as_slice()),
        };
        push_f32s(&mut frame_bytes, &[scene.exposure]);
        push_u32s(
            &mut frame_bytes,
            &[
                u32::try_from(lights.len()).expect("at most MAX_LIGHTS"),
                shadowed,
                0,
            ],
        );
        let environment = scene.environment.unwrap_or(&self.no_environment);
        let (to_sun, sun) = (environment.to_sun, environment.sun_radiance);
        push_f32s(&mut frame_bytes, &[to_sun.x, to_sun.y, to_sun.z, 0.0]);
        push_f32s(&mut frame_bytes, &[sun.x, sun.y, sun.z, 0.0]);
        // Each spot light that casts shadows with the index of its tile, in their order.
        let mut tiles = 0..;
        for light in lights {
            let tile = match light {
                Light::Spot(light) if light.casts_shadows => tiles.next().expect("endless"),
                _ => NO_TILE,
            };
            light.push_values(&mut frame_bytes, origin, tile);
        }
        // The slots of the lights the scene does not have: the shader does not read them.
        frame_bytes.resize(
            frame_bytes.len() + (MAX_LIGHTS - lights.len()) * LIGHT_VALUES_SIZE,
            0,
        );
        // The tiles, in the atlas's texture coordinates, and the width of their texels 1 m along
        // their light's axis.
        let atlas = SPOT_ATLAS_SIZE as f32;
        for (view, draw) in spot_views.iter().zip(&spot_draws) {
            let ShadowTile { x, y, size } = draw.tile;
            push_mat4(&mut frame_bytes, view.view_projection);
            push_f32s(
                &mut frame_bytes,
                &[
                    x as f32 / atlas,
                    y as f32 / atlas,
                    size as f32 / atlas,
                    2.0 * view.tan_half / size as f32,
                ],
            );
        }
        frame_bytes.resize(
            frame_bytes.len() + (MAX_LIGHTS - spot_draws.len()) * SHADOW_TILE_VALUES_SIZE,
            0,
        );
        // The cascades' tiles, in their atlas's texture coordinates, and the width of their texels
        // in metres; nothing without a shadowed light.
        let atlas = CASCADE_ATLAS_SIZE as f32;
        for cascade in cascades {
            let ShadowTile { x, y, size } = cascade.tile;
            push_mat4(&mut frame_bytes, cascade.view_projection);
            push_f32s(
                &mut frame_bytes,
                &[
                    x as f32 / atlas,
                    y as f32 / atlas,
                    size as f32 / atlas,
                    cascade.texel,
                ],
            );
        }
        frame_bytes.resize(FRAME_VALUES_SIZE, 0);
        gpu.write_uniform_buffer(&self.frame_uniform, &frame_bytes);

        // The views of the shadow passes, each in its slot: the cascades', then the spot tiles'.
        let slots = (CASCADES + MAX_LIGHTS) * self.shadow_view_stride;
        let mut view_bytes = Vec::with_capacity(slots);
        let cascade_views = (0..CASCADES).map(|index| {
            cascades
                .get(index)
                .map_or(Mat4::IDENTITY, |cascade| cascade.view_projection)
        });
        let views = cascade_views.chain(spot_views.iter().map(|view| view.view_projection));
        for view in views {
            push_mat4(&mut view_bytes, view);
            view_bytes.resize(
                view_bytes.len().next_multiple_of(self.shadow_view_stride),
                0,
            );
        }
        view_bytes.resize(slots, 0);
        gpu.write_uniform_buffer(&self.shadow_views, &view_bytes);
        if !self.frame_group.0.is(environment) {
            let group = frame_group(
                gpu,
                &self.layouts.frame,
                &self.frame_uniform,
                environment,
                &self.brdf_table,
                &self.environment_sampler,
            )?;
            self.frame_group = (environment.clone(), group);
        }
        let frame_group = &self.frame_group.1;

        let shape = Shape {
            width,
            height,
            shadow: shadow.is_some(),
            spot_shadows: !spot_draws.is_empty(),
            bloom,
        };
        if self.frame.as_ref().is_none_or(|frame| frame.shape != shape) {
            tracing::debug!(?shape, "building the frame's graph");
            self.frame = Some(self.build_frame(gpu, shape, self.frame.as_ref())?);
        }
        let frame = self.frame.as_ref().expect("built above");

        let mut stats = RenderStats::default();
        // In the order `declare_frame` imports them.
        let mut imported = vec![Imported::Color(target)];
        for (drawn, atlas) in [
            (shape.shadow, &self.cascade_atlas),
            (shape.spot_shadows, &self.spot_atlas),
        ] {
            if let (true, Some(atlas)) = (drawn, atlas) {
                imported.push(Imported::Depth(atlas));
            }
        }
        // The submissions are named after the output, so that capture tools show which it drew.
        frame.graph.record(
            gpu,
            target.label(),
            &imported,
            |index, pass, render_pass| {
                let group = frame.groups[index].as_ref();
                self.record(
                    pass,
                    render_pass,
                    group,
                    frame_group,
                    scene,
                    &scene_draws,
                    cascades,
                    &spot_draws,
                    &mut stats,
                );
            },
        );
        fold_gpu_times(
            frame
                .graph
                .declarations()
                .iter()
                .map(|declaration| declaration.pass),
            |index| frame.graph.gpu_time(index),
            &mut stats,
        );
        stats.cpu_time = started.elapsed();
        Ok(stats)
    }

    /// Draws the depths of the instances `instances` of the list, of `instance`'s mesh and
    /// material, in a shadow pass, with the shadow pipeline its material asks for, set when it
    /// differs from the one set last.
    fn draw_caster<'a>(
        &self,
        render_pass: &mut RenderPass<'_>,
        bound: &mut Bound<'a>,
        instance: &Instance<'a>,
        instances: Range<u32>,
    ) {
        let masked = instance.material.masked;
        if bound.pipeline != Some(u8::from(masked)) {
            render_pass.set_pipeline(if masked {
                &self.pipelines.masked_shadow
            } else {
                &self.pipelines.shadow
            });
            bound.pipeline = Some(u8::from(masked));
        }
        // Only the masked materials' pipeline reads the material.
        draw(render_pass, bound, instance, masked, instances);
    }

    /// Records `pass`, which reads its transient targets through `group`, drawing `scene`, and
    /// counts its draws in `stats`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the frame's parts that every pass may need, gathered by `render`"
    )]
    fn record(
        &self,
        pass: Pass,
        render_pass: &mut RenderPass<'_>,
        group: Option<&BindGroup>,
        frame_group: &BindGroup,
        scene: &Scene<'_>,
        scene_draws: &[Batch],
        cascades: &[CascadeDraw],
        spot_draws: &[SpotDraw],
        stats: &mut RenderStats,
    ) {
        match pass {
            Pass::Shadow => {
                render_pass.set_bind_group(0, frame_group, &[]);
                render_pass.set_bind_group(2, &self.instances.group, &[]);
                let mut bound = Bound::default();
                // Each cascade's view, in the first slots.
                for (view, cascade) in (0..).zip(cascades) {
                    let ShadowTile { x, y, size } = cascade.tile;
                    render_pass.set_viewport(x, y, size, size);
                    let offset = u32::try_from(view * self.shadow_view_stride)
                        .expect("at most CASCADES views");
                    render_pass.set_bind_group(3, &self.shadow_views_group, &[offset]);
                    for batch in &cascade.draws {
                        let instance = &scene.instances[batch.first];
                        self.draw_caster(
                            render_pass,
                            &mut bound,
                            instance,
                            batch.instances.clone(),
                        );
                        stats.shadow_draw_calls += 1;
                    }
                }
            }
            Pass::SpotShadows => {
                render_pass.set_bind_group(0, frame_group, &[]);
                render_pass.set_bind_group(2, &self.instances.group, &[]);
                let mut bound = Bound::default();
                // Each tile's view, in the slots after the cascades'.
                for (view, draw) in (CASCADES..).zip(spot_draws) {
                    let ShadowTile { x, y, size } = draw.tile;
                    render_pass.set_viewport(x, y, size, size);
                    let offset = u32::try_from(view * self.shadow_view_stride)
                        .expect("at most MAX_LIGHTS views");
                    render_pass.set_bind_group(3, &self.shadow_views_group, &[offset]);
                    for batch in &draw.draws {
                        let instance = &scene.instances[batch.first];
                        self.draw_caster(
                            render_pass,
                            &mut bound,
                            instance,
                            batch.instances.clone(),
                        );
                        stats.spot_shadow_draw_calls += 1;
                    }
                }
            }
            Pass::Scene => {
                render_pass.set_bind_group(0, frame_group, &[]);
                render_pass.set_bind_group(2, &self.instances.group, &[]);
                render_pass.set_bind_group(3, &self.shadow_group, &[]);
                // A pipeline is set only when a draw needs another, which the draws' order makes
                // rare.
                let mut bound = Bound::default();
                for batch in scene_draws {
                    let instance = &scene.instances[batch.first];
                    let cull = scene_pipeline(instance);
                    if bound.pipeline != Some(cull as u8) {
                        render_pass.set_pipeline(self.pipelines.scene.get(cull));
                        bound.pipeline = Some(cull as u8);
                    }
                    draw(
                        render_pass,
                        &mut bound,
                        instance,
                        true,
                        batch.instances.clone(),
                    );
                    let count = batch.instances.end - batch.instances.start;
                    stats.draw_calls += 1;
                    stats.instances += count;
                    stats.triangles += u64::from(instance.mesh.triangle_count()) * u64::from(count);
                }
            }
            Pass::Sky => {
                render_pass.set_pipeline(&self.pipelines.sky);
                render_pass.set_bind_group(0, frame_group, &[]);
                render_pass.draw(0..3);
            }
            Pass::Resolve => {
                render_pass.set_pipeline(&self.pipelines.resolve);
                render_pass.set_bind_group(0, group.expect("the resolve reads the samples"), &[]);
                render_pass.draw(0..3);
            }
            Pass::BloomDown(_) | Pass::BloomUp(_) => {
                render_pass.set_pipeline(if matches!(pass, Pass::BloomDown(_)) {
                    &self.pipelines.bloom_down
                } else {
                    &self.pipelines.bloom_up
                });
                render_pass.set_bind_group(0, group.expect("a bloom pass reads its levels"), &[]);
                render_pass.draw(0..3);
            }
            Pass::ToneMapping => render_pass.debug_group("tone mapping", |render_pass| {
                render_pass.set_pipeline(&self.pipelines.tone_mapping);
                render_pass.set_bind_group(0, group.expect("tone mapping reads the scene"), &[]);
                render_pass.draw(0..3);
            }),
        }
    }

    /// The frame of `shape`: its graph, compiled, with its transient targets created (those of
    /// `previous` reused where they match) and its timers, and the groups that read the targets.
    fn build_frame(
        &self,
        gpu: &Gpu,
        shape: Shape,
        previous: Option<&Frame>,
    ) -> Result<Frame, GfxError> {
        let mut graph = declare_frame(shape);
        // `declare_frame` builds every shape from the same rules, which the tests compile.
        if let Err(error) = graph.compile() {
            panic!("the renderer declared a frame it cannot draw: {error}");
        }
        graph.create(gpu, previous.map(|frame| &frame.graph))?;
        let levels = bloom_levels_of(shape);
        let groups = graph
            .declarations()
            .iter()
            .map(|declaration| {
                // The transient render target the pass declares at `slot` of its reads.
                let read = |slot: usize| declaration.reads[slot].map(|target| graph.color(target));
                let entries = match declaration.pass {
                    Pass::Shadow | Pass::SpotShadows | Pass::Scene | Pass::Sky => return Ok(None),
                    Pass::Resolve => (
                        "resolve".to_owned(),
                        &self.layouts.resolve,
                        vec![BindingResource::RenderTarget(read(0).expect("declared"))],
                        None,
                    ),
                    // The level above: the scene's light for the first.
                    Pass::BloomDown(level) => (
                        format!("bloom down {level}"),
                        &self.layouts.bloom_down,
                        vec![
                            BindingResource::RenderTarget(read(0).expect("declared")),
                            BindingResource::Sampler(&self.bloom_sampler),
                        ],
                        None,
                    ),
                    // The coarser average (the coarsest level's own light for the last), and the
                    // level's own light, mixed in by 1 / (the levels from it to the coarsest), so
                    // that each level ends with the same share of the blurred copy, and no average
                    // exceeds the light it averages, which a sum would beyond half precision near
                    // the sun.
                    Pass::BloomUp(level) => {
                        let mut weight = Vec::with_capacity(BLOOM_WEIGHT_SIZE);
                        push_f32s(&mut weight, &[1.0 / (levels - level) as f32, 0.0, 0.0, 0.0]);
                        (
                            format!("bloom up {level}"),
                            &self.layouts.bloom_up,
                            vec![
                                BindingResource::RenderTarget(read(0).expect("declared")),
                                BindingResource::Sampler(&self.bloom_sampler),
                                BindingResource::RenderTarget(read(1).expect("declared")),
                            ],
                            Some(weight),
                        )
                    }
                    // The scene's light and the blurred copy; without the bloom, the scene's light
                    // again: any texture does, since the pass then mixes it in by 0.
                    Pass::ToneMapping => {
                        let hdr = read(0).expect("declared");
                        (
                            "tone mapping".to_owned(),
                            &self.layouts.tone_mapping,
                            vec![
                                BindingResource::RenderTarget(hdr),
                                BindingResource::RenderTarget(read(1).unwrap_or(hdr)),
                                BindingResource::Sampler(&self.bloom_sampler),
                                BindingResource::UniformBuffer(&self.tone_mapping_settings),
                            ],
                            None,
                        )
                    }
                };
                let (label, layout, mut resources, weight) = entries;
                let weight = weight
                    .map(|weight| gpu.create_uniform_buffer(&label, &weight))
                    .transpose()?;
                if let Some(weight) = &weight {
                    resources.push(BindingResource::UniformBuffer(weight));
                }
                let entries: Vec<BindGroupEntry<'_>> = (0..)
                    .zip(resources)
                    .map(|(binding, resource)| BindGroupEntry { binding, resource })
                    .collect();
                gpu.create_bind_group(&label, layout, &entries).map(Some)
            })
            .collect::<Result<_, _>>()?;
        Ok(Frame {
            shape,
            graph,
            groups,
        })
    }
}

/// Folds the GPU time of each pass of `passes`, in order (`time(k)` for the k-th), into
/// `stats`: each kind of pass into its field, the bloom's passes summed, `None` when one of them
/// has no time, or when the frame has none.
fn fold_gpu_times(
    passes: impl Iterator<Item = Pass>,
    time: impl Fn(usize) -> Option<Duration>,
    stats: &mut RenderStats,
) {
    let mut bloom: Option<Option<Duration>> = None;
    for (index, pass) in passes.enumerate() {
        let time = time(index);
        match pass {
            Pass::Shadow => stats.shadow_gpu_time = time,
            Pass::SpotShadows => stats.spot_shadow_gpu_time = time,
            Pass::Scene => stats.scene_gpu_time = time,
            Pass::Sky => stats.sky_gpu_time = time,
            Pass::Resolve => stats.resolve_gpu_time = time,
            Pass::BloomDown(_) | Pass::BloomUp(_) => {
                bloom = Some(match bloom {
                    None => time,
                    Some(sum) => sum.zip(time).map(|(sum, time)| sum + time),
                });
            }
            Pass::ToneMapping => stats.tone_mapping_gpu_time = time,
        }
    }
    stats.bloom_gpu_time = bloom.flatten();
}

/// The graph of a frame of `shape`, declared in the order the passes run, not compiled yet.
fn declare_frame(shape: Shape) -> Graph {
    let mut graph = Graph::new(shape.width, shape.height);
    // Given to `Graph::record` in this order.
    let output = graph.import_color("output");
    let cascade_atlas = shape.shadow.then(|| graph.import_depth("cascades' atlas"));
    let spot_atlas = shape
        .spot_shadows
        .then(|| graph.import_depth("spot shadow atlas"));
    // The scene and the sky are drawn at four samples per pixel, which smooths the edges of what
    // they draw (ADR-0056), then resolved into a target of one sample that the later passes
    // sample. Labelled "scene", which names the scene and sky passes in capture tools.
    let samples = graph.transient_multisampled_color("scene", HDR_FORMAT);
    let depth = graph.transient_multisampled_depth("scene depth");
    let hdr = graph.transient_color("scene resolved", HDR_FORMAT, 0);
    let pass = |pass, draws, reads| PassDeclaration { pass, draws, reads };
    // Each pass that draws one triangle over its target clears it: a clear reads nothing back on
    // GPUs that keep targets in tiles.
    let cleared = |target| Draws::Color {
        target,
        depth: None,
        clear: true,
        resolve: None,
    };
    if let Some(atlas) = cascade_atlas {
        graph.add_pass(pass(Pass::Shadow, Draws::Depth(atlas), [None, None]));
    }
    if let Some(atlas) = spot_atlas {
        graph.add_pass(pass(Pass::SpotShadows, Draws::Depth(atlas), [None, None]));
    }
    // The sky pass draws every pixel that no surface covers. The scene pass binds both atlases, or
    // a texel in the place of one not yet created, whether their passes run or not: its shader
    // reads no tile that the frame did not draw.
    graph.add_pass(pass(
        Pass::Scene,
        Draws::Color {
            target: samples,
            depth: Some(depth),
            clear: true,
            resolve: None,
        },
        [cascade_atlas.map(Into::into), spot_atlas.map(Into::into)],
    ));
    // Over the scene, keeping its light and its depths: the sky is drawn only where the depth
    // target still holds the farthest depth, where no surface was drawn, sample by sample, so that
    // a pixel on a surface's edge holds both the surface and the sky.
    graph.add_pass(pass(
        Pass::Sky,
        Draws::Color {
            target: samples,
            depth: Some(depth),
            clear: false,
            resolve: None,
        },
        [None, None],
    ));
    // The samples averaged, each weighed by its brightness (ADR-0060), into the target that the
    // later passes read.
    graph.add_pass(pass(
        Pass::Resolve,
        cleared(hdr),
        [Some(samples.into()), None],
    ));
    let levels = bloom_levels_of(shape);
    let level_shift = |index: usize| u32::try_from(index + 1).expect("a few levels");
    let bloom_down: Vec<ColorId> = (0..levels)
        .map(|index| {
            graph.transient_color(
                format!("bloom down {index}"),
                HDR_FORMAT,
                level_shift(index),
            )
        })
        .collect();
    let bloom_up: Vec<ColorId> = (0..levels.saturating_sub(1))
        .map(|index| {
            graph.transient_color(format!("bloom up {index}"), HDR_FORMAT, level_shift(index))
        })
        .collect();
    for (index, &level) in bloom_down.iter().enumerate() {
        let above = if index == 0 {
            hdr
        } else {
            bloom_down[index - 1]
        };
        graph.add_pass(pass(
            Pass::BloomDown(index),
            cleared(level),
            [Some(above.into()), None],
        ));
    }
    for (index, &average) in bloom_up.iter().enumerate().rev() {
        let coarser = bloom_up.get(index + 1).unwrap_or(&bloom_down[index + 1]);
        graph.add_pass(pass(
            Pass::BloomUp(index),
            cleared(average),
            [Some((*coarser).into()), Some(bloom_down[index].into())],
        ));
    }
    // The tone-mapping pass clears rather than keeps the output: its triangle covers every pixel,
    // and a clear does not read what the window's image held before.
    let blurred = bloom_up.first().or(bloom_down.first()).copied();
    graph.add_pass(pass(
        Pass::ToneMapping,
        cleared(output),
        [Some(hdr.into()), blurred.map(Into::into)],
    ));
    graph
}

/// How many levels the bloom's chain has in a frame of `shape`: none without the bloom.
fn bloom_levels_of(shape: Shape) -> usize {
    if shape.bloom {
        bloom_levels(shape.width, shape.height)
    } else {
        0
    }
}

/// The group through which the scene pass reads the `cascades`' atlas, `sampler` and the spot
/// lights' atlas, `spots`.
fn shadow_group(
    gpu: &Gpu,
    layout: &BindGroupLayout,
    cascades: &DepthTarget,
    sampler: &ComparisonSampler,
    spots: &DepthTarget,
) -> Result<BindGroup, GfxError> {
    gpu.create_bind_group(
        "shadow atlases",
        layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::DepthTarget(cascades),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::ComparisonSampler(sampler),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::DepthTarget(spots),
            },
        ],
    )
}

/// The frame's group: the frame's values, `environment`, the BRDF table and their sampler.
fn frame_group(
    gpu: &Gpu,
    layout: &BindGroupLayout,
    uniform: &UniformBuffer,
    environment: &Environment,
    brdf_table: &Texture,
    sampler: &Sampler,
) -> Result<BindGroup, GfxError> {
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

/// `transform` followed by a move by −`origin`: from the mesh's space to world space moved so that
/// `origin` is at its origin, the space the renderer draws in (ADR-0048). Computed in `f64`, then
/// rounded to `f32`: a mesh 1 m from a camera 10 000 km from the world's origin keeps the precision
/// it has at the origin.
fn relative_to(origin: DVec3, transform: DAffine3) -> Mat4 {
    DMat4::from(DAffine3::from_translation(-origin) * transform).as_mat4()
}

/// The 3 × 3 part of an instance's transform, which turns, scales and mirrors it, in `f32`: what
/// its normals and its culling depend on, which no translation changes.
fn linear(transform: DAffine3) -> Mat3 {
    transform.matrix3.as_mat3()
}

/// The matrix that transforms the normals of an instance whose transform's 3 × 3 part is `linear`.
/// It gives directions only: the shader normalizes the normals.
///
/// It is the cofactor matrix of that part: the inverse transpose, which keeps
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
fn normal_matrix(linear: Mat3) -> Mat3 {
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
        / if is_mirrored(linear) {
            -largest
        } else {
            largest
        }
}

/// The layouts of the groups that the renderer's pipelines read, made once: the shaders' sources
/// can change, the layouts do not.
struct Layouts {
    /// The frame's group: the frame's values, the environment, the BRDF table and their sampler.
    frame: BindGroupLayout,
    /// A material's group: its values, and its five textures with their samplers.
    material: BindGroupLayout,
    /// An instance's values, at an offset given when the group is bound.
    instance: BindGroupLayout,
    /// The atlases and their comparison sampler, which the scene pass reads.
    shadow: BindGroupLayout,
    /// The view of a shadow pass's tile, at an offset given when the group is bound.
    shadow_views: BindGroupLayout,
    /// What the tone-mapping pass reads: the scene's light, the bloom's blurred copy, the sampler
    /// that magnifies it, and its settings.
    tone_mapping: BindGroupLayout,
    /// What the bloom's passes read: down, the finer level and the sampler; up, the coarser
    /// average, the sampler, the level's own light and its weight.
    bloom_down: BindGroupLayout,
    bloom_up: BindGroupLayout,
    /// The scene's samples, which the resolve pass reads one by one.
    resolve: BindGroupLayout,
}

/// The renderer's ten pipelines, made from the sources of its four shaders.
struct Pipelines {
    /// The scene pass's.
    scene: ScenePipelines,
    /// Draws the environment where no surface is.
    sky: RenderPipeline,
    /// Draws the instances' depths as the shadowed light sees them, culling no face.
    shadow: RenderPipeline,
    /// The same, discarding what masked materials discard.
    masked_shadow: RenderPipeline,
    /// Averages the samples of each pixel, weighed by their brightness.
    resolve: RenderPipeline,
    /// Filters a level of the bloom down from the one above, the scene's light for the first.
    bloom_down: RenderPipeline,
    /// Averages a level of the bloom with the coarser ones.
    bloom_up: RenderPipeline,
    /// Maps the light to the output's range, the bloom mixed in.
    tone_mapping: RenderPipeline,
}

impl Pipelines {
    /// The pipelines that `sources` make, reading groups of `layouts`, the tone-mapping pass's
    /// drawing into targets of `format`. Each shader module is named after its file, so that an
    /// error says which file its line and column are in.
    fn new(
        gpu: &Gpu,
        sources: &ShaderSources,
        layouts: &Layouts,
        format: TextureFormat,
    ) -> Result<Self, GfxError> {
        // The four modules first, so that a shader that does not compile is found before any
        // pipeline is made.
        let shader = gpu.create_shader_module("shader.wgsl", &sources.scene)?;
        let bloom_shader = gpu.create_shader_module("bloom.wgsl", &sources.bloom)?;
        let resolve_shader = gpu.create_shader_module("resolve.wgsl", &sources.resolve)?;
        let tone_mapping_shader =
            gpu.create_shader_module("tone_mapping.wgsl", &sources.tone_mapping)?;
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
                    &layouts.frame,
                    &layouts.material,
                    &layouts.instance,
                    &layouts.shadow,
                ],
                depth: Some(DepthState {
                    compare: DepthCompare::Greater,
                    write: true,
                    bias: DepthBias::default(),
                }),
                cull,
                multisampled: true,
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
                bind_group_layouts: &[
                    &layouts.frame,
                    &layouts.material,
                    &layouts.instance,
                    &layouts.shadow_views,
                ],
                depth: Some(DepthState {
                    compare: DepthCompare::Greater,
                    write: true,
                    bias: SHADOW_BIAS,
                }),
                cull: CullMode::None,
                multisampled: false,
            })
        };
        let masked_shadow = shadow_pipeline(
            "shadow, masked",
            Some(FragmentState {
                entry_point: "fs_shadow",
                target_format: None,
            }),
        )?;
        let shadow = shadow_pipeline("shadow", None)?;
        let scene = ScenePipelines {
            back_culled: scene_pipeline("scene, back faces culled", CullMode::Back)?,
            front_culled: scene_pipeline("scene, front faces culled", CullMode::Front)?,
            unculled: scene_pipeline("scene, no face culled", CullMode::None)?,
        };
        // Where the depth target still holds the farthest depth, which the sky's triangle has.
        let sky = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "sky",
            shader: &shader,
            vertex_entry_point: "vs_sky",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_sky",
                target_format: Some(HDR_FORMAT),
            }),
            bind_group_layouts: &[&layouts.frame],
            depth: Some(DepthState {
                compare: DepthCompare::GreaterEqual,
                write: false,
                bias: DepthBias::default(),
            }),
            cull: CullMode::None,
            multisampled: true,
        })?;
        // The passes that cover their target with one triangle, without depth.
        let fullscreen = |label: &str,
                          shader: &ShaderModule,
                          entry_point: &str,
                          layout: &BindGroupLayout,
                          target_format: TextureFormat| {
            gpu.create_render_pipeline(&RenderPipelineDescriptor {
                label,
                shader,
                vertex_entry_point: "vs_main",
                vertex_layout: None,
                fragment: Some(FragmentState {
                    entry_point,
                    target_format: Some(target_format),
                }),
                bind_group_layouts: &[layout],
                depth: None,
                cull: CullMode::None,
                multisampled: false,
            })
        };
        Ok(Self {
            scene,
            sky,
            shadow,
            masked_shadow,
            resolve: fullscreen(
                "resolve",
                &resolve_shader,
                "fs_main",
                &layouts.resolve,
                HDR_FORMAT,
            )?,
            bloom_down: fullscreen(
                "bloom down",
                &bloom_shader,
                "fs_downsample",
                &layouts.bloom_down,
                HDR_FORMAT,
            )?,
            bloom_up: fullscreen(
                "bloom up",
                &bloom_shader,
                "fs_upsample",
                &layouts.bloom_up,
                HDR_FORMAT,
            )?,
            tone_mapping: fullscreen(
                "tone mapping",
                &tone_mapping_shader,
                "fs_main",
                &layouts.tone_mapping,
                format,
            )?,
        })
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

/// Whether a transform whose 3 × 3 part is `linear` mirrors what it places, turning its front faces
/// clockwise on screen: the determinant of that part is negative beyond rounding. The determinant
/// of the columns
/// normalized is compared with −10⁻⁵. A mesh flattened between two rotations has columns in one
/// plane up to rounding, a determinant of noise of either sign, about 10⁻⁷ at most, which does
/// not count as mirrored, nor does a column of zero length. Uniform scales between about 10⁻¹⁹ and
/// 10¹⁹ are told apart, as by `normal_matrix`.
fn is_mirrored(linear: Mat3) -> bool {
    let [x, y, z] = [linear.x_axis, linear.y_axis, linear.z_axis].map(Vec3::normalize_or_zero);
    x.dot(y.cross(z)) < -MIRROR_THRESHOLD
}

/// Empties `entries` for a frame that needs `needed` of them, and gives back their memory once a
/// frame needs a quarter of it or less, down to [`INITIAL_INSTANCE_CAPACITY`], as the instances'
/// buffers shrink (`instance_capacity`): a peak does not keep its memory for every later frame.
fn reuse<T>(entries: &mut Vec<T>, needed: usize) {
    entries.clear();
    if needed <= entries.capacity() / 4 && entries.capacity() > INITIAL_INSTANCE_CAPACITY {
        entries.shrink_to(needed.max(INITIAL_INSTANCE_CAPACITY));
    }
}

/// How many entries a buffer of the instances has for a frame that needs `instances` when it has
/// `capacity`: twice as many as before until they fit, and back to the smallest such size from
/// [`INITIAL_INSTANCE_CAPACITY`] once the frame uses a quarter or less, so that a peak (a
/// loading screen, a burst of debris) does not keep its memory for every later frame, and a count
/// that hovers around a size does not resize the buffer at every frame.
fn instance_capacity(capacity: usize, instances: usize) -> usize {
    let fitting = |mut capacity: usize| {
        while capacity < instances {
            capacity *= 2;
        }
        capacity
    };
    if instances > capacity {
        fitting(capacity.max(1))
    } else if instances <= capacity / 4 && capacity > INITIAL_INSTANCE_CAPACITY {
        fitting(INITIAL_INSTANCE_CAPACITY)
    } else {
        capacity
    }
}

/// The storage buffers of the instances that a frame draws: the values of each, and the list, for
/// each draw of each pass, of the instances it draws, by where their values are; and the group
/// that binds both, at group 2, where the vertex shader reads, for the instance of number n, the
/// values at the n-th entry of the list.
struct InstanceBuffers {
    values: StorageBuffer,
    list: StorageBuffer,
    group: BindGroup,
    /// How many instances `values` holds the values of, and how many entries `list` holds.
    capacity: (usize, usize),
}

impl InstanceBuffers {
    fn new(
        gpu: &Gpu,
        layout: &BindGroupLayout,
        values: usize,
        list: usize,
    ) -> Result<Self, GfxError> {
        let buffers = (
            gpu.create_storage_buffer("instances", (values * INSTANCE_VALUES_SIZE) as u64)?,
            gpu.create_storage_buffer("instances drawn", (list * 4) as u64)?,
        );
        let group = gpu.create_bind_group(
            "instances",
            layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::StorageBuffer(&buffers.0),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::StorageBuffer(&buffers.1),
                },
            ],
        )?;
        Ok(Self {
            values: buffers.0,
            list: buffers.1,
            group,
            capacity: (values, list),
        })
    }
}

/// Draws the instances `instances` of the list, of `instance`'s mesh and material, with the
/// pipeline that `pass` has set: binds its material at group 1 unless the draw before bound it,
/// or, when the pipeline does not read it (`reads_material` false), unless a material is bound at
/// all, the pipeline's layout asking for one; and its mesh's page unless the draw before bound
/// that page.
fn draw<'a>(
    pass: &mut RenderPass<'_>,
    bound: &mut Bound<'a>,
    instance: &Instance<'a>,
    reads_material: bool,
    instances: Range<u32>,
) {
    let material_bound = bound.material.is_some_and(|material| {
        !reads_material || material.identity() == instance.material.identity()
    });
    if !material_bound {
        pass.set_bind_group(1, &instance.material.group, &[]);
        bound.material = Some(instance.material);
    }
    let place = &instance.mesh.place;
    if !bound
        .page
        .is_some_and(|page| Arc::ptr_eq(page, &place.page))
    {
        pass.set_vertex_buffer(0, &place.page.vertices);
        pass.set_index_buffer(&place.page.indices);
        bound.page = Some(&place.page);
    }
    let base_vertex =
        i32::try_from(place.vertices.start).expect("a page holds fewer than 2^31 vertices");
    pass.draw_indexed(place.indices.clone(), base_vertex, instances);
}

/// What a pass bound for the draws before the next: its pipeline, by the index that the pass gives
/// its pipelines, the material and the page of the meshes. A draw binds only what differs. The
/// pipelines of a pass share their groups' layouts and their vertices' layout, so that what is
/// bound stays bound when one pipeline replaces another.
#[derive(Default)]
struct Bound<'a> {
    pipeline: Option<u8>,
    material: Option<&'a Material>,
    page: Option<&'a Arc<Page>>,
}

/// How the scene pass draws `instance`: the faces its pipeline culls.
fn scene_pipeline(instance: &Instance<'_>) -> CullMode {
    cull_mode(
        instance.material.double_sided,
        is_mirrored(linear(instance.transform)),
    )
}

/// The order in which objects first appear in a frame's instances, by what each and its clones
/// share (`Mesh::identity`, `Material::identity`): what
/// [`draw_keys`] sorts by, which a frame of 10 000 instances asks 20 000 times.
#[derive(Default)]
struct FirstSeen {
    ranks: HashMap<*const (), u32>,
    /// The last object asked for, and its rank: instances that follow each other often share
    /// their material and their mesh.
    last: Option<(*const (), u32)>,
}

impl FirstSeen {
    /// The rank of `object` among those asked for: 0 for the first, then 1 for the next new one.
    fn rank(&mut self, object: *const ()) -> u32 {
        if let Some((last, rank)) = self.last
            && last == object
        {
            return rank;
        }
        let next = u32::try_from(self.ranks.len()).expect("fewer objects than 2^32");
        let rank = *self.ranks.entry(object).or_insert(next);
        self.last = Some((object, rank));
        rank
    }
}

/// Fills `values_of` with where the values of each instance that `passes` says a pass draws are,
/// in the scene's order, and [`NOT_DRAWN`] for the others, after leaving out of every pass the
/// instances drawn beyond the first `most`. Returns how many instances are drawn, and how many
/// were left out.
fn place_values(passes: &mut [u32], most: u32, values_of: &mut Vec<u32>) -> (u32, usize) {
    let (mut drawn, mut left_out) = (0_u32, 0_usize);
    values_of.clear();
    for pass in passes {
        if *pass != 0 && drawn == most {
            *pass = 0;
            left_out += 1;
        }
        values_of.push(if *pass == 0 { NOT_DRAWN } else { drawn });
        drawn += u32::from(*pass != 0);
    }
    (drawn, left_out)
}

/// Fills `scene` with the keys of the instances that the scene pass draws, as `passes` says, and
/// `shadows` with those of the instances that a shadow pass draws, each sorted in the order that
/// the passes draw them: by pipeline, then by material, then by mesh, each material and mesh in
/// the order it first appears among `instances`, then by slot, the index in `instances`. The
/// shadow passes' keys leave out the materials of the pipeline that does not read them, so that
/// the instances of one mesh are drawn together whatever their materials. Two instances' order is
/// the same from one frame to the next, whatever the addresses of their materials and meshes and
/// whatever the others that the passes draw, so that two surfaces at the same depth are drawn the
/// same way.
fn draw_keys(
    instances: &[Instance<'_>],
    passes: &[u32],
    scene: &mut Vec<u128>,
    shadows: &mut Vec<u128>,
) {
    keys(
        instances.iter().zip(passes).map(|(instance, &passes)| {
            let in_scene = passes & SCENE_PASS != 0;
            Draw {
                // Worked out only for the instances drawn: it normalizes the transform's axes.
                scene_pipeline: if in_scene {
                    scene_pipeline(instance) as u8
                } else {
                    0
                },
                masked: instance.material.masked,
                material: instance.material.identity(),
                mesh: instance.mesh.identity(),
                in_scene,
                in_shadows: passes & !SCENE_PASS != 0,
            }
        }),
        scene,
        shadows,
    );
}

/// What [`keys`] sorts a draw by.
struct Draw {
    scene_pipeline: u8,
    /// Whether its material is masked: the shadow passes draw it with the pipeline of masked
    /// materials, 1, which reads the material, rather than with the other, 0.
    masked: bool,
    material: *const (),
    mesh: *const (),
    in_scene: bool,
    in_shadows: bool,
}

/// [`draw_keys`], for draws given by what they are sorted by, in the order of their slots.
fn keys(draws: impl Iterator<Item = Draw>, scene: &mut Vec<u128>, shadows: &mut Vec<u128>) {
    let mut materials = FirstSeen::default();
    let mut meshes = FirstSeen::default();
    scene.clear();
    shadows.clear();
    for (slot, draw) in draws.enumerate() {
        let material = materials.rank(draw.material);
        let mesh = meshes.rank(draw.mesh);
        // One number per draw, which sorts as the tuple of its parts would, faster.
        let key = |pipeline: u8, material: u32| {
            u128::from(pipeline) << 96
                | u128::from(material) << 64
                | u128::from(mesh) << 32
                | slot as u128
        };
        if draw.in_scene {
            scene.push(key(draw.scene_pipeline, material));
        }
        if draw.in_shadows {
            let material = if draw.masked { material } else { 0 };
            shadows.push(key(u8::from(draw.masked), material));
        }
    }
    scene.sort_unstable();
    shadows.sort_unstable();
}

/// The slot of the instance of `key`: its low 32 bits; a frame holds fewer instances than 2^32.
fn slot_of(key: u128) -> usize {
    key as u32 as usize
}

/// The draws of the instances of `keys`, in their order: one for each run of keys of one pipeline,
/// material and mesh, the bits above the slot. Appends to `list` where the values of each instance
/// are, which `values_of` gives for each slot, as the buffer of the list holds them.
fn batches(keys: impl Iterator<Item = u128>, values_of: &[u32], list: &mut Vec<u8>) -> Vec<Batch> {
    let mut draws: Vec<Batch> = Vec::new();
    let mut drawn = None;
    for key in keys {
        let slot = slot_of(key);
        let entry = u32::try_from(list.len() / 4).expect("the list is smaller than 2^32 entries");
        push_u32s(list, &[values_of[slot]]);
        match draws.last_mut() {
            Some(draw) if drawn == Some(key >> 32) => draw.instances.end += 1,
            _ => {
                draws.push(Batch {
                    first: slot,
                    instances: entry..entry + 1,
                });
                drawn = Some(key >> 32);
            }
        }
    }
    draws
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use glam::{EulerRot, Quat, Vec4};

    use super::*;

    /// Draws of two materials and two meshes, in turn, and of two pipelines: each pass draws them
    /// by pipeline, then material, then mesh, as each first appears, then by slot, whatever their
    /// addresses; the shadow passes only those that cast shadows, by mesh alone for the materials
    /// that are not masked.
    #[test]
    fn draws_go_by_pipeline_then_material_then_mesh_as_they_first_appear() {
        // The second material and mesh at lower addresses than the first: the order is not theirs.
        let objects = [0_u8; 4];
        let address = |index: usize| ptr::from_ref(&objects[index]).cast::<()>();
        let (first_material, second_material) = (address(3), address(0));
        let (first_mesh, second_mesh) = (address(2), address(1));
        let draw = |pipeline: u8, material, mesh, in_shadows| Draw {
            scene_pipeline: pipeline,
            masked: pipeline == 1,
            material,
            mesh,
            in_scene: true,
            in_shadows,
        };
        let draws = [
            draw(0, first_material, first_mesh, true),
            draw(0, second_material, first_mesh, true),
            draw(1, first_material, second_mesh, true),
            draw(0, first_material, second_mesh, false),
            draw(0, second_material, first_mesh, true),
            draw(0, first_material, first_mesh, true),
        ];
        let (mut scene, mut shadows) = (Vec::new(), Vec::new());
        keys(draws.into_iter(), &mut scene, &mut shadows);
        let slots = |keys: &[u128]| keys.iter().map(|&key| slot_of(key)).collect::<Vec<_>>();
        assert_eq!(slots(&scene), [0, 5, 3, 1, 4, 2]);
        // The shadow pipeline of materials that are not masked reads no material.
        assert_eq!(slots(&shadows), [0, 1, 4, 5, 2]);
        // The draws of one pipeline, material and mesh that follow each other are one draw each.
        let mut list = Vec::new();
        let values_of = [10, 11, 12, 13, 14, 15];
        let draws = batches(scene.iter().copied(), &values_of, &mut list);
        let firsts: Vec<(usize, Range<u32>)> = draws
            .into_iter()
            .map(|draw| (draw.first, draw.instances))
            .collect();
        assert_eq!(firsts, [(0, 0..2), (3, 2..3), (1, 3..5), (2, 5..6)]);
        let entries: Vec<u32> = list
            .chunks(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(entries, [10, 15, 13, 11, 14, 12]);
    }

    /// G29: past the most instances that the buffer of values holds, the last ones drawn are left
    /// out of every pass, rather than make a buffer that the device refuses.
    #[test]
    fn the_instances_drawn_beyond_what_the_values_buffer_holds_are_left_out_of_every_pass() {
        let mut passes = [
            SCENE_PASS,
            0,
            cascade_pass(0),
            SCENE_PASS | spot_pass(3),
            SCENE_PASS,
        ];
        let mut values_of = Vec::new();
        assert_eq!(place_values(&mut passes, 2, &mut values_of), (2, 2));
        assert_eq!(passes, [SCENE_PASS, 0, cascade_pass(0), 0, 0]);
        assert_eq!(values_of, [0, NOT_DRAWN, 1, NOT_DRAWN, NOT_DRAWN]);
        // Within the most, every instance drawn keeps its passes.
        let mut passes = [SCENE_PASS, 0, cascade_pass(1)];
        assert_eq!(place_values(&mut passes, 2, &mut values_of), (2, 0));
        assert_eq!(values_of, [0, NOT_DRAWN, 1]);
    }

    /// Only the instances that the scene pass draws are sorted for it, and only those that a
    /// shadow pass draws for the shadows; their order is the one they have among all.
    #[test]
    fn only_the_instances_drawn_are_sorted_in_the_order_they_have_among_all() {
        let objects = [0_u8; 2];
        let address = |index: usize| ptr::from_ref(&objects[index]).cast::<()>();
        let draw = |material, in_scene, in_shadows| Draw {
            scene_pipeline: 0,
            masked: true,
            material: address(material),
            mesh: address(0),
            in_scene,
            in_shadows,
        };
        // The second material first appears at slot 1, which no pass draws: it still comes after
        // the first.
        let draws = [
            draw(0, false, false),
            draw(1, false, false),
            draw(1, true, true),
            draw(0, true, false),
        ];
        let (mut scene, mut shadows) = (Vec::new(), Vec::new());
        keys(draws.into_iter(), &mut scene, &mut shadows);
        let slots = |keys: &[u128]| keys.iter().map(|&key| slot_of(key)).collect::<Vec<_>>();
        assert_eq!(slots(&scene), [3, 2]);
        assert_eq!(slots(&shadows), [2]);
    }
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
            let matrix = normal_matrix(Mat3::from_mat4(transform));
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
        let matrix = normal_matrix(Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0)));
        assert_close(direction(matrix, Vec3::Z), Vec3::Z, "facing +Z");
        assert_close(direction(matrix, Vec3::X), -Vec3::X, "facing +X");
    }

    #[test]
    fn a_mesh_flattened_along_one_axis_keeps_the_normal_of_its_plane() {
        // A surface in the plane z = 0, flattened along Z, is unchanged: it still faces +Z.
        let matrix = normal_matrix(Mat3::from_diagonal(Vec3::new(1.0, 1.0, 0.0)));
        assert!(matrix.is_finite(), "{matrix}");
        assert_close(direction(matrix, Vec3::Z), Vec3::Z, "flattened along Z");
        // Turned, then flattened along the world's Y axis: the surface now lies in the plane
        // y = 0, so its normal is along Y.
        let turned = Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)) * Mat4::from_rotation_x(0.5);
        let matrix = normal_matrix(Mat3::from_mat4(turned));
        assert!(matrix.is_finite(), "{matrix}");
        let normal = direction(matrix, Vec3::Z);
        assert!(normal.x.abs() < 1e-6 && normal.z.abs() < 1e-6, "{normal}");
    }

    #[test]
    fn the_scene_shader_holds_as_many_lights_as_the_renderer_packs() {
        // WGSL sizes the array of lights with a constant of its own, which cannot come from Rust.
        let declared = format!("const MAX_LIGHTS: u32 = {MAX_LIGHTS}u;");
        assert!(
            ShaderSources::embedded().scene.contains(&declared),
            "shader.wgsl lacks `{declared}`"
        );
        // And the same bound on what one light gives.
        assert_eq!(crate::light::LARGEST_LIGHT, 1e30);
        assert!(
            ShaderSources::embedded()
                .scene
                .contains("const LARGEST_LIGHT: f32 = 1e30;"),
            "shader.wgsl lacks `const LARGEST_LIGHT: f32 = 1e30;`"
        );
        // And as many cascades.
        let cascades = format!("const CASCADES: u32 = {CASCADES}u;");
        assert!(
            ShaderSources::embedded().scene.contains(&cascades),
            "shader.wgsl lacks `{cascades}`"
        );
        // And the index of no tile, the largest u32.
        assert_eq!(NO_TILE, u32::MAX);
        assert!(
            ShaderSources::embedded()
                .scene
                .contains("const NO_TILE: u32 = 0xffffffffu;")
        );
    }

    /// The value of the WGSL constant `name` in the scene shader.
    fn shader_constant(name: &str) -> f64 {
        constant_of(&ShaderSources::embedded().scene, name)
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
        assert_eq!(
            constant_of(&ShaderSources::embedded().tone_mapping, "HEADROOM"),
            headroom
        );
        assert_eq!(
            constant_of(&ShaderSources::embedded().resolve, "HEADROOM"),
            headroom
        );
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

    /// The sizes of the bloom's levels, as the frame's graph creates them.
    fn bloom_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
        (1..=bloom_levels(width, height) as u32)
            .map(|index| (scaled_side(width, index), scaled_side(height, index)))
            .collect()
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
            assert_eq!(is_mirrored(Mat3::from_mat4(transform)), mirrored, "{case}");
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
                assert!(!is_mirrored(Mat3::from_mat4(transform)), "{transform}");
                assert!(
                    normal_matrix(Mat3::from_mat4(transform)).is_finite(),
                    "{transform}"
                );
            }
        }
    }

    #[test]
    fn every_shape_of_the_frame_runs_its_passes_in_the_order_they_draw_and_read() {
        for (width, height) in [(1920, 1080), (1280, 720), (15, 15), (14, 14), (1, 1)] {
            for (shadow, spot_shadows) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                for bloom in [false, true] {
                    let shape = Shape {
                        width,
                        height,
                        shadow,
                        spot_shadows,
                        bloom: bloom && bloom_levels(width, height) > 0,
                    };
                    let mut graph = declare_frame(shape);
                    if let Err(error) = graph.compile() {
                        panic!("{shape:?}: {error}");
                    }
                    let levels = if shape.bloom {
                        bloom_levels(width, height)
                    } else {
                        0
                    };
                    let mut expected = Vec::new();
                    if shadow {
                        expected.push(Pass::Shadow);
                    }
                    if spot_shadows {
                        expected.push(Pass::SpotShadows);
                    }
                    expected.extend([Pass::Scene, Pass::Sky, Pass::Resolve]);
                    expected.extend((0..levels).map(Pass::BloomDown));
                    expected.extend((0..levels.saturating_sub(1)).rev().map(Pass::BloomUp));
                    expected.push(Pass::ToneMapping);
                    let passes: Vec<Pass> = graph
                        .declarations()
                        .iter()
                        .map(|declaration| declaration.pass)
                        .collect();
                    assert_eq!(passes, expected, "{shape:?}");
                }
            }
        }
    }

    #[test]
    fn what_a_frame_works_out_gives_its_memory_back_after_a_peak() {
        let mut entries: Vec<u32> = Vec::with_capacity(100_000);
        reuse(&mut entries, 50_000);
        assert!(entries.capacity() >= 100_000, "half of it is needed: kept");
        reuse(&mut entries, 10);
        assert!(entries.is_empty());
        assert!(entries.capacity() < 1_000, "{}", entries.capacity());
    }

    #[test]
    fn the_instance_buffer_grows_to_fit_and_shrinks_back_after_a_peak() {
        let initial = INITIAL_INSTANCE_CAPACITY;
        assert_eq!(instance_capacity(initial, 0), initial);
        assert_eq!(instance_capacity(initial, initial), initial);
        assert_eq!(instance_capacity(initial, initial + 1), 2 * initial);
        // A peak of 100 000 instances, then a frame of 10: back to the first size.
        let peak = instance_capacity(initial, 100_000);
        assert_eq!(peak, initial << 11);
        assert_eq!(instance_capacity(peak, 10), initial);
        // A frame using more than a quarter keeps the size; a quarter or less shrinks it to the
        // smallest size that fits, never below the first.
        assert_eq!(instance_capacity(4 * initial, initial + 1), 4 * initial);
        assert_eq!(instance_capacity(8 * initial, 2 * initial), 2 * initial);
        assert_eq!(instance_capacity(8 * initial, 2 * initial + 1), 8 * initial);
        // A count hovering around a size does not resize the buffer back and forth.
        let grown = instance_capacity(initial, initial + 1);
        assert_eq!(instance_capacity(grown, initial), grown);
    }

    #[test]
    fn each_pass_s_gpu_time_goes_to_its_field_and_the_bloom_s_are_summed() {
        let millis = |k: u64| Duration::from_millis(k);
        let passes = [
            Pass::Shadow,
            Pass::SpotShadows,
            Pass::Scene,
            Pass::Sky,
            Pass::Resolve,
            Pass::BloomDown(0),
            Pass::BloomDown(1),
            Pass::BloomUp(0),
            Pass::ToneMapping,
        ];
        let mut stats = RenderStats::default();
        fold_gpu_times(
            passes.into_iter(),
            |k| Some(millis(k as u64 + 1)),
            &mut stats,
        );
        assert_eq!(
            (
                stats.shadow_gpu_time,
                stats.spot_shadow_gpu_time,
                stats.scene_gpu_time,
                stats.sky_gpu_time,
                stats.resolve_gpu_time,
                stats.bloom_gpu_time,
                stats.tone_mapping_gpu_time
            ),
            (
                Some(millis(1)),
                Some(millis(2)),
                Some(millis(3)),
                Some(millis(4)),
                Some(millis(5)),
                Some(millis(6 + 7 + 8)),
                Some(millis(9))
            )
        );
        // A bloom pass without a time leaves the sum unknown, and a frame without the bloom's
        // passes has none.
        let mut stats = RenderStats::default();
        fold_gpu_times(
            passes.into_iter(),
            |k| (k != 5).then(|| millis(1)),
            &mut stats,
        );
        assert_eq!(stats.bloom_gpu_time, None);
        let mut stats = RenderStats::default();
        fold_gpu_times(
            [Pass::Scene, Pass::Sky, Pass::ToneMapping].into_iter(),
            |_| Some(millis(1)),
            &mut stats,
        );
        assert_eq!((stats.bloom_gpu_time, stats.shadow_gpu_time), (None, None));
    }
}
