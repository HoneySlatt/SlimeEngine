//! S, the control: today's canvas. Each item is nine `vec4`s in a storage buffer, which the vertex
//! shader reads to place its quad and the fragment shader reads again by the item's index;
//! `storage.wgsl` is `slime-render`'s `canvas.wgsl` unchanged, and so are its bindings.
//!
//! S2, the same buffers and bindings, its shaders `storage_fields.wgsl`: each reads the item's
//! fields that it needs, one by one, as T's read their texels, where S's read the whole item.
//! Between S and T, two things change: storage buffer or texture, whole item or fields; S2 parts
//! them.

use crate::canvas::{Batch, Canvas, ITEM_SIZE, item_bytes};
use crate::gpu::{self, Gpu, Images, Layout, Rewrite, Uploads};

pub const SOURCE: &str = include_str!("storage.wgsl");
/// S2's shaders.
pub const FIELDS_SOURCE: &str = include_str!("storage_fields.wgsl");

/// The most items a canvas draws today, `slime-render`'s `MAX_CANVAS_ITEMS`.
pub const MAX_ITEMS: usize = 1 << 19;

pub struct Storage {
    frame_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    transforms: wgpu::Buffer,
    /// The items' buffer, its size, and the frame's bind group that holds it.
    items: Option<(wgpu::Buffer, u64, wgpu::BindGroup)>,
    image_groups: Vec<wgpu::BindGroup>,
    batches: Vec<Batch>,
}

fn storage_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    size: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

impl Storage {
    /// S's pipeline and buffers, its shaders `source` (S's or S2's) labelled `label`, or wgpu's
    /// validation error where the device cannot hold them.
    pub fn new(gpu: &Gpu, images: &Images, label: &str, source: &str) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("S frame"),
                        entries: &[
                            gpu::uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, 16),
                            storage_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT, ITEM_SIZE as u64),
                            storage_entry(2, wgpu::ShaderStages::VERTEX, 32),
                        ],
                    });
            let image_layout = gpu::image_layout(gpu);
            let module = gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
            let layout = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(label),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipeline = gpu::pipeline(gpu, label, &module, &layout, "vs_main", "fs_main", &[]);
            let transforms = gpu.buffer("S transforms", 32, wgpu::BufferUsages::STORAGE);
            let mut identity = Vec::new();
            crate::canvas::push_f32s(&mut identity, &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            gpu.queue.write_buffer(&transforms, 0, &identity);
            Self {
                frame: gpu.buffer("S frame", 16, wgpu::BufferUsages::UNIFORM),
                image_groups: gpu::image_groups(gpu, &image_layout, images),
                frame_layout,
                pipeline,
                transforms,
                items: None,
                batches: Vec::new(),
            }
        });
        match error {
            None => Ok(made),
            Some(error) => Err(error),
        }
    }
}

impl Layout for Storage {
    fn upload(
        &mut self,
        gpu: &Gpu,
        uploads: &mut Uploads,
        canvas: &Canvas,
        size: (u32, u32),
    ) -> u64 {
        let count = canvas.items.len().min(MAX_ITEMS);
        // As `slime-render` grows it: by powers of two, from 64 items.
        let bytes = (count as u64 * ITEM_SIZE as u64)
            .next_power_of_two()
            .max(ITEM_SIZE as u64 * 64);
        if self.items.as_ref().is_none_or(|(_, held, _)| *held < bytes) {
            let buffer = gpu.buffer("S items", bytes, wgpu::BufferUsages::STORAGE);
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("S frame"),
                layout: &self.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.frame.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.transforms.as_entire_binding(),
                    },
                ],
            });
            self.items = Some((buffer, bytes, group));
        }
        let (buffer, _, _) = self.items.as_ref().expect("made above");
        let data = item_bytes(&canvas.items[..count], true);
        let frame = gpu::frame_bytes(size);
        let bytes = (data.len() + frame.len()) as u64;
        uploads.write_buffer(buffer, 0, data);
        uploads.write_buffer(&self.frame, 0, frame.to_vec());
        self.batches = canvas.batches(count);
        bytes
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize) {
        let Some((_, _, frame_group)) = &self.items else {
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

impl Rewrite for Storage {
    fn rewrite(&mut self, uploads: &mut Uploads, canvas: &Canvas, index: usize) -> u64 {
        let (buffer, _, _) = self.items.as_ref().expect("uploaded first");
        let data = item_bytes(&canvas.items[index..=index], true);
        let bytes = data.len() as u64;
        uploads.write_buffer(buffer, (index * ITEM_SIZE) as u64, data);
        bytes
    }
}
