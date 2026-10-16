//! Replacing a lost device: a device lost on purpose is reported as such, a new one opened from the
//! same instance draws the same pixels as the lost one did once its objects are created again, an
//! object of the lost device is refused by the new one, and only a lost device is replaced. Runs on
//! whatever adapter the machine offers, like `device.rs`.

use slime_color::LinearColor;
use slime_gfx::{
    CullMode, FragmentState, GfxError, Gpu, Image, LossKind, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderTarget, TextureFormat,
};

/// A triangle over the left half of the target and its middle, in orange.
const SHADER: &str = "
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corners = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0));
    return vec4<f32>(corners[index], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.0, 1.0);
}
";

const SIZE: u32 = 8;

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// A target of `gpu`.
fn target(gpu: &Gpu) -> RenderTarget {
    gpu.create_render_target("restored", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it")
}

/// The triangle drawn by `gpu` into `target`, read back: every object created with `gpu`.
fn draw(gpu: &Gpu, target: &RenderTarget) -> Image {
    let shader = gpu
        .create_shader_module("triangle", SHADER)
        .expect("the shader compiles");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "triangle",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(target.format()),
            }),
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let pass = RenderPassDescriptor {
        target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
        resolve: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.draw(0..3);
    });
    gpu.read_render_target(target)
        .expect("the image is read back")
}

#[test]
fn a_new_device_draws_what_the_lost_one_drew() {
    let lost = gpu();
    let before = draw(&lost, &target(&lost));
    lost.lose();
    let Err(GfxError::DeviceLost { kind, .. }) = lost.check() else {
        panic!("the loss is reported at once");
    };
    assert_eq!(kind, LossKind::Destroyed);

    let gpu = lost.restore().expect("a new device opens");
    assert!(gpu.check().is_ok());
    let after = draw(&gpu, &target(&gpu));
    assert_eq!(after.pixels, before.pixels);
}

#[test]
#[should_panic(expected = "wgpu error")]
fn an_object_of_the_lost_device_is_refused_by_the_new_one() {
    let lost = gpu();
    let old_target = target(&lost);
    lost.lose();
    let gpu = lost.restore().expect("a new device opens");
    // A bug: the target belongs to the lost device.
    draw(&gpu, &old_target);
}

#[test]
#[should_panic(expected = "only a lost device is restored")]
fn a_device_that_is_not_lost_is_not_replaced() {
    let _ = gpu().restore();
}
