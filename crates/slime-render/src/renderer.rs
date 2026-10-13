//! The renderer: draws instances of meshes, seen from a camera and lit by a directional light,
//! into a render target.

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

/// The shader of every draw.
const SHADER: &str = include_str!("shader.wgsl");
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
    /// From the mesh's space to world space.
    pub transform: Mat4,
}

/// What the renderer draws in one frame.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// Where the scene is seen from.
    pub camera: Camera,
    /// What lights the surfaces.
    pub light: DirectionalLight,
    /// The color of the background, linear.
    pub clear: Color,
    /// What to draw, in this order.
    pub instances: &'a [Instance<'a>],
}

/// What drawing a scene took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderStats {
    /// Draw calls recorded: one per instance.
    pub draw_calls: u32,
    /// Triangles submitted to the GPU, before culling.
    pub triangles: u64,
    /// How long [`Renderer::render`] took on the CPU: packing the values, recording the pass and
    /// submitting it.
    pub cpu_time: Duration,
    /// How long the GPU took to draw a scene: the latest time that has come back, a few frames
    /// old. `None` on adapters that cannot time their work
    /// ([`Gpu::can_time`](slime_gfx::Gpu::can_time)), and until the first time comes back.
    pub gpu_time: Option<Duration>,
}

/// Draws scenes into render targets of one format.
///
/// It owns the pipeline, the depth target, which it creates at the size of the target it draws
/// into and again when that size changes, and the buffer of the instances' values.
pub struct Renderer {
    format: TextureFormat,
    pipeline: RenderPipeline,
    material_layout: BindGroupLayout,
    instance_layout: BindGroupLayout,
    frame_uniform: UniformBuffer,
    frame_group: BindGroup,
    /// One white pixel, bound as the texture of materials that have none.
    white: Texture,
    /// The sampler of that pixel.
    default_sampler: Sampler,
    instances: InstanceSlots,
    depth: Option<DepthTarget>,
    /// Times the scene's pass on the GPU, when the adapter can.
    timer: Option<GpuTimer>,
}

impl Renderer {
    /// Creates a renderer for targets of `format`.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] if the graphics driver rejects the renderer's shader or pipeline, or
    /// if the device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let shader = gpu.create_shader_module("scene", SHADER)?;
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
        let pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "scene",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: Some(VERTEX_LAYOUT),
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: format,
            }),
            bind_group_layouts: &[&frame_layout, &material_layout, &instance_layout],
            depth: Some(DepthState {
                compare: DepthCompare::Less,
                write: true,
                bias: DepthBias::default(),
            }),
            cull: CullMode::Back,
        })?;

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
        let timer = gpu.create_timer("scene");
        if timer.is_none() {
            tracing::info!("the adapter cannot time its work: the statistics have no GPU time");
        }
        Ok(Self {
            format,
            pipeline,
            material_layout,
            instance_layout,
            frame_uniform,
            frame_group,
            white,
            default_sampler,
            instances,
            depth: None,
            timer,
        })
    }

    /// Uploads a mesh. `label` names its buffers in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] if `data` cannot be drawn: a vertex array of another length
    /// than the positions, no vertex, no triangle, a number of indices that is not a multiple of
    /// three, or an index that names no vertex.
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

    /// Draws `scene` into `target`: clears it to the scene's color, then draws every instance
    /// with the depth test, so that nearer surfaces hide farther ones whatever their order, each
    /// surface lit by the scene's light according to its normal and colored by its material. The
    /// work runs on the GPU after this method returns, as with
    /// [`Gpu::render`](slime_gfx::Gpu::render).
    ///
    /// The depth target is created at the size of `target` the first time, and again whenever
    /// that size changes; the instance buffer grows when a scene has more instances than before.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the renderer was created for, or if the instances
    /// need a buffer larger than the device allows (256 MiB with the default limits: about a
    /// million instances): these are bugs in the caller.
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
            .depth
            .as_ref()
            .is_none_or(|depth| (depth.width(), depth.height()) != (width, height))
        {
            tracing::debug!(width, height, "creating the depth target");
            self.depth = Some(gpu.create_depth_target("scene depth", width, height));
        }
        let depth = self.depth.as_ref().expect("created above");

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

        let mut stats = RenderStats::default();
        let pass = RenderPassDescriptor {
            target,
            clear: Some(scene.clear),
            depth: Some(depth),
            timer: self.timer.as_ref(),
        };
        gpu.render(&pass, |pass| {
            pass.set_pipeline(&self.pipeline);
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
        stats.gpu_time = self.timer.as_ref().and_then(|timer| timer.latest(gpu));
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
/// stay within ±1 whatever the scale. A mesh flattened along two axes or three gets zero, but has
/// no triangle left to draw.
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
