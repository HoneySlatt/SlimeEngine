//! The renderer: draws instances of meshes, seen from a camera and lit by a directional light,
//! into a render target, in two passes: the scene in high dynamic range, then tone mapping.

use std::time::{Duration, Instant};

use glam::{Mat3, Mat4};
use slime_gfx::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    Color, CullMode, DepthBias, DepthCompare, DepthState, DepthTarget, FragmentState, Gpu,
    GpuTimer, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RenderTarget,
    Sampler, SamplerDescriptor, ShaderStages, Texture, TextureFormat, UniformBuffer,
};

use crate::bytes::{push_f32s, push_mat3, push_mat4};
use crate::camera::Camera;
use crate::error::RenderError;
use crate::light::DirectionalLight;
use crate::material::{Material, MaterialData};
use crate::mesh::{Mesh, MeshData, VERTEX_LAYOUT};

/// The shader of the scene pass.
const SCENE_SHADER: &str = include_str!("shader.wgsl");
/// The shader of the tone-mapping pass.
const TONE_MAPPING_SHADER: &str = include_str!("tone_mapping.wgsl");
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
/// The size of the values of the tone-mapping pass: the exposure, padded to 16 bytes.
const TONE_MAPPING_VALUES_SIZE: usize = 16;
/// The size of the values of a frame: the camera's `mat4x4<f32>` and the light's three
/// `vec4<f32>`.
const FRAME_VALUES_SIZE: usize = 64 + 3 * 16;
/// The size of the values each instance has in the instance buffer: its `mat4x4<f32>` transform
/// and the `mat3x3<f32>` that transforms its normals, three columns padded to 16 bytes.
const INSTANCE_VALUES_SIZE: usize = 64 + 3 * 16;
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
    /// determinant) turns its front faces clockwise on screen, so they are culled and its back
    /// faces drawn: the instance is drawn inside out.
    pub transform: Mat4,
}

/// What the renderer draws in one frame.
///
/// The light that reaches each pixel before the exposure, the background's or a surface's, must
/// stay at or below 65 504 in each channel, the largest value of the high-dynamic-range target.
/// Beyond it, a driver stores 65 504 or an infinity: the pixel is clipped, or shows a color that
/// has nothing to do with the scene (ADR-0034).
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// Where the scene is seen from.
    pub camera: Camera,
    /// What lights the surfaces.
    pub light: DirectionalLight,
    /// The light behind the scene, where no surface is drawn: linear, in the units of the light
    /// (it may exceed 1, up to the bound above), not negative, and multiplied by the exposure and
    /// tone mapped like the surfaces. Its alpha is not used: the output is opaque.
    pub clear: Color,
    /// What the scene's light, the background's included, is multiplied by before tone mapping
    /// (ADR-0034): a linear factor. 1 leaves it as it is, which shows a white surface lit at
    /// normal incidence by a light of 1 at 0.87 (sRGB 240); 2 is one photographic stop brighter;
    /// 0 gives black. Finite and not negative.
    pub exposure: f32,
    /// What to draw, in this order.
    pub instances: &'a [Instance<'a>],
}

/// What drawing a scene took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderStats {
    /// Draw calls of the scene pass: one per instance. The tone-mapping pass's single draw is not
    /// counted.
    pub draw_calls: u32,
    /// Triangles the scene pass submitted to the GPU, before culling.
    pub triangles: u64,
    /// How long [`Renderer::render`] took on the CPU: packing the values, recording both passes
    /// and submitting them.
    pub cpu_time: Duration,
    /// How long the GPU took to draw the scene into the high-dynamic-range target: the latest time
    /// that has come back, a few frames old. `None` on adapters that cannot time their work
    /// ([`Gpu::can_time`](slime_gfx::Gpu::can_time)), and until the first time comes back.
    pub scene_gpu_time: Option<Duration>,
    /// How long the GPU took for the tone-mapping pass, under the same rules. It may come from
    /// another frame than `scene_gpu_time`, and their sum is not the GPU time of a frame
    /// (ADR-0033).
    pub tone_mapping_gpu_time: Option<Duration>,
}

/// Draws scenes into output targets of one format.
///
/// Each frame takes two passes (ADR-0034): the scene pass draws the instances into a target of
/// half-precision floats, where light may exceed 1, with a depth target; the tone-mapping pass then
/// multiplies that light by the scene's exposure and maps it to the output's range with Khronos
/// PBR Neutral. The renderer owns both pipelines; the high-dynamic-range and depth targets, which
/// it creates at the size of the output it draws into and again when that size changes; the buffer
/// of the instances' values; and a GPU timer per pass.
pub struct Renderer {
    /// The output's format.
    format: TextureFormat,
    scene_pipeline: RenderPipeline,
    tone_mapping_pipeline: RenderPipeline,
    material_layout: BindGroupLayout,
    instance_layout: BindGroupLayout,
    /// The layout of the group through which the tone-mapping pass reads the scene's target and
    /// its own values.
    tone_mapping_layout: BindGroupLayout,
    frame_uniform: UniformBuffer,
    frame_group: BindGroup,
    /// The exposure, read by the tone-mapping pass.
    tone_mapping_uniform: UniformBuffer,
    /// One white pixel, bound as the texture of materials that have none.
    white: Texture,
    /// The sampler of that pixel.
    default_sampler: Sampler,
    instances: InstanceSlots,
    /// Created by the first `render`, and again when the output's size changes.
    targets: Option<SizedTargets>,
    /// Time each pass on the GPU, when the adapter can.
    scene_timer: Option<GpuTimer>,
    tone_mapping_timer: Option<GpuTimer>,
}

/// The targets that follow the output's size: what the scene pass draws into, and the group
/// through which the tone-mapping pass reads it. They are created together, and again together
/// when the size changes, since a group keeps the target it was created with (ADR-0033).
struct SizedTargets {
    /// The scene's light, in high dynamic range. Labelled "scene", which names the scene pass in
    /// capture tools.
    hdr: RenderTarget,
    depth: DepthTarget,
    tone_mapping_group: BindGroup,
}

impl SizedTargets {
    fn new(
        gpu: &Gpu,
        layout: &BindGroupLayout,
        uniform: &UniformBuffer,
        width: u32,
        height: u32,
    ) -> Self {
        let hdr = gpu.create_render_target("scene", width, height, HDR_FORMAT);
        let depth = gpu.create_depth_target("scene depth", width, height);
        let tone_mapping_group = gpu.create_bind_group(
            "tone mapping",
            layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::RenderTarget(&hdr),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::UniformBuffer(uniform),
                },
            ],
        );
        Self {
            hdr,
            depth,
            tone_mapping_group,
        }
    }
}

impl Renderer {
    /// Creates a renderer that draws into output targets of `format`: the window's, for a
    /// windowed application (`Context::surface_format` in `slime-app`). An sRGB format encodes the
    /// tone-mapped light for display; `Rgba16Float` holds the tone-mapped values themselves;
    /// `Rgba8Unorm` stores them unencoded, which displays too dark.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] if the graphics driver rejects one of the renderer's two shaders or
    /// two pipelines, or if the device is lost.
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
        let frame_layout = uniform("frame", false);
        let material_layout = gpu.create_bind_group_layout(
            "material",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::UniformBuffer {
                        dynamic_offset: false,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Sampler,
                },
            ],
        );
        let instance_layout = uniform("instance", true);
        let scene_pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "scene",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: Some(VERTEX_LAYOUT),
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: HDR_FORMAT,
            }),
            bind_group_layouts: &[&frame_layout, &material_layout, &instance_layout],
            depth: Some(DepthState {
                compare: DepthCompare::Less,
                write: true,
                bias: DepthBias::default(),
            }),
            cull: CullMode::Back,
        })?;

        let tone_mapping_shader = gpu.create_shader_module("tone mapping", TONE_MAPPING_SHADER)?;
        let tone_mapping_layout = gpu.create_bind_group_layout(
            "tone mapping",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::UniformBuffer {
                        dynamic_offset: false,
                    },
                },
            ],
        );
        let tone_mapping_pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "tone mapping",
            shader: &tone_mapping_shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: format,
            }),
            bind_group_layouts: &[&tone_mapping_layout],
            depth: None,
            cull: CullMode::None,
        })?;
        let tone_mapping_uniform =
            gpu.create_uniform_buffer("tone mapping", &[0; TONE_MAPPING_VALUES_SIZE]);

        let frame_uniform = gpu.create_uniform_buffer("frame", &[0; FRAME_VALUES_SIZE]);
        let frame_group = gpu.create_bind_group(
            "frame",
            &frame_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::UniformBuffer(&frame_uniform),
            }],
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
        if scene_timer.is_none() {
            tracing::info!("the adapter cannot time its work: the statistics have no GPU time");
        }
        Ok(Self {
            format,
            scene_pipeline,
            tone_mapping_pipeline,
            material_layout,
            instance_layout,
            tone_mapping_layout,
            frame_uniform,
            frame_group,
            tone_mapping_uniform,
            white,
            default_sampler,
            instances,
            targets: None,
            scene_timer,
            tone_mapping_timer,
        })
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
    /// material keeps its texture and sampler alive.
    pub fn create_material(&self, gpu: &Gpu, label: &str, data: &MaterialData<'_>) -> Material {
        let mut bytes = Vec::new();
        push_f32s(&mut bytes, &data.base_color);
        let uniform = gpu.create_uniform_buffer(label, &bytes);
        let (texture, sampler) = match data.base_color_texture {
            Some(sampled) => (sampled.texture, sampled.sampler),
            None => (&self.white, &self.default_sampler),
        };
        let group = gpu.create_bind_group(
            label,
            &self.material_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::UniformBuffer(&uniform),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Texture(texture),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        );
        Material { group }
    }

    /// Draws `scene` into `target`, in two passes. The scene pass clears a high-dynamic-range
    /// target to the scene's background, then draws every instance into it with the depth test,
    /// so that nearer surfaces hide farther ones whatever their order, each surface lit by the
    /// scene's light according to its normal and colored by its material. The tone-mapping pass
    /// then multiplies that light by the scene's exposure, maps it to the range a display shows,
    /// and writes it into `target`, opaque. The work runs on the GPU after this method returns, as
    /// with [`Gpu::render`](slime_gfx::Gpu::render).
    ///
    /// The high-dynamic-range and depth targets are created at the size of `target` the first
    /// time, and again whenever that size changes; the instance buffer grows when a scene has more
    /// instances than before.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the renderer was created for, or if the instances
    /// need a buffer larger than the device allows (256 MiB with the default limits: about a
    /// million instances): these are bugs in the caller.
    ///
    /// With glam's assertions on (see [`Camera`]), also for a camera or a light that would
    /// otherwise draw nothing usable: as [`Camera::view_projection`] does, and for a zero
    /// [`DirectionalLight::direction`].
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, scene: &Scene<'_>) -> RenderStats {
        let started = Instant::now();
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
                "creating the high-dynamic-range and depth targets"
            );
            self.targets = Some(SizedTargets::new(
                gpu,
                &self.tone_mapping_layout,
                &self.tone_mapping_uniform,
                width,
                height,
            ));
        }
        let targets = self.targets.as_ref().expect("created above");

        let mut frame_bytes = Vec::with_capacity(FRAME_VALUES_SIZE);
        push_mat4(
            &mut frame_bytes,
            scene.camera.view_projection(width as f32 / height as f32),
        );
        let to_light = -scene.light.direction.normalize();
        push_f32s(&mut frame_bytes, &[to_light.x, to_light.y, to_light.z, 0.0]);
        let color = scene.light.color;
        push_f32s(&mut frame_bytes, &[color.x, color.y, color.z, 0.0]);
        let ambient = scene.light.ambient;
        push_f32s(&mut frame_bytes, &[ambient.x, ambient.y, ambient.z, 0.0]);
        gpu.write_uniform_buffer(&self.frame_uniform, &frame_bytes);

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
            instance_bytes[slot * stride..slot * stride + INSTANCE_VALUES_SIZE]
                .copy_from_slice(&bytes);
        }
        gpu.write_uniform_buffer(&self.instances.buffer, &instance_bytes);

        let mut tone_mapping_bytes = Vec::with_capacity(TONE_MAPPING_VALUES_SIZE);
        push_f32s(&mut tone_mapping_bytes, &[scene.exposure, 0.0, 0.0, 0.0]);
        gpu.write_uniform_buffer(&self.tone_mapping_uniform, &tone_mapping_bytes);

        let mut stats = RenderStats::default();
        let scene_pass = RenderPassDescriptor {
            target: &targets.hdr,
            clear: Some(scene.clear),
            depth: Some(&targets.depth),
            timer: self.scene_timer.as_ref(),
        };
        gpu.render(&scene_pass, |pass| {
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.frame_group, &[]);
            for (slot, instance) in scene.instances.iter().enumerate() {
                let offset = u32::try_from(slot * stride)
                    .expect("the instance buffer is smaller than the device allows");
                pass.set_bind_group(1, &instance.material.group, &[]);
                pass.set_bind_group(2, &self.instances.group, &[offset]);
                pass.set_vertex_buffer(0, &instance.mesh.vertices);
                pass.set_index_buffer(&instance.mesh.indices);
                pass.draw_indexed(0..instance.mesh.indices.count());
                stats.draw_calls += 1;
                stats.triangles += u64::from(instance.mesh.triangle_count());
            }
        });
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
        stats.cpu_time = started.elapsed();
        stats
    }
}

/// The matrix that transforms the normals of an instance placed by `transform`. It gives
/// directions only: the shader normalizes the normals.
///
/// It is the cofactor matrix of the transform's 3 × 3 part: the inverse transpose, which keeps
/// normals perpendicular to surfaces scaled unevenly, times the determinant. It turns normals the
/// same way where the inverse exists, and stays finite where it does not: a mesh flattened along
/// one axis stays visible, and keeps the normal of its plane. It is negated when the determinant
/// is negative, so that the normals of a mirrored instance still point out of its surfaces (a
/// zero determinant counts as positive), and divided by its largest value, so that its values
/// stay within ±1, which the shader normalizes without overflow or underflow. Its cofactors are
/// products of two scales in f32, which bounds the uniform scales it handles to about 1e-19 to
/// 1e19, far beyond those of the inverse (about 1e-13 to 7e12). A mesh flattened along two axes
/// or three gets zero, but has no triangle left to draw.
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
    let determinant = linear.x_axis.dot(cofactor.x_axis);
    cofactor / if determinant < 0.0 { -largest } else { largest }
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
}

#[cfg(test)]
mod tests {
    use glam::{EulerRot, Quat, Vec3};

    use super::*;

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
}
