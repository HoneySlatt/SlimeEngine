//! Render passes: drawing into a render target.

use std::ops::Range;
use std::sync::atomic::Ordering;

use crate::bind_group::BindGroup;
use crate::depth_target::DepthTarget;
use crate::gpu::Gpu;
use crate::index_buffer::IndexBuffer;
use crate::pipeline::RenderPipeline;
use crate::render_target::RenderTarget;
use crate::vertex_buffer::VertexBuffer;

/// A color with linear components, not sRGB-encoded, each from 0.0 to 1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f64,
    /// Green.
    pub g: f64,
    /// Blue.
    pub b: f64,
    /// Alpha (opacity).
    pub a: f64,
}

impl Color {
    fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

/// What a render pass draws into ([`Gpu::render`]).
#[derive(Clone, Copy, Debug)]
pub struct RenderPassDescriptor<'a> {
    /// The image to draw into.
    pub target: &'a RenderTarget,
    /// `Some(color)` clears the target to that color before drawing, and the depth target, if
    /// there is one, to the farthest depth; `None` keeps what both hold, so that the pass draws
    /// over an earlier one.
    pub clear: Option<Color>,
    /// The depth target that pipelines with a depth test read and write, or `None` for a pass
    /// without one. It must have the size of `target`. A pass with a depth target takes only
    /// pipelines with a depth test, and a pass without takes only pipelines without
    /// ([`RenderPipelineDescriptor::depth`](crate::RenderPipelineDescriptor::depth)).
    pub depth: Option<&'a DepthTarget>,
}

/// The drawing commands of one render pass, recorded inside [`Gpu::render`].
#[derive(Debug)]
pub struct RenderPass<'a> {
    pass: wgpu::RenderPass<'a>,
}

impl RenderPass<'_> {
    /// Uses `pipeline` for the next draws.
    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        self.pass.set_pipeline(&pipeline.pipeline);
    }

    /// Binds `group` at `index` for the next draws: the `@group` number of its layout in the
    /// pipeline's [`bind_group_layouts`](crate::RenderPipelineDescriptor::bind_group_layouts).
    /// `offsets` gives the start of the bound range of each binding that has a dynamic offset
    /// ([`BindingType::UniformBuffer`](crate::BindingType::UniformBuffer)), in the order of the
    /// bindings, each a multiple of [`Gpu::uniform_offset_alignment`]; it is empty for a group
    /// without any.
    pub fn set_bind_group(&mut self, index: u32, group: &BindGroup, offsets: &[u32]) {
        self.pass.set_bind_group(index, &group.group, offsets);
    }

    /// Reads the vertices of the next draws from `buffer`, at the pipeline's vertex buffer `slot`.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &VertexBuffer) {
        self.pass.set_vertex_buffer(slot, buffer.buffer.slice(..));
    }

    /// Reads the indices of the next indexed draws from `buffer`.
    pub fn set_index_buffer(&mut self, buffer: &IndexBuffer) {
        self.pass
            .set_index_buffer(buffer.buffer.slice(..), wgpu::IndexFormat::Uint32);
    }

    /// Draws the vertices of the range `vertices`, three by three as triangles.
    pub fn draw(&mut self, vertices: Range<u32>) {
        self.pass.draw(vertices, 0..1);
    }

    /// Draws the vertices that the indices of the range `indices` of the index buffer name, three
    /// by three as triangles.
    pub fn draw_indexed(&mut self, indices: Range<u32>) {
        self.pass.draw_indexed(indices, 0, 0..1);
    }

    /// Groups the commands that `record` gives under `label`, for GPU capture tools such as
    /// RenderDoc, which show the group as a named section of the pass. Groups can be nested. They
    /// reach the driver when wgpu's debug labels are on: in debug builds, or with `WGPU_DEBUG=1`.
    pub fn debug_group(&mut self, label: &str, record: impl FnOnce(&mut Self)) {
        self.pass.push_debug_group(label);
        record(self);
        self.pass.pop_debug_group();
    }
}

impl Gpu {
    /// Runs one render pass as `descriptor` says: clears or keeps the target, lets `record` draw
    /// into it, then submits the work to the GPU.
    ///
    /// The work runs on the GPU after this function returns; reading the target back waits for
    /// it. Submitting it is recorded as a `trace`-level profiling span named `submit`.
    ///
    /// # Panics
    ///
    /// If the depth target does not have the size of the render target, or if wgpu rejects a
    /// command that `record` gave, for example a pipeline built for another target format or
    /// without the depth test the pass needs: these are bugs in the caller.
    pub fn render(
        &self,
        descriptor: &RenderPassDescriptor<'_>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
        let target = descriptor.target;
        if let Some(depth) = descriptor.depth {
            assert!(
                (depth.width(), depth.height()) == (target.width(), target.height()),
                "depth target `{}` is {}x{} but render target `{}` is {}x{}",
                depth.label,
                depth.width(),
                depth.height(),
                target.label,
                target.width(),
                target.height()
            );
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&target.label),
            });
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&target.label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match descriptor.clear {
                            Some(color) => wgpu::LoadOp::Clear(color.to_wgpu()),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: descriptor.depth.map(|depth| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: &depth.view,
                        depth_ops: Some(wgpu::Operations {
                            load: if descriptor.clear.is_some() {
                                wgpu::LoadOp::Clear(1.0)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            record(&mut RenderPass { pass });
        }
        let _span = tracing::trace_span!("submit").entered();
        self.queue.submit([encoder.finish()]);
        target.drawn.store(true, Ordering::Relaxed);
    }
}
