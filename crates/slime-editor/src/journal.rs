//! The editor's journal: its entries written as text, and read back (`docs/adr/0134-the-editors-core.md`).
//! [`Editor::keep_journal`](crate::Editor::keep_journal) says what each entry holds.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use slime_reflect::{ReflectError, Value};
use slime_serialize::{Composer, Document, Mode};
use slime_world::{PersistentId, World};

use crate::editor::EditError;

/// A journal being written, to a file.
pub(crate) struct Journal {
    path: PathBuf,
    file: File,
}

impl Journal {
    /// A journal written to `path`, which starts empty.
    pub(crate) fn create(path: &Path) -> Result<Self, EditError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|error| io(path, error))?;
        Ok(Self {
            path: path.to_owned(),
            file,
        })
    }

    /// Adds `text`, entries whole, and hands it to the system at once: what a crash of the editor
    /// does not lose, which a crash of the system may.
    pub(crate) fn add(&mut self, text: &str) -> Result<(), EditError> {
        self.file
            .write_all(text.as_bytes())
            .and_then(|()| self.file.flush())
            .map_err(|error| io(&self.path, error))
    }
}

fn io(path: &Path, error: io::Error) -> EditError {
    EditError::Io {
        path: path.to_owned(),
        error,
    }
}

fn journal(error: ReflectError) -> EditError {
    EditError::Journal(error.to_string())
}

/// The entry of the level of `key` whose file's text is `text`.
pub(crate) fn level(world: &World, key: &str, text: &str) -> Result<String, EditError> {
    let mut composer = Composer::new(world.registry());
    for (name, value) in [("key", key), ("text", text)] {
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

/// An entry of a journal read.
pub(crate) enum Entry<'t> {
    /// The level, by its key and its file's text.
    Level {
        key: String,
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
