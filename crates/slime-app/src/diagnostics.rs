//! Logging and panic reporting for engine applications.

use std::io::IsTerminal;
use std::panic::{self, PanicHookInfo};
use std::sync::Once;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

/// The environment variable that selects which log events are recorded.
///
/// Its value uses the directive syntax of `tracing-subscriber`'s `EnvFilter`: a level
/// (`SLIME_LOG=debug`), levels per module (`SLIME_LOG=warn,slime_app=trace`), or `off`. A module
/// without a level of its own records `info` and above: when the value names no bare level, as
/// `slime_app=trace` does not, `info` is added for the other modules. Spaces around a directive and
/// empty directives, as a stray comma leaves, are ignored; invalid directives are reported on
/// standard error and ignored. When the variable is unset or has no directive,
/// [`DEFAULT_LOG_FILTER`] applies.
pub const LOG_ENV_VAR: &str = "SLIME_LOG";

/// The log filter used when [`LOG_ENV_VAR`] is unset or has no directive: `info` and above, except
/// for wgpu-hal, whose `info` messages list what the GPU supports (`warn` and above), for the
/// wgpu-hal module that relays the messages of the Vulkan loader and layers (`error` only), and for
/// the module of winit's client-side decorations that asks the desktop settings portal for the
/// theme (nothing): it logs an error whenever no session bus or portal answers, which the engine
/// cannot act on (ADR-0019).
pub const DEFAULT_LOG_FILTER: &str =
    "info,wgpu_hal=warn,wgpu_hal::vulkan::instance=error,sctk_adwaita::config=off";

/// Sets up logging and panic reporting for the process, then logs a line with the engine version,
/// the operating system, the CPU architecture and the number of logical CPUs available to the
/// process.
///
/// - Installs a global [`tracing`] subscriber that writes events to standard error with a
///   timestamp, the level and the module that emitted them, filtered by [`LOG_ENV_VAR`]. Colors
///   are used only when standard error is a terminal and `NO_COLOR` is unset or empty (on Windows,
///   only in Windows Terminal: the older console host prints the escape sequences as text). Records that
///   dependencies emit through the `log` crate reach the same subscriber. When a span that the
///   filter lets through closes, a line records how long it was busy and idle, which makes the
///   profiling spans of the main loop visible with `SLIME_LOG=trace`. An event that cannot be
///   written, because the reader of standard error exited or the terminal closed, is dropped and
///   the process goes on.
/// - If the process already has a global subscriber, for example because the game installed its
///   own, that subscriber is kept and receives the engine's events. A `log` logger installed before
///   this call keeps receiving the records that dependencies emit through `log`.
/// - Installs a panic hook that logs the panic message, its location and the name of the thread
///   as an `error` event, then runs the hook that was installed before it. By default that is the
///   standard library's hook, which prints the message and, when `RUST_BACKTRACE` asks for it, a
///   backtrace.
///
/// The subscriber and the panic hook are global to the process, so only the first call does
/// anything; later calls return immediately.
pub fn init_diagnostics() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        install_subscriber();
        install_panic_hook();
        log_system_information();
    });
}

fn install_subscriber() {
    let directives = std::env::var(LOG_ENV_VAR)
        .ok()
        .and_then(|value| with_default_level(&value))
        .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned());
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(directives);
    let installed = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(use_colors())
        .with_span_events(FmtSpan::CLOSE)
        // A write to standard error can fail: its reader exited, the terminal closed. By default the
        // subscriber then reports the failure with `eprintln!`, which panics on the same error, and
        // since the panic hook logs through the subscriber the process aborts. The events are lost
        // either way; the engine must go on.
        .log_internal_errors(false)
        .try_init();
    if let Err(error) = installed {
        tracing::debug!(%error, "the process already has a log subscriber or logger, keeping it");
    }
}

/// The directives of `value`, with `info,` in front when none of them is a bare level, or `None`
/// when it has no directive. `EnvFilter` otherwise records nothing from the modules that the
/// directives do not name, and reads a misspelt level as the name of a module, which would silence
/// everything.
///
/// Spaces around a directive and empty directives are dropped first. `EnvFilter` would read a
/// level with a space around it as the name of a module, and skips an empty directive, which
/// `LevelFilter` parses as a level: either way, the value would have no bare level left.
fn with_default_level(value: &str) -> Option<String> {
    let directives: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|directive| !directive.is_empty())
        .collect();
    if directives.is_empty() {
        return None;
    }
    let has_bare_level = directives
        .iter()
        .any(|directive| directive.parse::<LevelFilter>().is_ok());
    let directives = directives.join(",");
    Some(if has_bare_level {
        directives
    } else {
        format!("info,{directives}")
    })
}

/// Escape sequences only make sense on a terminal that interprets them, and `NO_COLOR`
/// (<https://no-color.org>) turns them off when it is set to a non-empty value. On Windows, only
/// Windows Terminal, which sets `WT_SESSION`, is assumed to interpret them: the console host that
/// `cmd.exe` opens on older systems prints them as text unless a program turns that on.
fn use_colors() -> bool {
    std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && (!cfg!(windows) || std::env::var_os("WT_SESSION").is_some())
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log_panic(info);
        previous(info);
    }));
}

fn log_panic(info: &PanicHookInfo<'_>) {
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("<unnamed>");
    let message = info
        .payload_as_str()
        .unwrap_or("<panic payload is not a string>");
    match info.location() {
        Some(location) => tracing::error!(thread = %thread, %location, "panic: {message}"),
        None => tracing::error!(thread = %thread, "panic: {message}"),
    }
}

fn log_system_information() {
    let cpus = std::thread::available_parallelism()
        .map_or_else(|_| "unknown".to_owned(), |count| count.to_string());
    tracing::info!(
        version = %env!("CARGO_PKG_VERSION"),
        os = %std::env::consts::OS,
        arch = %std::env::consts::ARCH,
        cpus = %cpus,
        "SlimeEngine starting"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_without_a_bare_level_get_info_for_the_other_modules() {
        for value in [
            "slime_app=trace",
            "slime_app",
            "garbage",
            "debg,slime_gfx=warn",
        ] {
            assert_eq!(with_default_level(value), Some(format!("info,{value}")));
        }
        for value in [
            "debug",
            "DEBUG",
            "off",
            "3",
            "warn,slime_app=trace",
            "slime_app=trace,error",
        ] {
            assert_eq!(with_default_level(value).as_deref(), Some(value));
        }
    }

    #[test]
    fn stray_commas_and_spaces_are_dropped() {
        for (value, directives) in [
            ("slime_app=trace,", "info,slime_app=trace"),
            (",slime_app=trace", "info,slime_app=trace"),
            ("slime_app=trace,,", "info,slime_app=trace"),
            (" debug", "debug"),
            ("debug ", "debug"),
            ("slime_app=trace, debug", "slime_app=trace,debug"),
            ("warn, slime_app=trace", "warn,slime_app=trace"),
        ] {
            assert_eq!(
                with_default_level(value).as_deref(),
                Some(directives),
                "{value:?}"
            );
        }
        for value in ["", " ", ",", " , "] {
            assert_eq!(with_default_level(value), None, "{value:?}");
        }
    }
}
