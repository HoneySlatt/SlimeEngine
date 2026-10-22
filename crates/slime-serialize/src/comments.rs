//! The comments of a document written by hand, kept when a program writes the document again
//! (`docs/adr/0134-the-editors-core.md`).

use std::collections::HashMap;

/// `after`, a document that a program wrote in place of `before`, with the comments of `before`
/// put back where they were: what the editor writes when it writes again a level that people
/// wrote, so that their explanations stay.
///
/// A comment goes back before what it preceded in `before`, a header or a key, found in `after`
/// by its path of keys, whichever way each text writes it: `[changed."lamp 1".Transform]` and a key
/// `Transform` of the table `[changed."lamp 1"]` are one place, and the comment goes before that
/// key's line even when the header that holds it now comes right above. The comment of a key that
/// `after` holds within a value written on its parent's line, `translation.x` within
/// `translation = { x = 4.0 }`, goes before that line. The comments at the top of `before`, before
/// a blank line and any key, stay at the top, and those at its end after every key, at the end. A
/// comment at the end of a line goes back to the end of its key's line. The comments of what
/// `after` no longer holds, an entity removed, are left out, and so are those within a value
/// written on several lines, but for the end of its first line.
///
/// It costs a reading of each line of both texts.
///
/// ```
/// let before = "# The yard.\n\n[entities.well]\n# Deep.\nprefab = \"well\" # stone\n";
/// let after = "[entities]\nwell = { prefab = \"well\" }\nbucket = { prefab = \"bucket\" }\n";
/// assert_eq!(
///     slime_serialize::keep_comments(before, after),
///     "# The yard.\n\n[entities]\n# Deep.\nwell = { prefab = \"well\" } # stone\n\
///      bucket = { prefab = \"bucket\" }\n"
/// );
/// let after = "[entities.well]\nprefab = \"well\"\n";
/// assert_eq!(
///     slime_serialize::keep_comments(before, after),
///     "# The yard.\n\n[entities.well]\n# Deep.\nprefab = \"well\" # stone\n"
/// );
/// ```
pub fn keep_comments(before: &str, after: &str) -> String {
    let kept = Kept::of(before);
    if kept.is_empty() {
        return after.to_owned();
    }
    let lines: Vec<Line<'_>> = scan(after);
    // The first line of each place that `after` holds, and whether it is a header's.
    let mut places: HashMap<&[String], (usize, bool)> = HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        match &line.kind {
            Kind::Header(path) => places.entry(path).or_insert((index, true)),
            Kind::Key(path) => places.entry(path).or_insert((index, false)),
            _ => continue,
        };
    }
    // The line of `after` that a comment of the place `path` goes with: the place's own, or the key
    // whose value holds it on one line.
    let line_of = |path: &[String]| {
        if let Some(&(index, _)) = places.get(path) {
            return Some(index);
        }
        (1..path.len())
            .rev()
            .find_map(|length| match places.get(&path[..length]) {
                Some(&(index, false)) => {
                    let (_, rest) = keys(lines[index].text.trim_start());
                    let value = rest.strip_prefix('=').unwrap_or_default();
                    Some(inline_holds(value, &path[length..]).then_some(index))
                }
                Some(&(_, true)) => Some(None),
                None => None,
            })
            .flatten()
    };
    let mut blocks: HashMap<usize, Vec<&Block<'_>>> = HashMap::new();
    for block in &kept.blocks {
        if let Some(index) = line_of(&block.path) {
            blocks.entry(index).or_default().push(block);
        }
    }
    let mut ends: HashMap<usize, Vec<&str>> = HashMap::new();
    for (path, comment) in &kept.ends {
        if let Some(index) = line_of(path) {
            ends.entry(index).or_default().push(comment);
        }
    }
    let mut out = String::with_capacity(before.len() + after.len());
    for line in &kept.top {
        out.push_str(line);
        out.push('\n');
    }
    if !kept.top.is_empty() {
        out.push('\n');
    }
    for (index, line) in lines.iter().enumerate() {
        for block in blocks.get(&index).into_iter().flatten() {
            for comment in &block.lines {
                out.push_str(comment);
                out.push('\n');
            }
            if block.blank_after {
                out.push('\n');
            }
        }
        out.push_str(line.text);
        for comment in ends.get(&index).into_iter().flatten() {
            out.push(' ');
            out.push_str(comment);
        }
        out.push('\n');
    }
    if !kept.bottom.is_empty() {
        if kept.blank_before_bottom && !out.ends_with("\n\n") {
            out.push('\n');
        }
        for line in &kept.bottom {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The comments of a document, by the places they go with.
#[derive(Default)]
struct Kept<'t> {
    /// The lines at the top, comments and the blank lines between them.
    top: Vec<&'t str>,
    /// The comments before a place.
    blocks: Vec<Block<'t>>,
    /// The comments at the end of a place's line.
    ends: Vec<(Vec<String>, &'t str)>,
    /// The lines at the end, after every place.
    bottom: Vec<&'t str>,
    /// Whether a blank line came before them.
    blank_before_bottom: bool,
}

/// The comment lines before a place, and the blank lines between them.
struct Block<'t> {
    path: Vec<String>,
    lines: Vec<&'t str>,
    /// Whether a blank line came between them and the place.
    blank_after: bool,
}

impl<'t> Kept<'t> {
    fn of(text: &'t str) -> Self {
        let mut kept = Self::default();
        let mut lines: Vec<&'t str> = Vec::new();
        let mut blank_after = false;
        // Whether the line before is blank, and whether one was before the comments gathered.
        let (mut blank, mut blank_before) = (false, false);
        let mut seen = false;
        for line in scan(text) {
            let was_blank = std::mem::replace(&mut blank, matches!(line.kind, Kind::Blank));
            let path = match line.kind {
                Kind::Blank => {
                    blank_after = !lines.is_empty();
                    continue;
                }
                Kind::Comment => {
                    if lines.is_empty() {
                        blank_before = was_blank;
                    }
                    if blank_after {
                        lines.push("");
                    }
                    blank_after = false;
                    lines.push(line.text.trim());
                    continue;
                }
                Kind::Header(path) | Kind::Key(path) => path,
                Kind::Within => {
                    // A comment within a value written on several lines has no place of its own.
                    lines.clear();
                    blank_after = false;
                    continue;
                }
            };
            if !seen && !lines.is_empty() {
                // What comes before a blank line at the top is the document's.
                let split = if blank_after {
                    lines.len()
                } else {
                    lines.iter().rposition(|line| line.is_empty()).unwrap_or(0)
                };
                kept.top = lines.drain(..split).collect();
                if kept.top.last() == Some(&"") {
                    kept.top.pop();
                }
                if lines.first() == Some(&"") {
                    lines.remove(0);
                }
            }
            if !lines.is_empty() {
                kept.blocks.push(Block {
                    path: path.clone(),
                    lines: std::mem::take(&mut lines),
                    blank_after,
                });
            }
            if let Some(comment) = line.end {
                kept.ends.push((path, comment));
            }
            blank_after = false;
            seen = true;
        }
        if seen {
            kept.bottom = lines;
            kept.blank_before_bottom = blank_before;
        } else {
            kept.top = lines;
        }
        kept
    }

    fn is_empty(&self) -> bool {
        self.top.is_empty()
            && self.blocks.is_empty()
            && self.ends.is_empty()
            && self.bottom.is_empty()
    }
}

/// A line of a document, as far as its comments go.
struct Line<'t> {
    text: &'t str,
    kind: Kind,
    /// The comment at its end, from `#`, without the space before it.
    end: Option<&'t str>,
}

/// What a line of a document holds.
enum Kind {
    Blank,
    /// A comment, alone on its line.
    Comment,
    /// A header, `[a.b]` or `[[a.b]]`, by its path of keys; a table of an array, by its index too,
    /// `[1]`.
    Header(Vec<String>),
    /// A key and its value, by its path from the document's root.
    Key(Vec<String>),
    /// A line within a value written on several lines.
    Within,
}

/// The lines of `text`. A line that is not TOML is a key of no path, which no comment goes with.
fn scan(text: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut table: Vec<String> = Vec::new();
    let mut arrays: HashMap<Vec<String>, usize> = HashMap::new();
    let mut open = Open::default();
    for text in text.lines() {
        let trimmed = text.trim_start();
        let (kind, end) = if open.within() {
            let end = open.scan(text);
            (Kind::Within, end)
        } else if trimmed.is_empty() {
            (Kind::Blank, None)
        } else if trimmed.starts_with('#') {
            (Kind::Comment, None)
        } else if let Some(rest) = trimmed.strip_prefix('[') {
            let (array, rest) = match rest.strip_prefix('[') {
                Some(rest) => (true, rest),
                None => (false, rest),
            };
            let (mut path, rest) = keys(rest);
            if array {
                let count = arrays.entry(path.clone()).or_insert(0);
                path.push(format!("[{count}]"));
                *count += 1;
            }
            table.clone_from(&path);
            (Kind::Header(path), comment(rest))
        } else {
            let (key, rest) = keys(trimmed);
            let path = table.iter().cloned().chain(key).collect();
            let end = rest
                .strip_prefix('=')
                .map_or_else(|| comment(rest), |value| open.scan(value));
            (Kind::Key(path), end)
        };
        lines.push(Line { text, kind, end });
    }
    lines
}

/// Whether `value`, a value written on its key's line, is an inline table that holds the place
/// `path` within it, whichever way it writes its keys: `{ x = { y = 1 } }` and `{ x.y = 1 }` hold
/// `x.y`.
fn inline_holds(value: &str, path: &[String]) -> bool {
    let Some(mut rest) = value.trim_start().strip_prefix('{') else {
        return false;
    };
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with('}') {
            return false;
        }
        let (entry, after) = keys(rest);
        let Some(after) = after.strip_prefix('=') else {
            return false;
        };
        let end = value_end(after);
        let common = entry.iter().zip(path).take_while(|(a, b)| a == b).count();
        if common == path.len()
            || (common == entry.len() && inline_holds(&after[..end], &path[common..]))
        {
            return true;
        }
        match after[end..].trim_start().strip_prefix(',') {
            Some(next) => rest = next,
            None => return false,
        }
    }
}

/// Where the value at the start of `text`, within an inline table, ends: at the comma or the brace
/// that follows it, out of its strings, arrays and tables.
fn value_end(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut depth, mut index) = (0_usize, 0);
    let mut string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        match string {
            Some(quote) if byte == quote => string = None,
            Some(b'"') if byte == b'\\' => index += 1,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => string = Some(byte),
                b'[' | b'{' => depth += 1,
                b']' | b'}' if depth > 0 => depth -= 1,
                b',' | b'}' => return index,
                _ => {}
            },
        }
        index += 1;
    }
    bytes.len()
}

/// The comment at the end of what is left of a line once its header or keys are read.
fn comment(rest: &str) -> Option<&str> {
    rest.find('#').map(|start| rest[start..].trim_end())
}

/// The path of keys at the start of `text`, dotted, each bare or quoted, and what follows it.
fn keys(text: &str) -> (Vec<String>, &str) {
    let mut path = Vec::new();
    let mut rest = text.trim_start();
    loop {
        let (key, after) = if let Some(quoted) = rest.strip_prefix('"') {
            basic(quoted)
        } else if let Some(quoted) = rest.strip_prefix('\'') {
            match quoted.find('\'') {
                Some(end) => (quoted[..end].to_owned(), &quoted[end + 1..]),
                None => (quoted.to_owned(), ""),
            }
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .unwrap_or(rest.len());
            (rest[..end].to_owned(), &rest[end..])
        };
        path.push(key);
        rest = after.trim_start();
        match rest.strip_prefix('.') {
            Some(after) => rest = after.trim_start(),
            None => break,
        }
    }
    let rest = rest.trim_start_matches(']').trim_start();
    (path, rest)
}

/// The text of a basic string whose opening quote is read, its escapes of a quote and a backslash
/// read, and what follows its closing quote.
fn basic(text: &str) -> (String, &str) {
    let mut key = String::new();
    let mut chars = text.char_indices();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return (key, &text[index + 1..]),
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    key.push(escaped);
                }
            }
            c => key.push(c),
        }
    }
    (key, "")
}

/// What a value written on several lines leaves open at the end of a line: its arrays and inline
/// tables, and a string of several lines.
#[derive(Default)]
struct Open {
    depth: usize,
    string: Option<&'static str>,
}

impl Open {
    fn within(&self) -> bool {
        self.depth > 0 || self.string.is_some()
    }

    /// Reads `text`, part of a value, and returns the comment at its end, if it has one.
    fn scan<'t>(&mut self, text: &'t str) -> Option<&'t str> {
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            // Bytes, not text: an index within a character of several bytes cannot slice text.
            let rest = &bytes[index..];
            if let Some(close) = self.string {
                if rest.starts_with(close.as_bytes()) {
                    self.string = None;
                    index += close.len();
                } else if close.starts_with('"') && bytes[index] == b'\\' {
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            for open in ["\"\"\"", "'''", "\"", "'"] {
                if rest.starts_with(open.as_bytes()) {
                    self.string = Some(open);
                    index += open.len();
                    break;
                }
            }
            if self.string.is_some() {
                continue;
            }
            match bytes[index] {
                b'[' | b'{' => self.depth += 1,
                b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                b'#' => return Some(text[index..].trim_end()),
                _ => {}
            }
            index += 1;
        }
        // A string of one line ends with it, written right or not.
        if matches!(self.string, Some("\"" | "'")) {
            self.string = None;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_go_back_before_their_places_however_each_text_writes_them() {
        let before = "\
# A level.
# Two lines.

[assets]
lamp = { id = \"1\", path = \"lamp.scene\" } # the lamp

[entities]
# By the gate.
gate = { prefab = \"lamp\" }
# Gone.
porch = { prefab = \"lamp\" }

# The gate's light, dimmed.

[changed.gate.Transform]
translation.x = 4.0 # a step east

# The end.
";
        let after = "\
[assets]
lamp = { id = \"1\", path = \"lamp.scene\" }

[entities]
gate = { prefab = \"lamp\" }
well = { prefab = \"well\" }

[changed.gate.Transform]
translation = { x = 4.0 }
";
        assert_eq!(
            keep_comments(before, after),
            "\
# A level.
# Two lines.

[assets]
lamp = { id = \"1\", path = \"lamp.scene\" } # the lamp

[entities]
# By the gate.
gate = { prefab = \"lamp\" }
well = { prefab = \"well\" }

# The gate's light, dimmed.

[changed.gate.Transform]
translation = { x = 4.0 } # a step east

# The end.
"
        );
    }

    #[test]
    fn a_header_and_a_key_of_its_parent_are_one_place() {
        let after = "[entities]\n\"lamp 1\" = { prefab = \"lamp\" }\n";
        let kept = "[entities]\n# The first.\n\"lamp 1\" = { prefab = \"lamp\" }\n";
        for before in [
            "[entities]\n# The first.\n'lamp 1' = { prefab = 'lamp' }\n",
            "# The first.\n[entities.\"lamp 1\"]\nprefab = 'lamp'\n",
            "[entities.\"lamp 1\"]\n# The first.\nprefab = 'lamp'\n",
        ] {
            assert_eq!(keep_comments(before, after), kept, "{before}");
        }
        // A header written as a key of a table under its own header: the comment stays by the key.
        assert_eq!(
            keep_comments(
                "# The moon.\n[changed.moon.Light.Directional]\nin_sky = true\n",
                "[changed.moon.Light]\nDirectional = { in_sky = true }\n"
            ),
            "[changed.moon.Light]\n# The moon.\nDirectional = { in_sky = true }\n"
        );
    }

    #[test]
    fn a_text_without_comments_or_with_strange_lines_is_written_as_it_comes() {
        let after = "[a]\nb = 1\n";
        assert_eq!(keep_comments("[a]\nb = 2\n", after), after);
        assert_eq!(keep_comments("", after), after);
        let before = "name = \"café # crème\" # brûlée\ntext = \"\"\"\nété # là\n\"\"\" # fin\n";
        assert_eq!(
            keep_comments(before, "name = \"thé\"\ntext = \"\"\n"),
            "name = \"thé\" # brûlée\ntext = \"\"\n",
            "the end of a value of several lines is within it"
        );
        let before = "x = \"# not a comment\" # one\ny = [\n  1, # within\n  2,\n]\n= ]] \" #\n";
        assert_eq!(
            keep_comments(before, "x = \"a\"\ny = [1, 2]\n"),
            "x = \"a\" # one\ny = [1, 2]\n"
        );
        assert_eq!(
            keep_comments("# all\n# comments\n", after),
            "# all\n# comments\n\n[a]\nb = 1\n"
        );
    }

    #[test]
    fn the_comment_of_a_key_gone_from_a_value_on_one_line_goes_and_the_end_keeps_its_spacing() {
        let before = "[a]\n# About c.\nc = 1 # one\nd = 2\n";
        assert_eq!(keep_comments(before, "a = { d = 2 }\n"), "a = { d = 2 }\n");
        assert_eq!(
            keep_comments(before, "a = { d = 2, c = { e = 'x, }' } }\n"),
            "# About c.\na = { d = 2, c = { e = 'x, }' } } # one\n"
        );
        assert_eq!(
            keep_comments(before, "a = { c.e = 1 }\n"),
            "# About c.\na = { c.e = 1 } # one\n"
        );
        assert_eq!(
            keep_comments("[a]\nx = 1\n# The end.\n", "[a]\nx = 1\n"),
            "[a]\nx = 1\n# The end.\n"
        );
        assert_eq!(
            keep_comments("[a]\nx = 1\n\n# The end.\n", "[a]\nx = 1\n"),
            "[a]\nx = 1\n\n# The end.\n"
        );
    }

    #[test]
    fn tables_of_arrays_are_told_apart_by_their_indices() {
        let before = "# First.\n[[posts]]\nn = 1\n# Second.\n[[posts]]\nn = 2\n";
        let after = "[[posts]]\nn = 1\n\n[[posts]]\nn = 2\n";
        assert_eq!(
            keep_comments(before, after),
            "# First.\n[[posts]]\nn = 1\n\n# Second.\n[[posts]]\nn = 2\n"
        );
    }
}
