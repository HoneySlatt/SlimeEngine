//! The last lines of the log, kept for the tools that read them through the development channel.

use std::collections::VecDeque;
use std::io;
use std::sync::{Mutex, PoisonError};

use slime_channel::LogLine;
use slime_channel::page::{Page, Pager, cut};
use tracing_subscriber::fmt::MakeWriter;

/// How many lines are kept: the oldest go first.
const KEPT: usize = slime_channel::MAX_LOG_LINES;

/// The lines kept, each with its number and whether it was cut, and the number of the last one.
static LINES: Mutex<(u64, VecDeque<LogLine>)> = Mutex::new((0, VecDeque::new()));

/// What a line takes in JSON besides its text, at most: its number, its fields' names, the mark of
/// a cut and the comma between lines.
const LINE_OVERHEAD: usize = 64;

/// A page of the lines kept after the line numbered `after`, oldest first, each cut to fit `page`
/// alone, as a line is its item's only text:
/// the lines, and whether lines kept after them were left for the next page. Only the lines of
/// the page are copied, under the lock that the log's writers take.
pub(crate) fn page(after: u64, page: Page) -> (Vec<LogLine>, bool) {
    let lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    page_of(&lines.1, after, page)
}

/// A page of `lines` after the line numbered `after`, as [`page`] gives it.
fn page_of(lines: &VecDeque<LogLine>, after: u64, page: Page) -> (Vec<LogLine>, bool) {
    // The lines are kept in the order of their numbers.
    let first = lines.partition_point(|line| line.number <= after);
    let mut pager = Pager::new(page, || false);
    let mut taken = Vec::new();
    for kept in lines.range(first..) {
        let mut line = kept.clone();
        line.cut |= cut(&mut line.text, page.bytes.saturating_sub(LINE_OVERHEAD));
        if !pager.take(&line) {
            return (taken, true);
        }
        taken.push(line);
    }
    (taken, false)
}

/// Keeps `text`'s lines, each numbered one more than the last, each cut to
/// [`MAX_LOG_LINE`](slime_channel::MAX_LOG_LINE) bytes, on a character's boundary.
fn keep(text: &str) {
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    for line in text.lines().filter(|line| !line.is_empty()) {
        lines.0 += 1;
        let number = lines.0;
        let (text, cut) = cut_to(line, slime_channel::MAX_LOG_LINE);
        lines.1.push_back(LogLine {
            number,
            text: text.to_owned(),
            cut,
        });
        if lines.1.len() > KEPT {
            lines.1.pop_front();
        }
    }
}

/// `line`, cut to at most `bytes` bytes on a character's boundary, and whether it was cut.
fn cut_to(line: &str, bytes: usize) -> (&str, bool) {
    if line.len() <= bytes {
        return (line, false);
    }
    let mut end = bytes;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    (&line[..end], true)
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

    /// A page that holds them all.
    const WHOLE: Page = Page {
        program: 1,
        items: 1000,
        bytes: 1 << 20,
    };

    #[test]
    fn the_newest_lines_are_kept_numbered_and_cut() {
        let before = page(0, WHOLE).0.last().map_or(0, |line| line.number);
        // Three bytes a character: the cut falls inside one, and steps back to its start.
        let long = "日".repeat(slime_channel::MAX_LOG_LINE);
        keep(&format!("one\ntwo\n{long}\n"));
        let (kept, _) = page(before, WHOLE);
        let texts: Vec<&str> = kept.iter().take(2).map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["one", "two"]);
        assert_eq!(kept[1].number, kept[0].number + 1);
        assert!(!kept[0].cut);
        // The most whole characters that fit.
        let most = slime_channel::MAX_LOG_LINE / 3 * 3;
        assert!(
            kept[2].cut && kept[2].text.len() == most,
            "{}",
            kept[2].text.len()
        );
        assert!(long.starts_with(&kept[2].text));
        for _ in 0..KEPT {
            keep("more");
        }
        let (all, partial) = page(0, WHOLE);
        assert_eq!((all.len(), partial), (KEPT, false), "the oldest went");
    }

    /// Lines long enough that a page holds a few are read by pages, each after the last given,
    /// and a line longer than a page is cut to fit it.
    #[test]
    fn long_lines_are_read_by_pages_of_their_bytes() {
        let lines: VecDeque<LogLine> = (1..=20)
            .map(|number| LogLine {
                number,
                text: format!("{number} {}", "\"".repeat(100 << 10)),
                cut: false,
            })
            .collect();
        let page = Page {
            program: 1,
            items: 1000,
            bytes: 64 << 10,
        };
        let (mut after, mut read, mut pages) = (0, Vec::new(), 0);
        loop {
            let (lines, partial) = page_of(&lines, after, page);
            pages += 1;
            // Each `"` written as two bytes of JSON.
            let written: usize = lines.iter().map(|line| 2 * line.text.len()).sum();
            assert!(written <= page.bytes, "within the page");
            assert!(!lines.is_empty(), "each page gives a line");
            after = lines.last().expect("a line").number;
            read.extend(lines);
            if !partial {
                break;
            }
            assert!(pages < 100, "pages that go on");
        }
        let numbers: Vec<u64> = read.iter().map(|line| line.number).collect();
        assert_eq!(numbers, (1..=20).collect::<Vec<_>>());
        assert!(read.iter().all(|line| line.cut), "each cut to fit");
        assert!(
            read.iter().all(|line| 2 * line.text.len() > page.bytes / 2),
            "each cut to its page, not to a part of it"
        );
        assert!(pages > 1);
    }
}
