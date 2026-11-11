//! T: S's items, the same bytes, written each frame into an `Rgba32Uint` texture that both shaders
//! read with `textureLoad` (`texture.wgsl`). One pipeline, one draw per run of one image, as S.

use crate::canvas::{Batch, Canvas, ITEM_SIZE, item_bytes};
use crate::gpu::{self, Gpu, Images, Layout, Rewrite};

pub const SOURCE: &str = include_str!("texture.wgsl");

/// The texture's width, in texels: WebGL2's largest.
pub const WIDTH: u32 = 2048;
/// Its largest height; it grows to it by powers of two.
pub const MAX_HEIGHT: u32 = 2048;
/// A texel's bytes: four 32-bit values.
const TEXEL: usize = 16;
/// The texels of an item.
const ITEM_TEXELS: usize = ITEM_SIZE / TEXEL;
/// The most items the largest texture holds.
pub const MAX_ITEMS: usize = (WIDTH as usize * MAX_HEIGHT as usize) / ITEM_TEXELS;

pub struct Texture {
    frame_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    transforms: wgpu::Buffer,
    /// The items' texture, its height, and the frame's bind group that holds it.
    items: Option<(wgpu::Texture, u32, wgpu::BindGroup)>,
    image_groups: Vec<wgpu::BindGroup>,
    batches: Vec<Batch>,
    warned: bool,
}

impl Texture {
    pub fn new(gpu: &Gpu, images: &Images) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("T frame"),
                        entries: &[
                            gpu::uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, 16),
                            gpu::uniform_entry(
                                1,
                                wgpu::ShaderStages::VERTEX,
                                gpu::TRANSFORMS * gpu::TRANSFORM_BYTES,
                            ),
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
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
                    label: Some("T"),
                    source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
                });
            let layout = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("T"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipeline = gpu::pipeline(gpu, "T", &module, &layout, "vs_main", "fs_main", &[]);
            Self {
                frame: gpu.buffer("T frame", 16, wgpu::BufferUsages::UNIFORM),
                transforms: gpu::transforms_buffer(gpu),
                image_groups: gpu::image_groups(gpu, &image_layout, images),
                frame_layout,
                pipeline,
                items: None,
                batches: Vec::new(),
                warned: false,
            }
        });
        match error {
            None => Ok(made),
            Some(error) => Err(error),
        }
    }

    /// A texture of `height` rows and the frame's bind group that holds it.
    fn create_items(&self, gpu: &Gpu, height: u32) -> (wgpu::Texture, u32, wgpu::BindGroup) {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T items"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("T frame"),
            layout: &self.frame_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.transforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        (texture, height, group)
    }
}

impl Layout for Texture {
    fn upload(&mut self, gpu: &Gpu, canvas: &Canvas, size: (u32, u32)) -> u64 {
        let count = canvas.items.len().min(MAX_ITEMS);
        if count < canvas.items.len() && !self.warned {
            eprintln!(
                "T: {} items left out past {MAX_ITEMS}",
                canvas.items.len() - count
            );
            self.warned = true;
        }
        let texels = count * ITEM_TEXELS;
        let rows = texels.div_ceil(WIDTH as usize).max(1) as u32;
        let height = rows.next_power_of_two().clamp(16, MAX_HEIGHT);
        if self
            .items
            .as_ref()
            .is_none_or(|(_, held, _)| *held < height)
        {
            self.items = Some(self.create_items(gpu, height));
        }
        let (texture, _, _) = self.items.as_ref().expect("made above");
        let data = item_bytes(&canvas.items[..count], true);
        // The full rows in one write, the last row's start in another: no padding is written.
        let row_bytes = WIDTH as usize * TEXEL;
        let full_rows = texels / WIDTH as usize;
        let write = |row: usize, bytes: &[u8], width: u32, height: u32| {
            if height == 0 || width == 0 {
                return;
            }
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: row as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        };
        write(0, &data[..full_rows * row_bytes], WIDTH, full_rows as u32);
        let rest = &data[full_rows * row_bytes..];
        write(full_rows, rest, (rest.len() / TEXEL) as u32, 1);
        let frame = gpu::frame_bytes(size);
        gpu.queue.write_buffer(&self.frame, 0, &frame);
        self.batches = canvas.batches(count);
        (data.len() + frame.len()) as u64
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

impl Rewrite for Texture {
    fn rewrite(&mut self, gpu: &Gpu, canvas: &Canvas, index: usize) -> u64 {
        let (texture, _, _) = self.items.as_ref().expect("uploaded first");
        let data = item_bytes(&canvas.items[index..=index], true);
        // An item's nine texels may end on the next row: one write each side.
        gpu::write_texels(gpu, texture, index * ITEM_TEXELS, &data);
        data.len() as u64
    }
}
