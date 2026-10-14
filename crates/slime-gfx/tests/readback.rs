//! Reading render targets back: the rows are unpadded and the channels ordered whatever the size
//! and the byte order of the target, and 16-bit float targets keep values beyond 0 to 1. GPU
//! copies pad rows to 256 bytes, so widths whose rows are not a multiple of that exercise the
//! unpadding, which the 256-pixel-wide reference image does not. Runs on whatever adapter the
//! machine offers, like `device.rs`.

use slime_gfx::{Gpu, LinearColor, RenderPassDescriptor, TextureFormat};

/// A color whose sRGB encoding is known: 0.5 linear is 187.5 of 255, so 187 or 188.
const CLEAR: LinearColor = LinearColor::rgb(1.0, 0.5, 0.0);

#[test]
fn readback_unpads_rows_and_orders_channels_for_every_size() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    for (width, height) in [(1, 1), (3, 5), (63, 2), (64, 1), (65, 2), (257, 2)] {
        for format in [TextureFormat::Rgba8UnormSrgb, TextureFormat::Bgra8UnormSrgb] {
            let target = gpu
                .create_render_target("readback", width, height, format)
                .expect("the GPU has memory for it");
            let pass = RenderPassDescriptor {
                target: &target,
                clear: Some(CLEAR),
                depth: None,
                timer: None,
            };
            gpu.render(&pass, |_pass| {});
            let image = gpu
                .read_render_target(&target)
                .expect("the image is read back");
            assert_eq!((image.width, image.height), (width, height));
            assert_eq!(
                image.pixels.len(),
                (width * height * 4) as usize,
                "{width}x{height} {format:?}"
            );
            for pixel in image.pixels.chunks(4) {
                let expected = [255, 188, 0, 255];
                let close = pixel
                    .iter()
                    .zip(expected)
                    .all(|(value, expected)| value.abs_diff(expected) <= 1);
                assert!(close, "{width}x{height} {format:?}: pixel {pixel:?}");
            }
        }
    }
}

#[test]
fn a_float_target_keeps_values_beyond_0_and_1_for_every_size() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // Each value is exact in half precision, or rounds to the same one whichever way the driver
    // rounds: 60 000 is 1875 × 32, and 1/3 becomes 1365/4096 (0x3555) both to the nearest and
    // toward zero.
    let clear = LinearColor::rgba(2.5, -1.25, 60_000.0, 1.0 / 3.0);
    let expected = [2.5, -1.25, 60_000.0, 1365.0 / 4096.0];
    // Rows of 8, 24, 248, 256, 264 and 2056 bytes, padded to 256, 256, 256, 256, 512 and 2304.
    for (width, height) in [(1, 1), (3, 5), (31, 2), (32, 1), (33, 2), (257, 2)] {
        let target = gpu
            .create_render_target("hdr", width, height, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(clear),
            depth: None,
            timer: None,
        };
        gpu.render(&pass, |_pass| {});
        let image = gpu
            .read_float_render_target(&target)
            .expect("the image is read back");
        assert_eq!((image.width, image.height), (width, height));
        assert_eq!(image.pixels.len(), (width * height * 4) as usize);
        for pixel in image.pixels.chunks(4) {
            assert_eq!(pixel, expected, "{width}x{height}");
        }
    }
}
