//! Comparing a rendered image with a reference PNG stored in `tests/reference/`, within the
//! tolerances of ADR-0018, ADR-0028, ADR-0030 and ADR-0054, and rendering the references again on
//! request.

#[path = "png_file.rs"]
mod png_file;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use slime_gfx::Image;

/// How far a rendered image may be from its reference.
struct Tolerance {
    /// The largest difference allowed in one channel of one pixel, out of 255.
    channel: u8,
    /// The fraction of the pixels that may differ by more than that.
    pixels: f64,
}

/// On the driver that rendered the references, a difference is a change of the rendering
/// (ADR-0018).
const ON_REFERENCE_DRIVER: Tolerance = Tolerance {
    channel: 2,
    pixels: 0.0,
};
/// On other drivers, which filter textures and place edges differently (ADR-0028, ADR-0030,
/// ADR-0054).
const ON_OTHER_DRIVERS: Tolerance = Tolerance {
    channel: 8,
    pixels: 0.002,
};

/// Whether the references are to be rendered again instead of being compared with:
/// `SLIME_UPDATE_REFERENCE_IMAGES` is set.
pub fn update_requested() -> bool {
    std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some()
}

/// Writes `image` as the reference named `name`.
pub fn write(name: &str, image: &Image) {
    png_file::write(&reference_path(name), image).expect("the reference image is written");
}

/// Compares `image` with the reference named `name`, within the tolerance of the driver of
/// `adapter`. When they differ beyond it, the image is written as `<actual_name>.actual.png` in
/// the build directory, and the test fails with its path, the size of the difference and the
/// adapter that rendered it.
pub fn check(name: &str, actual_name: &str, image: &Image, adapter: &str) {
    let reference = read_png(&reference_path(name));
    let difference = compare(image, &reference);
    // The references are rendered with lavapipe, which names its adapter after llvmpipe.
    let tolerance = if adapter.starts_with("llvmpipe") {
        ON_REFERENCE_DRIVER
    } else {
        ON_OTHER_DRIVERS
    };
    let different = difference.over(tolerance.channel);
    let allowed = (f64::from(image.width * image.height) * tolerance.pixels).floor();
    // Visible with `--nocapture`: the measurements that justify the tolerances.
    eprintln!(
        "reference `{name}` on `{adapter}`: {different} pixels differ by more than {} \
         ({allowed} allowed); over 2: {}, over 4: {}, over 8: {}, over 16: {}; largest \
         difference {}",
        tolerance.channel,
        difference.over(2),
        difference.over(4),
        difference.over(8),
        difference.over(16),
        difference.largest
    );
    if different as f64 > allowed {
        let actual_path =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{actual_name}.actual.png"));
        png_file::write(&actual_path, image).expect("the rendered image is written");
        panic!(
            "{different} pixels differ from the reference `{name}` by more than {}, {allowed} \
             allowed (largest difference: {}) on adapter `{adapter}`; rendered image: {}",
            tolerance.channel,
            difference.largest,
            actual_path.display()
        );
    }
}

fn reference_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/reference/{name}.png"))
}

/// How two images differ: for each pixel, the largest difference in any of its channels.
struct Difference {
    largest_per_pixel: Vec<u8>,
    /// The largest difference found in any channel of any pixel.
    largest: u8,
}

impl Difference {
    /// How many pixels differ by more than `threshold` in at least one channel.
    fn over(&self, threshold: u8) -> usize {
        self.largest_per_pixel
            .iter()
            .filter(|&&largest| largest > threshold)
            .count()
    }
}

fn compare(image: &Image, reference: &Image) -> Difference {
    assert_eq!(
        (image.width, image.height),
        (reference.width, reference.height),
        "the images have different sizes"
    );
    let largest_per_pixel: Vec<u8> = image
        .pixels
        .chunks(4)
        .zip(reference.pixels.chunks(4))
        .map(|(pixel, expected)| {
            pixel
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let largest = largest_per_pixel.iter().copied().max().unwrap_or(0);
    Difference {
        largest_per_pixel,
        largest,
    }
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
