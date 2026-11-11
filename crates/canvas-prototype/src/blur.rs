//! A blur of what lies behind a glass, and the copy of the canvas's texture onto the target
//! (`blur.wgsl`).
//!
//! The canvas draws into a texture of its own, since a window's image cannot be read on GL. For
//! each glass, the region behind it, grown by the blur's reach, is halved `levels` times, each
//! pixel the average of four, then blurred by a Gaussian across and then down at that size. The
//! glass reads the result through its sampler, which brings it back to full size.
//!
//! The radius follows the canvas's shadows (`Canvas::draw_shadow`, CSS's `box-shadow`): twice the
//! Gaussian's deviation. The halvings are as many as keep the deviation left at the smallest size
//! between 2 and 4 texels: two at 1280 × 800 and 1920 × 1080, three at 3840 × 2160, so that a
//! blur pass reads at most 25 taps across 3 deviations as 13 samples whatever the screen (9 at
//! 1280 × 800, 11 at the others). Dual Kawase was not taken: it reads
//! its texels at every size twice, down then up, the last pass at half the screen's size, and its
//! reach is not a Gaussian's, so that a radius in pixels does not set it.

use crate::canvas::PixelRect;
use crate::gpu::{Gpu, TARGET_FORMAT};

pub const SOURCE: &str = include_str!("blur.wgsl");

/// The bytes of `Pass` in `blur.wgsl`.
const PASS_BYTES: u64 = 160;
/// The most taps a blur reads, the size of `Pass::taps`.
const MOST_TAPS: usize = 8;

pub struct Blur {
    layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    gaussian: wgpu::RenderPipeline,
    copy: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Down,
    Gaussian,
}

/// One pass of a blur: its target and what it reads.
pub struct BlurPass {
    pub name: String,
    step: Step,
    target: wgpu::TextureView,
    group: wgpu::BindGroup,
}

/// What lies behind one glass: its passes, and the result that the glass reads.
pub struct Backdrop {
    /// The region of the canvas's texture blurred, in its pixels.
    pub region: PixelRect,
    pub passes: Vec<BlurPass>,
    pub result: wgpu::TextureView,
    /// The taps each blur pass reads, each but the first on both sides.
    pub taps: usize,
}

/// A texture that a pass draws into and a later one reads.
pub fn render_texture(gpu: &Gpu, label: &str, (width, height): (u32, u32)) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// The Gaussian's deviation for a blur of `radius` pixels: half of it, as a shadow's.
pub fn deviation(radius: f32) -> f32 {
    radius / 2.0
}

/// The halvings for a deviation `sigma`: at least one, and as many as leave at least 2 texels.
pub fn levels(sigma: f32) -> u32 {
    ((sigma / 2.0).log2().floor() as u32).max(1)
}

/// The taps of a Gaussian of deviation `sigma` texels across 3 deviations, each but the first
/// read between two texels: offset and weight.
fn taps(sigma: f32) -> Vec<[f32; 2]> {
    let reach = (3.0 * sigma).ceil() as usize;
    let weight = |i: usize| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp();
    let total: f32 = weight(0) + 2.0 * (1..=reach).map(weight).sum::<f32>();
    let mut taps = vec![[0.0, weight(0) / total]];
    for i in (1..=reach).step_by(2) {
        let (near, far) = (weight(i), if i < reach { weight(i + 1) } else { 0.0 });
        let both = near + far;
        taps.push([
            (i as f32 * near + (i + 1) as f32 * far) / both,
            both / total,
        ]);
    }
    taps
}

impl Blur {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blur"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    crate::gpu::uniform_entry(2, wgpu::ShaderStages::FRAGMENT, PASS_BYTES),
                ],
            });
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("blur"),
                source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("blur"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = |fragment: &str| {
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(fragment),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_cover"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(fragment),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: TARGET_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        };
        Self {
            down: pipeline("fs_down"),
            gaussian: pipeline("fs_blur"),
            copy: pipeline("fs_copy"),
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("blur"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            layout,
        }
    }

    fn group(&self, gpu: &Gpu, source: &wgpu::TextureView, params: &[u8]) -> wgpu::BindGroup {
        let uniform = gpu.buffer("blur pass", PASS_BYTES, wgpu::BufferUsages::UNIFORM);
        gpu.queue.write_buffer(&uniform, 0, params);
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur pass"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        })
    }

    /// The passes that blur, by `radius` pixels, the region behind `glass` of `canvas`, a
    /// texture of `size`; the region grows by the blur's reach, kept within the texture.
    pub fn backdrop(
        &self,
        gpu: &Gpu,
        canvas: &wgpu::TextureView,
        size: (u32, u32),
        glass: PixelRect,
        radius: f32,
        name: &str,
    ) -> Backdrop {
        let sigma = deviation(radius);
        let levels = levels(sigma);
        let unit = (1u32 << levels) as f32;
        let reach = 3.0 * sigma;
        let snap_down = |value: f32| ((value / unit).floor() * unit).max(0.0);
        let snap_up = |value: f32, most: u32| ((value / unit).ceil() * unit).min(most as f32);
        let left = snap_down(glass.x - reach);
        let top = snap_down(glass.y - reach);
        let right = snap_up(glass.x + glass.width + reach, size.0);
        let bottom = snap_up(glass.y + glass.height + reach, size.1);
        let region = PixelRect::new(left, top, right - left, bottom - top);
        let level_size = |level: u32| {
            (
                (region.width as u32 >> level).max(1),
                (region.height as u32 >> level).max(1),
            )
        };
        // The deviation left once the halvings, boxes of 2, 4, ... pixels, have blurred.
        let boxes = ((1u32 << (2 * levels)) - 1) as f32 / 12.0;
        let left_sigma = (sigma * sigma - boxes).max(0.25).sqrt() / unit;
        let taps = taps(left_sigma);
        assert!(taps.len() <= MOST_TAPS, "{} taps", taps.len());

        let mut passes = Vec::new();
        let mut source = (canvas.clone(), size, [left, top]);
        for level in 1..=levels {
            let target = render_texture(gpu, "blur level", level_size(level));
            let params = pass_bytes(source.2, 2.0, source.1, [0.0, 0.0], &[]);
            passes.push(BlurPass {
                name: format!("{name} down {level}"),
                step: Step::Down,
                group: self.group(gpu, &source.0, &params),
                target: target.clone(),
            });
            source = (target, level_size(level), [0.0, 0.0]);
        }
        let smallest = source.0.clone();
        let across = render_texture(gpu, "blur across", level_size(levels));
        for (axis, from, to, direction) in [
            ("across", &smallest, &across, [1.0, 0.0]),
            ("down the column", &across, &smallest, [0.0, 1.0]),
        ] {
            let params = pass_bytes([0.0, 0.0], 1.0, level_size(levels), direction, &taps);
            passes.push(BlurPass {
                name: format!("{name} blur {axis}"),
                step: Step::Gaussian,
                group: self.group(gpu, from, &params),
                target: to.clone(),
            });
        }
        Backdrop {
            region,
            passes,
            result: smallest,
            taps: taps.len(),
        }
    }

    /// The view a pass draws into.
    pub fn target<'a>(&self, pass: &'a BlurPass) -> &'a wgpu::TextureView {
        &pass.target
    }

    /// Records one pass of a backdrop.
    pub fn record(&self, render: &mut wgpu::RenderPass<'_>, pass: &BlurPass) -> (usize, usize) {
        render.set_pipeline(match pass.step {
            Step::Down => &self.down,
            Step::Gaussian => &self.gaussian,
        });
        render.set_bind_group(0, &pass.group, &[]);
        render.draw(0..3, 0..1);
        (1, 1)
    }

    /// The bind group of the copy of `canvas` onto the target.
    pub fn copy_group(&self, gpu: &Gpu, canvas: &wgpu::TextureView) -> wgpu::BindGroup {
        self.group(gpu, canvas, &[0; PASS_BYTES as usize])
    }

    /// Records the copy of the canvas's texture onto the target.
    pub fn record_copy(
        &self,
        render: &mut wgpu::RenderPass<'_>,
        group: &wgpu::BindGroup,
    ) -> (usize, usize) {
        render.set_pipeline(&self.copy);
        render.set_bind_group(0, group, &[]);
        render.draw(0..3, 0..1);
        (1, 1)
    }

    /// The sampler through which a glass reads its backdrop.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

/// `Pass` of `blur.wgsl`.
fn pass_bytes(
    origin: [f32; 2],
    step: f32,
    size: (u32, u32),
    direction: [f32; 2],
    taps: &[[f32; 2]],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PASS_BYTES as usize);
    crate::canvas::push_f32s(&mut bytes, &[origin[0], origin[1], step]);
    crate::canvas::push_u32s(&mut bytes, &[taps.len() as u32]);
    crate::canvas::push_f32s(
        &mut bytes,
        &[
            1.0 / size.0 as f32,
            1.0 / size.1 as f32,
            direction[0],
            direction[1],
        ],
    );
    for index in 0..MOST_TAPS {
        let [offset, weight] = taps.get(index).copied().unwrap_or([0.0, 0.0]);
        crate::canvas::push_f32s(&mut bytes, &[offset, weight, 0.0, 0.0]);
    }
    bytes
}
