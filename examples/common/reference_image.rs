//! Comparing a rendered image with a reference PNG stored in `tests/reference/`, within the
//! tolerance of ADR-0018, and rendering the references again on request.

#[path = "png_file.rs"]
mod png_file;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use slime_gfx::Image;

/// The largest difference allowed in one channel of one pixel, out of 255.
const CHANNEL_TOLERANCE: u8 = 2;
/// How many pixels may differ by more than `CHANNEL_TOLERANCE`.
const MAX_DIFFERENT_PIXELS: usize = 0;

/// Whether the references are to be rendered again instead of being compared with:
/// `SLIME_UPDATE_REFERENCE_IMAGES` is set.
pub fn update_requested() -> bool {
    std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some()
}

/// Writes `image` as the reference named `name`.
pub fn write(name: &str, image: &Image) {
    png_file::write(&reference_path(name), image).expect("the reference image is written");
}

/// Compares `image` with the reference named `name`. When they differ beyond the tolerance, the
/// image is written as `<actual_name>.actual.png` in the build directory, and the test fails
/// with its path, the size of the difference and the adapter that rendered it.
pub fn check(name: &str, actual_name: &str, image: &Image, adapter: &str) {
    let reference = read_png(&reference_path(name));
    let difference = compare(image, &reference);
    // Visible with `--nocapture`: the measurements that justify the tolerance.
    eprintln!(
        "reference `{name}` on `{adapter}`: {} pixels differ by more than {CHANNEL_TOLERANCE}, \
         largest difference {}",
        difference.different_pixels, difference.largest
    );
    if difference.different_pixels > MAX_DIFFERENT_PIXELS {
        let actual_path =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{actual_name}.actual.png"));
        png_file::write(&actual_path, image).expect("the rendered image is written");
        panic!(
            "{} pixels differ from the reference `{name}` by more than {CHANNEL_TOLERANCE} \
             (largest difference: {}) on adapter `{adapter}`; rendered image: {}",
            difference.different_pixels,
            difference.largest,
            actual_path.display()
        );
    }
}

fn reference_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/reference/{name}.png"))
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
