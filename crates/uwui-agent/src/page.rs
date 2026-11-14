//! A long list read by pages: what bounds a page, the cursor to the rest, and texts cut to fit
//! (`docs/adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md`).
//!
//! A request of a list gives `cursor`, none for its start, and may ask a smaller page, `limit`
//! items and `bytes` of JSON. Its answer gives `next` while the list may go on: the cursor to ask
//! for the rest, an opaque text. A page may be short, or empty, and still give `next`, as its work
//! is bounded by time.
//!
//! A cursor is bound to the process that made it, by a number that its carrier draws
//! ([`Page::program`]), to the *maker* that made it, a world, an interface or a journal, by the
//! maker's number, and to its list and its filter. One of another process or maker faults
//! [`Fault::STALE`]: the tool starts the list again. One that does not read, or of another list
//! or filter, faults [`Fault::BAD_PARAMS`].

use std::hash::{DefaultHasher, Hash, Hasher};

use serde::Serialize;

use crate::message::Fault;
use crate::typed::json_size;

/// The most items that a page gives.
pub const MAX_ITEMS: usize = 1000;

/// The most bytes of JSON that a page's items take.
pub const MAX_BYTES: usize = 1 << 20;

/// The bytes of an answer's line that a page leaves for what is not its items: the envelope, the
/// reply's own fields and `next`. A cursor of elements grows with the depth of the tree, some 15
/// bytes a level: past some 250 levels, a page on a line under [`MAX_BYTES`] and this margin may
/// pass the tool's line.
pub const MARGIN: usize = 4096;

/// The items that a page visits at least, whatever its time: a list always moves on.
pub const MIN_VISITS: usize = 64;

/// How often a page reads its clock: every this many visits, once it made [`MIN_VISITS`].
const CLOCK_EVERY: usize = 32;

/// What bounds a page of a list: given by the program's host to the code that makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    /// The number of the process, which its carrier drew once: cursors of another process are
    /// stale.
    pub program: u64,
    /// The most items that it gives.
    pub items: usize,
    /// The most bytes of JSON that its items take.
    pub bytes: usize,
}

impl Page {
    /// The most that a page holds on a connection whose tool reads lines of `line` bytes at most,
    /// in the process numbered `program`: [`MAX_ITEMS`] items, and the smaller of [`MAX_BYTES`]
    /// and the tool's line less [`MARGIN`].
    pub fn new(program: u64, line: usize) -> Self {
        Self {
            program,
            items: MAX_ITEMS,
            bytes: MAX_BYTES.min(line.saturating_sub(MARGIN)),
        }
    }

    /// The page that a request asks within this one: at most `limit` items and `bytes` of JSON,
    /// each none for this page's own. A `limit` of 0 reads as 1.
    #[must_use]
    pub fn within(self, limit: Option<u64>, bytes: Option<u64>) -> Self {
        let within = |most: usize, asked: Option<u64>| {
            asked.map_or(most, |asked| {
                usize::try_from(asked).map_or(most, |asked| asked.min(most))
            })
        };
        Self {
            program: self.program,
            items: within(self.items, limit).max(1),
            bytes: within(self.bytes, bytes),
        }
    }

    /// The most bytes of JSON that a text of an item writes, once cut ([`cut`]): an eighth of the
    /// page, so that an item of a few texts takes at most part of it.
    pub fn text_bytes(&self) -> usize {
        self.bytes / 8
    }
}

/// A page being made: it counts the items visited, given and their bytes, and says when to stop.
pub struct Pager<'a> {
    page: Page,
    time_up: Box<dyn FnMut() -> bool + 'a>,
    visits: usize,
    items: usize,
    bytes: usize,
    full: bool,
}

impl<'a> Pager<'a> {
    /// A page bounded by `page`, and by `time_up`, which says whether its time has passed: read
    /// every few visits once it made [`MIN_VISITS`].
    pub fn new(page: Page, time_up: impl FnMut() -> bool + 'a) -> Self {
        Self {
            page,
            time_up: Box::new(time_up),
            visits: 0,
            items: 0,
            bytes: 0,
            full: false,
        }
    }

    /// Whether the page may visit one more item: not once it is full, nor, after
    /// [`MIN_VISITS`], once its time has passed. Counts the visit when it may.
    pub fn visit(&mut self) -> bool {
        if self.full {
            return false;
        }
        if self.visits >= MIN_VISITS
            && (self.visits - MIN_VISITS).is_multiple_of(CLOCK_EVERY)
            && (self.time_up)()
        {
            return false;
        }
        self.visits += 1;
        true
    }

    /// Takes `item` into the page if it fits, its count and its bytes of JSON: whether it did. One
    /// that does not fit fills the page, and is left for the next. The first item of a page is
    /// always taken, as its texts are cut to fit.
    pub fn take(&mut self, item: &impl Serialize) -> bool {
        // The comma between items.
        let size = json_size(item) + usize::from(self.items > 0);
        if self.items > 0 && (self.items >= self.page.items || self.bytes + size > self.page.bytes)
        {
            self.full = true;
            return false;
        }
        self.items += 1;
        self.bytes += size;
        if self.items >= self.page.items {
            self.full = true;
        }
        true
    }

    /// How many items it visited.
    pub fn visits(&self) -> usize {
        self.visits
    }
}

/// A cursor's list and filter, which it must be given with again.
#[derive(Clone, Copy, Debug)]
pub struct Listing<'a> {
    /// The list's name, `entities`, without `:`.
    pub list: &'a str,
    /// Its filter, as the match reads it: lowercased where the match ignores the case.
    pub filter: &'a str,
}

impl Listing<'_> {
    /// The cursor to the rest of the list, after `place`, the list's own numbers, made by the
    /// maker numbered `maker` in the process numbered `program`.
    pub fn cursor(&self, program: u64, maker: u64, place: &[u64]) -> String {
        let mut text = format!(
            "{}:{program:x}:{maker:x}:{:x}:",
            self.list,
            fingerprint(self.filter)
        );
        for (index, number) in place.iter().enumerate() {
            if index > 0 {
                text.push('.');
            }
            text.push_str(&format!("{number:x}"));
        }
        text
    }

    /// The place that `cursor` gives, if it is this list's, with the same filter, of the maker
    /// numbered `maker` in the process numbered `program`.
    ///
    /// # Errors
    ///
    /// [`Fault::STALE`] for a cursor of another process or maker; [`Fault::BAD_PARAMS`] for one
    /// that does not read, or of another list or filter.
    pub fn place(&self, cursor: &str, program: u64, maker: u64) -> Result<Vec<u64>, Fault> {
        let bad = || {
            Fault::new(
                Fault::BAD_PARAMS,
                format!("`{}` is not a cursor of this list", shown(cursor)),
            )
        };
        let mut parts = cursor.split(':');
        let (Some(list), Some(of), Some(by), Some(filter), Some(place), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(bad());
        };
        let number = |text: &str| {
            (!text.is_empty() && text.len() <= 16)
                .then(|| u64::from_str_radix(text, 16).ok())
                .flatten()
        };
        let (Some(of), Some(by), Some(filter)) = (number(of), number(by), number(filter)) else {
            return Err(bad());
        };
        let place = if place.is_empty() {
            Vec::new()
        } else {
            place
                .split('.')
                .map(number)
                .collect::<Option<Vec<u64>>>()
                .ok_or_else(bad)?
        };
        if list != self.list || filter != fingerprint(self.filter) {
            return Err(bad());
        }
        if of != program || by != maker {
            return Err(Fault::new(
                Fault::STALE,
                "the list changed whole since this cursor was given, its program, world or \
                 interface another: list it again from its start",
            ));
        }
        Ok(place)
    }
}

/// A number that tells filters apart within a process.
fn fingerprint(filter: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    filter.hash(&mut hasher);
    hasher.finish()
}

/// At most the first 64 characters of a cursor given, for a fault that quotes it.
fn shown(cursor: &str) -> String {
    cursor.chars().take(64).collect()
}

/// Cuts `text` on a character's boundary so that it writes at most `bytes` bytes of JSON,
/// quotes included: whether it cut it. A text whose quotes alone pass `bytes` is cut to nothing.
pub fn cut(text: &mut String, bytes: usize) -> bool {
    if json_size(text) <= bytes {
        return false;
    }
    // The two quotes, then each character as JSON escapes it.
    let mut written = 2;
    let mut end = 0;
    for (at, character) in text.char_indices() {
        let size = match character {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
            control if control < ' ' => 6,
            other => other.len_utf8(),
        };
        if written + size > bytes {
            break;
        }
        written += size;
        end = at + character.len_utf8();
    }
    text.truncate(end);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: Listing<'static> = Listing {
        list: "entities",
        filter: "door",
    };

    #[test]
    fn a_cursor_gives_its_place_back_to_its_list_alone() {
        let cursor = LIST.cursor(7, 3, &[12, 0, u64::MAX]);
        assert_eq!(LIST.place(&cursor, 7, 3), Ok(vec![12, 0, u64::MAX]));
        assert_eq!(LIST.place(&LIST.cursor(7, 3, &[]), 7, 3), Ok(Vec::new()));
        let other_list = Listing {
            list: "elements",
            ..LIST
        };
        let other_filter = Listing {
            filter: "doors",
            ..LIST
        };
        for (listing, program, maker) in [(other_list, 7, 3), (other_filter, 7, 3)] {
            let fault = listing.place(&cursor, program, maker).expect_err("refused");
            assert_eq!(fault.code, Fault::BAD_PARAMS, "{fault:?}");
        }
        for (program, maker) in [(8, 3), (7, 4)] {
            let fault = LIST.place(&cursor, program, maker).expect_err("stale");
            assert_eq!(fault.code, Fault::STALE, "{fault:?}");
        }
    }

    #[test]
    fn a_cursor_that_does_not_read_is_a_bad_parameter_never_a_panic() {
        let cursor = LIST.cursor(7, 3, &[12, 5]);
        let mut garbled = vec![
            String::new(),
            ":::::".to_owned(),
            "entities".to_owned(),
            format!("{cursor}:"),
            cursor.replace(':', ";"),
            cursor.replacen("7", "x", 1),
            format!("{cursor}."),
            format!("{cursor}.10000000000000000"),
            "entities:7:3:0:é".to_owned(),
        ];
        // Every cut of the cursor, on a character's boundary.
        garbled.extend((0..cursor.len()).map(|end| cursor[..end].to_owned()));
        for text in garbled {
            match LIST.place(&text, 7, 3) {
                Err(fault) => assert_eq!(fault.code, Fault::BAD_PARAMS, "{text:?}: {fault:?}"),
                // A cut that leaves a whole number shorter is another place, which its list reads
                // as one, and checks.
                Ok(place) => assert!(cursor.starts_with(&text), "{text:?} read as {place:?}"),
            }
        }
    }

    #[test]
    fn a_page_asked_is_within_the_most_and_the_tool_s_line() {
        let page = Page::new(1, 16 << 20);
        assert_eq!((page.items, page.bytes), (MAX_ITEMS, MAX_BYTES));
        let page = Page::new(1, 64 << 10).within(Some(20), Some(u64::MAX));
        assert_eq!((page.items, page.bytes), (20, (64 << 10) - MARGIN));
        let page = Page::new(1, 16 << 20).within(Some(0), Some(100));
        assert_eq!((page.items, page.bytes), (1, 100));
        assert_eq!(
            page.within(Some(5), Some(500)).items,
            1,
            "within, never past"
        );
        assert_eq!(Page::new(1, 100).bytes, 0);
    }

    #[test]
    fn a_page_stops_at_its_count_and_its_bytes_but_takes_its_first_item() {
        let page = Page {
            program: 1,
            items: 3,
            bytes: 20,
        };
        let mut pager = Pager::new(page, || false);
        assert!(pager.take(&"a very long text that passes the page"));
        assert!(!pager.take(&"b"), "past its bytes");
        assert!(!pager.visit(), "a full page visits no more");

        let mut pager = Pager::new(page, || false);
        for text in ["a", "b", "c"] {
            assert!(pager.visit());
            assert!(pager.take(&text));
        }
        assert!(!pager.visit(), "its count reached");

        // `["a","b","c"]`: 3 bytes each, and a comma between them, 11 bytes.
        let page = Page {
            program: 1,
            items: 5,
            bytes: 10,
        };
        let mut pager = Pager::new(page, || false);
        assert!(pager.take(&"a") && pager.take(&"b"));
        assert!(!pager.take(&"c"), "the commas counted");
    }

    #[test]
    fn a_page_visits_its_floor_whatever_its_time_then_stops_once_it_passed() {
        let page = Page {
            program: 1,
            items: MAX_ITEMS,
            bytes: MAX_BYTES,
        };
        let mut pager = Pager::new(page, || true);
        let mut visits = 0;
        while pager.visit() {
            visits += 1;
        }
        assert_eq!(visits, MIN_VISITS);

        let mut asked = 0;
        let mut pager = Pager::new(page, || {
            asked += 1;
            asked > 3
        });
        let mut visits = 0;
        while pager.visit() {
            visits += 1;
        }
        assert_eq!(visits, MIN_VISITS + 3 * CLOCK_EVERY);
    }

    #[test]
    fn a_text_cut_writes_within_its_bytes_on_a_character_s_boundary() {
        for (text, bytes) in [
            ("plain text".repeat(1000), 100),
            ("\"\\".repeat(1000), 101),
            ("\u{1}".repeat(1000), 50),
            ("é日本".repeat(1000), 99),
            ("short".to_owned(), 1000),
        ] {
            let mut cut_text = text.clone();
            let was_cut = cut(&mut cut_text, bytes);
            assert!(json_size(&cut_text) <= bytes, "{}", json_size(&cut_text));
            assert_eq!(was_cut, cut_text != text);
            assert!(text.starts_with(&cut_text));
            // The most that fits: one character more would not.
            if was_cut {
                let next = text[cut_text.len()..].chars().next().expect("more");
                let mut longer = cut_text.clone();
                longer.push(next);
                assert!(json_size(&longer) > bytes);
            }
        }
        let mut text = "abc".to_owned();
        assert!(cut(&mut text, 1));
        assert_eq!(text, "");
    }
}
