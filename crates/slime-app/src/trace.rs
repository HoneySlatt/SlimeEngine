//! A trace of the spans that the process enters and leaves, which profilers read
//! (`docs/adr/0104-the-build-ci-and-measurements.md`).

use std::cell::Cell;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use tracing::span;
use tracing::subscriber::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// The environment variable that names the file into which the process writes a trace of the
/// spans it enters and leaves, every thread's, in the Chrome Trace Event format, which Perfetto
/// (<https://ui.perfetto.dev>, which reads the file in the browser, without sending it anywhere)
/// and Chrome's `chrome://tracing` open.
///
/// [`init_diagnostics`](crate::init_diagnostics) reads it. Unset or empty, nothing is written. The
/// trace holds every span, the `trace` level included, whatever [`LOG_ENV_VAR`](crate::LOG_ENV_VAR)
/// keeps for the log: each frame of a run with its phases (see [`run`](crate::run)), each
/// submission of commands to the GPU (`submit`), winit's calls, and any span that the game enters.
/// The renderer's passes and the tasks of loads have none of their own. A thread's events reach
/// the file each time it leaves its outermost span, at the end of each frame for the main thread:
/// the frame in progress when the process ends is not in it.
///
/// Writing the trace costs each span a lock and a few hundred bytes, which the times it shows
/// include. A trace also lets every log record of the dependencies reach the subscriber, which
/// drops those that the log leaves out, where the `log` crate dropped them at once: wgpu writes
/// several at each command of a pass, at the `trace` level. Neither cost is measured.
pub const TRACE_ENV_VAR: &str = "SLIME_TRACE";

/// A layer that writes each span entered and left as a pair of events of the Chrome Trace Event
/// format: a JSON array that its readers take without its closing bracket, written as the process
/// goes, so that a process killed still leaves a trace that they open.
pub(crate) struct TraceLayer {
    start: Instant,
    file: Mutex<BufWriter<File>>,
}

/// The number that the trace gives each thread, from 1, in the order they first enter a span.
static NEXT_THREAD: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// This thread's number in the trace, 0 until it is given one, and how many spans it is in.
    static THREAD: Cell<(u64, usize)> = const { Cell::new((0, 0)) };
}

impl TraceLayer {
    /// A layer that writes into `file`, which it starts with the array's opening bracket.
    pub(crate) fn new(file: File) -> Self {
        let mut file = BufWriter::new(file);
        // A trace that cannot be written is lost, as a log line is: the process goes on.
        let _ = file.write_all(b"[\n");
        Self {
            start: Instant::now(),
            file: Mutex::new(file),
        }
    }

    /// Writes the event `phase` (`B` for a span entered, `E` for one left) of the span `name` of
    /// `target`, on the current thread; flushes the file when the thread leaves its outermost span.
    fn write(&self, name: &str, target: &str, phase: char) {
        let micros = self.start.elapsed().as_secs_f64() * 1e6;
        let (mut thread, depth) = THREAD.get();
        let mut events = String::new();
        if thread == 0 {
            thread = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
            let current = std::thread::current();
            let thread_name = current.name().unwrap_or("unnamed");
            let _ = writeln!(
                events,
                "{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{thread},\
                 \"args\":{{\"name\":\"{}\"}}}},",
                escaped(thread_name)
            );
        }
        let depth = if phase == 'B' {
            depth + 1
        } else {
            depth.saturating_sub(1)
        };
        THREAD.set((thread, depth));
        let _ = writeln!(
            events,
            "{{\"name\":\"{}\",\"cat\":\"{}\",\"ph\":\"{phase}\",\"ts\":{micros:.3},\"pid\":1,\
             \"tid\":{thread}}},",
            escaped(name),
            escaped(target)
        );
        // A writer that panicked left the file as whole as any other: the trace goes on.
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = file.write_all(events.as_bytes());
        if depth == 0 {
            let _ = file.flush();
        }
    }
}

impl<S> Layer<S> for TraceLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_enter(&self, id: &span::Id, context: Context<'_, S>) {
        if let Some(span) = context.span(id) {
            self.write(span.name(), span.metadata().target(), 'B');
        }
    }

    fn on_exit(&self, id: &span::Id, context: Context<'_, S>) {
        if let Some(span) = context.span(id) {
            self.write(span.name(), span.metadata().target(), 'E');
        }
    }
}

/// `text` as the inside of a JSON string.
fn escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            control if control < ' ' => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(control));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::escaped;

    #[test]
    fn names_are_escaped_as_json_strings() {
        assert_eq!(escaped("frame"), "frame");
        assert_eq!(escaped("a \"b\" \\ c\n"), "a \\\"b\\\" \\\\ c\\u000a");
    }
}
