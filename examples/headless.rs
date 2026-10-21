//! Runs a game's rules, a simulation, without a window, the way automated tests run it. Frames run
//! back to back with simulated time: each frame represents one fixed step (1/60 s by default),
//! whatever the speed of the machine. A dedicated server, which keeps to real time, is a later
//! stage's work.
//!
//! ```sh
//! cargo run --example headless                        # 600 frames: 10 s of simulated time
//! cargo run --example headless -- --frames 60
//! SLIME_LOG=trace cargo run --example headless -- --frames 2   # shows the profiling spans
//! ```

use std::process::ExitCode;
use std::time::{Duration, Instant};

use slime_app::{AppConfig, AppError, Simulation, SimulationContext};
use slime_world::World;

/// Counts what the engine asks it to do.
#[derive(Default)]
struct Counter {
    fixed_steps: u64,
    simulated: Duration,
}

impl Simulation for Counter {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        _world: &mut World,
    ) -> Result<(), AppError> {
        tracing::info!("start");
        Ok(())
    }

    fn fixed_step(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
        self.fixed_steps += 1;
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        let frame = ctx.time().real();
        self.simulated = frame.elapsed();
        if frame.index().is_multiple_of(60) {
            tracing::debug!(index = frame.index(), elapsed = ?frame.elapsed(), "frame");
        }
    }

    fn stop(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
        tracing::info!(
            fixed_steps = self.fixed_steps,
            simulated = ?self.simulated,
            "stop"
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
    slime_app::run(&mut Counter::default(), &mut (), &config).expect("this game starts");
    tracing::info!(wall_clock = ?started.elapsed(), "done");
    ExitCode::SUCCESS
}

fn parse_frames() -> Result<u64, String> {
    let mut frames = 600;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                frames = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(frames)
}
