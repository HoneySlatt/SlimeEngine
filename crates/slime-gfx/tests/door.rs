//! The guarded door through which the engine lends its wgpu device (`Gpu::with_wgpu`,
//! `docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`): a lost device runs
//! nothing, a loss inside the closure is reported, memory that runs out is an error the device goes
//! on after, an object of another device panics, an error of wgpu panics with its message, and a
//! broken shader that the closure checks in scopes of its own stays out of the door's; what is
//! lent draws into a target, an sRGB one through its view without the encoding. Built with the
//! `wgpu` feature, which `cargo test --workspace` turns on through `slime-app`. Runs on whatever
//! adapter the machine offers.

#[path = "common/memory_budget.rs"]
mod memory_budget;

use memory_budget::{CHILD_ENV, PROBE, run_out_of_memory_in_a_child};
use slime_gfx::{GfxError, Gpu, MEMORY_BUDGET_ENV_VAR, RenderTarget, TextureFormat, WgpuLoan};

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

fn target(gpu: &Gpu, format: TextureFormat) -> RenderTarget {
    gpu.create_render_target("lent", 4, 4, format)
        .expect("the GPU has memory for it")
}

/// Records a pass that clears the texture lent for `target`, seen in `format`, to `color`.
fn clear(loan: &mut WgpuLoan<'_>, target: &RenderTarget, format: TextureFormat, color: f64) {
    let texture = loan.objects.target(target);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(WgpuLoan::texture_format(format)),
        ..Default::default()
    });
    loan.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("cleared"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: color,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
}

#[test]
fn what_is_lent_draws_into_a_target_an_srgb_one_through_its_view_without_the_encoding() {
    let gpu = gpu();
    let srgb = target(&gpu, TextureFormat::Rgba8UnormSrgb);
    let drawn = gpu
        .with_wgpu("lent", |loan| {
            // Half, stored as it is through the view without the encoding: 128, where the
            // encoded view stores 188.
            clear(loan, &srgb, TextureFormat::Rgba8Unorm, 0.5);
            loan.downlevel.clone()
        })
        .expect("the device is there");
    assert!(
        drawn.flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
        "Vulkan's and Direct3D 12's adapters run compute shaders: {drawn:?}"
    );
    let image = gpu.read_render_target(&srgb).expect("read back");
    assert!(
        image.pixels[0].abs_diff(128) <= 1,
        "{:?}",
        &image.pixels[..4]
    );
}

#[test]
fn a_lost_device_runs_nothing() {
    let gpu = gpu();
    gpu.lose();
    let mut ran = false;
    let result = gpu.with_wgpu("lost", |_| ran = true);
    assert!(
        matches!(result, Err(GfxError::DeviceLost { .. })),
        "{result:?}"
    );
    assert!(!ran);
}

/// As a driver reset comes, unknown to the engine until the door asks after the closure.
#[test]
fn a_device_lost_inside_the_closure_is_reported_once_it_returns() {
    let gpu = gpu();
    let lent = target(&gpu, TextureFormat::Rgba8UnormSrgb);
    let result = gpu.with_wgpu("lost inside", |loan| {
        loan.device.destroy();
        // Objects made and used after the loss, which wgpu holds invalid.
        let texture = loan.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("after the loss"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        loan.queue.write_texture(
            texture.as_image_copy(),
            &[0; 64],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16),
                rows_per_image: Some(4),
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );
        clear(loan, &lent, TextureFormat::Rgba8Unorm, 1.0);
    });
    assert!(
        matches!(result, Err(GfxError::DeviceLost { .. })),
        "{result:?}"
    );
    assert!(gpu.check().is_err(), "the loss is recorded");
}

#[test]
#[should_panic(expected = "a render target of another GPU device used with this one")]
fn a_target_of_another_device_is_refused() {
    let (gpu, other) = (gpu(), gpu());
    let theirs = target(&other, TextureFormat::Rgba8UnormSrgb);
    let _ = gpu.with_wgpu("another device", |loan| {
        loan.objects.target(&theirs);
    });
}

#[test]
#[should_panic(expected = "a texture of another GPU device used with this one")]
fn a_texture_of_another_device_is_refused() {
    let (gpu, other) = (gpu(), gpu());
    let theirs = other
        .create_texture("theirs", 1, 1, TextureFormat::Rgba8Unorm, &[0; 4])
        .expect("the GPU has memory for it");
    let _ = gpu.with_wgpu("another device", |loan| {
        loan.objects.texture_view(&theirs);
    });
}

#[test]
#[should_panic(expected = "wgpu error in `misused`")]
fn an_error_of_wgpu_inside_the_closure_panics_with_its_message() {
    let gpu = gpu();
    let _ = gpu.with_wgpu("misused", |loan| {
        // A buffer mapped at creation must be a multiple of 4 bytes.
        loan.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("misused"),
            size: 3,
            usage: wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: true,
        })
    });
}

#[test]
fn a_broken_shader_checked_in_the_closure_s_own_scopes_stays_out_of_the_door_s() {
    let gpu = gpu();
    let caught = gpu
        .with_wgpu("checked", |loan| {
            let scope = loan.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let _module = loan
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("broken"),
                    source: wgpu::ShaderSource::Wgsl("@fragment fn main( -> {}".into()),
                });
            pollster::block_on(scope.pop())
        })
        .expect("the door saw no error");
    assert!(
        matches!(caught, Some(wgpu::Error::Validation { .. })),
        "{caught:?}"
    );
    assert!(gpu.check().is_ok());
}

/// The most textures of 512 MiB that a child creates: 4 GiB.
const FILLERS: usize = 8;

#[test]
fn memory_that_runs_out_inside_the_closure_is_an_error_and_the_device_goes_on() {
    let Ok(mode) = std::env::var(CHILD_ENV) else {
        let Some(stdout) = run_out_of_memory_in_a_child(
            "memory_that_runs_out_inside_the_closure_is_an_error_and_the_device_goes_on",
        ) else {
            eprintln!("skipped: no memory budget makes the memory of this driver run out");
            return;
        };
        assert!(stdout.contains("the device goes on"), "{stdout}");
        return;
    };
    let gpu = match Gpu::new() {
        Ok(gpu) => gpu,
        Err(GfxError::RequestDevice(_)) => {
            println!("no device");
            return;
        }
        Err(error) => panic!("a GPU adapter must be available: {error}"),
    };
    // Textures that a library built on wgpu creates through the door, outside scopes of its own.
    let mut fillers = Vec::new();
    let error = loop {
        if fillers.len() == FILLERS {
            println!("never ran out");
            return;
        }
        let made = gpu.with_wgpu("filler", |loan| {
            loan.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("filler"),
                size: wgpu::Extent3d {
                    width: 8192,
                    height: 8192,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        match made {
            Ok(texture) => fillers.push(texture),
            Err(error) => break error,
        }
    };
    if mode == PROBE {
        return;
    }
    assert!(
        matches!(&error, GfxError::OutOfMemory { label } if label == "filler"),
        "{error:?}"
    );
    assert!(gpu.check().is_ok(), "the device is not lost");
    drop(fillers);
    let small = target(&gpu, TextureFormat::Rgba8UnormSrgb);
    gpu.with_wgpu("after", |loan| {
        clear(loan, &small, TextureFormat::Rgba8UnormSrgb, 1.0);
    })
    .expect("the device goes on");
    let image = gpu.read_render_target(&small).expect("read back");
    assert_eq!(image.pixels[..4], [255, 0, 0, 255]);
    println!("the device goes on");
}
