//! Logging and panic reporting for engine applications.

use std::io::IsTerminal;
use std::panic::{self, PanicHookInfo};
use std::sync::Once;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

use crate::trace::{TRACE_ENV_VAR, TraceLayer};

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
///   timestamp, the level and the module that emitted them, filtered by [`LOG_ENV_VAR`]. Colors are
///   used only when standard error is a terminal and `NO_COLOR` is unset or empty, and on Windows
///   only when the console accepts to interpret the escape sequences, which Windows Terminal does
///   already and the older console host does once asked. Records that dependencies emit through the
///   `log` crate reach the same subscriber. When a span that the filter lets through closes, a line
///   records how long it was busy and idle, which makes the profiling spans of the main loop
///   visible with `SLIME_LOG=slime_app=trace,slime_gfx=trace`. That line is written while the
///   enclosing span is still entered, so a span's busy time includes writing the lines of the spans
///   it contains; a bare `trace` also turns on wgpu's own trace logs, a line per command of each
///   draw, whose writing counts in the spans too. An event that cannot be written, because the
///   reader of standard error exited or the terminal closed, is dropped and the process goes on.
/// - When [`TRACE_ENV_VAR`] names a file, the subscriber also writes into it a trace of every span
///   that the process enters and leaves, whatever the log filter keeps, which profilers read; a
///   file that cannot be created is reported in the log, and no trace is written.
/// - If the process already has a global subscriber, for example because the game installed its
///   own, that subscriber is kept and receives the engine's events, and no trace is written: the
///   file that [`TRACE_ENV_VAR`] names is created, without a span, and a warning says why. A
///   `log` logger installed before this call keeps receiving the records that dependencies emit
///   through `log`.
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

/// The message of `error` followed by the message of each of its causes in turn, as
/// [`std::error::Error::source`] gives them, joined by `": "`: "error: cause: cause of cause".
///
/// An error's own message is its most general one, and by convention does not repeat its causes,
/// which often say what to do (no GPU adapter, a missing file). Some errors write their cause into
/// their message all the same, as a cooked file's errors write what is wrong with it after its
/// path: a cause that the chain already holds is not written again. [`run`](crate::run) logs a
/// failed run with this message; a program that does not use `run` logs its errors with it too.
///
/// ```
/// use slime_app::{RunError, error_chain};
///
/// let failed = RunError::Init("the save file is missing".into());
/// assert_eq!(
///     error_chain(&failed),
///     "the application failed to start: the save file is missing"
/// );
/// let missing = std::io::Error::other("the save file is missing");
/// assert_eq!(error_chain(&missing), "the save file is missing");
/// ```
pub fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let message = cause.to_string();
        if !chain.contains(&message) {
            chain.push_str(": ");
            chain.push_str(&message);
        }
        source = cause.source();
    }
    chain
}

fn install_subscriber() {
    let directives = std::env::var(LOG_ENV_VAR)
        .ok()
        .and_then(|value| with_default_level(&value))
        .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned());
    let filter = |directives: &str| {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse_lossy(directives)
    };
    let log = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(use_colors())
        .with_span_events(FmtSpan::CLOSE)
        // A write to standard error can fail: its reader exited, the terminal closed. By default
        // the subscriber then reports the failure with `eprintln!`, which panics on the same
        // error, and since the panic hook logs through the subscriber the process aborts. The
        // events are lost either way; the engine must go on.
        .log_internal_errors(false)
        // The log's filter is its own, so that the trace holds the spans that the log leaves out.
        .with_filter(filter(&directives));
    // What the log writes, kept for the tools that read it through the development channel: in a
    // run that a tool started with a token only, so that another run formats its events once.
    #[cfg(feature = "development")]
    let kept = std::env::var_os(slime_channel::TOKEN_ENV_VAR)
        .is_some()
        .then(|| {
            tracing_subscriber::fmt::layer()
                .with_writer(crate::log_lines::Kept)
                .with_ansi(false)
                .without_time()
                .log_internal_errors(false)
                .with_filter(filter(&directives))
        });
    #[cfg(not(feature = "development"))]
    let kept = None::<tracing_subscriber::layer::Identity>;
    let trace_path = std::env::var_os(TRACE_ENV_VAR).filter(|path| !path.is_empty());
    let trace_file = trace_path
        .as_ref()
        .map(|path| (path, std::fs::File::create(path)));
    let mut unwritable = None;
    // The trace takes spans only: the events that the log leaves out, the records of dependencies
    // among them, are then dropped before they are made.
    let trace = match trace_file {
        Some((_, Ok(file))) => {
            Some(TraceLayer::new(file).with_filter(filter_fn(|metadata| metadata.is_span())))
        }
        Some((path, Err(error))) => {
            unwritable = Some((path.clone(), error));
            None
        }
        None => None,
    };
    let installed = tracing_subscriber::registry()
        .with(log)
        .with(kept)
        .with(trace)
        .try_init();
    if let Err(error) = installed {
        tracing::debug!(%error, "the process already has a log subscriber or logger, keeping it");
        if trace_path.is_some() {
            tracing::warn!("no trace is written: the process already had a subscriber");
        }
    }
    if let Some((path, error)) = unwritable {
        tracing::error!(
            %error,
            "no trace is written: {} cannot be created",
            std::path::Path::new(&path).display()
        );
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
/// (<https://no-color.org>) turns them off when it is set to a non-empty value. On Windows, the
/// console is asked to interpret them, which Windows Terminal does already and the older console
/// host does once asked: colors are used only when it accepts.
fn use_colors() -> bool {
    std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && console_interprets_escape_sequences()
}

/// Whether the console of the process interprets escape sequences: always on other systems than
/// Windows, where terminals do.
#[cfg(not(windows))]
fn console_interprets_escape_sequences() -> bool {
    true
}

/// Whether the console of the process interprets escape sequences, after asking it to: Windows
/// Terminal does already, the older console host does once asked, and the request fails without a
/// console (`nu-ansi-term` turns on `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on `CONOUT$`).
#[cfg(windows)]
fn console_interprets_escape_sequences() -> bool {
    nu_ansi_term::enable_ansi_support().is_ok()
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
    use crate::RunError;

    /// An error that writes its cause into its own message too, as a cooked file's errors do.
    #[derive(Debug, thiserror::Error)]
    #[error("cooked/a.mesh: {source}")]
    struct Damaged {
        source: std::io::Error,
    }

    #[test]
    fn a_cause_that_an_error_writes_into_its_message_is_not_written_again() {
        let damaged = || Damaged {
            source: std::io::Error::other("a text is not UTF-8"),
        };
        assert_eq!(
            error_chain(&damaged()),
            "cooked/a.mesh: a text is not UTF-8"
        );
        assert_eq!(
            error_chain(&RunError::App(Box::new(damaged()))),
            "the application stopped with an error: cooked/a.mesh: a text is not UTF-8"
        );
    }

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
