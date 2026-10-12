//! Writing images to PNG files, for the examples and the reference image test.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use slime_gfx::Image;

/// Writes `image`, whose pixels are sRGB-encoded RGBA, to a PNG file at `path`.
pub fn write(path: &Path, image: &Image) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(path)?),
        image.width,
        image.height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.pixels)?;
    writer.finish()?;
    Ok(())
}
