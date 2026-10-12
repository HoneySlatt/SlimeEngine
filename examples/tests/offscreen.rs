//! The reference image test of Stage 2: the samples' triangle, rendered offscreen, must match
//! `tests/reference/triangle.png` within a tolerance.
//!
//! The reference was rendered with lavapipe, Mesa's Vulkan driver that renders on the CPU, which
//! `scripts/check.sh` and Linux CI use. Other drivers (WARP in Windows CI, hardware GPUs) may
//! differ slightly in how they round colors; the tolerance absorbs that. To render the reference
//! again, after a deliberate change of the triangle:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test offscreen
//! ```
//!
//! When the test fails, it writes the image it rendered next to the build artifacts and prints
//! its path, so that the two images can be compared by eye.

#[path = "../common/png_file.rs"]
mod png_file;
#[path = "../common/triangle.rs"]
mod triangle;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use slime_gfx::{Gpu, Image, RenderPassDescriptor, TextureFormat};

use triangle::{BACKGROUND, Triangle};

const SIZE: u32 = 256;
/// The largest difference allowed in one channel of one pixel, out of 255.
const CHANNEL_TOLERANCE: u8 = 2;
/// How many pixels may differ by more than `CHANNEL_TOLERANCE`.
const MAX_DIFFERENT_PIXELS: usize = 0;

#[test]
fn the_triangle_matches_its_reference_image() {
    check_triangle(TextureFormat::Rgba8UnormSrgb, "triangle");
}

/// Most windows show images in blue, green, red order; once read back, the triangle must be the
/// same.
#[test]
fn the_triangle_in_the_usual_window_format_matches_the_same_reference() {
    check_triangle(TextureFormat::Bgra8UnormSrgb, "triangle-bgra");
}

/// Renders the triangle into a target of `format` and compares it with the reference. A rendered
/// image that does not match is written as `<name>.actual.png`.
fn check_triangle(format: TextureFormat, name: &str) {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu.create_render_target("reference triangle", SIZE, SIZE, format);
    let triangle =
        Triangle::new(&gpu, target.format()).expect("the triangle's GPU objects are valid");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(BACKGROUND),
        depth: None,
    };
    gpu.render(&pass, |pass| triangle.draw(pass));
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");

    let reference_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/triangle.png");
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        // One reference for both formats, written by the test of the format it was made in.
        if format == TextureFormat::Rgba8UnormSrgb {
            png_file::write(&reference_path, &image).expect("the reference image is written");
        }
        return;
    }
    let reference = read_png(&reference_path);
    let difference = compare(&image, &reference);
    if difference.different_pixels > MAX_DIFFERENT_PIXELS {
        let actual_path =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.actual.png"));
        png_file::write(&actual_path, &image).expect("the rendered image is written");
        panic!(
            "{} pixels differ from the reference by more than {CHANNEL_TOLERANCE} (largest \
             difference: {}) on adapter `{}`; rendered image: {}",
            difference.different_pixels,
            difference.largest,
            gpu.adapter_name(),
            actual_path.display()
        );
    }
}

struct Difference {
    /// Pixels where at least one channel differs by more than `CHANNEL_TOLERANCE`.
    different_pixels: usize,
    /// The largest difference found in any channel.
    largest: u8,
}

fn compare(image: &Image, reference: &Image) -> Difference {
    assert_eq!(
        (image.width, image.height),
        (reference.width, reference.height),
        "the images have different sizes"
    );
    let mut difference = Difference {
        different_pixels: 0,
        largest: 0,
    };
    for (pixel, expected) in image.pixels.chunks(4).zip(reference.pixels.chunks(4)) {
        let largest = pixel
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        difference.largest = difference.largest.max(largest);
        if largest > CHANNEL_TOLERANCE {
            difference.different_pixels += 1;
        }
    }
    difference
}

fn read_png(path: &Path) -> Image {
    let file =
        File::open(path).unwrap_or_else(|error| panic!("cannot open {}: {error}", path.display()));
    let mut reader = png::Decoder::new(BufReader::new(file))
        .read_info()
        .expect("the reference is a valid PNG file");
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the reference is not too large")
    ];
    let info = reader
        .next_frame(&mut pixels)
        .expect("the reference image decodes");
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "the reference must be 8-bit RGBA"
    );
    pixels.truncate(info.buffer_size());
    Image {
        width: info.width,
        height: info.height,
        pixels,
    }
}
