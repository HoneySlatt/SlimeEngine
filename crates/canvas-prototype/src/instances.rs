//! I: one pipeline for each kind of item, each with its own compact instance format in one vertex
//! buffer, six vertices an instance (`instances.wgsl`). Items keep their order: one draw takes each
//! run of consecutive items of one kind and one image, so that a change of kind costs a draw and a
//! pipeline.
//!
//! The formats, each kind's region of the buffer laid out as its attributes below say:
//!
//! - BOX, 72 bytes: shape, radii, clip (`Float32x4` each); fill, border (`Unorm16x4`); border's
//!   width (`Float32`); transform (`Uint32`). Its quad is computed from its shape and clip.
//! - SHADOW, 104 bytes: quad, shape, radii, exponents, clip (`Float32x4`); shorter side, scale,
//!   inverse deviation (`Float32x3`); colour (`Unorm16x4`); transform.
//! - GRADIENT, 84 bytes: shape, radii, clip (`Float32x4`); four corners' colours (`Unorm16x4`);
//!   transform. Its quad is computed as a box's.
//! - COVERAGE and IMAGE, 48 bytes: quad, uv (`Float32x4`); the clip's offsets from the quad
//!   (`Snorm16x4`, [`clip_offsets`]); colour (`Unorm8x4`); transform.
//!
//! Colours are those S writes, mixed on the CPU, then rounded. Boxes, shadows and gradients round
//! them to 16 bits: they are the translucent layers that stack, and at 8 bits their errors of half
//! a step each added up to 3/255 against S where a translucent bordered box lies over a shadow.
//! Glyphs and images, the most items and the top layer, round them to 8 bits: an error of half a
//! step, one step at most in the target.

use std::ops::Range;

use crate::canvas::{
    self, BOX, COVERAGE, Canvas, GRADIENT, IMAGE, ImageId, Item, KINDS, SHADOW, push_f32s,
    push_u32s,
};
use crate::gpu::{self, Gpu, Images, Layout};

pub const SOURCE: &str = include_str!("instances.wgsl");

const BOX_ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm16x4, 4 => Unorm16x4, 5 => Float32,
    6 => Uint32
];
const SHADOW_ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
    5 => Float32x3, 6 => Unorm16x4, 7 => Uint32
];
const GRADIENT_ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Unorm16x4, 4 => Unorm16x4,
    5 => Unorm16x4, 6 => Unorm16x4, 7 => Uint32
];
const IMAGE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Snorm16x4, 3 => Unorm8x4, 4 => Uint32
];

/// A kind's pipeline: its entry points, the vertex shader's output in `instances.wgsl`, and its
/// instance's attributes.
pub struct KindFormat {
    pub name: &'static str,
    pub vertex: &'static str,
    pub fragment: &'static str,
    pub output: &'static str,
    pub attributes: &'static [wgpu::VertexAttribute],
    /// Whether it reads an image (bind group 1).
    pub image: bool,
}

/// Each kind's format, by the kind's number.
pub const FORMATS: [KindFormat; KINDS] = [
    KindFormat {
        name: "box",
        vertex: "vs_box",
        fragment: "fs_box",
        output: "BoxOut",
        attributes: &BOX_ATTRIBUTES,
        image: false,
    },
    KindFormat {
        name: "shadow",
        vertex: "vs_shadow",
        fragment: "fs_shadow",
        output: "ShadowOut",
        attributes: &SHADOW_ATTRIBUTES,
        image: false,
    },
    KindFormat {
        name: "coverage",
        vertex: "vs_image",
        fragment: "fs_coverage",
        output: "ImageOut",
        attributes: &IMAGE_ATTRIBUTES,
        image: true,
    },
    KindFormat {
        name: "image",
        vertex: "vs_image",
        fragment: "fs_image",
        output: "ImageOut",
        attributes: &IMAGE_ATTRIBUTES,
        image: true,
    },
    KindFormat {
        name: "gradient",
        vertex: "vs_gradient",
        fragment: "fs_gradient",
        output: "GradientOut",
        attributes: &GRADIENT_ATTRIBUTES,
        image: false,
    },
];

/// An instance's bytes: the end of its last attribute.
pub fn stride(attributes: &[wgpu::VertexAttribute]) -> u64 {
    attributes
        .iter()
        .map(|attribute| attribute.offset + attribute.format.size())
        .max()
        .unwrap_or(0)
}

/// Consecutive items of one kind and one image: one draw.
struct Run {
    kind: usize,
    image: Option<ImageId>,
    /// The instances drawn, in the kind's region.
    instances: Range<u32>,
}

pub struct Instances {
    pipelines: Vec<wgpu::RenderPipeline>,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    image_groups: Vec<wgpu::BindGroup>,
    /// The vertex buffer and its size.
    vertices: Option<(wgpu::Buffer, u64)>,
    /// Each kind's bytes in the vertex buffer.
    regions: [Range<u64>; KINDS],
    runs: Vec<Run>,
}

impl Instances {
    pub fn new(gpu: &Gpu, images: &Images) -> Result<Self, String> {
        let (made, error) = gpu.catch(|| {
            let frame_layout =
                gpu.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("I frame"),
                        entries: &[
                            gpu::uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, 16),
                            gpu::uniform_entry(
                                1,
                                wgpu::ShaderStages::VERTEX,
                                gpu::TRANSFORMS * gpu::TRANSFORM_BYTES,
                            ),
                        ],
                    });
            let image_layout = gpu::image_layout(gpu);
            let module = gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("I"),
                    source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
                });
            let shapes = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("I shapes"),
                    bind_group_layouts: &[Some(&frame_layout)],
                    immediate_size: 0,
                });
            let pictures = gpu
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("I images"),
                    bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
                    immediate_size: 0,
                });
            let pipelines = FORMATS
                .iter()
                .map(|format| {
                    gpu::pipeline(
                        gpu,
                        &format!("I {}", format.name),
                        &module,
                        if format.image { &pictures } else { &shapes },
                        format.vertex,
                        format.fragment,
                        &[Some(wgpu::VertexBufferLayout {
                            array_stride: stride(format.attributes),
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: format.attributes,
                        })],
                    )
                })
                .collect();
            let frame = gpu.buffer("I frame", 16, wgpu::BufferUsages::UNIFORM);
            let transforms = gpu::transforms_buffer(gpu);
            let frame_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("I frame"),
                layout: &frame_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: frame.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: transforms.as_entire_binding(),
                    },
                ],
            });
            Self {
                pipelines,
                frame,
                frame_group,
                image_groups: gpu::image_groups(gpu, &image_layout, images),
                vertices: None,
                regions: std::array::from_fn(|_| 0..0),
                runs: Vec::new(),
            }
        });
        match error {
            None => Ok(made),
            Some(error) => Err(error),
        }
    }
}

/// Appends a colour of `[0, 1]` as `Unorm8x4`: the nearest of 256 steps.
fn push_unorm8x4(bytes: &mut Vec<u8>, color: [f32; 4]) {
    bytes.extend(color.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8));
}

/// Appends a colour of `[0, 1]` as `Unorm16x4`: the nearest of 65 536 steps.
fn push_unorm16x4(bytes: &mut Vec<u8>, color: [f32; 4]) {
    for value in color {
        bytes.extend_from_slice(&((value.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes());
    }
}

/// The edges of `clip` less those of `quad`, each kept within [-1, 1], for `Snorm16x4`.
///
/// No pixel of the quad sees the difference: `Canvas::push` cut the quad to the clip grown to whole
/// pixels, so that a clip's left edge lies less than a pixel left of the quad's (likewise for each
/// edge), and a clip's edge more than a pixel beyond the quad's changes no pixel whose centre the
/// quad holds, whose share inside the clip (`clip_share`) then reads the pixel's own edge, not the
/// clip's. Within [-1, 1], 16 bits keep the edge to 1/32767 of a pixel.
fn clip_offsets(quad: [f32; 4], clip: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|edge| (clip[edge] - quad[edge]).clamp(-1.0, 1.0))
}

fn push_snorm16x4(bytes: &mut Vec<u8>, values: [f32; 4]) {
    for value in values {
        bytes.extend_from_slice(&((value * 32767.0).round() as i16).to_le_bytes());
    }
}

/// Mixes colours as S's `item_bytes` does: consecutive items of one colour mix it once.
struct Colours {
    last: [([f32; 4], [f32; 4]); 2],
}

impl Colours {
    fn new() -> Self {
        Self {
            last: [([f32::NAN; 4], [0.0; 4]); 2],
        }
    }

    fn mix(&mut self, color: [f32; 4], slot: usize) -> [f32; 4] {
        if color != self.last[slot].0 {
            self.last[slot] = (color, canvas::mixed(color, true));
        }
        self.last[slot].1
    }
}

/// Appends `item`'s instance to `bytes`, in its kind's format.
fn pack(item: &Item, colours: &mut Colours, bytes: &mut Vec<u8>) {
    let transform = item.kind[1];
    match item.kind[0] {
        BOX => {
            push_f32s(bytes, &item.shape);
            push_f32s(bytes, &item.radii);
            push_f32s(bytes, &item.clip);
            push_unorm16x4(bytes, colours.mix(item.color, 0));
            push_unorm16x4(bytes, colours.mix(item.border, 1));
            push_f32s(bytes, &item.widths[..1]);
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
        _ => {
            push_f32s(bytes, &item.quad);
            push_f32s(bytes, &item.uv);
            push_snorm16x4(bytes, clip_offsets(item.quad, item.clip));
            push_unorm8x4(bytes, colours.mix(item.color, 0));
        }
    }
    push_u32s(bytes, &[transform]);
}

impl Layout for Instances {
    fn upload(&mut self, gpu: &Gpu, canvas: &Canvas, size: (u32, u32)) -> u64 {
        let mut counts = [0usize; KINDS];
        for item in &canvas.items {
            counts[item.kind[0] as usize] += 1;
        }
        let mut bytes: [Vec<u8>; KINDS] = std::array::from_fn(|kind| {
            Vec::with_capacity(counts[kind] * stride(FORMATS[kind].attributes) as usize)
        });
        let mut next = [0u32; KINDS];
        let mut runs: Vec<Run> = Vec::new();
        let mut colours = Colours::new();
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
            pack(item, &mut colours, &mut bytes[kind]);
        }
        let mut offset = 0;
        for (region, kind_bytes) in self.regions.iter_mut().zip(&bytes) {
            *region = offset..offset + kind_bytes.len() as u64;
            offset = region.end;
        }
        let total = offset;
        let needed = total.next_power_of_two().max(1 << 16);
        if self
            .vertices
            .as_ref()
            .is_none_or(|(_, held)| *held < needed)
        {
            self.vertices = Some((
                gpu.buffer("I instances", needed, wgpu::BufferUsages::VERTEX),
                needed,
            ));
        }
        let (buffer, _) = self.vertices.as_ref().expect("made above");
        for (region, kind_bytes) in self.regions.iter().zip(&bytes) {
            if !kind_bytes.is_empty() {
                gpu.queue.write_buffer(buffer, region.start, kind_bytes);
            }
        }
        let frame = gpu::frame_bytes(size);
        gpu.queue.write_buffer(&self.frame, 0, &frame);
        self.runs = runs;
        total + frame.len() as u64
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize) {
        let Some((buffer, _)) = &self.vertices else {
            return (0, 0);
        };
        pass.set_bind_group(0, &self.frame_group, &[]);
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
            if run.kind == COVERAGE as usize || run.kind == IMAGE as usize {
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
