//! The uploader (ADR-0107): ranges of vertex and index buffers that it brings, drawn once their
//! upload is done; copies that wait for no budget before the first frame, nor long after the last;
//! the copies still waiting submitted when it is dropped; an upload told of a lost device; the
//! copies of the lowest rank first, a rank changed while they wait included (ADR-0113). Runs on
//! whatever adapter the machine offers, like `device.rs`.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use slime_color::LinearColor;
use slime_gfx::{
    CullMode, FragmentState, Gpu, RenderPassDescriptor, RenderPipelineDescriptor, TextureFormat,
    UploadRank, UploadTarget, Uploader, VertexAttribute, VertexBuffer, VertexFormat, VertexLayout,
};

/// White triangles from positions in clip space.
const SHADER: &str = "
@vertex
fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 0.5, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
";

/// How long a test waits for an upload, before it says it never ended.
const PATIENCE: Duration = Duration::from_secs(10);

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// The bytes of `values`, little-endian.
fn bytes<T: Copy>(values: &[T], to_bytes: impl Fn(T) -> [u8; 4]) -> Vec<u8> {
    values.iter().flat_map(|&value| to_bytes(value)).collect()
}

#[test]
fn ranges_of_buffers_that_an_uploader_brings_draw_once_their_upload_is_done() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    // A triangle over the bottom left quarter, written after a first one left at zero, its indices
    // after three others: the ranges of buffers that meshes sharing them would take.
    let vertices = gpu
        .create_zeroed_vertex_buffer("vertices", 6 * 8)
        .expect("the GPU has memory for it");
    let indices = gpu
        .create_zeroed_index_buffer("indices", 6)
        .expect("the GPU has memory for it");
    let quarter = bytes(&[-1.0_f32, -1.0, 0.0, -1.0, -1.0, 0.0], f32::to_le_bytes);
    let corners = bytes(&[3_u32, 4, 5], u32::to_le_bytes);
    let (done, uploaded) = mpsc::channel();
    uploader
        .buffers(
            "triangle",
            &UploadRank::default(),
            &[
                (UploadTarget::Vertices(&vertices), 3 * 8, &quarter),
                (UploadTarget::Indices(&indices), 3 * 4, &corners),
            ],
            move |result| done.send(result.is_ok()).expect("the test waits"),
        )
        .expect("the GPU has memory for it");
    assert_eq!(uploaded.recv_timeout(PATIENCE), Ok(true));

    let target = gpu
        .create_render_target("uploads", 8, 8, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let shader = gpu
        .create_shader_module("uploads", SHADER)
        .expect("the shader compiles");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "uploads",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: Some(VertexLayout {
                stride: 8,
                attributes: &[VertexAttribute {
                    location: 0,
                    offset: 0,
                    format: VertexFormat::Float32x2,
                }],
            }),
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(target.format()),
                blend: None,
            }),
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
        resolve: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &vertices);
        pass.set_index_buffer(&indices);
        pass.draw_indexed(3..6, 0, 0..1);
    });
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");
    let pixel = |x: u32, y: u32| image.pixels[((y * 8 + x) * 4) as usize];
    assert_eq!(pixel(1, 6), 255, "inside the triangle");
    assert_eq!(pixel(6, 1), 0, "outside it");
}

#[test]
fn dropping_the_uploader_submits_the_copies_that_wait_whatever_the_budget() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    // Once a frame is drawn, a megabyte at 64 KiB a budget takes sixteen budgets, a quarter of a
    // second without other frames; the drop submits it at once.
    uploader.frame();
    let vertices = gpu
        .create_zeroed_vertex_buffer("vertices", 1 << 20)
        .expect("the GPU has memory for it");
    let (done, uploaded) = mpsc::channel();
    uploader
        .buffers(
            "a megabyte",
            &UploadRank::default(),
            &[(UploadTarget::Vertices(&vertices), 0, &vec![7; 1 << 20])],
            move |result| done.send(result.is_ok()).expect("the test waits"),
        )
        .expect("the GPU has memory for it");
    drop(uploader);
    assert_eq!(
        uploaded.try_recv(),
        Ok(true),
        "told before the drop returned"
    );
}

#[test]
fn an_upload_is_told_of_a_device_lost_before_its_copies() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    uploader.frame();
    let vertices = gpu
        .create_zeroed_vertex_buffer("vertices", 1 << 20)
        .expect("the GPU has memory for it");
    let (done, uploaded) = mpsc::channel();
    uploader
        .buffers(
            "a megabyte",
            &UploadRank::default(),
            &[(UploadTarget::Vertices(&vertices), 0, &vec![7; 1 << 20])],
            move |result| done.send(result.is_err()).expect("the test waits"),
        )
        .expect("the GPU has memory for it");
    // Lost while most of the copies wait for their budget, a frame drawn: a frame again, so that
    // the copies stay held to the budget however long the staging took.
    uploader.frame();
    gpu.lose();
    assert_eq!(
        uploaded.recv_timeout(PATIENCE),
        Ok(true),
        "told of the loss"
    );
}

/// A vertex buffer of four megabytes, all zero, and as many bytes for it: sixty-four budgets of
/// 64 KiB, more than a second at one budget per 16.7 ms.
fn four_megabytes(gpu: &Gpu) -> (VertexBuffer, Vec<u8>) {
    let vertices = gpu
        .create_zeroed_vertex_buffer("vertices", 4 << 20)
        .expect("the GPU has memory for it");
    (vertices, vec![7; 4 << 20])
}

/// Uploads `bytes` into `vertices` through `uploader`, and waits for the upload's end.
fn upload(uploader: &Uploader, vertices: &VertexBuffer, bytes: &[u8]) {
    let (done, uploaded) = mpsc::channel();
    uploader
        .buffers(
            "four megabytes",
            &UploadRank::default(),
            &[(UploadTarget::Vertices(vertices), 0, bytes)],
            move |result| done.send(result.is_ok()).expect("the test waits"),
        )
        .expect("the GPU has memory for it");
    assert_eq!(uploaded.recv_timeout(PATIENCE), Ok(true));
}

#[test]
fn copies_wait_for_no_budget_before_the_first_frame() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    let (vertices, bytes) = four_megabytes(&gpu);
    let started = Instant::now();
    upload(&uploader, &vertices, &bytes);
    let took = started.elapsed();
    assert!(took < Duration::from_millis(600), "took {took:?}");
}

#[test]
fn copies_wait_for_no_budget_a_tenth_of_a_second_after_the_last_frame() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    let (vertices, bytes) = four_megabytes(&gpu);
    uploader.frame();
    let framed = Instant::now();
    upload(&uploader, &vertices, &bytes);
    let took = framed.elapsed();
    // Held to the budget for 100 ms after the frame, a few budgets of the sixty-four, then the
    // rest at once.
    assert!(
        (Duration::from_millis(95)..Duration::from_millis(700)).contains(&took),
        "took {took:?}"
    );
}

#[test]
fn the_copies_of_the_lowest_rank_go_first_and_a_rank_changes_while_they_wait() {
    let gpu = gpu();
    let uploader = Uploader::new(&gpu, 64 << 10);
    let vertices = gpu
        .create_zeroed_vertex_buffer("vertices", 6 * (64 << 10))
        .expect("the GPU has memory for it");
    let budget = vec![7; 64 << 10];
    let (done, order) = mpsc::channel();
    // A budget each: once a frame is drawn, the first takes the budget, and the others wait for
    // the next renewals, given before any.
    uploader.frame();
    let ranks = [0, 5, 1, 9, 8, 3].map(UploadRank::new);
    for (slot, (name, rank)) in ["filler", "fifth", "first", "raised", "eighth", "third"]
        .into_iter()
        .zip(&ranks)
        .enumerate()
    {
        let done = done.clone();
        uploader
            .buffers(
                name,
                rank,
                &[(
                    UploadTarget::Vertices(&vertices),
                    slot as u64 * (64 << 10),
                    &budget,
                )],
                move |result| {
                    assert!(result.is_ok());
                    done.send(name).expect("the test waits");
                },
            )
            .expect("the GPU has memory for it");
    }
    // Raised from 9 to 0 while it waits.
    ranks[3].set(0);
    let names: Vec<&str> = (0..6)
        .map(|_| order.recv_timeout(PATIENCE).expect("the uploads end"))
        .collect();
    assert_eq!(
        names,
        ["filler", "raised", "first", "third", "fifth", "eighth"]
    );
}
