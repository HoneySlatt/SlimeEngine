//! Render passes: drawing into a render target.

use std::marker::PhantomData;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bind_group::BindGroup;
use crate::depth_target::DepthTarget;
use crate::gpu::Gpu;
use crate::index_buffer::IndexBuffer;
use crate::pipeline::RenderPipeline;
use crate::render_target::RenderTarget;
use crate::timer::{GpuTimer, TimedPass};
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

/// What a render pass draws into ([`Gpu::render`], [`Commands::render`]). A pass without a render
/// target is described by a [`DepthPassDescriptor`].
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

/// What a render pass without a render target draws into ([`Gpu::render_depth`],
/// [`Commands::render_depth`]): a depth target only, for pipelines that write depth only.
#[derive(Clone, Copy, Debug)]
pub struct DepthPassDescriptor<'a> {
    /// The depth target to draw into, cleared to the farthest depth (1.0) when the pass starts. It
    /// can have any size.
    pub depth: &'a DepthTarget,
    /// A timer that measures how long the pass takes on the GPU, read later through
    /// [`GpuTimer::latest`]; `None` leaves the pass untimed.
    pub timer: Option<&'a GpuTimer>,
}

/// The drawing commands of one render pass, recorded inside [`Gpu::render`], [`Gpu::render_depth`],
/// [`Commands::render`] or [`Commands::render_depth`]. The lifetime is the closure's: a pass cannot
/// be kept beyond it.
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
    /// RenderDoc, which show the group as a named section of the pass. Groups can be nested. On
    /// Vulkan, they reach the driver only when wgpu's debug labels are on: in debug builds, or with
    /// `WGPU_DEBUG=1`. On Direct3D 12, they do in every build, unless `WGPU_DISCARD_HAL_LABELS=1`
    /// is set.
    pub fn debug_group(&mut self, label: &str, record: impl FnOnce(&mut Self)) {
        self.pass.push_debug_group(label);
        record(self);
        self.pass.pop_debug_group();
    }
}

/// Render passes recorded one after another into one command encoder, which [`Gpu::submit`]
/// submits to the GPU at once: the passes of a frame, for example. The GPU runs them in the order
/// they were recorded, each seeing what the passes before it drew. `submit` lends them to its
/// closure only, so that every pass recorded is submitted.
pub struct Commands {
    encoder: wgpu::CommandEncoder,
    /// The timed passes, whose timestamps are read back once submitted.
    timed: Vec<TimedPass>,
    /// The flags of the render targets drawn into, set once submitted.
    drawn: Vec<Arc<AtomicBool>>,
}

impl std::fmt::Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commands")
            .field("timed", &self.timed.len())
            .field("drawn", &self.drawn.len())
            .finish_non_exhaustive()
    }
}

impl Gpu {
    /// Records the render passes that `record` gives, one after another, into one command encoder,
    /// then submits them to the GPU at once: one submission, however many passes. `label` names
    /// the commands in GPU capture tools; each pass is named after its target, as with
    /// [`Gpu::render`].
    ///
    /// The work runs on the GPU after this function returns, in the order the passes were
    /// recorded; reading a target back waits for it. Submitting it is recorded as a `trace`-level
    /// profiling span named `submit`; wgpu's validation and encoding of the recorded commands, just
    /// before, are not part of it. Timed passes get their times as with [`Gpu::render`]; a timer
    /// given to several passes of one submission times each of them, one slot each, and reports
    /// whichever came back last.
    ///
    /// # Panics
    ///
    /// As [`Commands::render`] and [`Commands::render_depth`] say: a pass whose depth target does
    /// not fit when it is recorded, a command wgpu rejects when the commands are finished, inside
    /// this method.
    pub fn submit(&self, label: &str, record: impl FnOnce(&mut Commands)) {
        let mut commands = Commands {
            encoder: self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) }),
            timed: Vec::new(),
            drawn: Vec::new(),
        };
        record(&mut commands);
        let Commands {
            encoder,
            timed,
            drawn,
        } = commands;
        // Finishing the encoder is where wgpu validates and encodes every command recorded, work
        // that grows with the draws: it stays out of the span, which times the submission.
        let commands = encoder.finish();
        {
            let _span = tracing::trace_span!("submit").entered();
            self.queue.submit([commands]);
        }
        for flag in drawn {
            flag.store(true, Ordering::Relaxed);
        }
        // A readback can be mapped only once the commands that write it are submitted.
        for pass in timed {
            pass.map();
        }
    }

    /// Runs one render pass as `descriptor` says: clears or keeps the target, lets `record` draw
    /// into it, then submits the work to the GPU. It is [`Gpu::submit`] with one pass, named
    /// after the target.
    ///
    /// The work runs on the GPU after this function returns; reading the target back waits for
    /// it. Submitting it is recorded as a `trace`-level profiling span named `submit`; wgpu's
    /// validation and encoding of the recorded commands, just before, are not part of it.
    ///
    /// # Panics
    ///
    /// As [`Commands::render`] says.
    pub fn render(
        &self,
        descriptor: &RenderPassDescriptor<'_>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
        self.submit(&descriptor.target.label, |commands| {
            commands.render(descriptor, record);
        });
    }

    /// Runs one render pass without a render target, as [`Commands::render_depth`] says, then
    /// submits it: [`Gpu::submit`] with one pass, named after the depth target.
    ///
    /// # Panics
    ///
    /// As [`Commands::render_depth`] says.
    pub fn render_depth(
        &self,
        descriptor: &DepthPassDescriptor<'_>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
        self.submit(&descriptor.depth.label, |commands| {
            commands.render_depth(descriptor, record);
        });
    }
}

impl Commands {
    /// Records one render pass as `descriptor` says: clears or keeps the target, and lets
    /// `record` draw into it. The pass and its commands are named after the target in capture
    /// tools.
    ///
    /// # Panics
    ///
    /// If the depth target does not have the size of the render target. When the commands are
    /// submitted, if wgpu rejects a command that `record` gave, which it reports when the pass is
    /// finished: a pipeline built for another target format, or whose depth test does not match
    /// the pass (a depth test in a pass without a depth target, or none in a pass with one); a
    /// pipeline without a fragment stage (it draws only in [`Commands::render_depth`]); a draw
    /// with a pipeline that has a vertex layout and no vertex buffer set; a bind group that holds
    /// the pass's own target or depth target, since a pass cannot sample what it draws into.
    /// These are bugs in the caller.
    pub fn render(
        &mut self,
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
        self.drawn.push(Arc::clone(&target.drawn));
    }

    /// Records one render pass without a render target: clears `descriptor.depth` to the
    /// farthest depth, lets `record` draw with pipelines that have a depth test and no fragment
    /// stage ([`RenderPipelineDescriptor::fragment`](crate::RenderPipelineDescriptor::fragment) is
    /// `None`), such as a shadow map's, timed by `descriptor.timer` when there is one. Later
    /// passes can then sample the depth target
    /// ([`BindingResource::DepthTarget`](crate::BindingResource::DepthTarget)). The pass is named
    /// after the depth target in capture tools.
    ///
    /// # Panics
    ///
    /// When the commands are submitted, if wgpu rejects a command that `record` gave, which it
    /// reports when the pass is finished: a pipeline with a fragment stage that has a target format
    /// (it draws only in [`Commands::render`]), a pipeline without a depth test, a draw with a
    /// pipeline that has a vertex layout and no vertex buffer set, or a bind group that holds
    /// `descriptor.depth`. These are bugs in the caller.
    pub fn render_depth(
        &mut self,
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

    /// Records one render pass into the encoder, with `color` and `depth` as its attachments,
    /// timed by `timer` when it has a free slot.
    fn run_pass(
        &mut self,
        label: &str,
        color: Option<wgpu::RenderPassColorAttachment<'_>>,
        depth: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
        timer: Option<&GpuTimer>,
        record: impl FnOnce(&mut RenderPass<'_>),
    ) {
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
            let pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments,
                depth_stencil_attachment: depth,
                timestamp_writes: timing.map(|(timer, slot)| timer.writes(slot)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // The pass ends when this block does, before the encoder records anything else, which
            // is what wgpu asks of a pass freed from the encoder's lifetime.
            record(&mut RenderPass {
                pass: pass.forget_lifetime(),
                closure: PhantomData,
            });
        }
        if let Some((timer, slot)) = timing {
            timer.resolve(&mut self.encoder, slot);
            self.timed.push(timer.timed_pass(slot));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextureFormat;

    #[test]
    fn a_target_counts_as_drawn_once_the_pass_that_draws_it_is_submitted() {
        // What `SurfaceFrame::present` relies on to give wgpu a submission when nothing drew.
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let drawn = gpu.create_render_target("drawn", 4, 4, TextureFormat::Rgba8UnormSrgb);
        let untouched = gpu.create_render_target("untouched", 4, 4, TextureFormat::Rgba8UnormSrgb);
        let is_drawn = |target: &RenderTarget| target.drawn.load(Ordering::Relaxed);
        gpu.submit("drawn", |commands| {
            let pass = RenderPassDescriptor {
                target: &drawn,
                clear: None,
                depth: None,
                timer: None,
            };
            commands.render(&pass, |_pass| {});
            assert!(!is_drawn(&drawn), "recorded, not submitted yet");
        });
        assert!(is_drawn(&drawn), "submitted");
        assert!(!is_drawn(&untouched), "no pass drew it");
    }
}
