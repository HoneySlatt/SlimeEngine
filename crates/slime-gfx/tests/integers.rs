//! Render targets of integers, and reading a target back without waiting
//! (`docs/adr/0130-picking.md`): numbers that a shader writes come back as they were written, a
//! frame or more later, without waiting for the GPU; on a lost device, a readback gives the loss.
//! Runs on whatever adapter the machine offers.

use std::time::Duration;

use slime_color::LinearColor;
use slime_gfx::{
    CullMode, FragmentState, GfxError, Gpu, Readback, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderTarget, TextureFormat,
};

/// A triangle over the target, each pixel writing its column and its row plus 1000.
const SHADER: &str = "
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec2<u32> {
    return vec2<u32>(u32(position.x), u32(position.y) + 1000u);
}
";

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// An integer target of 4 × 2 texels, each holding its column and its row plus 1000.
fn numbered(gpu: &Gpu) -> RenderTarget {
    let target = gpu
        .create_render_target("numbers", 4, 2, TextureFormat::Rg32Uint)
        .expect("the GPU has memory for it");
    let shader = gpu
        .create_shader_module("numbers", SHADER)
        .expect("the shader compiles");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "numbers",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(TextureFormat::Rg32Uint),
                blend: None,
            }),
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    gpu.render(
        &RenderPassDescriptor {
            target: &target,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: None,
            resolve: None,
        },
        |pass| {
            pass.set_pipeline(&pipeline);
            pass.draw(0..3);
        },
    );
    target
}

/// Submits work, a frame's, until `readback` gives what it read, for up to ten seconds.
fn read_after_frames(gpu: &Gpu, readback: &Readback) -> Result<Vec<u8>, GfxError> {
    let other = gpu
        .create_render_target("other work", 1, 1, TextureFormat::Rgba8Unorm)
        .expect("the GPU has memory for it");
    for _ in 0..10_000 {
        if let Some(read) = readback.try_read(gpu) {
            return read;
        }
        gpu.render(
            &RenderPassDescriptor {
                target: &other,
                clear: Some(LinearColor::BLACK),
                depth: None,
                timer: None,
                resolve: None,
            },
            |_| {},
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the readback did not come back in ten seconds of frames");
}

#[test]
fn numbers_drawn_into_an_integer_target_come_back_later_without_waiting() {
    let gpu = gpu();
    let target = numbered(&gpu);
    let readback = gpu
        .read_render_target_later(&target)
        .expect("the copy starts");
    // Nothing submitted since: wgpu has not seen the copy done.
    assert!(readback.try_read(&gpu).is_none(), "came back at once");
    drop(target);
    let bytes = read_after_frames(&gpu, &readback).expect("the texels come back");
    let numbers: Vec<[u32; 2]> = bytes
        .chunks(8)
        .map(|texel| {
            let number = |at: usize| u32::from_le_bytes(texel[at..at + 4].try_into().unwrap());
            [number(0), number(4)]
        })
        .collect();
    assert_eq!(
        numbers,
        [
            [0, 1000],
            [1, 1000],
            [2, 1000],
            [3, 1000],
            [0, 1001],
            [1, 1001],
            [2, 1001],
            [3, 1001]
        ]
    );
    // Asked again, they are the same.
    assert_eq!(readback.try_read(&gpu).map(Result::ok), Some(Some(bytes)));
}

#[test]
fn a_readback_on_a_lost_device_gives_the_loss() {
    let gpu = gpu();
    let target = numbered(&gpu);
    let readback = gpu
        .read_render_target_later(&target)
        .expect("the copy starts");
    gpu.lose();
    let Some(Err(error)) = readback.try_read(&gpu) else {
        panic!("a readback of a lost device gave texels, or nothing");
    };
    assert!(matches!(error, GfxError::DeviceLost { .. }), "{error}");
}
