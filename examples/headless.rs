//! Runs an application without a window, the way automated tests and dedicated servers run it.
//! Frames run back to back with simulated time: each frame represents one fixed step (1/60 s by
//! default), whatever the speed of the machine.
//!
//! ```sh
//! cargo run --example headless                        # 600 frames: 10 s of simulated time
//! cargo run --example headless -- --frames 60
//! SLIME_LOG=trace cargo run --example headless -- --frames 2   # shows the profiling spans
//! ```

use std::process::ExitCode;
use std::time::{Duration, Instant};

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, Context};

/// Counts what the engine asks it to do.
#[derive(Default)]
struct Counter {
    fixed_steps: u64,
    simulated: Duration,
}

impl App for Counter {
    fn init(&mut self, _ctx: &mut Context) {
        tracing::info!("init");
    }

    fn fixed_update(&mut self, _ctx: &mut Context, _step: Duration) {
        self.fixed_steps += 1;
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        self.simulated = frame.elapsed();
        if frame.index().is_multiple_of(60) {
            tracing::debug!(index = frame.index(), elapsed = ?frame.elapsed(), "frame");
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        tracing::info!(
            fixed_steps = self.fixed_steps,
            simulated = ?self.simulated,
            "shutdown"
        );
    }
}

fn main() -> ExitCode {
    let frames = match parse_frames() {
        Ok(frames) => frames,
        Err(message) => {
            eprintln!("{message}\nusage: headless [--frames <count>]");
            return ExitCode::from(2);
        }
    };

    let config = AppConfig {
        frame_limit: Some(frames),
        ..AppConfig::default()
    };
    let started = Instant::now();
    slime_app::run(&mut Counter::default(), &config);
    tracing::info!(wall_clock = ?started.elapsed(), "done");
    ExitCode::SUCCESS
}

fn parse_frames() -> Result<u64, String> {
    let mut frames = 600;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--frames" => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                frames = value
                    .parse()
                    .map_err(|_| format!("invalid frame count `{value}`"))?;
            }
            _ => return Err(format!("unknown argument `{argument}`")),
        }
    }
    Ok(frames)
}
