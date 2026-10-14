//! Render pipelines: how vertices are read, which shader entry points run, what they read, and
//! what they render into.

use crate::bind_group::BindGroupLayout;
use crate::depth_target::DEPTH_FORMAT;
use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::TextureFormat;
use crate::shader::ShaderModule;

/// The type of one vertex attribute in a vertex buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VertexFormat {
    /// Two 32-bit floats; `vec2<f32>` in WGSL.
    Float32x2,
    /// Three 32-bit floats; `vec3<f32>` in WGSL.
    Float32x3,
    /// Four 32-bit floats; `vec4<f32>` in WGSL.
    Float32x4,
}

impl VertexFormat {
    fn to_wgpu(self) -> wgpu::VertexFormat {
        match self {
            Self::Float32x2 => wgpu::VertexFormat::Float32x2,
            Self::Float32x3 => wgpu::VertexFormat::Float32x3,
            Self::Float32x4 => wgpu::VertexFormat::Float32x4,
        }
    }
}

/// One attribute of a vertex: where it sits in the vertex and which shader input receives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexAttribute {
    /// The `@location` of the shader input that receives the attribute.
    pub location: u32,
    /// The offset of the attribute from the start of the vertex, in bytes.
    pub offset: u64,
    /// The type of the attribute.
    pub format: VertexFormat,
}

/// How the vertices of one vertex buffer are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexLayout<'a> {
    /// The distance from one vertex to the next, in bytes.
    pub stride: u64,
    /// The attributes of each vertex.
    pub attributes: &'a [VertexAttribute],
}

/// How a depth is compared with the one stored in a depth target, passing where
/// `new <op> stored`. For a pipeline's depth test, the new depth is a fragment's; for a
/// [`ComparisonSampler`](crate::ComparisonSampler), it is the reference the shader gives.
///
/// Depth is reversed (ADR-0047): it runs from 1, the nearest, to 0, the farthest, to which passes
/// clear their depth targets, so that nearer is greater. Floating-point depths are finest near
/// 0, which reversing spends on the far distances that a perspective projection crowds there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DepthCompare {
    /// Passes where the new depth is greater than the stored one: nearer.
    Greater,
    /// Passes where the new depth is greater than or equal to the stored one.
    GreaterEqual,
    /// Passes whatever the stored depth.
    Always,
}

impl DepthCompare {
    pub(crate) fn to_wgpu(self) -> wgpu::CompareFunction {
        match self {
            Self::Greater => wgpu::CompareFunction::Greater,
            Self::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            Self::Always => wgpu::CompareFunction::Always,
        }
    }
}

/// The depth test of a pipeline: how it uses the depth target of the pass it draws in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthState {
    /// The comparison that decides whether a fragment is drawn.
    pub compare: DepthCompare,
    /// Whether the depth of what is drawn is written to the depth target.
    pub write: bool,
    /// How far the depth of what the pipeline draws is pushed away before the test and the write;
    /// [`DepthBias::default()`] pushes nothing.
    pub bias: DepthBias,
}

/// An offset added to the depth of each fragment a pipeline draws, before the depth test:
/// `constant` × r + `slope_scale` × m, where
///
/// - r is the smallest depth difference the depth target resolves at the triangle's depth:
///   2^(e − 23) for its 32-bit floats, e being the exponent of the triangle's largest depth
///   (2^-24 for depths from 0.5 to 1);
/// - m is the triangle's depth slope, in depth per pixel: at least the larger of its slopes along
///   x and along y, at most the length of its gradient (Direct3D 12 takes the former, Vulkan
///   drivers either).
///
/// Negative values push surfaces away, towards 0, the farthest depth. A shadow map is drawn with
/// one, so that a surface does not shadow itself where its depth and the stored one differ only by
/// rounding (shadow acne). The offset is not clamped, and `slope_scale` must be finite: wgpu passes it to the driver
/// unchecked, and a NaN or an infinity gives undefined depths. The default, zero, is no bias.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DepthBias {
    /// The constant part, in units of r.
    pub constant: i32,
    /// The part proportional to the depth slope m.
    pub slope_scale: f32,
}

/// Which faces of the triangles are not drawn. A face is front-facing when its vertices wind
/// counter-clockwise on screen, as in glTF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    /// Every face is drawn.
    None,
    /// Back faces are not drawn: the usual choice for closed meshes, whose back faces are hidden
    /// by their front faces anyway.
    Back,
    /// Front faces are not drawn.
    Front,
}

impl CullMode {
    fn to_wgpu(self) -> Option<wgpu::Face> {
        match self {
            Self::None => None,
            Self::Back => Some(wgpu::Face::Back),
            Self::Front => Some(wgpu::Face::Front),
        }
    }
}

/// What [`Gpu::create_render_pipeline`] needs to build a pipeline.
///
/// The pipeline draws lists of triangles, without blending, from at most one vertex buffer, into
/// one render target of its fragment stage's format, or into a depth target only.
#[derive(Clone, Copy, Debug)]
pub struct RenderPipelineDescriptor<'a> {
    /// Names the pipeline in error messages and in GPU capture tools.
    pub label: &'a str,
    /// The module holding the entry points.
    pub shader: &'a ShaderModule,
    /// The name of the vertex shader's entry point.
    pub vertex_entry_point: &'a str,
    /// The layout of the vertex buffer bound at slot 0, or `None` for a pipeline that reads no
    /// vertex buffer: its vertex shader builds each vertex from `@builtin(vertex_index)`, as a
    /// fullscreen triangle does, and its draws set no vertex buffer. A vertex shader with
    /// `@location` inputs needs a layout.
    pub vertex_layout: Option<VertexLayout<'a>>,
    /// The fragment shader and the format of the render targets it writes, or `None` for a
    /// pipeline that writes depth only: it has no fragment shader, needs a depth test
    /// ([`depth`](Self::depth)), and draws only in passes without a render target
    /// ([`Gpu::render_depth`]).
    pub fragment: Option<FragmentState<'a>>,
    /// The layouts of the bind groups the shaders read, by `@group` index. A binding that the
    /// shaders declare and that no layout holds is rejected.
    pub bind_group_layouts: &'a [&'a BindGroupLayout],
    /// The depth test, or `None` for none. A pipeline with a depth test draws only in passes that
    /// have a depth target, and one without only in passes without
    /// ([`RenderPassDescriptor::depth`](crate::RenderPassDescriptor::depth)).
    pub depth: Option<DepthState>,
    /// Which faces are not drawn.
    pub cull: CullMode,
}

/// The fragment stage of a pipeline: its entry point, and the format of the render targets it
/// draws into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FragmentState<'a> {
    /// The name of the fragment shader's entry point, in the pipeline's shader module.
    pub entry_point: &'a str,
    /// The format of the render targets the pipeline draws into, or `None` for a fragment stage
    /// that writes no color and only discards fragments, in a pass without a render target
    /// ([`Gpu::render_depth`]).
    pub target_format: Option<TextureFormat>,
}

/// A render pipeline, ready to be set in a [`RenderPass`](crate::RenderPass).
///
/// Clones refer to the same pipeline.
#[derive(Clone, Debug)]
pub struct RenderPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
}

impl Gpu {
    /// Builds a render pipeline.
    ///
    /// # Errors
    ///
    /// [`GfxError::Pipeline`] if wgpu rejects the pipeline, for example because an entry point
    /// does not exist in the shader, its inputs do not match the vertex layout (or the vertex
    /// shader reads `@location` inputs without one), the pipeline has neither a fragment stage nor
    /// a depth test, or the shaders declare a binding that the bind group layouts do not hold or
    /// hold as another kind (a `texture_depth_2d` or `sampler_comparison` against a layout entry
    /// that is not a [`BindingType::DepthTexture`](crate::BindingType::DepthTexture) or
    /// [`BindingType::ComparisonSampler`](crate::BindingType::ComparisonSampler), or the reverse);
    /// or, on Direct3D 12, if the shader compiler or the graphics driver rejects it. The message
    /// names the pipeline's label. [`GfxError::DeviceLost`] once the device is lost
    /// ([`Gpu::check`]), including when creating the pipeline loses it: on Vulkan, wgpu treats a
    /// driver that fails to create it (which a conformant one does only when it runs out of
    /// memory) as a lost device.
    pub fn create_render_pipeline(
        &self,
        descriptor: &RenderPipelineDescriptor<'_>,
    ) -> Result<RenderPipeline, GfxError> {
        self.check()?;
        let attributes: Vec<wgpu::VertexAttribute> = descriptor
            .vertex_layout
            .iter()
            .flat_map(|layout| layout.attributes)
            .map(|attribute| wgpu::VertexAttribute {
                format: attribute.format.to_wgpu(),
                offset: attribute.offset,
                shader_location: attribute.location,
            })
            .collect();
        // A pipeline without a vertex layout declares no vertex buffer at all: one declared without
        // attributes would still have to be bound at every draw.
        let vertex_buffers: Vec<Option<wgpu::VertexBufferLayout<'_>>> = descriptor
            .vertex_layout
            .iter()
            .map(|layout| {
                Some(wgpu::VertexBufferLayout {
                    array_stride: layout.stride,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })
            })
            .collect();
        let color_targets: Vec<Option<wgpu::ColorTargetState>> = descriptor
            .fragment
            .iter()
            .filter_map(|fragment| fragment.target_format)
            .map(|format| {
                Some(wgpu::ColorTargetState {
                    format: format.to_wgpu(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();
        let bind_group_layouts: Vec<Option<&wgpu::BindGroupLayout>> = descriptor
            .bind_group_layouts
            .iter()
            .map(|layout| Some(&layout.layout))
            .collect();
        let (pipeline, error) = self.catch_errors(|| {
            let layout = self
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(descriptor.label),
                    bind_group_layouts: &bind_group_layouts,
                    immediate_size: 0,
                });
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(descriptor.label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &descriptor.shader.module,
                        entry_point: Some(descriptor.vertex_entry_point),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &vertex_buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        cull_mode: descriptor.cull.to_wgpu(),
                        ..wgpu::PrimitiveState::default()
                    },
                    depth_stencil: descriptor.depth.map(|depth| wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: Some(depth.write),
                        depth_compare: Some(depth.compare.to_wgpu()),
                        stencil: wgpu::StencilState::default(),
                        // A clamp would need a downlevel flag, and nothing needs one.
                        bias: wgpu::DepthBiasState {
                            constant: depth.bias.constant,
                            slope_scale: depth.bias.slope_scale,
                            clamp: 0.0,
                        },
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: descriptor.fragment.map(|fragment| wgpu::FragmentState {
                        module: &descriptor.shader.module,
                        entry_point: Some(fragment.entry_point),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &color_targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        })?;
        match error {
            None => Ok(RenderPipeline { pipeline }),
            Some(error) => Err(GfxError::Pipeline {
                label: descriptor.label.to_owned(),
                message: error.to_string().trim_end().to_owned(),
            }),
        }
    }
}
