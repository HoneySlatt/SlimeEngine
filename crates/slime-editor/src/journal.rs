//! The editor's journal: its entries written as text, and read back
//! (`docs/adr/0134-the-editors-core.md`). [`Editor::keep_journal`](crate::Editor::keep_journal)
//! says what each entry holds.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use slime_app::HeldFile;
use slime_reflect::{ReflectError, Value};
use slime_serialize::{Composer, Document, Mode};
use slime_world::{PersistentId, World};

use crate::editor::EditError;
use crate::level::Level;

/// A journal being written, to a file that this editor holds, so that no other writes it.
pub(crate) struct Journal {
    file: File,
    held: HeldFile,
}

impl Journal {
    /// A journal written to the file `held`, which starts empty.
    pub(crate) fn create(held: HeldFile) -> Result<Self, EditError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(held.path())
            .map_err(|error| failed(held.path(), &error))?;
        Ok(Self { file, held })
    }

    /// The journal `text` going on in the file `held`: `text` is written there first, whole or not
    /// at all, unless the file holds it already.
    pub(crate) fn resume(held: HeldFile, text: &str) -> Result<Self, EditError> {
        let path = held.path();
        if fs::read(path).ok().as_deref() != Some(text.as_bytes()) {
            slime_cook::write(path, text.as_bytes()).map_err(|error| failed(path, &error))?;
        }
        let file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|error| failed(path, &error))?;
        Ok(Self { file, held })
    }

    /// Adds `text`, entries whole, and hands it to the system at once: what a crash of the editor
    /// does not lose, which a crash of the system may.
    pub(crate) fn add(&mut self, text: &str) -> Result<(), EditError> {
        self.file
            .write_all(text.as_bytes())
            .and_then(|()| self.file.flush())
            .map_err(|error| failed(self.held.path(), &error))
    }
}

/// The error of a journal's file that cannot be written.
fn failed(path: &Path, error: &io::Error) -> EditError {
    EditError::Journal(format!("{} cannot be written: {error}", path.display()))
}

fn journal(error: ReflectError) -> EditError {
    EditError::Journal(error.to_string())
}

/// The entry of `level`: its key, its file by its whole path, and its file's text.
pub(crate) fn level(world: &World, level: &Level) -> Result<String, EditError> {
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
    Ok(format!("# level\n{}", composer.finish().map_err(journal)?))
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
/// `None` if it does not start with the level's entry, readable, whatever follows.
pub(crate) fn level_of(world: &World, journal: &str) -> Option<(String, String)> {
    // The first entry alone: the line that names the next one ends it.
    let end = journal
        .match_indices("\n# ")
        .map(|(at, _)| at + 1)
        .next()
        .unwrap_or(journal.len());
    match entries(world, &journal[..end]).ok()?.into_iter().next()? {
        (_, Entry::Level { key, file, .. }) => Some((key, file)),
        _ => None,
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

/// The entries of `text`, a journal, each with the number of the line that names it.
///
/// # Errors
///
/// [`EditError::Journal`] for text that is not a journal: a line before the first entry, an entry
/// of an unknown kind, a command out of a step, or a document that is not the entry's.
pub(crate) fn entries<'t>(
    world: &World,
    text: &'t str,
) -> Result<Vec<(usize, Entry<'t>)>, EditError> {
    let wrong = |line: usize, why: &str| EditError::Journal(format!("line {line}: {why}"));
    // Each entry's kind, the line that names it, and the bounds of the text that follows it.
    let mut parts: Vec<(&str, usize, usize, usize)> = Vec::new();
    let mut offset = 0;
    for (index, content) in text.split_inclusive('\n').enumerate() {
        if let Some(kind) = content.strip_prefix("# ") {
            if let Some(last) = parts.last_mut() {
                last.3 = offset;
            }
            parts.push((kind.trim(), index + 1, offset + content.len(), text.len()));
        } else if parts.is_empty() && !content.trim().is_empty() {
            return Err(wrong(
                index + 1,
                "a journal starts with an entry, `# level`",
            ));
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
            other => return Err(wrong(line, &format!("`# {other}` is not an entry"))),
        };
        entries.push((line, entry));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

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
