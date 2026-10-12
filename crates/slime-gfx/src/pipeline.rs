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

/// How a pipeline compares the depth of what it draws with the depth already in the depth target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DepthCompare {
    /// Draw where the new depth is less than the stored one: nearer, with the usual projections.
    Less,
    /// Draw where the new depth is less than or equal to the stored one.
    LessEqual,
    /// Draw whatever the stored depth.
    Always,
}

impl DepthCompare {
    fn to_wgpu(self) -> wgpu::CompareFunction {
        match self {
            Self::Less => wgpu::CompareFunction::Less,
            Self::LessEqual => wgpu::CompareFunction::LessEqual,
            Self::Always => wgpu::CompareFunction::Always,
        }
    }
}

/// The depth test of a pipeline: how it uses the depth target of the pass it draws in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthState {
    /// The comparison that decides whether a fragment is drawn.
    pub compare: DepthCompare,
    /// Whether the depth of what is drawn is written to the depth target.
    pub write: bool,
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
/// The pipeline draws lists of triangles, without blending, from one vertex buffer, into one
/// render target of `target_format`.
#[derive(Clone, Copy, Debug)]
pub struct RenderPipelineDescriptor<'a> {
    /// Names the pipeline in error messages and in GPU capture tools.
    pub label: &'a str,
    /// The module holding both entry points.
    pub shader: &'a ShaderModule,
    /// The name of the vertex shader's entry point.
    pub vertex_entry_point: &'a str,
    /// The name of the fragment shader's entry point.
    pub fragment_entry_point: &'a str,
    /// The layout of the vertex buffer bound at slot 0.
    pub vertex_layout: VertexLayout<'a>,
    /// The layouts of the bind groups the shaders read, by `@group` index. A binding that the
    /// shaders declare and that no layout holds is rejected.
    pub bind_group_layouts: &'a [&'a BindGroupLayout],
    /// The depth test, or `None` for none. A pipeline with a depth test draws only in passes that
    /// have a depth target, and one without only in passes without
    /// ([`RenderPassDescriptor::depth`](crate::RenderPassDescriptor::depth)).
    pub depth: Option<DepthState>,
    /// Which faces are not drawn.
    pub cull: CullMode,
    /// The format of the render targets the pipeline draws into.
    pub target_format: TextureFormat,
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
    /// does not exist in the shader, its inputs do not match the vertex layout, or the shaders
    /// declare a binding that the bind group layouts do not hold; or if the graphics driver
    /// rejects it. The message names the pipeline's label. [`GfxError::DeviceLost`] once the
    /// device is lost ([`Gpu::check`]).
    pub fn create_render_pipeline(
        &self,
        descriptor: &RenderPipelineDescriptor<'_>,
    ) -> Result<RenderPipeline, GfxError> {
        self.check()?;
        let attributes: Vec<wgpu::VertexAttribute> = descriptor
            .vertex_layout
            .attributes
            .iter()
            .map(|attribute| wgpu::VertexAttribute {
                format: attribute.format.to_wgpu(),
                offset: attribute.offset,
                shader_location: attribute.location,
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
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: descriptor.vertex_layout.stride,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &attributes,
                        })],
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
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &descriptor.shader.module,
                        entry_point: Some(descriptor.fragment_entry_point),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: descriptor.target_format.to_wgpu(),
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        });
        match error {
            None => Ok(RenderPipeline { pipeline }),
            Some(error) => Err(GfxError::Pipeline {
                label: descriptor.label.to_owned(),
                message: error.to_string().trim_end().to_owned(),
            }),
        }
    }
}
