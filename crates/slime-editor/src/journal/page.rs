//! A page of the editor's journal read from its file
//! (`docs/adr/0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md`).
//!
//! A page reads its file by pieces from its byte, never further than what the journal wrote. It
//! takes the text that is UTF-8 by blocks of [`BLOCK`] bytes, each a visit of its [`Pager`], their
//! JSON counted by bytes; each run of bytes that is not UTF-8 is one U+FFFD. It stops on its bytes
//! of JSON, or once its time has passed, past a floor of visits, then steps back to its last
//! line's end, unless it reached the journal's end.

use std::io::{self, Read, Seek, SeekFrom};

use slime_channel::Fault;
use slime_channel::page::{MIN_VISITS, Page, Pager, json_char_size};

/// The most bytes that a character takes in UTF-8: a page reads as many past its bytes, at
/// least, so that it ends on a character whole.
const LONGEST_CHARACTER: u64 = 4;

/// The bytes of text that a page counts as one visit of its [`Pager`], which reads its clock
/// every few visits: a page reads `MIN_VISITS` of them at least, whatever its time, and steps
/// back from them to its last line's end.
const BLOCK: usize = 64;

/// The bytes of a page's first read: its floor of visits, which it reads whatever its time. Each
/// read after it reads twice as many, up to [`PIECE`], once the clock says that its time has not
/// passed.
const FIRST_PIECE: u64 = (MIN_VISITS * BLOCK) as u64;

/// The most bytes that a page reads from its file at a time.
const PIECE: u64 = 64 << 10;

/// A page read from a journal's file.
#[derive(Debug, PartialEq)]
pub(super) struct PageRead {
    /// Its text.
    pub(super) text: String,
    /// The byte of the file where it ended, where the next starts.
    pub(super) end: u64,
    /// Whether bytes that are not UTF-8 were replaced in it.
    pub(super) lossy: bool,
}

/// A page of `file`, a journal that holds `written` bytes in whole entries, from its byte
/// `start`: at most `page.bytes` bytes of JSON in its text, quotes included, but one character
/// at least, read by pieces, stopped once `time_up` says past a floor of visits, never a piece
/// read past it. It ends just
/// after its last `\n` when it holds one and ends before `written`, else at a character's
/// boundary.
///
/// # Errors
///
/// [`Fault::STALE`] when `file` is shorter than `written`, cut apart from the editor;
/// [`Fault::BAD_PARAMS`] when `start` falls inside a character; [`Fault::REFUSED`] when it cannot
/// be read.
pub(super) fn read(
    file: &mut (impl Read + Seek),
    start: u64,
    written: u64,
    page: Page,
    time_up: &mut dyn FnMut() -> bool,
) -> Result<PageRead, Fault> {
    let unread = |error: io::Error| {
        Fault::new(
            Fault::REFUSED,
            format!("the journal cannot be read: {error}"),
        )
    };
    if file.seek(SeekFrom::End(0)).map_err(unread)? < written {
        return Err(Fault::new(
            Fault::STALE,
            "the journal's file is shorter than what the editor wrote in it: it was cut apart \
             from the editor",
        ));
    }
    // The bytes before `start`, which tell whether it falls inside a character.
    let back = start.min(LONGEST_CHARACTER - 1);
    file.seek(SeekFrom::Start(start - back)).map_err(unread)?;
    let mut before = vec![0; back as usize];
    file.read_exact(&mut before).map_err(unread)?;
    // As each byte writes one byte of JSON at least, a page never takes more than its bytes;
    // a character more ends it whole.
    let window = (written - start).min(page.bytes.max(1) as u64 + LONGEST_CHARACTER - 1);
    let mut taking = Taking::new(page, time_up);
    let mut pending: Vec<u8> = Vec::new();
    let (mut left, mut piece) = (window, FIRST_PIECE);
    loop {
        if left < window && taking.pager.time_passed() {
            break;
        }
        let size = left.min(piece);
        let at = pending.len();
        pending.resize(at + size as usize, 0);
        file.read_exact(&mut pending[at..]).map_err(unread)?;
        if left == window && inside_a_character(&before, &pending) {
            return Err(Fault::new(
                Fault::BAD_PARAMS,
                "a cursor of the journal inside a character: list it from its start",
            ));
        }
        left -= size;
        piece = (piece * 2).min(PIECE);
        let all_read = left == 0;
        let (used, stopped) = taking.runs(&pending, all_read, start + window == written);
        pending.drain(..used);
        if stopped || all_read {
            break;
        }
    }
    Ok(taking.finish(start, written))
}

/// What a page holds once it reached the end of a line, which it steps back to when it ends
/// before the journal does.
#[derive(Clone, Copy)]
struct LineEnd {
    text: usize,
    read: u64,
    lossy: bool,
}

/// A page being taken: its text, the bytes of JSON that it writes and the bytes of the file that
/// it took, and where its last line ended.
struct Taking<'a> {
    pager: Pager<'a>,
    /// The most bytes of JSON that its text takes.
    bytes: usize,
    text: String,
    /// The bytes of JSON of its text, quotes included.
    json: usize,
    /// The bytes of the file that it took.
    read: u64,
    lossy: bool,
    line_end: Option<LineEnd>,
}

impl<'a> Taking<'a> {
    fn new(page: Page, time_up: &'a mut dyn FnMut() -> bool) -> Self {
        Self {
            pager: Pager::new(page, time_up),
            bytes: page.bytes,
            text: String::new(),
            json: 2,
            read: 0,
            lossy: false,
            line_end: None,
        }
    }

    /// Takes the runs of `pending`, bytes read from the file: how many it used, and whether the
    /// page stopped. A run at `pending`'s end that may be a character that the read split waits
    /// for the next bytes; once the last were read, `all_read`, the page stops there, unless they
    /// reach the journal's end, `ends_journal`, where it is a run of its own.
    fn runs(&mut self, pending: &[u8], all_read: bool, ends_journal: bool) -> (usize, bool) {
        let before = self.read;
        let mut position = 0;
        let stopped = loop {
            if position == pending.len() {
                break false;
            }
            let (valid, invalid, split) = run(&pending[position..]);
            position += valid.len() + invalid;
            if !self.valid(valid) {
                break true;
            }
            if invalid == 0 {
                continue;
            }
            if split && !(all_read && ends_journal) {
                break all_read;
            }
            if !self.invalid(invalid) {
                break true;
            }
        };
        ((self.read - before) as usize, stopped)
    }

    /// Takes what of `valid`, UTF-8, fits, by blocks: whether all of it did.
    fn valid(&mut self, valid: &str) -> bool {
        let mut taken = 0;
        let mut whole = true;
        while taken < valid.len() {
            let mut end = (taken + BLOCK).min(valid.len());
            while !valid.is_char_boundary(end) {
                end += 1;
            }
            if !self.pager.visit() {
                whole = false;
                break;
            }
            let block = &valid[taken..end];
            let size: usize = block
                .bytes()
                .map(|byte| usize::from(JSON_BYTES[usize::from(byte)]))
                .sum();
            if self.json + size > self.bytes {
                taken += self.fitting(block, taken);
                whole = false;
                break;
            }
            self.json += size;
            taken = end;
        }
        self.keep(&valid[..taken]);
        whole
    }

    /// How many bytes of `block`, after `before` bytes of its run that the page took, fit within
    /// its bytes, character by character: one at least, on a page that holds none.
    fn fitting(&mut self, block: &str, before: usize) -> usize {
        for (at, character) in block.char_indices() {
            let size = json_char_size(character);
            let holds_some = self.read > 0 || before + at > 0;
            if holds_some && self.json + size > self.bytes {
                return at;
            }
            self.json += size;
        }
        block.len()
    }

    /// Adds `text`, counted already, to the page, noting its last line's end.
    fn keep(&mut self, text: &str) {
        if let Some(at) = text.rfind('\n') {
            self.line_end = Some(LineEnd {
                text: self.text.len() + at + 1,
                read: self.read + (at + 1) as u64,
                lossy: self.lossy,
            });
        }
        self.text.push_str(text);
        self.read += text.len() as u64;
    }

    /// Takes a run of `bytes` bytes that are not UTF-8, as one U+FFFD, if it fits: whether it did.
    fn invalid(&mut self, bytes: usize) -> bool {
        let size = json_char_size('\u{fffd}');
        if !self.pager.visit() || (self.read > 0 && self.json + size > self.bytes) {
            return false;
        }
        self.text.push('\u{fffd}');
        self.json += size;
        self.read += bytes as u64;
        self.lossy = true;
        true
    }

    /// The page, from `start` of a journal of `written` bytes: stepped back to its last line's
    /// end, unless it reached the journal's end.
    fn finish(mut self, start: u64, written: u64) -> PageRead {
        if start + self.read < written
            && let Some(line_end) = self.line_end
        {
            self.text.truncate(line_end.text);
            self.read = line_end.read;
            self.lossy = line_end.lossy;
        }
        PageRead {
            text: self.text,
            end: start + self.read,
            lossy: self.lossy,
        }
    }
}

/// The first run of `bytes`: its text in UTF-8; how many bytes follow it that are not, one
/// character's worth at most, none at the end; and whether those bytes may be the start of a
/// character that `bytes` ends inside, as when a read split it.
fn run(bytes: &[u8]) -> (&str, usize, bool) {
    match std::str::from_utf8(bytes) {
        Ok(valid) => (valid, 0, false),
        Err(error) => {
            let up_to = error.valid_up_to();
            // UTF-8 up to there, as the error says: never the default.
            let valid = std::str::from_utf8(&bytes[..up_to]).unwrap_or_default();
            match error.error_len() {
                Some(invalid) => (valid, invalid, false),
                None => (valid, bytes.len() - up_to, true),
            }
        }
    }
}

/// The bytes that each byte of a text in UTF-8 writes in JSON: an ASCII byte its character's,
/// as [`json_char_size`] counts it; one for each byte of another character, which JSON writes as
/// it is.
const JSON_BYTES: [u8; 256] = {
    let mut sizes = [1; 256];
    let mut byte = 0;
    while byte < 128 {
        sizes[byte] = json_char_size(byte as u8 as char) as u8;
        byte += 1;
    }
    sizes
};

/// Whether the place between `before`, the bytes before it, and `after`, those from it, falls
/// inside a character: one that starts in `before` and ends in `after`.
fn inside_a_character(before: &[u8], after: &[u8]) -> bool {
    let joined: Vec<u8> = before
        .iter()
        .chain(after.iter().take(LONGEST_CHARACTER as usize))
        .copied()
        .collect();
    (0..before.len()).any(|at| {
        joined[at..]
            .utf8_chunks()
            .next()
            .and_then(|chunk| chunk.valid().chars().next())
            .is_some_and(|character| at + character.len_utf8() > before.len())
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use slime_channel::json_size;

    use super::*;

    /// A page of `bytes` bytes of JSON at most, in the process numbered 1.
    fn page(bytes: usize) -> Page {
        Page::new(1, usize::MAX).within(None, Some(bytes as u64))
    }

    /// The pages of `file`, a journal of `written` bytes, each bounded by `bytes`, from its start
    /// to its end: each page with the byte where it started.
    fn pages(file: &[u8], written: usize, bytes: usize) -> Vec<(u64, PageRead)> {
        let mut pages = Vec::new();
        let mut start = 0;
        while start < written as u64 {
            let read = super::read(
                &mut Cursor::new(file),
                start,
                written as u64,
                page(bytes),
                &mut || false,
            )
            .expect("a page");
            assert!(read.end > start, "a page moves on: {start}, {bytes}");
            let end = read.end;
            pages.push((start, read));
            start = end;
        }
        pages
    }

    /// Text that JSON escapes and characters of every length, on lines short and long.
    fn corpus() -> Vec<u8> {
        let mut text = String::from("# level\ntext = \"\\\"quoted\\\" back\\\\slash\"\n");
        text.push_str("tab\tbell\u{7}\r\ncrlf\r\n");
        text.push_str(&"é日😀".repeat(40));
        text.push('\n');
        text.push_str(&"a long line ".repeat(30));
        text.push_str("\n# do\nlabel = \"x\"\n");
        text.into_bytes()
    }

    #[test]
    fn pages_joined_are_the_journal_byte_for_byte_each_within_its_bytes() {
        let file = corpus();
        for bytes in (0..=64).chain([100, 333, 4096]) {
            let mut joined = Vec::new();
            for (start, read) in pages(&file, file.len(), bytes) {
                assert!(!read.lossy);
                assert_eq!(
                    read.text.as_bytes(),
                    &file[start as usize..read.end as usize]
                );
                // Within its bytes, but the one character that a page always gives.
                assert!(
                    json_size(&read.text) <= bytes || read.text.chars().count() == 1,
                    "{bytes}: {:?}",
                    read.text
                );
                // A page that ends before the journal and holds a line's end ends after one.
                if read.end < file.len() as u64 && read.text.contains('\n') {
                    assert!(read.text.ends_with('\n'), "{bytes}: {:?}", read.text);
                }
                joined.extend_from_slice(read.text.as_bytes());
            }
            assert_eq!(joined, file, "{bytes}");
        }
    }

    #[test]
    fn a_page_holds_as_much_as_its_bytes_allow_where_no_line_ends() {
        // One line, as the level's entry: each page full but its last.
        let file = "x".repeat(1000).into_bytes();
        let pages = pages(&file, file.len(), 100);
        assert_eq!(pages.len(), 11, "98 characters and two quotes a page");
        assert!(pages[..10].iter().all(|(_, read)| read.text.len() == 98));
    }

    #[test]
    fn bytes_not_utf8_are_replaced_by_run_and_marked_on_their_pages_alone() {
        let mut file = b"start\n".to_vec();
        file.extend_from_slice(b"\xff one\n\xe2\x82 two\n\x80\x80\x80 three\n");
        file.extend_from_slice("\u{fffd}".as_bytes());
        file.extend_from_slice(b"\xff literal\n");
        file.extend_from_slice(&"日😀".repeat(20).into_bytes());
        file.extend_from_slice(b"\nend\xf0\x9f\x98");
        let lossy = String::from_utf8_lossy(&file);
        for bytes in (0..=40).chain([4096]) {
            let mut joined = String::new();
            for (start, read) in pages(&file, file.len(), bytes) {
                let raw = &file[start as usize..read.end as usize];
                assert_eq!(
                    read.lossy,
                    std::str::from_utf8(raw).is_err(),
                    "{bytes}: {raw:?}"
                );
                assert_eq!(read.text, String::from_utf8_lossy(raw), "{bytes}");
                assert!(
                    json_size(&read.text) <= bytes || read.text.chars().count() == 1,
                    "{bytes}: {:?}",
                    read.text
                );
                joined.push_str(&read.text);
            }
            assert_eq!(joined, lossy, "{bytes}");
        }
    }

    #[test]
    fn a_page_reads_nothing_past_what_the_journal_wrote() {
        let mut file = corpus();
        let written = file.len();
        file.extend_from_slice(b"# do\nlabel = \"torn");
        for bytes in [3, 64, 4096] {
            let joined: Vec<u8> = pages(&file, written, bytes)
                .into_iter()
                .flat_map(|(_, read)| read.text.into_bytes())
                .collect();
            assert_eq!(joined, file[..written], "{bytes}");
        }
    }

    #[test]
    fn a_place_inside_a_character_is_a_bad_parameter_a_file_cut_stale_never_a_panic() {
        let file = corpus();
        let written = file.len() as u64;
        for start in 0..=written {
            let read = super::read(
                &mut Cursor::new(&file),
                start,
                written,
                page(64),
                &mut || false,
            );
            let boundary = std::str::from_utf8(&file[..start as usize]).is_ok();
            match read {
                Ok(read) => assert!(boundary && read.end >= start, "{start}"),
                Err(fault) => {
                    assert!(!boundary, "{start}: {fault:?}");
                    assert_eq!(fault.code, Fault::BAD_PARAMS, "{start}");
                }
            }
        }
        // A stray byte that continues no character is a place: a page may start at it.
        let stray = b"a\x80\x80b";
        let read =
            super::read(&mut Cursor::new(stray), 1, 4, page(64), &mut || false).expect("a page");
        assert_eq!(
            (read.text.as_str(), read.lossy),
            ("\u{fffd}\u{fffd}b", true)
        );
        // The file cut apart from the editor.
        let fault = super::read(
            &mut Cursor::new(&file[..10]),
            0,
            written,
            page(64),
            &mut || false,
        )
        .expect_err("cut");
        assert_eq!(fault.code, Fault::STALE, "{fault:?}");
    }

    #[test]
    fn a_page_of_many_pieces_reads_on_past_a_character_split_between_two() {
        // Nine bytes a run: the pieces, of 4, 8, 16 and 32 KiB, then 64, end inside characters of
        // two, three and four bytes.
        let file = "é日😀".repeat(30_000).into_bytes();
        assert_ne!(PIECE % 9, 0);
        let pages = pages(&file, file.len(), 200 << 10);
        assert!(pages[0].1.text.len() as u64 > 2 * PIECE, "past many pieces");
        let joined: Vec<u8> = pages
            .into_iter()
            .flat_map(|(_, read)| {
                assert!(!read.lossy);
                read.text.into_bytes()
            })
            .collect();
        assert_eq!(joined, file);
    }

    #[test]
    fn a_page_whose_time_passed_steps_back_to_its_last_line_s_end() {
        // Lines of seven bytes: the floor of visits ends inside one.
        let file = "a line\n".repeat(10_000).into_bytes();
        let read = super::read(
            &mut Cursor::new(&file),
            0,
            file.len() as u64,
            page(1 << 20),
            &mut || true,
        )
        .expect("a page");
        assert!(read.text.ends_with('\n'), "{:?}", read.text);
        assert_eq!(read.text.len(), MIN_VISITS * BLOCK / 7 * 7);
    }

    /// A reader that counts the bytes read from it.
    struct Counted<'a> {
        file: Cursor<&'a [u8]>,
        read: usize,
    }

    impl Read for Counted<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let read = self.file.read(buffer)?;
            self.read += read;
            Ok(read)
        }
    }

    impl Seek for Counted<'_> {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            self.file.seek(to)
        }
    }

    #[test]
    fn a_page_whose_time_passed_reads_no_piece_past_its_first() {
        let file = "x".repeat(100_000).into_bytes();
        let mut counted = Counted {
            file: Cursor::new(&file),
            read: 0,
        };
        let read = super::read(
            &mut counted,
            0,
            file.len() as u64,
            page(1 << 20),
            &mut || true,
        )
        .expect("a page");
        assert_eq!(counted.read as u64, FIRST_PIECE);
        assert_eq!(read.text.len() as u64, FIRST_PIECE);
    }

    #[test]
    fn a_page_whose_time_passed_gives_its_floor_of_characters() {
        let file = "x".repeat(10_000).into_bytes();
        let read = super::read(
            &mut Cursor::new(&file),
            0,
            file.len() as u64,
            page(1 << 20),
            &mut || true,
        )
        .expect("a page");
        assert_eq!(read.text.len(), MIN_VISITS * BLOCK);
    }
}
