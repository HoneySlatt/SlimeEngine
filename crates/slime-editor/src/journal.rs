//! The editor's journal: its entries written as text, read back, and read by pages
//! (`docs/adr/0134-the-editors-core.md`,
//! `docs/adr/0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md`).
//! [`Editor::keep_journal`](crate::Editor::keep_journal) says what each entry holds.

mod page;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use slime_channel::Fault;
use slime_channel::page::{Listing, Page};
use slime_files::HeldFile;
use slime_reflect::{ReflectError, Value};
use slime_serialize::{Composer, Document, Mode};
use slime_world::{PersistentId, World};

use crate::editor::{EditError, Editor};
use crate::level::Level;

/// The list that a journal's cursors name, which has no filter.
const LISTING: Listing<'static> = Listing {
    list: "journal",
    filter: "",
};

/// The number that the next journal made in this process draws.
static NUMBERS: AtomicU64 = AtomicU64::new(1);

/// A journal being written, to a file that this editor holds, so that no other writes it.
pub(crate) struct Journal {
    file: File,
    held: HeldFile,
    /// Its number in the process, which its cursors hold: one made anew, by
    /// [`Journal::open`], has another, which makes them stale.
    number: u64,
    /// The bytes of the file that it wrote in whole entries: a page reads no further.
    written: u64,
    /// Whether a write failed, which may have left part of an entry after `written`: it adds
    /// nothing more, as what it would add would follow that part, and refuses its pages, as it
    /// lacks the step that the editor made, which stops the editor at its next update.
    torn: bool,
}

/// A page of a journal, as a tool reads it.
pub(crate) struct JournalPage {
    /// Its text.
    pub(crate) text: String,
    /// The cursor to the rest, while the journal goes on; none at its end.
    pub(crate) next: Option<String>,
    /// A cursor at the journal's end, when the page reaches it.
    pub(crate) end: Option<String>,
    /// Whether bytes that are not UTF-8 were replaced in it.
    pub(crate) lossy: bool,
}

impl Journal {
    /// The journal `text` going on in the file `held`, a new journal's first entry or a journal
    /// recovered: `text` is written there first, whole or not at all, unless the file holds it
    /// already, so that a crash meanwhile leaves no part of an entry. Later entries are appended,
    /// so that a file cut apart from the editor stays shorter than what the journal wrote, rather
    /// than holding zeros up to where the journal writes next.
    pub(crate) fn open(held: HeldFile, text: &str) -> Result<Self, EditError> {
        let path = held.path();
        if fs::read(path).ok().as_deref() != Some(text.as_bytes()) {
            crate::level::write_whole(path, text.as_bytes())
                .map_err(|error| failed(path, &error))?;
        }
        let file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|error| failed(path, &error))?;
        Ok(Self {
            file,
            held,
            number: NUMBERS.fetch_add(1, Ordering::Relaxed),
            written: text.len() as u64,
            torn: false,
        })
    }

    /// Adds `text`, entries whole, and hands it to the system at once: what a crash of the editor
    /// does not lose, which a crash of the system may. Once a write failed, it adds nothing
    /// more, as its file may end with part of an entry.
    pub(crate) fn add(&mut self, text: &str) -> Result<(), EditError> {
        let path = self.held.path();
        if self.torn {
            return Err(EditError::Journal(format!(
                "{} holds part of an entry that could not be written: nothing more is added",
                path.display()
            )));
        }
        if let Err(error) = self
            .file
            .write_all(text.as_bytes())
            .and_then(|()| self.file.flush())
        {
            self.torn = true;
            return Err(failed(path, &error));
        }
        self.written += text.len() as u64;
        Ok(())
    }

    /// A page of the journal from `cursor`, none for its start, bounded by `page`'s bytes and
    /// by `time_up`, read from its file by seek: as `slime_channel::Edit::Journal` says.
    ///
    /// # Errors
    ///
    /// [`Fault::STALE`] for a cursor of another journal or process, or given while the file is
    /// shorter than what the journal wrote; [`Fault::BAD_PARAMS`] for a cursor that does not
    /// read, or whose place falls inside a character or past what the journal wrote;
    /// [`Fault::REFUSED`] when the file cannot be read, is shorter than what the journal wrote
    /// from its start, as no list again would help, or when a write failed ([`Journal::add`]).
    pub(crate) fn page(
        &self,
        cursor: Option<&str>,
        page: Page,
        time_up: &mut dyn FnMut() -> bool,
    ) -> Result<JournalPage, Fault> {
        self.whole()?;
        let start = match cursor {
            None => 0,
            Some(cursor) => self.place(cursor, page.program)?,
        };
        let path = self.held.path();
        let unread = |error: io::Error| {
            Fault::new(
                Fault::REFUSED,
                format!("the journal {} cannot be read: {error}", path.display()),
            )
        };
        let mut file = File::open(path).map_err(unread)?;
        let read = page::read(&mut file, start, self.written, page, time_up).map_err(|fault| {
            match (fault.code, cursor) {
                // Read from its start, the journal is still cut: a tool listing it again would
                // loop.
                (Fault::STALE, None) => Fault::new(
                    Fault::REFUSED,
                    format!("{}, and cannot be read by pages", fault.message),
                ),
                (Fault::STALE, Some(_)) => Fault::new(
                    Fault::STALE,
                    format!("{}: read the journal from its start", fault.message),
                ),
                _ => fault,
            }
        })?;
        let cursor = LISTING.cursor(page.program, self.number, &[read.end]);
        let (next, end) = if read.end < self.written {
            (Some(cursor), None)
        } else {
            (None, Some(cursor))
        };
        Ok(JournalPage {
            text: read.text,
            next,
            end,
            lossy: read.lossy,
        })
    }

    /// Whether `cursor`, given in the process numbered `program`, is at the journal's end, where a
    /// page that waits finds nothing yet: found without reading the file, so that a request that
    /// waits costs a cursor's parse each time it is asked again.
    ///
    /// # Errors
    ///
    /// As [`Journal::page`]'s, for a write that failed and a cursor that is not this journal's.
    pub(crate) fn at_end(&self, cursor: &str, program: u64) -> Result<bool, Fault> {
        self.whole()?;
        Ok(self.place(cursor, program)? == self.written)
    }

    /// Refuses every page once a write failed ([`Journal::add`]).
    fn whole(&self) -> Result<(), Fault> {
        if self.torn {
            return Err(Fault::new(
                Fault::REFUSED,
                format!(
                    "the journal {} could not be written whole, and the editor stops",
                    self.held.path().display()
                ),
            ));
        }
        Ok(())
    }

    /// The byte where the page that `cursor` asks starts, if it is this journal's.
    fn place(&self, cursor: &str, program: u64) -> Result<u64, Fault> {
        let bad = |why: &str| {
            Fault::new(
                Fault::BAD_PARAMS,
                format!("a cursor of the journal {why}: list it from its start"),
            )
        };
        let place = LISTING
            .place(cursor, program, self.number)
            .map_err(|fault| match fault.code {
                Fault::STALE => Fault::new(
                    Fault::STALE,
                    "the editor made its journal anew since this cursor was given, as it opened \
                     a level, or its program is another: read the journal from its start",
                ),
                _ => fault,
            })?;
        match place[..] {
            [at] if at <= self.written => Ok(at),
            [_] => Err(bad("past what the journal holds")),
            _ => Err(bad("that does not read")),
        }
    }
}

/// The error of a journal's file that cannot be written.
fn failed(path: &Path, error: &io::Error) -> EditError {
    EditError::Journal(format!("{} cannot be written: {error}", path.display()))
}

fn journal(error: ReflectError) -> EditError {
    EditError::Journal(error.to_string())
}

/// The first line of a journal, or of a copy, that names its format's version
/// ([`Editor::JOURNAL_VERSION`]).
pub(crate) fn version_line() -> String {
    format!("# journal {}\n", Editor::JOURNAL_VERSION)
}

/// The text of `bytes`, a journal's, up to its first byte that is not UTF-8, which a crash of the
/// system may leave: enough to read its version and its first entry.
pub(crate) fn readable(bytes: &[u8]) -> &str {
    match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(&bytes[..error.valid_up_to()])
            .expect("text up to its first byte that is not UTF-8"),
    }
}

/// The start of a journal of `level`: the line that names the journal's version, then the level's
/// entry, its key, its file by its whole path, and its file's text.
pub(crate) fn start(world: &World, level: &Level) -> Result<String, EditError> {
    let mut composer = Composer::new(world.registry());
    let file = level.whole.display().to_string();
    for (name, value) in [
        ("key", level.key.as_str()),
        ("file", &file),
        ("text", &level.text),
    ] {
        composer
            .put(&[name], "String", &Value::Text(value.to_owned()))
            .map_err(journal)?;
    }
    Ok(format!(
        "{}# level\n{}",
        version_line(),
        composer.finish().map_err(journal)?
    ))
}

/// The entries of a step: its label, the selection after it if it chose one, and the texts of its
/// commands.
pub(crate) fn step(
    world: &World,
    label: &str,
    selection: Option<&[PersistentId]>,
    commands: &[String],
) -> Result<String, EditError> {
    let mut composer = Composer::new(world.registry());
    composer
        .put(&["label"], "String", &Value::Text(label.to_owned()))
        .map_err(journal)?;
    if let Some(selection) = selection {
        composer
            .put(&["selection"], "Vec<String>", &identities(selection))
            .map_err(journal)?;
    }
    let mut text = format!("# do\n{}", composer.finish().map_err(journal)?);
    for command in commands {
        text.push_str("# command\n");
        text.push_str(command);
    }
    Ok(text)
}

/// The entry of a selection made apart from a step.
pub(crate) fn select(world: &World, selection: &[PersistentId]) -> Result<String, EditError> {
    let mut composer = Composer::new(world.registry());
    composer
        .put(&["selection"], "Vec<String>", &identities(selection))
        .map_err(journal)?;
    Ok(format!("# select\n{}", composer.finish().map_err(journal)?))
}

fn identities(ids: &[PersistentId]) -> Value {
    Value::List(ids.iter().map(|id| Value::Text(id.to_string())).collect())
}

/// Whether `journal` holds a step made, undone or done again after the level was last written, or
/// opened if it never was: what a crash, or a stop without writing, leaves.
pub(crate) fn unwritten(journal: &str) -> bool {
    let mut unwritten = false;
    for line in journal.lines() {
        match line.trim_end() {
            "# do" | "# undo" | "# redo" => unwritten = true,
            "# written" | "# level" => unwritten = false,
            _ => {}
        }
    }
    unwritten
}

/// The key and the file of the level that `journal` is the journal of, as its first entry says;
/// `None` if it does not start with the level's entry, readable, whatever follows, or if its
/// version's line is written wrongly.
///
/// # Errors
///
/// [`EditError::LaterJournal`] for a journal of a later version, whose level this editor cannot
/// read.
pub(crate) fn level_of(
    world: &World,
    journal: &str,
) -> Result<Option<(String, String)>, EditError> {
    let start = match entries_start(journal) {
        Ok((start, _)) => start,
        Err(later @ EditError::LaterJournal { .. }) => return Err(later),
        Err(_) => return Ok(None),
    };
    let rest = &journal[start..];
    // The first entry alone: the line that names the next one ends it.
    let end = rest
        .match_indices("\n# ")
        .map(|(at, _)| at + 1)
        .next()
        .unwrap_or(rest.len());
    Ok(
        match entries(world, &rest[..end])
            .ok()
            .and_then(|entries| entries.into_iter().next())
        {
            Some((_, Entry::Level { key, file, .. })) => Some((key, file)),
            _ => None,
        },
    )
}

/// The byte where the entries of `journal` start, and the number of lines before it: past its
/// first line when that line names the journal's version, `# journal 1`; at its start when it
/// names none, as a journal of version 1 may, written before journals named their version
/// (`docs/adr/0297-one-crate-of-safe-files-and-the-editors-journal-versioned-built-by-s1.md`).
///
/// The version is a whole number from 1, alone on the line, read first: one above
/// [`Editor::JOURNAL_VERSION`] is a later version whatever else the line holds, so that a later
/// format may write more there.
///
/// # Errors
///
/// [`EditError::LaterJournal`] for a later version; [`EditError::Journal`] for a version that is
/// not a whole number from 1, or a line that holds more than its version.
fn entries_start(journal: &str) -> Result<(usize, usize), EditError> {
    let first = journal.split_inclusive('\n').next().unwrap_or_default();
    let Some(words) = first.strip_prefix("# ").map(str::split_ascii_whitespace) else {
        return Ok((0, 0));
    };
    let mut words = words.peekable();
    if words.next() != Some("journal") {
        return Ok((0, 0));
    }
    let version = words
        .next()
        .filter(|word| word.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|word| word.parse::<u64>().ok())
        .filter(|version| *version >= 1);
    match version {
        Some(version) if version > Editor::JOURNAL_VERSION => {
            Err(EditError::LaterJournal { version })
        }
        Some(_) if words.peek().is_none() => Ok((first.len(), 1)),
        _ => Err(EditError::Journal(format!(
            "line 1: `# journal` names the version of the journal's format, a whole number from 1, \
             alone: `# journal {}`",
            Editor::JOURNAL_VERSION
        ))),
    }
}

/// An entry of a journal read.
pub(crate) enum Entry<'t> {
    /// The level, by its key, its file and its file's text.
    Level {
        key: String,
        file: String,
        text: String,
    },
    /// A step, its label, the selection after it if it chose one, and its commands' texts.
    Do {
        label: String,
        selection: Option<Vec<PersistentId>>,
        commands: Vec<&'t str>,
    },
    Undo,
    Redo,
    /// A selection made apart from a step.
    Select(Vec<PersistentId>),
    /// The level written back to its file.
    Written,
}

/// The entries of `text`, a journal, each with the number of the line that names it, after the
/// line that names its version, if it has one (`entries_start`).
///
/// # Errors
///
/// [`EditError::LaterJournal`] for a journal of a later version, of which nothing more is read;
/// [`EditError::Journal`] for text that is not a journal: a version written wrongly or out of the
/// first line, a line before the first entry, an entry of an unknown kind, a command out of a
/// step, or a document that is not the entry's.
pub(crate) fn entries<'t>(
    world: &World,
    text: &'t str,
) -> Result<Vec<(usize, Entry<'t>)>, EditError> {
    let wrong = |line: usize, why: &str| EditError::Journal(format!("line {line}: {why}"));
    let (start, lines_before) = entries_start(text)?;
    // Each entry's kind, the line that names it, and the bounds of the text that follows it.
    let mut parts: Vec<(&str, usize, usize, usize)> = Vec::new();
    let mut offset = start;
    for (index, content) in text[start..].split_inclusive('\n').enumerate() {
        let line = lines_before + index + 1;
        if let Some(kind) = content.strip_prefix("# ") {
            if let Some(last) = parts.last_mut() {
                last.3 = offset;
            }
            parts.push((kind.trim(), line, offset + content.len(), text.len()));
        } else if parts.is_empty() && !content.trim().is_empty() {
            return Err(wrong(line, "a journal starts with an entry, `# level`"));
        }
        offset += content.len();
    }
    let mut entries: Vec<(usize, Entry<'t>)> = Vec::new();
    for (kind, line, start, end) in parts {
        let body = &text[start..end];
        let read = |ty: &str, key: &str| -> Result<Option<Value>, EditError> {
            Document::parse(world.registry(), body, Mode::Strict)
                .and_then(|mut document| document.read(&[key], ty, None))
                .map_err(|error| wrong(line, &error.to_string()))
        };
        let text_at = |key: &str| -> Result<Option<String>, EditError> {
            Ok(match read("String", key)? {
                Some(Value::Text(text)) => Some(text),
                _ => None,
            })
        };
        let selection = || -> Result<Option<Vec<PersistentId>>, EditError> {
            let Some(Value::List(ids)) = read("Vec<String>", "selection")? else {
                return Ok(None);
            };
            ids.iter()
                .map(|id| match id {
                    Value::Text(id) => id.parse().map_err(|error| wrong(line, &format!("{error}"))),
                    _ => Err(wrong(line, "a selection is a list of identities")),
                })
                .collect::<Result<_, _>>()
                .map(Some)
        };
        let entry = match kind {
            "command" => {
                let Some((_, Entry::Do { commands, .. })) = entries.last_mut() else {
                    return Err(wrong(line, "a command comes within a step, after `# do`"));
                };
                commands.push(body);
                continue;
            }
            "undo" | "redo" | "written" if !body.trim().is_empty() => {
                return Err(wrong(line, &format!("`# {kind}` holds nothing")));
            }
            "undo" => Entry::Undo,
            "redo" => Entry::Redo,
            "written" => Entry::Written,
            "level" => Entry::Level {
                key: text_at("key")?.ok_or_else(|| wrong(line, "the level has no `key`"))?,
                file: text_at("file")?.ok_or_else(|| wrong(line, "the level has no `file`"))?,
                text: text_at("text")?.ok_or_else(|| wrong(line, "the level has no `text`"))?,
            },
            "do" => Entry::Do {
                label: text_at("label")?.unwrap_or_default(),
                selection: selection()?,
                commands: Vec::new(),
            },
            "select" => Entry::Select(selection()?.unwrap_or_default()),
            version if version.split_ascii_whitespace().next() == Some("journal") => {
                return Err(wrong(
                    line,
                    "`# journal` names the journal's version on its first line alone",
                ));
            }
            other => return Err(wrong(line, &format!("`# {other}` is not an entry"))),
        };
        entries.push((line, entry));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write that fails leaves the journal torn: it adds nothing more, and refuses its pages,
    /// those that wait at its end with the others.
    #[test]
    fn a_journal_whose_write_failed_adds_nothing_more_and_refuses_its_pages() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-torn-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory");
        // A journal of one entry, whose handle then refuses every write, as a full disk would:
        // it was opened to read.
        let path = directory.join("journal");
        fs::write(&path, "# level\n").expect("its first entry");
        let held = HeldFile::hold(path.clone()).expect("held");
        let refusing = File::open(&path).expect("the journal, to read");
        let mut journal = Journal {
            file: refusing,
            held,
            number: 1,
            written: 8,
            torn: false,
        };
        let (page, end) = (Page::new(1, usize::MAX), LISTING.cursor(1, 1, &[8]));
        assert_eq!(journal.at_end(&end, 1), Ok(true));
        assert!(journal.add("# do\n").is_err());
        assert!(journal.torn);
        // The disk takes writes again: still nothing more, after what may be part of an entry.
        journal.file = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("the journal");
        assert!(journal.add("# undo\n").is_err(), "nothing more");
        assert_eq!(fs::read_to_string(&path).expect("read"), "# level\n");
        let fault = journal
            .page(None, page, &mut || false)
            .err()
            .expect("refused");
        assert_eq!(fault.code, Fault::REFUSED, "{fault:?}");
        let fault = journal.at_end(&end, 1).expect_err("refused");
        assert_eq!(fault.code, Fault::REFUSED, "{fault:?}");
        fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_journal_holds_steps_unwritten_after_its_last_writing_or_its_start() {
        for (journal, unwritten_steps) in [
            ("", false),
            ("# level\n", false),
            ("# level\n# do\n# command\n", true),
            ("# level\n# do\n# written\n", false),
            ("# level\n# do\n# written\n# undo\n", true),
            ("# level\n# do\n# written\n# redo\n", true),
            (
                "# level\n# do\n# written\n# select\nselection = []\n",
                false,
            ),
            ("# level\r\n# do\r\n", true),
        ] {
            assert_eq!(unwritten(journal), unwritten_steps, "{journal:?}");
        }
    }
}
