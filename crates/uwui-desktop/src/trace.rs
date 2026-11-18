//! The host's trace, which [`TRACE_ENV_VAR`] asks for
//! (`docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md`):
//! each frame, when its image showed, and each interval from a person's input to the present of the
//! image after it, which `scripts/host-cost.py` reads.
//!
//! The file is a JSON array of events in the Chrome Trace Event format, which Perfetto and
//! Chrome's `chrome://tracing` open, as the engine's traces are. It is written as the run goes,
//! one write after each frame's present and one as the platform tells when an image showed,
//! without its closing bracket, so that a reader finds the frames of a program still running;
//! nothing is written at rest. Its times are microseconds since
//! the Unix epoch: the system's time read once as the trace opens, together with the host's clock,
//! and followed on the host's clock after, so that a tool that noted when it started the program
//! reads its start from the first frame's end.
//!
//! The blank window on the platform layer, `examples/blank.rs`, writes its trace with this same
//! file: it depends on nothing of the host's but this writer.

use std::fs::File;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use uwui::time::Instant;

/// The environment variable that names the file into which the host writes its trace. Unset or
/// empty, nothing is written.
///
/// The file is a JSON array of events in the Chrome Trace Event format, without its closing
/// bracket, its times in microseconds since the Unix epoch:
///
/// - `frame`, from a frame's start to the end of its image's present, its arguments `wheel` the
///   notches of the wheel that the frame took since the frame before, and `image` its image's
///   number, counted from 0 as the platform layer counts them;
/// - `shown`, an instant event when the image numbered by its argument `image` showed, where the
///   platform tells it (on Wayland, `wp_presentation`): nothing for an image that never showed;
/// - `missed`, from the time given to a frame to when its image numbered `image` showed, half a
///   period or more later: the frame missed the refresh that it was drawn for; the host's alone;
/// - `input to present`, from the time of the oldest input that no image followed yet to the end
///   of the present of the next image. An input's time is the platform's own where it gives one,
///   placed on the host's clock, else its reception: on Wayland, the compositor's, to the
///   millisecond, so that an interval may read up to a millisecond long. An input that the
///   interface answers with no image is not counted, nor one before the first image, nor one
///   across a device or a surface made again.
///
/// Each frame's times are read before its write to the file, which they do not include.
pub const TRACE_ENV_VAR: &str = "UWUI_TRACE";

/// The track of the frames.
const FRAMES: u32 = 1;
/// The track of the intervals from an input to the present of the image after it.
const INPUTS: u32 = 2;
/// The track of the images' presentations: when each showed, and the frames that missed.
const SHOWN: u32 = 3;

/// A trace being written.
pub(crate) struct Trace {
    file: File,
    /// The host's clock when the trace opened.
    origin: Instant,
    /// The system's time then, in nanoseconds since the Unix epoch.
    origin_since_epoch: u128,
    /// The time of the oldest input that no image presented followed yet.
    waiting: Option<Instant>,
    /// The notches of the wheel taken since the last image presented.
    wheel: u32,
    /// Whether an image was presented yet: inputs before the first are part of the start.
    shown: bool,
}

impl Trace {
    /// The trace that [`TRACE_ENV_VAR`] names, its times anchored at `now` on the host's clock:
    /// none where it names none, else the trace or why its file cannot be made.
    pub(crate) fn from_env(now: Instant) -> Option<io::Result<Self>> {
        let path = std::env::var_os(TRACE_ENV_VAR).filter(|path| !path.is_empty())?;
        Some(Self::create(Path::new(&path), now, SystemTime::now()))
    }

    /// Makes the trace at `path`, `now` on the host's clock being `wall` on the system's.
    fn create(path: &Path, now: Instant, wall: SystemTime) -> io::Result<Self> {
        let mut file = File::create(path)?;
        file.write_all(
            format!(
                "[\n{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{FRAMES},\
                 \"args\":{{\"name\":\"frames\"}}}},\n\
                 {{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{INPUTS},\
                 \"args\":{{\"name\":\"input to present\"}}}},\n\
                 {{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{SHOWN},\
                 \"args\":{{\"name\":\"shown\"}}}},\n"
            )
            .as_bytes(),
        )?;
        Ok(Self {
            file,
            origin: now,
            origin_since_epoch: wall
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            waiting: None,
            wheel: 0,
            shown: false,
        })
    }

    /// A person's input of time `at` reached the host, `wheel` notches of the wheel among it: the
    /// next image presented follows it, unless an older input waits.
    pub(crate) fn input(&mut self, at: Instant, wheel: u32) {
        self.wheel = self.wheel.saturating_add(wheel);
        if self.shown {
            let oldest = self.waiting.map_or(at, |waiting| waiting.min(at));
            self.waiting = Some(oldest);
        }
    }

    /// The inputs that wait are answered by no image of their own: the interface showed nothing
    /// of them, or the device or the surface is made again. None of them is counted.
    pub(crate) fn unanswered(&mut self) {
        self.waiting = None;
    }

    /// A frame started at `start` and its image numbered `image`'s present ended at `end`: writes
    /// the frame, and the interval from the oldest input that waited.
    ///
    /// # Errors
    ///
    /// When the file refuses the write.
    pub(crate) fn presented(&mut self, start: Instant, end: Instant, image: u64) -> io::Result<()> {
        let wheel = std::mem::take(&mut self.wheel);
        let mut events = self.complete("frame", FRAMES, start, end);
        with_args(&mut events, &format!("\"wheel\":{wheel},\"image\":{image}"));
        if let Some(input) = self.waiting.take() {
            events.push_str(&self.complete("input to present", INPUTS, input, end));
        }
        self.shown = true;
        self.file.write_all(events.as_bytes())
    }

    /// The image numbered `image` showed `at`.
    ///
    /// # Errors
    ///
    /// When the file refuses the write.
    pub(crate) fn shown(&mut self, image: u64, at: Instant) -> io::Result<()> {
        let mut event = format!(
            "{{\"name\":\"shown\",\"cat\":\"uwui_desktop\",\"ph\":\"i\",\"s\":\"t\",\"ts\":{},\
             \"pid\":1,\"tid\":{SHOWN}}},\n",
            micros(self.since_epoch(at)),
        );
        with_args(&mut event, &format!("\"image\":{image}"));
        self.file.write_all(event.as_bytes())
    }

    /// The image numbered `image` showed at `shown`, half a period or more after the time `given`
    /// to its frame.
    ///
    /// # Errors
    ///
    /// When the file refuses the write.
    pub(crate) fn missed(&mut self, image: u64, given: Instant, shown: Instant) -> io::Result<()> {
        let mut event = self.complete("missed", SHOWN, given, shown);
        with_args(&mut event, &format!("\"image\":{image}"));
        self.file.write_all(event.as_bytes())
    }

    /// A complete event, a line, named `name` on track `track`, from `from` to `to`.
    fn complete(&self, name: &str, track: u32, from: Instant, to: Instant) -> String {
        let (ts, dur) = (self.since_epoch(from), to.saturating_duration_since(from));
        format!(
            "{{\"name\":\"{name}\",\"cat\":\"uwui_desktop\",\"ph\":\"X\",\"ts\":{},\"dur\":{},\
             \"pid\":1,\"tid\":{track}}},\n",
            micros(ts),
            micros(dur.as_nanos()),
        )
    }

    /// Nanoseconds since the Unix epoch at `at` on the host's clock.
    fn since_epoch(&self, at: Instant) -> u128 {
        let since = at.saturating_duration_since(self.origin).as_nanos();
        let before = self.origin.saturating_duration_since(at).as_nanos();
        (self.origin_since_epoch + since).saturating_sub(before)
    }
}

/// Gives the event `event`, a line, the arguments `args`, written as JSON's members.
fn with_args(event: &mut String, args: &str) {
    let close = event.len() - "},\n".len();
    event.insert_str(close, &format!(",\"args\":{{{args}}}"));
}

/// `nanos` as microseconds, to the nanosecond.
fn micros(nanos: u128) -> String {
    format!("{}.{:03}", nanos / 1000, nanos % 1000)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(micros: u64) -> Instant {
        Instant::from_nanos(1_000_000_000 + micros * 1000)
    }

    fn event(name: &str, ts: &str, dur: &str, track: u32, args: &str) -> String {
        let args = if args.is_empty() {
            String::new()
        } else {
            format!(",\"args\":{{{args}}}")
        };
        format!(
            "{{\"name\":\"{name}\",\"cat\":\"uwui_desktop\",\"ph\":\"X\",\"ts\":{ts},\
             \"dur\":{dur},\"pid\":1,\"tid\":{track}{args}}},"
        )
    }

    fn frame(ts: &str, dur: &str, wheel: u32, image: u64) -> String {
        event(
            "frame",
            ts,
            dur,
            1,
            &format!("\"wheel\":{wheel},\"image\":{image}"),
        )
    }

    #[test]
    fn frames_inputs_to_present_and_presentations_are_events_since_the_epoch() {
        let path = std::env::temp_dir().join(format!("uwui-trace-{}.json", std::process::id()));
        let wall = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let mut trace = Trace::create(&path, at(0), wall).expect("the trace opens");
        // An input before the first image is part of the start, not counted.
        trace.input(at(100), 0);
        trace.presented(at(200), at(700), 0).expect("written");
        // Two inputs before a frame, the second older by the platform's time: the oldest counts.
        trace.input(at(2_000), 1);
        trace.input(at(1_000), 1);
        trace.presented(at(3_000), at(4_500), 1).expect("written");
        // The first image showed; the second, later than the time given to its frame.
        trace.shown(0, at(4_167)).expect("written");
        trace.shown(1, at(12_500)).expect("written");
        trace.missed(1, at(8_333), at(12_500)).expect("written");
        // A frame that no input waited for.
        trace.presented(at(8_000), at(9_000), 2).expect("written");
        // An input answered with no image is not counted at the next frame, its notch is.
        trace.input(at(10_000), 1);
        trace.unanswered();
        trace.presented(at(20_000), at(20_250), 4).expect("written");
        drop(trace);
        let written = std::fs::read_to_string(&path).expect("the trace reads");
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 12, "{written}");
        assert_eq!(lines[0], "[");
        assert!(lines[1].contains("\"tid\":1,\"args\":{\"name\":\"frames\"}"));
        assert!(lines[2].contains("\"tid\":2,\"args\":{\"name\":\"input to present\"}"));
        assert!(lines[3].contains("\"tid\":3,\"args\":{\"name\":\"shown\"}"));
        let second = |micros: u64| format!("{}.000", 1_800_000_000_000_000 + micros);
        assert_eq!(lines[4], frame(&second(200), "500.000", 0, 0));
        assert_eq!(lines[5], frame(&second(3_000), "1500.000", 2, 1));
        assert_eq!(
            lines[6],
            event("input to present", &second(1_000), "3500.000", 2, "")
        );
        let shown = |micros: u64, image: u64| {
            format!(
                "{{\"name\":\"shown\",\"cat\":\"uwui_desktop\",\"ph\":\"i\",\"s\":\"t\",\
                 \"ts\":{},\"pid\":1,\"tid\":3,\"args\":{{\"image\":{image}}}}},",
                second(micros)
            )
        };
        assert_eq!(lines[7], shown(4_167, 0));
        assert_eq!(lines[8], shown(12_500, 1));
        assert_eq!(
            lines[9],
            event("missed", &second(8_333), "4167.000", 3, "\"image\":1")
        );
        assert_eq!(lines[10], frame(&second(8_000), "1000.000", 0, 2));
        assert_eq!(lines[11], frame(&second(20_000), "250.000", 1, 4));
        // Every line, its comma taken off, is JSON's object.
        for line in &lines[1..] {
            assert!(line.starts_with('{') && line.ends_with("},"), "{line}");
        }
    }

    #[test]
    fn a_time_before_the_trace_opened_stays_on_the_epochs_line() {
        let path = std::env::temp_dir().join(format!("uwui-trace-b-{}.json", std::process::id()));
        let wall = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let trace = Trace::create(&path, at(5_000), wall).expect("the trace opens");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            trace.since_epoch(at(4_000)),
            1_800_000_000_000_000_000 - 1_000_000
        );
    }

    #[test]
    fn microseconds_keep_the_nanoseconds() {
        assert_eq!(micros(1_234_567), "1234.567");
        assert_eq!(micros(5), "0.005");
    }
}
