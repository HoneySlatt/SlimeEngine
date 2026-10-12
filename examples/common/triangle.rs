//! The triangle that the samples draw, shared by the examples and the reference image test: its
//! shader, its vertices and the commands that draw it.

use slime_gfx::{
    Color, CullMode, GfxError, Gpu, RenderPass, RenderPipeline, RenderPipelineDescriptor,
    TextureFormat, VertexAttribute, VertexBuffer, VertexFormat, VertexLayout,
};

/// The color behind the triangle.
pub const BACKGROUND: Color = Color {
    r: 0.02,
    g: 0.06,
    b: 0.04,
    a: 1.0,
};

/// For each corner: position in clip space (x, y), then color (red, green, blue).
const VERTICES: [[f32; 5]; 3] = [
    [0.0, 0.6, 1.0, 0.2, 0.2],
    [-0.6, -0.5, 0.2, 1.0, 0.2],
    [0.6, -0.5, 0.2, 0.3, 1.0],
];

/// The GPU objects that draw the triangle into targets of one format.
pub struct Triangle {
    pipeline: RenderPipeline,
    vertices: VertexBuffer,
}

impl Triangle {
    /// Compiles the shader and uploads the vertices, for targets of `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, GfxError> {
        let shader = gpu.create_shader_module("triangle", include_str!("triangle.wgsl"))?;
        let pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "triangle",
            shader: &shader,
            vertex_entry_point: "vs_main",
            fragment_entry_point: "fs_main",
            vertex_layout: VertexLayout {
                stride: 5 * 4,
                attributes: &[
                    VertexAttribute {
                        location: 0,
                        offset: 0,
                        format: VertexFormat::Float32x2,
                    },
                    VertexAttribute {
                        location: 1,
                        offset: 2 * 4,
                        format: VertexFormat::Float32x3,
                    },
                ],
            },
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
            target_format: format,
        })?;
        let bytes: Vec<u8> = VERTICES
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let vertices = gpu.create_vertex_buffer("triangle vertices", &bytes);
        Ok(Self { pipeline, vertices })
    }

    /// Records the commands that draw the triangle, grouped under `triangle` for capture tools.
    pub fn draw(&self, pass: &mut RenderPass<'_>) {
        pass.debug_group("triangle", |pass| {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, &self.vertices);
            pass.draw(0..3);
        });
    }
}
