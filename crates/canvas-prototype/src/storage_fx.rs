//! S with the effects (`storage_fx.wgsl`): S's items in their storage buffer, unchanged; the
//! transforms they name in a second storage buffer, 32 bytes each, written each frame; the paths'
//! segments in a third, 16 bytes each. Vulkan only, as S.

use crate::canvas::{Affine, Batch, Canvas, ITEM_SIZE, item_bytes, push_f32s};
use crate::gpu::{self, FxLayout, Gpu, Layout, Uploads};
use crate::storage::MAX_ITEMS;

pub const SOURCE: &str = concat!(
    include_str!("fx_common.wgsl"),
    include_str!("storage_fx.wgsl")
);

/// A transform as `Transform` in the shaders: `[a, b, c, 0]`, `[d, e, f, 0]`.
pub fn transform_bytes(transforms: &[Affine]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(transforms.len() * 32);
    for &[a, b, c, d, e, f] in transforms {
        push_f32s(&mut bytes, &[a, b, c, 0.0, d, e, f, 0.0]);
    }
    bytes
}

/// The paths' segments as the shaders read them: `[x0, y0, x1, y1]`, 16 bytes each.
pub fn segment_bytes(segments: &[[f32; 4]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(segments.len() * 16);
    for segment in segments {
        push_f32s(&mut bytes, segment);
    }
    bytes
}

pub struct StorageFx {
    frame_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    items: Option<(wgpu::Buffer, u64)>,
    transforms: Option<(wgpu::Buffer, u64)>,
    segments: Option<(wgpu::Buffer, u64)>,
    frame_group: Option<wgpu::BindGroup>,
    image_groups: Vec<wgpu::BindGroup>,
    batches: Vec<Batch>,
}

fn storage_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl StorageFx {
    pub fn new(gpu: &Gpu) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("S fx frame"),
                        entries: &[
                            gpu::uniform_entry(
                                0,
                                wgpu::ShaderStages::VERTEX_FRAGMENT,
                                gpu::FX_FRAME_BYTES,
                            ),
                            storage_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
                            storage_entry(2, wgpu::ShaderStages::VERTEX),
                            storage_entry(3, wgpu::ShaderStages::FRAGMENT),
                        ],
                    });
            let image_layout = gpu::image_layout(gpu);
            let module = gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("S fx"),
                    source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
                });
            let layout = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("S fx"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipeline = gpu::pipeline(gpu, "S fx", &module, &layout, "vs_main", "fs_main", &[]);
            Self {
                frame: gpu.buffer(
                    "S fx frame",
                    gpu::FX_FRAME_BYTES,
                    wgpu::BufferUsages::UNIFORM,
                ),
                frame_layout,
                image_layout,
                pipeline,
                items: None,
                transforms: None,
                segments: None,
                frame_group: None,
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

impl Layout for StorageFx {
    fn upload(
        &mut self,
        gpu: &Gpu,
        uploads: &mut Uploads,
        canvas: &Canvas,
        size: (u32, u32),
    ) -> u64 {
        let count = canvas.items.len().min(MAX_ITEMS);
        let items = item_bytes(&canvas.items[..count], true);
        let transforms = transform_bytes(&canvas.transform_table());
        let segments = segment_bytes(&canvas.segments);
        let mut made = ensure(
            gpu,
            &mut self.items,
            "S fx items",
            items.len(),
            ITEM_SIZE * 64,
        );
        made |= ensure(
            gpu,
            &mut self.transforms,
            "S fx transforms",
            transforms.len(),
            256,
        );
        made |= ensure(
            gpu,
            &mut self.segments,
            "S fx segments",
            segments.len(),
            256,
        );
        if made || self.frame_group.is_none() {
            fn entry(binding: u32, slot: &Option<(wgpu::Buffer, u64)>) -> wgpu::BindGroupEntry<'_> {
                wgpu::BindGroupEntry {
                    binding,
                    resource: slot.as_ref().expect("made above").0.as_entire_binding(),
                }
            }
            self.frame_group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("S fx frame"),
                layout: &self.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.frame.as_entire_binding(),
                    },
                    entry(1, &self.items),
                    entry(2, &self.transforms),
                    entry(3, &self.segments),
                ],
            }));
        }
        let frame = gpu::fx_frame_bytes(size, 0, 0);
        let bytes = (items.len() + transforms.len() + segments.len() + frame.len()) as u64;
        for (slot, data) in [
            (&self.items, items),
            (&self.transforms, transforms),
            (&self.segments, segments),
        ] {
            uploads.write_buffer(&slot.as_ref().expect("made above").0, 0, data);
        }
        uploads.write_buffer(&self.frame, 0, frame.to_vec());
        self.batches = canvas.batches(count);
        bytes
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize) {
        let Some(frame_group) = &self.frame_group else {
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

impl FxLayout for StorageFx {
    fn set_images(&mut self, gpu: &Gpu, views: &[&wgpu::TextureView], sampler: &wgpu::Sampler) {
        self.image_groups = gpu::image_groups_of(gpu, &self.image_layout, views, sampler);
    }
}

/// A storage buffer of at least `needed` bytes, grown by powers of two; true if made anew.
fn ensure(
    gpu: &Gpu,
    slot: &mut Option<(wgpu::Buffer, u64)>,
    label: &str,
    needed: usize,
    least: usize,
) -> bool {
    gpu::ensure_buffer(
        gpu,
        slot,
        label,
        needed as u64,
        least as u64,
        wgpu::BufferUsages::STORAGE,
    )
}
