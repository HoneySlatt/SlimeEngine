//! The smallest engine program: it sets up the engine's diagnostics
//! (`slime_app::init_diagnostics`), logs a few lines through `tracing`, which the `SLIME_LOG`
//! environment variable filters, and, with `--panic`, panics on purpose to show how a panic is
//! reported.
//!
//! ```sh
//! cargo run -p slime-examples --example hello
//! SLIME_LOG=debug cargo run -p slime-examples --example hello
//! cargo run -p slime-examples --example hello -- --panic
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut panic_requested = false;
    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("--panic") => panic_requested = true,
            _ => {
                eprintln!(
                    "unknown argument `{}`\nusage: hello [--panic]",
                    argument.display()
                );
                return ExitCode::from(2);
            }
        }
    }

    slime_app::init_diagnostics();
    tracing::info!("hello from SlimeEngine");
    tracing::debug!("this line is recorded when SLIME_LOG is `debug` or more verbose");

    if panic_requested {
        panic!("panic requested with --panic");
    }
    ExitCode::SUCCESS
}
