//! Renders the samples' triangle without a window, reads the image back to the CPU and writes it
//! to a PNG file. It runs wherever a GPU adapter exists, software ones included (lavapipe on Linux,
//! WARP on Windows), and needs no windowing system.
//!
//! ```sh
//! cargo run --example offscreen                        # writes triangle.png
//! cargo run --example offscreen -- --output out.png
//! VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo run --example offscreen   # on lavapipe
//! ```

#[path = "common/error_chain.rs"]
mod error_chain;
#[path = "common/png_file.rs"]
mod png_file;
#[path = "common/triangle.rs"]
mod triangle;

use std::path::PathBuf;
use std::process::ExitCode;

use slime_gfx::{Gpu, RenderPassDescriptor, TextureFormat};

use error_chain::error_chain;
use triangle::{BACKGROUND, Triangle};

/// The size of the image, in pixels.
const SIZE: u32 = 256;

fn main() -> ExitCode {
    let output = match parse_output() {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}\nusage: offscreen [--output <file.png>]");
            return ExitCode::from(2);
        }
    };
    slime_app::init_diagnostics();
    match render_to_file(&output) {
        Ok(()) => {
            tracing::info!(path = %output.display(), "image written");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!("{}", error_chain(error.as_ref()));
            ExitCode::FAILURE
        }
    }
}

fn render_to_file(output: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let gpu = Gpu::new()?;
    let target = gpu.create_render_target("offscreen", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let triangle = Triangle::new(&gpu, target.format())?;
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(BACKGROUND),
        depth: None,
    };
    gpu.render(&pass, |pass| triangle.draw(pass));
    let image = gpu.read_render_target(&target)?;
    png_file::write(output, &image)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    Ok(())
}

fn parse_output() -> Result<PathBuf, String> {
    let mut output = PathBuf::from("triangle.png");
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--output") => output = arguments.next().ok_or("--output needs a path")?.into(),
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(output)
}
