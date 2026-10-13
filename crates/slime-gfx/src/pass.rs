//! Render passes: drawing into a render target.

use std::marker::PhantomData;
use std::ops::Range;
use std::sync::atomic::Ordering;

use crate::bind_group::BindGroup;
use crate::depth_target::DepthTarget;
use crate::gpu::Gpu;
use crate::index_buffer::IndexBuffer;
use crate::pipeline::RenderPipeline;
use crate::render_target::RenderTarget;
use crate::timer::GpuTimer;
use crate::vertex_buffer::VertexBuffer;

/// A color with linear components, not sRGB-encoded. 8-bit targets keep each component between 0
/// and 1. `Rgba16Float` targets keep a half-precision value next to each component, beyond 0 to 1
/// included: the driver rounds to the nearest value or toward zero, and a magnitude beyond 65 504
/// becomes ±65 504 or an infinity.
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

/// What a render pass draws into ([`Gpu::render`]). A pass without a render target is described
/// by a [`DepthPassDescriptor`].
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
    /// A timer that measures how long the pass takes on the GPU, read later through
    /// [`GpuTimer::latest`]; `None` leaves the pass untimed.
    pub timer: Option<&'a GpuTimer>,
}

/// What a render pass without a render target draws into ([`Gpu::render_depth`]): a depth target
/// only, for pipelines that write depth only.
#[derive(Clone, Copy, Debug)]
pub struct DepthPassDescriptor<'a> {
    /// The depth target to draw into, cleared to the farthest depth (1.0) when the pass starts. It
    /// can have any size.
    pub depth: &'a DepthTarget,
    /// A timer that measures how long the pass takes on the GPU, read later through
    /// [`GpuTimer::latest`]; `None` leaves the pass untimed.
    pub timer: Option<&'a GpuTimer>,
}

/// The drawing commands of one render pass, recorded inside [`Gpu::render`] or
/// [`Gpu::render_depth`]. The lifetime is the closure's: a pass cannot be kept beyond it.
#[derive(Debug)]
pub struct RenderPass<'a> {
    pass: wgpu::RenderPass<'static>,
    closure: PhantomData<&'a mut ()>,
}

impl RenderPass<'_> {
    /// The wgpu pass behind this one, for the engine's tooling only (ADR-0031): it draws with
    /// libraries built on wgpu, such as egui's renderer. Games must not use it.
    #[cfg(feature = "wgpu")]
    pub fn wgpu_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        &mut self.pass
    }

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

    /// Draws the vertices of the range `vertices`, three by three as triangles. A pipeline
    /// without a vertex layout reads no vertex buffer: its vertex shader gets each index of the
    /// range as `@builtin(vertex_index)`.
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
    /// command that `record` gave, which it reports when the pass is finished inside this method:
    /// a pipeline built for another target format, or whose depth test does not match the pass
    /// (a depth test in a pass without a depth target, or none in a pass with one); a
    /// pipeline without a fragment stage (it draws only with [`Gpu::render_depth`]); a draw with
    /// a pipeline that has a vertex layout and no vertex buffer set; a bind group that holds the
    /// pass's own target or depth target, since a pass cannot sample what it draws into. These
    /// are bugs in the caller.
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
        let color = wgpu::RenderPassColorAttachment {
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
        };
        let depth = descriptor
            .depth
            .map(|depth| wgpu::RenderPassDepthStencilAttachment {
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
            });
        self.run_pass(&target.label, Some(color), depth, descriptor.timer, record);
        target.drawn.store(true, Ordering::Relaxed);
    }

    /// Runs one render pass without a render target: clears `descriptor.depth` to the farthest
    /// depth, lets `record` draw with pipelines that have a depth test and no fragment stage
    /// ([`RenderPipelineDescriptor::fragment`](crate::RenderPipelineDescriptor::fragment) is
    /// `None`), such as a shadow map's, then submits the work, timed by `descriptor.timer` when
    /// there is one. Later passes can then sample the depth target
    /// ([`BindingResource::DepthTarget`](crate::BindingResource::DepthTarget)).
    ///
    /// The pass and its commands are named after the depth target's label in capture tools. The
    /// work runs on the GPU after this function returns; submitting it is recorded as a
    /// `trace`-level profiling span named `submit`.
    ///
    /// # Panics
    ///
    /// If wgpu rejects a command that `record` gave, which it reports when the pass is finished
    /// inside this method: a pipeline with a fragment stage (it draws only with
    /// [`Gpu::render`]), a pipeline without a depth test, a draw with a pipeline that has a
    /// vertex layout and no vertex buffer set, or a bind group that holds `descriptor.depth`.
    /// These are bugs in the caller.
    pub fn render_depth(
        &self,
        descriptor: &DepthPassDescriptor<'_>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
        let depth = wgpu::RenderPassDepthStencilAttachment {
            view: &descriptor.depth.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };
        self.run_pass(
            &descriptor.depth.label,
            None,
            Some(depth),
            descriptor.timer,
            record,
        );
    }

    /// Records one render pass into its own command encoder, with `color` and `depth` as its
    /// attachments, timed by `timer` when it has a free slot, and submits it.
    fn run_pass(
        &self,
        label: &str,
        color: Option<wgpu::RenderPassColorAttachment<'_>>,
        depth: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
        timer: Option<&GpuTimer>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        // A timer whose slots are all in flight leaves this pass untimed rather than waiting.
        let timing = timer.and_then(|timer| timer.reserve().map(|slot| (timer, slot)));
        // A pass without a render target has no color attachment at all, not one empty slot:
        // wgpu compares this list with the pipelines' targets exactly, and a depth-only pipeline
        // has none.
        let color_attachments = [color];
        let color_attachments: &[Option<wgpu::RenderPassColorAttachment<'_>>] =
            if color_attachments[0].is_some() {
                &color_attachments
            } else {
                &[]
            };
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments,
                depth_stencil_attachment: depth,
                timestamp_writes: timing.map(|(timer, slot)| timer.writes(slot)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // The pass ends when this block does, before the encoder is finished, which is what
            // wgpu asks of a pass freed from the encoder's lifetime.
            record(&mut RenderPass {
                pass: pass.forget_lifetime(),
                closure: PhantomData,
            });
        }
        if let Some((timer, slot)) = timing {
            timer.resolve(&mut encoder, slot);
        }
        let _span = tracing::trace_span!("submit").entered();
        self.queue.submit([encoder.finish()]);
        if let Some((timer, slot)) = timing {
            timer.map(slot);
        }
    }
}
