//! I with the effects (`instances_fx.wgsl`): one pipeline per kind, eight kinds, each instance
//! carrying its transform's six values and its clip in the target's pixels; the paths' segments in
//! an `Rgba32Float` texture 2048 texels wide. Runs and order as I's.
//!
//! The formats, by kind, against I's (`instances.rs`): the transform's index (`Uint32`) becomes
//! its six values (`Float32x3` twice), +20 bytes and +1 attribute; an image's clip, offsets within
//! a pixel of its quad (`Snorm16x4`), becomes the clip itself (`Float32x4`), +8 bytes, since a
//! transformed quad is not cut to it.
//!
//! - BOX, 92 bytes, 8 attributes; GLASS, 108 bytes, 9: a box's and the map to what lies behind.
//! - SHADOW, 124 bytes, 9 attributes; GRADIENT, 104 bytes, 9.
//! - COVERAGE, IMAGE and LAYER, 76 bytes, 6 attributes.
//! - PATH, 68 bytes, 6 attributes: quad, clip, colour (`Unorm8x4`), first segment and count
//!   (`Uint32x2`), transform.

use std::ops::Range;

use crate::canvas::{
    self, Affine, BOX, COVERAGE, Canvas, FX_KINDS, GLASS, GRADIENT, IMAGE, ImageId, Item, LAYER,
    PATH, SHADOW, push_f32s, push_u32s,
};
use crate::gpu::{self, DATA_TEXEL, FxLayout, Gpu, Layout, Uploads};
use crate::instances::{self, KindFormat};
use crate::storage_fx::segment_bytes;

pub const SOURCE: &str = concat!(
    include_str!("fx_common.wgsl"),
    include_str!("instances_fx.wgsl")
);

const BOX_ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm16x4, 4 => Unorm16x4, 5 => Float32,
    6 => Float32x3, 7 => Float32x3
];
const GLASS_ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm16x4, 4 => Unorm16x4, 5 => Float32,
    6 => Float32x4, 7 => Float32x3, 8 => Float32x3
];
const SHADOW_ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
    5 => Float32x3, 6 => Unorm16x4, 7 => Float32x3, 8 => Float32x3
];
const GRADIENT_ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm16x4, 4 => Unorm16x4,
    5 => Unorm16x4, 6 => Unorm16x4, 7 => Float32x3, 8 => Float32x3
];
const IMAGE_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm8x4, 4 => Float32x3, 5 => Float32x3
];
const PATH_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Unorm8x4, 3 => Uint32x2, 4 => Float32x3, 5 => Float32x3
];

const fn format(
    name: &'static str,
    vertex: &'static str,
    fragment: &'static str,
    output: &'static str,
    attributes: &'static [wgpu::VertexAttribute],
    image: bool,
) -> KindFormat {
    KindFormat {
        name,
        vertex,
        fragment,
        output,
        attributes,
        image,
    }
}

/// Each kind's format, by the kind's number.
pub const FORMATS: [KindFormat; FX_KINDS] = [
    format("box", "vs_box", "fs_box", "BoxOut", &BOX_ATTRIBUTES, false),
    format(
        "shadow",
        "vs_shadow",
        "fs_shadow",
        "ShadowOut",
        &SHADOW_ATTRIBUTES,
        false,
    ),
    format(
        "coverage",
        "vs_image",
        "fs_coverage",
        "ImageOut",
        &IMAGE_ATTRIBUTES,
        true,
    ),
    format(
        "image",
        "vs_image",
        "fs_image",
        "ImageOut",
        &IMAGE_ATTRIBUTES,
        true,
    ),
    format(
        "gradient",
        "vs_gradient",
        "fs_gradient",
        "GradientOut",
        &GRADIENT_ATTRIBUTES,
        false,
    ),
    format(
        "path",
        "vs_path",
        "fs_path",
        "PathOut",
        &PATH_ATTRIBUTES,
        false,
    ),
    format(
        "glass",
        "vs_glass",
        "fs_glass",
        "GlassOut",
        &GLASS_ATTRIBUTES,
        true,
    ),
    format(
        "layer",
        "vs_image",
        "fs_layer",
        "ImageOut",
        &IMAGE_ATTRIBUTES,
        true,
    ),
];

struct Run {
    kind: usize,
    image: Option<ImageId>,
    instances: Range<u32>,
}

pub struct InstancesFx {
    pipelines: Vec<wgpu::RenderPipeline>,
    frame_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    frame: wgpu::Buffer,
    vertices: Option<(wgpu::Buffer, u64)>,
    /// The segments' texture, its rows, and the frame's bind group that holds it.
    segments: Option<(wgpu::Texture, u32, wgpu::BindGroup)>,
    regions: [Range<u64>; FX_KINDS],
    runs: Vec<Run>,
    image_groups: Vec<wgpu::BindGroup>,
}

impl InstancesFx {
    pub fn new(gpu: &Gpu) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("I fx frame"),
                        entries: &[
                            gpu::uniform_entry(
                                0,
                                wgpu::ShaderStages::VERTEX_FRAGMENT,
                                gpu::FX_FRAME_BYTES,
                            ),
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
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
                    label: Some("I fx"),
                    source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
                });
            let shapes = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("I fx shapes"),
                    bind_group_layouts: &[Some(&frame_layout)],
                    immediate_size: 0,
                });
            let pictures = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("I fx images"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipelines = FORMATS
                .iter()
                .map(|format| {
                    gpu::pipeline(
                        gpu,
                        &format!("I fx {}", format.name),
                        &module,
                        if format.image { &pictures } else { &shapes },
                        format.vertex,
                        format.fragment,
                        &[Some(wgpu::VertexBufferLayout {
                            array_stride: instances::stride(format.attributes),
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: format.attributes,
                        })],
                    )
                })
                .collect();
            Self {
                pipelines,
                frame: gpu.buffer(
                    "I fx frame",
                    gpu::FX_FRAME_BYTES,
                    wgpu::BufferUsages::UNIFORM,
                ),
                frame_layout,
                image_layout,
                vertices: None,
                segments: None,
                regions: std::array::from_fn(|_| 0..0),
                runs: Vec::new(),
                image_groups: Vec::new(),
            }
        });
        match error {
            None => Ok(made),
            Some(error) => Err(error),
        }
    }
}

fn push_unorm8x4(bytes: &mut Vec<u8>, color: [f32; 4]) {
    bytes.extend(color.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8));
}

fn push_unorm16x4(bytes: &mut Vec<u8>, color: [f32; 4]) {
    for value in color {
        bytes.extend_from_slice(&((value.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes());
    }
}

/// Mixes colours as S's `item_bytes` does: consecutive items of one colour mix it once.
struct Colours {
    last: [([f32; 4], [f32; 4]); 2],
}

impl Colours {
    fn mix(&mut self, color: [f32; 4], slot: usize) -> [f32; 4] {
        if color != self.last[slot].0 {
            self.last[slot] = (color, canvas::mixed(color, true));
        }
        self.last[slot].1
    }
}

/// Appends `item`'s instance to `bytes`, in its kind's format, its transform `transform`.
fn pack(item: &Item, transform: &Affine, colours: &mut Colours, bytes: &mut Vec<u8>) {
    match item.kind[0] {
        BOX | GLASS => {
            push_f32s(bytes, &item.shape);
            push_f32s(bytes, &item.radii);
            push_f32s(bytes, &item.clip);
            push_unorm16x4(bytes, colours.mix(item.color, 0));
            push_unorm16x4(bytes, colours.mix(item.border, 1));
            push_f32s(bytes, &item.widths[..1]);
            if item.kind[0] == GLASS {
                push_f32s(bytes, &item.uv);
            }
        }
        SHADOW => {
            push_f32s(bytes, &item.quad);
            push_f32s(bytes, &item.shape);
            push_f32s(bytes, &item.radii);
            push_f32s(bytes, &item.border);
            push_f32s(bytes, &item.clip);
            push_f32s(bytes, &item.widths[..3]);
            push_unorm16x4(bytes, colours.mix(item.color, 0));
        }
        GRADIENT => {
            push_f32s(bytes, &item.shape);
            push_f32s(bytes, &item.radii);
            push_f32s(bytes, &item.clip);
            push_unorm16x4(bytes, colours.mix(item.color, 0));
            for corner in [item.border, item.uv, item.widths] {
                push_unorm16x4(bytes, canvas::mixed(corner, true));
            }
        }
        PATH => {
            push_f32s(bytes, &item.quad);
            push_f32s(bytes, &item.clip);
            push_unorm8x4(bytes, colours.mix(item.color, 0));
            push_u32s(bytes, &item.kind[2..]);
        }
        COVERAGE | IMAGE | LAYER => {
            push_f32s(bytes, &item.quad);
            push_f32s(bytes, &item.uv);
            push_f32s(bytes, &item.clip);
            push_unorm8x4(bytes, colours.mix(item.color, 0));
        }
        kind => unreachable!("no kind {kind}"),
    }
    let [a, b, c, d, e, f] = *transform;
    push_f32s(bytes, &[a, b, c, d, e, f]);
}

impl Layout for InstancesFx {
    fn upload(
        &mut self,
        gpu: &Gpu,
        uploads: &mut Uploads,
        canvas: &Canvas,
        size: (u32, u32),
    ) -> u64 {
        let transforms = canvas.transform_table();
        let mut counts = [0usize; FX_KINDS];
        for item in &canvas.items {
            counts[item.kind[0] as usize] += 1;
        }
        let mut bytes: [Vec<u8>; FX_KINDS] = std::array::from_fn(|kind| {
            Vec::with_capacity(counts[kind] * instances::stride(FORMATS[kind].attributes) as usize)
        });
        let mut next = [0u32; FX_KINDS];
        let mut runs: Vec<Run> = Vec::new();
        let mut colours = Colours {
            last: [([f32::NAN; 4], [0.0; 4]); 2],
        };
        for (item, &image) in canvas.items.iter().zip(&canvas.item_images) {
            let kind = item.kind[0] as usize;
            match runs.last_mut() {
                Some(run) if run.kind == kind && run.image == image => run.instances.end += 1,
                _ => runs.push(Run {
                    kind,
                    image,
                    instances: next[kind]..next[kind] + 1,
                }),
            }
            next[kind] += 1;
            pack(
                item,
                &transforms[item.kind[1] as usize],
                &mut colours,
                &mut bytes[kind],
            );
        }
        let mut offset = 0;
        for (region, kind_bytes) in self.regions.iter_mut().zip(&bytes) {
            *region = offset..offset + kind_bytes.len() as u64;
            offset = region.end;
        }
        gpu::ensure_buffer(
            gpu,
            &mut self.vertices,
            "I fx instances",
            offset,
            1 << 16,
            wgpu::BufferUsages::VERTEX,
        );
        let (buffer, _) = self.vertices.as_ref().expect("made above");
        for (region, kind_bytes) in self.regions.iter().zip(bytes) {
            uploads.write_buffer(buffer, region.start, kind_bytes);
        }
        let segments = segment_bytes(&canvas.segments);
        let rows = gpu::data_rows(segments.len() / DATA_TEXEL);
        if self
            .segments
            .as_ref()
            .is_none_or(|(_, held, _)| *held < rows)
        {
            let texture =
                gpu::data_texture(gpu, "I fx segments", wgpu::TextureFormat::Rgba32Float, rows);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("I fx frame"),
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
            self.segments = Some((texture, rows, group));
        }
        let (texture, _, _) = self.segments.as_ref().expect("made above");
        let frame = gpu::fx_frame_bytes(size, 0, 0);
        let bytes = offset + (segments.len() + frame.len()) as u64;
        uploads.write_texels(texture, 0, segments);
        uploads.write_buffer(&self.frame, 0, frame.to_vec());
        self.runs = runs;
        bytes
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize) {
        let (Some((buffer, _)), Some((_, _, frame_group))) = (&self.vertices, &self.segments)
        else {
            return (0, 0);
        };
        pass.set_bind_group(0, frame_group, &[]);
        let mut kind = None;
        let mut image = None;
        let mut pipelines = 0;
        for run in &self.runs {
            if kind != Some(run.kind) {
                pass.set_pipeline(&self.pipelines[run.kind]);
                pass.set_vertex_buffer(0, buffer.slice(self.regions[run.kind].clone()));
                kind = Some(run.kind);
                pipelines += 1;
            }
            if FORMATS[run.kind].image {
                let wanted = gpu::view_index(run.image);
                if image != Some(wanted) {
                    pass.set_bind_group(1, &self.image_groups[wanted], &[]);
                    image = Some(wanted);
                }
            }
            pass.draw(0..6, run.instances.clone());
        }
        (self.runs.len(), pipelines)
    }
}

impl FxLayout for InstancesFx {
    fn set_images(&mut self, gpu: &Gpu, views: &[&wgpu::TextureView], sampler: &wgpu::Sampler) {
        self.image_groups = gpu::image_groups_of(gpu, &self.image_layout, views, sampler);
    }
}
