//! A trace of the spans that the process enters and leaves, which profilers read
//! (`docs/adr/0104-the-build-ci-and-measurements.md`).

use std::cell::Cell;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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
/// submission of commands to the GPU (`submit`), winit's calls, and any span that the game enters;
/// and, on a track of its own, each interval from a person's input to the end of the present of the
/// image after it, `input to present` (`docs/adr/0189-first-measures-of-time.md`). The renderer's
/// passes and the tasks of loads have none of their own. A thread's events reach
/// the file each time it leaves its outermost span, at the end of each frame for the main thread:
/// the frame in progress when the process ends is not in it.
///
/// Writing the trace costs each span a lock and a few hundred bytes, which the times it shows
/// include. A trace also lets every log record of the dependencies reach the subscriber, which
/// drops those that the log leaves out, where the `log` crate dropped them at once: wgpu writes
/// several at each command of a pass, at the `trace` level. Neither cost is measured.
pub const TRACE_ENV_VAR: &str = "SLIME_TRACE";

/// A layer that writes each span entered and left as a pair of events of the Chrome Trace Event
/// format into a [`TraceFile`].
pub(crate) struct TraceLayer(pub(crate) Arc<TraceFile>);

/// A trace being written: a JSON array of events of the Chrome Trace Event format, which its
/// readers take without its closing bracket, written as the process goes, so that a process killed
/// still leaves a trace that they open. Its times count from the program's start
/// ([`since_start`](crate::since_start)).
pub(crate) struct TraceFile {
    start: Instant,
    file: Mutex<BufWriter<File>>,
    /// The number of the track that holds the intervals between an input and the present of the
    /// image after it, given at the first one.
    input_track: OnceLock<u64>,
}

/// The trace that the process writes, once its subscriber holds the layer that writes its spans.
static TRACE: OnceLock<Arc<TraceFile>> = OnceLock::new();

/// The number that the trace gives each thread, from 1, in the order they first enter a span.
static NEXT_THREAD: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// This thread's number in the trace, 0 until it is given one, and how many spans it is in.
    static THREAD: Cell<(u64, usize)> = const { Cell::new((0, 0)) };
}

impl TraceFile {
    /// A trace written into `file`, which it starts with the array's opening bracket, its times
    /// counted from `start`.
    pub(crate) fn new(file: File, start: Instant) -> Self {
        let mut file = BufWriter::new(file);
        // A trace that cannot be written is lost, as a log line is: the process goes on.
        let _ = file.write_all(b"[\n");
        Self {
            start,
            file: Mutex::new(file),
            input_track: OnceLock::new(),
        }
    }

    /// Makes `trace` the one into which [`input_presented`] writes.
    pub(crate) fn set_current(trace: Arc<Self>) {
        let _ = TRACE.set(trace);
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
        self.append(&events, depth == 0);
    }

    /// Writes the interval from an input's arrival, at `input`, to the end of the present of the
    /// first image after it, at `presented`, as a complete event (`X`) on a track of its own, which
    /// a profiler shows apart from the threads' spans, which it would overlap; flushes the file.
    fn input_interval(&self, input: Instant, presented: Instant) {
        let mut events = String::new();
        let track = *self.input_track.get_or_init(|| {
            let track = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
            let _ = writeln!(
                events,
                "{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{track},\
                 \"args\":{{\"name\":\"{INPUT_TO_PRESENT}\"}}}},"
            );
            track
        });
        let micros =
            |instant: Instant| instant.saturating_duration_since(self.start).as_secs_f64() * 1e6;
        let start = micros(input);
        let duration = micros(presented) - start;
        let _ = writeln!(
            events,
            "{{\"name\":\"{INPUT_TO_PRESENT}\",\"cat\":\"slime_app\",\"ph\":\"X\",\
             \"ts\":{start:.3},\"dur\":{duration:.3},\"pid\":1,\"tid\":{track}}},"
        );
        self.append(&events, true);
    }

    /// Appends `events` to the file, flushing it if `flush`.
    fn append(&self, events: &str, flush: bool) {
        // A writer that panicked left the file as whole as any other: the trace goes on.
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = file.write_all(events.as_bytes());
        if flush {
            let _ = file.flush();
        }
    }
}

/// The name of the intervals from an input to the present of the image after it, and of their
/// track (`docs/adr/0189-first-measures-of-time.md`).
const INPUT_TO_PRESENT: &str = "input to present";

/// Writes into the process's trace, if it writes one, the interval from the arrival of the oldest
/// input that no image presented yet followed, at `input`, to the end of the present of the image
/// after it, at `presented`.
pub(crate) fn input_presented(input: Instant, presented: Instant) {
    if let Some(trace) = TRACE.get() {
        trace.input_interval(input, presented);
    }
}

impl<S> Layer<S> for TraceLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_enter(&self, id: &span::Id, context: Context<'_, S>) {
        if let Some(span) = context.span(id) {
            self.0.write(span.name(), span.metadata().target(), 'B');
        }
    }

    fn on_exit(&self, id: &span::Id, context: Context<'_, S>) {
        if let Some(span) = context.span(id) {
            self.0.write(span.name(), span.metadata().target(), 'E');
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
    use std::time::{Duration, Instant};

    use super::{TraceFile, escaped};

    #[test]
    fn an_input_to_present_is_a_complete_event_on_a_track_of_its_own() {
        let path =
            std::env::temp_dir().join(format!("slime-app-trace-{}.json", std::process::id()));
        let start = Instant::now();
        let trace = TraceFile::new(std::fs::File::create(&path).expect("a file"), start);
        let at = |micros| start + Duration::from_micros(micros);
        trace.input_interval(at(1_000), at(3_500));
        trace.input_interval(at(10_000), at(11_000));
        let written = std::fs::read_to_string(&path).expect("the trace is written");
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 4, "{written}");
        assert_eq!(lines[0], "[");
        let track = lines[1]
            .strip_prefix("{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":")
            .and_then(|rest| rest.strip_suffix(",\"args\":{\"name\":\"input to present\"}},"))
            .expect("the track named once");
        for (line, (ts, dur)) in lines[2..]
            .iter()
            .zip([("1000.000", "2500.000"), ("10000.000", "1000.000")])
        {
            assert_eq!(
                *line,
                format!(
                    "{{\"name\":\"input to present\",\"cat\":\"slime_app\",\"ph\":\"X\",\
                     \"ts\":{ts},\"dur\":{dur},\"pid\":1,\"tid\":{track}}},"
                )
            );
        }
    }

    #[test]
    fn names_are_escaped_as_json_strings() {
        assert_eq!(escaped("frame"), "frame");
        assert_eq!(escaped("a \"b\" \\ c\n"), "a \\\"b\\\" \\\\ c\\u000a");
    }
}
