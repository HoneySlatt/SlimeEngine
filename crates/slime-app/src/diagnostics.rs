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
/// without a level of its own records `info` and above. Invalid directives are reported on standard
/// error and ignored. When the variable is unset or empty, [`DEFAULT_LOG_FILTER`] applies.
pub const LOG_ENV_VAR: &str = "SLIME_LOG";

/// The log filter used when [`LOG_ENV_VAR`] is unset or empty: `info` and above, except for
/// wgpu-hal, whose `info` messages list what the GPU supports (`warn` and above), and for the
/// wgpu-hal module that relays the messages of the Vulkan loader and layers (`error` only)
/// (ADR-0019).
pub const DEFAULT_LOG_FILTER: &str = "info,wgpu_hal=warn,wgpu_hal::vulkan::instance=error";

/// Sets up logging and panic reporting for the process, then logs a line with the engine version,
/// the operating system, the CPU architecture and the number of logical CPUs available to the
/// process.
///
/// - Installs a global [`tracing`] subscriber that writes events to standard error with a
///   timestamp, the level and the module that emitted them, filtered by [`LOG_ENV_VAR`]. Colors
///   are used only when standard error is a terminal and `NO_COLOR` is unset or empty. Records that
///   dependencies emit through the `log` crate reach the same subscriber. When a span that the
///   filter lets through closes, a line records how long it was busy and idle, which makes the
///   profiling spans of the main loop visible with `SLIME_LOG=trace`.
/// - If the process already has a global subscriber, for example because the game installed its
///   own, that subscriber is kept and receives the engine's events.
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
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned());
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(directives);
    let installed = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(use_colors())
        .with_span_events(FmtSpan::CLOSE)
        .try_init();
    if installed.is_err() {
        tracing::debug!("a global log subscriber already exists, keeping it");
    }
}

/// Escape sequences only make sense on a terminal, and `NO_COLOR` (<https://no-color.org>) turns
/// them off when it is set to a non-empty value.
fn use_colors() -> bool {
    std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
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
