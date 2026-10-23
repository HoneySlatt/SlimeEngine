//! The interface's text, without a GPU (`docs/adr/0136-the-interfaces-text.md`): a sentence of each
//! writing system of slime-ui's tests laid out in a box 560 logical pixels wide, Arabic and Hebrew
//! from right to left, and a label cut with an ellipsis; each glyph rasterized and put on a white
//! image by the CPU, which is written to a PNG file. The renderer draws text from Stage 6b's third
//! milestone on; until then, this shows what the text module gives it.
//!
//! ```sh
//! cargo run --example text                                  # writes text.png
//! cargo run --example text -- --scale 2 --output text@2.png
//! ```
//!
//! It reads the fonts of slime-ui's tests, parts of Noto fonts that hold the characters of those
//! sentences alone (`crates/slime-ui/tests/fonts/README.md`).

#[path = "common/png_file.rs"]
mod png_file;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use slime_gfx::Image;
use slime_ui::text::{Fonts, GlyphPixels, GlyphRasterizer, TextBounds, TextLayout, TextStyle};

/// The width of the box, and the margin around the text, in logical pixels.
const WIDTH: f32 = 560.0;
const MARGIN: f32 = 20.0;

/// Each writing system: its test text, its stack, its language.
const SCRIPTS: &[(&str, [&str; 2], &str)] = &[
    ("english", ["Noto Sans", "Noto Sans"], "en"),
    ("arabic", ["Noto Sans Arabic", "Noto Sans"], "ar"),
    ("hebrew", ["Noto Sans Hebrew", "Noto Sans"], "he"),
    ("hindi", ["Noto Sans Devanagari", "Noto Sans"], "hi"),
    ("thai", ["Noto Sans Thai", "Noto Sans"], "th"),
    ("japanese", ["Noto Sans CJK JP", "Noto Sans"], "ja"),
    ("chinese", ["Noto Sans CJK SC", "Noto Sans"], "zh-CN"),
    ("emoji", ["Noto Sans", "Noto Color Emoji"], "en"),
];

fn main() -> ExitCode {
    let (output, scale) = match parse() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\nusage: text [--output <file.png>] [--scale <factor>]");
            return ExitCode::from(2);
        }
    };
    slime_app::init_diagnostics();
    match draw(&output, scale) {
        Ok(()) => {
            tracing::info!(path = %output.display(), "image written");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Lays the sentences out, then draws them into an image written to `output`.
fn draw(output: &Path, scale: f32) -> Result<(), String> {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/slime-ui/tests");
    let mut fonts = Fonts::new();
    for entry in std::fs::read_dir(tests.join("fonts")).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf")
        ) {
            let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
            fonts
                .register(bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    let mut layouts: Vec<TextLayout> = Vec::new();
    for &(name, stack, locale) in SCRIPTS {
        let path = tests.join(format!("texts/{name}.txt"));
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let style = TextStyle {
            fonts: stack.iter().map(|&family| family.to_owned()).collect(),
            size: 16.0,
            locale: Some(locale.to_owned()),
            ..TextStyle::default()
        };
        let sentence = text.lines().next().unwrap_or_default();
        layouts.push(fonts.lay_out(sentence, &style, scale, TextBounds::width(WIDTH)));
    }
    let label = TextStyle {
        fonts: vec!["Noto Sans".to_owned()],
        size: 16.0,
        ..TextStyle::default()
    };
    layouts.push(fonts.lay_out(
        "Every panel can be docked, undocked, resized, or grouped into tabs",
        &label,
        scale,
        TextBounds::line(240.0),
    ));

    let margin = (MARGIN * scale).round() as i32;
    let width = ((WIDTH + 2.0 * MARGIN) * scale).ceil() as u32;
    let height = layouts
        .iter()
        .map(|layout| layout.height().ceil() as i32 + margin)
        .sum::<i32>()
        + margin;
    let mut image = Image {
        width,
        height: u32::try_from(height).map_err(|error| error.to_string())?,
        pixels: vec![255; (width * u32::try_from(height).unwrap_or(0) * 4) as usize],
    };
    let mut rasterizer = GlyphRasterizer::new();
    let mut top = margin;
    for layout in &layouts {
        for run in layout.runs() {
            for glyph in run.placed() {
                if let Some(drawn) = rasterizer.rasterize(&fonts, glyph.key) {
                    let x = margin + glyph.x + drawn.left;
                    let y = top + glyph.y - drawn.top;
                    blend(&mut image, x, y, drawn.width, &drawn.pixels);
                }
            }
        }
        top += layout.height().ceil() as i32 + margin;
    }
    png_file::write(output, &image)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))
}

/// Puts a glyph's pixels on `image` from (`x`, `y`): coverage in black, colors as they are.
fn blend(image: &mut Image, x: i32, y: i32, width: u32, pixels: &GlyphPixels) {
    let (bytes, channels) = match pixels {
        GlyphPixels::Coverage(bytes) => (bytes, 1),
        GlyphPixels::Color(bytes) => (bytes, 4),
    };
    for (index, pixel) in bytes.chunks(channels).enumerate() {
        let (column, row) = (index as u32 % width, index as u32 / width);
        let (Ok(px), Ok(py)) = (
            u32::try_from(x + column as i32),
            u32::try_from(y + row as i32),
        ) else {
            continue;
        };
        if px >= image.width || py >= image.height {
            continue;
        }
        let at = ((py * image.width + px) * 4) as usize;
        let (color, alpha) = match pixel {
            [coverage] => ([0, 0, 0], *coverage),
            [red, green, blue, alpha] => ([*red, *green, *blue], *alpha),
            _ => continue,
        };
        let alpha = u32::from(alpha);
        for (channel, &over) in color.iter().enumerate() {
            let under = u32::from(image.pixels[at + channel]);
            image.pixels[at + channel] =
                ((u32::from(over) * alpha + under * (255 - alpha)) / 255) as u8;
        }
    }
}

fn parse() -> Result<(PathBuf, f32), String> {
    let mut output = PathBuf::from("text.png");
    let mut scale = 1.0;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--output") => output = arguments.next().ok_or("--output needs a path")?.into(),
            Some("--scale") => {
                scale = arguments
                    .next()
                    .and_then(|value| value.to_str()?.parse::<f32>().ok())
                    .filter(|&scale| scale > 0.0 && scale <= 8.0)
                    .ok_or("--scale needs a factor above 0 and up to 8")?;
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok((output, scale))
}
