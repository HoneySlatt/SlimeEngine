//! Render passes: drawing into a render target.

use std::ops::Range;

use crate::gpu::Gpu;
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

    /// Reads the vertices of the next draws from `buffer`, at the pipeline's vertex buffer `slot`.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &VertexBuffer) {
        self.pass.set_vertex_buffer(slot, buffer.buffer.slice(..));
    }

    /// Draws the vertices of the range `vertices`, three by three as triangles.
    pub fn draw(&mut self, vertices: Range<u32>) {
        self.pass.draw(vertices, 0..1);
    }
}

impl Gpu {
    /// Clears `target` to `clear`, lets `record` draw into it, then submits the work to the GPU.
    ///
    /// The work runs on the GPU after this function returns; reading the target back waits for
    /// it.
    ///
    /// # Panics
    ///
    /// If wgpu rejects a command that `record` gave, for example a pipeline built for another
    /// target format: this is a bug in the caller.
    pub fn render(
        &self,
        target: &RenderTarget,
        clear: Color,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
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
                        load: wgpu::LoadOp::Clear(clear.to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            record(&mut RenderPass { pass });
        }
        self.queue.submit([encoder.finish()]);
    }
}
