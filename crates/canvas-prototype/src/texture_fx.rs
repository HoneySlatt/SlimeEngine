//! T with the effects (`texture_fx.wgsl`): one `Rgba32Uint` texture 2048 texels wide holds the
//! frame's variable data, written each frame: S's items, nine texels each; the transforms they
//! name, two texels each; the paths' segments, one texel each. The frame's uniform says where the
//! transforms and the segments begin. One pipeline, one draw per run of one image, as S.

use crate::canvas::{Batch, Canvas, ITEM_SIZE, item_bytes};
use crate::gpu::{self, DATA_TEXEL, FxLayout, Gpu, Layout};
use crate::storage_fx::{segment_bytes, transform_bytes};

pub const SOURCE: &str = concat!(
    include_str!("fx_common.wgsl"),
    include_str!("texture_fx.wgsl")
);

/// The texels of an item.
pub const ITEM_TEXELS: usize = ITEM_SIZE / DATA_TEXEL;
/// The texels of a transform.
pub const TRANSFORM_TEXELS: usize = 2;
/// The largest texture's texels.
pub const MAX_TEXELS: usize = gpu::DATA_WIDTH as usize * 2048;

pub struct TextureFx {
    frame_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    /// The data's texture, its rows, and the frame's bind group that holds it.
    data: Option<(wgpu::Texture, u32, wgpu::BindGroup)>,
    image_groups: Vec<wgpu::BindGroup>,
    batches: Vec<Batch>,
}

impl TextureFx {
    pub fn new(gpu: &Gpu) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("T fx frame"),
                        entries: &[
                            gpu::uniform_entry(
                                0,
                                wgpu::ShaderStages::VERTEX_FRAGMENT,
                                gpu::FX_FRAME_BYTES,
                            ),
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Uint,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                        ],
                    });
            let image_layout = gpu::image_layout(gpu);
            let module = gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("T fx"),
                    source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
                });
            let layout = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("T fx"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipeline = gpu::pipeline(gpu, "T fx", &module, &layout, "vs_main", "fs_main", &[]);
            Self {
                frame: gpu.buffer(
                    "T fx frame",
                    gpu::FX_FRAME_BYTES,
                    wgpu::BufferUsages::UNIFORM,
                ),
                frame_layout,
                image_layout,
                pipeline,
                data: None,
                image_groups: Vec::new(),
                batches: Vec::new(),
            }
        });
        match error {
            None => Ok(made),
            Some(error) => Err(error),
        }
    }
}

impl Layout for TextureFx {
    fn upload(&mut self, gpu: &Gpu, canvas: &Canvas, size: (u32, u32)) -> u64 {
        let mut data = item_bytes(&canvas.items, true);
        let transforms_at = data.len() / DATA_TEXEL;
        data.extend(transform_bytes(&canvas.transform_table()));
        let segments_at = data.len() / DATA_TEXEL;
        data.extend(segment_bytes(&canvas.segments));
        let texels = data.len() / DATA_TEXEL;
        assert!(
            texels <= MAX_TEXELS,
            "T fx: {texels} texels, more than a texture of 2048 × 2048 holds"
        );
        let rows = gpu::data_rows(texels);
        if self.data.as_ref().is_none_or(|(_, held, _)| *held < rows) {
            let texture =
                gpu::data_texture(gpu, "T fx data", wgpu::TextureFormat::Rgba32Uint, rows);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("T fx frame"),
                layout: &self.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.frame.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            });
            self.data = Some((texture, rows, group));
        }
        let (texture, _, _) = self.data.as_ref().expect("made above");
        gpu::write_texels(gpu, texture, 0, &data);
        let frame = gpu::fx_frame_bytes(size, transforms_at as u32, segments_at as u32);
        gpu.queue.write_buffer(&self.frame, 0, &frame);
        self.batches = canvas.batches(canvas.items.len());
        (data.len() + frame.len()) as u64
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize) {
        let Some((_, _, frame_group)) = &self.data else {
            return (0, 0);
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, frame_group, &[]);
        for (range, image) in &self.batches {
            pass.set_bind_group(1, &self.image_groups[gpu::view_index(*image)], &[]);
            pass.draw(range.start as u32 * 6..range.end as u32 * 6, 0..1);
        }
        (self.batches.len(), 1)
    }
}

impl FxLayout for TextureFx {
    fn set_images(&mut self, gpu: &Gpu, views: &[&wgpu::TextureView], sampler: &wgpu::Sampler) {
        self.image_groups = gpu::image_groups_of(gpu, &self.image_layout, views, sampler);
    }
}
