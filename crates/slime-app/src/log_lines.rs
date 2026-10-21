//! The last lines of the log, kept for the tools that read them through the development channel.

use std::collections::VecDeque;
use std::io;
use std::sync::{Mutex, PoisonError};

use tracing_subscriber::fmt::MakeWriter;

/// How many lines are kept: the oldest go first.
const KEPT: usize = slime_channel::MAX_LOG_LINES;

/// The lines kept, each with its number, and the number of the last one.
static LINES: Mutex<(u64, VecDeque<(u64, String)>)> = Mutex::new((0, VecDeque::new()));

/// The lines kept after the line numbered `after`, oldest first.
pub(crate) fn after(after: u64) -> Vec<(u64, String)> {
    let lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    lines
        .1
        .iter()
        .filter(|(number, _)| *number > after)
        .cloned()
        .collect()
}

/// Keeps `text`'s lines, each numbered one more than the last.
fn keep(text: &str) {
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    for line in text.lines().filter(|line| !line.is_empty()) {
        lines.0 += 1;
        let number = lines.0;
        lines.1.push_back((number, line.to_owned()));
        if lines.1.len() > KEPT {
            lines.1.pop_front();
        }
    }
}

/// What the log's layer writes its events into, to keep them.
pub(crate) struct Kept;

/// One event, as the log formats it, kept once it is written whole.
pub(crate) struct Event(Vec<u8>);

impl<'a> MakeWriter<'a> for Kept {
    type Writer = Event;

    fn make_writer(&'a self) -> Self::Writer {
        Event(Vec::new())
    }
}

impl io::Write for Event {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        keep(&String::from_utf8_lossy(&self.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_lines_are_kept_numbered() {
        let before = after(0).last().map_or(0, |(number, _)| *number);
        keep("one\ntwo\n");
        let kept = after(before);
        let texts: Vec<&str> = kept.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(texts, ["one", "two"]);
        assert_eq!(kept[1].0, kept[0].0 + 1);
        for _ in 0..KEPT {
            keep("more");
        }
        assert_eq!(after(0).len(), KEPT, "the oldest went");
    }
}
