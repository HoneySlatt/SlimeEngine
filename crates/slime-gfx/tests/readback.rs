//! Reading render targets back: the rows are unpadded and the channels ordered whatever the size
//! and the byte order of the target. GPU copies pad rows to 256 bytes, so widths whose rows are
//! not a multiple of that exercise the unpadding, which the 256-pixel-wide reference image does
//! not. Runs on whatever adapter the machine offers, like `device.rs`.

use slime_gfx::{Color, Gpu, TextureFormat};

/// A color whose sRGB encoding is known: 0.5 linear is 187.5 of 255, so 187 or 188.
const CLEAR: Color = Color {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    a: 1.0,
};

#[test]
fn readback_unpads_rows_and_orders_channels_for_every_size() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    for (width, height) in [(1, 1), (3, 5), (63, 2), (64, 1), (65, 2), (257, 2)] {
        for format in [TextureFormat::Rgba8UnormSrgb, TextureFormat::Bgra8UnormSrgb] {
            let target = gpu.create_render_target("readback", width, height, format);
            gpu.render(&target, CLEAR, |_pass| {});
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
