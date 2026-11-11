//! Reading KDL 2.0's text into a [`Document`], within [`Limits`], by the grammar of KDL's
//! specification, its "Full Grammar", at the commit that `tests/kdl-suite/README.md` names; the
//! functions that read a production are named after it.
//!
//! The reader is one loop over the text, which never recurses, however deep the document. A stack
//! holds the blocks open, each with the node that owns it. A node finished waits in `pending` until
//! its parent's block closes; then the block's children move together to the document's nodes,
//! where they lie next to each other, as the flat tree wants. A node's entries are all read before
//! its children's, so that they go to the document's entries as they are read.
//!
//! What a slashdash, `/-`, comments out is read like the rest, then left out: a node, an entry or
//! a block, and all that a block left out holds.

use crate::chars::{self, Bare};
use crate::string::{self, Piece};
use crate::{Document, EntryData, Error, Form, Limits, NodeData, Token, ValueData, number};

/// Reads `text` into a document, within `limits`.
pub(crate) fn document<'t>(text: &'t str, limits: &Limits) -> Result<Document<'t>, Error> {
    // Spans are kept in `u32`: a text beyond 4 GiB is refused, whatever the limit.
    let largest = limits.size.min(u32::MAX as usize);
    if text.len() > largest {
        let offset = text.floor_char_boundary(largest);
        return Err(error_at(
            text,
            offset,
            format!("the text is larger than its limit of {largest} bytes"),
        ));
    }
    Reader {
        text,
        at: 0,
        limits: *limits,
        items: 0,
        nodes: Vec::new(),
        entries: Vec::new(),
        pending: Vec::new(),
        blocks: Vec::new(),
    }
    .document()
}

/// The state of a reading.
struct Reader<'t> {
    text: &'t str,
    /// The byte being read.
    at: usize,
    limits: Limits,
    /// The nodes and entries read so far, slashdashed ones included (see [`Reader::count_item`]).
    items: usize,
    /// The document's nodes, each block's children next to each other, once the block is closed.
    nodes: Vec<NodeData>,
    /// The document's entries, each node's next to each other.
    entries: Vec<EntryData>,
    /// The nodes kept and finished whose parent's block is still open: the document's own first,
    /// then each open block's from its [`Block::first_pending`].
    pending: Vec<NodeData>,
    /// The blocks open, the innermost last.
    blocks: Vec<Block>,
}

/// A node being read.
struct OpenNode {
    data: NodeData,
    /// What it may still hold.
    phase: Phase,
    /// Whether it is left out of the document: slashdashed, or in a block left out.
    discarded: bool,
}

/// What a node may still hold, in the order of KDL's `base-node`: its entries, then its blocks,
/// slashdashed or not, of which one at most is not slashdashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Entries, and blocks.
    Entries,
    /// After a slashdashed block: blocks only.
    Blocks,
    /// After its block: slashdashed blocks only.
    AfterBlock,
}

/// A children block open.
struct Block {
    /// The node it belongs to.
    owner: OpenNode,
    /// Where its own nodes start in [`Reader::pending`].
    first_pending: usize,
    /// Whether it is slashdashed.
    slashdashed: bool,
    /// Where its `{` lies.
    opened_at: usize,
}

impl Block {
    /// Whether what it holds is left out of the document.
    fn discarded(&self) -> bool {
        self.slashdashed || self.owner.discarded
    }
}

impl<'t> Reader<'t> {
    /// Reads KDL's `document`: a byte order mark, if any, then nodes.
    fn document(mut self) -> Result<Document<'t>, Error> {
        if self.text.starts_with('\u{FEFF}') {
            self.at = '\u{FEFF}'.len_utf8();
        }
        let mut node = None;
        loop {
            node = match node {
                Some(open) => self.node_part(open)?,
                None => {
                    self.line_space()?;
                    match self.peek() {
                        None => break,
                        Some('}') => Some(self.close_block()?),
                        Some(_) => Some(self.node_start()?),
                    }
                }
            };
        }
        if let Some(block) = self.blocks.last() {
            let (line, column) = line_and_column(&self.text[..block.opened_at]);
            return Err(self.error(
                self.text.len(),
                format!("the block opened at line {line}, column {column} is not closed"),
            ));
        }
        let first_root = self.nodes.len();
        self.nodes.append(&mut self.pending);
        Ok(Document {
            text: self.text,
            roots: narrow(first_root)..narrow(self.nodes.len()),
            nodes: self.nodes,
            entries: self.entries,
        })
    }

    /// Reads a node's start, up to its name: a slashdash, its type annotation, its name.
    fn node_start(&mut self) -> Result<OpenNode, Error> {
        let slashdashed = self.slashdash()?;
        let start = self.at;
        // Slashdashed nodes count toward the depth too, so that the stack of blocks stays bounded.
        if self.blocks.len() > self.limits.depth {
            return Err(self.error(
                start,
                format!(
                    "a node deeper than the limit of {} levels",
                    self.limits.depth
                ),
            ));
        }
        self.count_item(start)?;
        let annotation = self.annotation()?;
        let name = self.string("a node's name")?;
        let entries = narrow(self.entries.len());
        Ok(OpenNode {
            data: NodeData {
                span: narrow(start)..name.span.end,
                name,
                annotation,
                entries: entries..entries,
                children: 0..0,
            },
            phase: Phase::Entries,
            discarded: slashdashed || self.blocks.last().is_some_and(Block::discarded),
        })
    }

    /// Reads the next part of `node`: an entry, a block, or its end. Gives the node back if it
    /// goes on; none once it ended, or once its block opened, whose nodes come next.
    fn node_part(&mut self, mut node: OpenNode) -> Result<Option<OpenNode>, Error> {
        let spaced = self.node_space()?;
        let rest = self.rest();
        if rest.is_empty()
            || rest.starts_with([';', '}'])
            || rest.starts_with("//")
            || chars::newline_len(rest).is_some()
        {
            self.node_terminator()?;
            if !node.discarded {
                self.pending.push(node.data);
            }
            return Ok(None);
        }
        let slashdashed = self.slashdash()?;
        if self.rest().starts_with('{') {
            self.open_block(node, slashdashed)?;
            return Ok(None);
        }
        if !spaced && !slashdashed && self.peek().is_some_and(starts_entry) {
            return Err(self.error(self.at, "an entry with no space before it"));
        }
        self.entry(&mut node, slashdashed)?;
        Ok(Some(node))
    }

    /// Reads KDL's `node-terminator`, which ends a node: `;`, a newline, or a single-line comment.
    /// The end of the text, and a block's `}`, end it too, and are left to read.
    fn node_terminator(&mut self) -> Result<(), Error> {
        if self.rest().starts_with(';') {
            self.at += 1;
        } else if let Some(length) = chars::newline_len(self.rest()) {
            self.at += length;
        } else if self.rest().starts_with("//") {
            self.single_line_comment()?;
        }
        Ok(())
    }

    /// Reads an entry of `node`, KDL's `node-prop-or-arg`: an argument, `(type)value`, or a
    /// property, `name=(type)value`.
    fn entry(&mut self, node: &mut OpenNode, slashdashed: bool) -> Result<(), Error> {
        let start = self.at;
        match node.phase {
            Phase::Entries => {}
            Phase::Blocks => {
                return Err(self.error(
                    start,
                    "an entry after a slashdashed block: a node's entries come before its blocks",
                ));
            }
            Phase::AfterBlock => {
                return Err(self.error(
                    start,
                    "an entry after a node's block: end the node first, with `;` or a new line",
                ));
            }
        }
        self.count_item(start)?;
        let mut name = None;
        let mut annotation = self.annotation()?;
        let mut value = self.value("an entry")?;
        // A string with no annotation, then `=`, is a property's name: KDL's `prop`.
        if let (None, ValueData::String(string)) = (&annotation, &value) {
            let string = string.clone();
            if self.equals_sign()? {
                name = Some(string);
                annotation = self.annotation()?;
                value = self.value("a property's value")?;
            }
        }
        if !(slashdashed || node.discarded) {
            self.entries.push(EntryData {
                name,
                annotation,
                value,
                span: narrow(start)..narrow(self.at),
            });
            node.data.entries.end = narrow(self.entries.len());
            node.data.span.end = narrow(self.at);
        }
        Ok(())
    }

    /// Reads `=` and the space around it, if it comes next: then the string before it is a
    /// property's name.
    fn equals_sign(&mut self) -> Result<bool, Error> {
        let after_name = self.at;
        self.node_space()?;
        if !self.rest().starts_with('=') {
            self.at = after_name;
            return Ok(false);
        }
        self.at += 1;
        self.node_space()?;
        Ok(true)
    }

    /// Opens `owner`'s block at its `{`.
    fn open_block(&mut self, owner: OpenNode, slashdashed: bool) -> Result<(), Error> {
        if !slashdashed && owner.phase == Phase::AfterBlock {
            return Err(self.error(
                self.at,
                "a second block: a node has one block at most, and only slashdashed ones after it",
            ));
        }
        self.blocks.push(Block {
            owner,
            first_pending: self.pending.len(),
            slashdashed,
            opened_at: self.at,
        });
        self.at += 1;
        Ok(())
    }

    /// Closes the innermost block at its `}`, and gives back the node it belongs to.
    fn close_block(&mut self) -> Result<OpenNode, Error> {
        let Some(block) = self.blocks.pop() else {
            return Err(self.error(self.at, "a block closed that was never opened"));
        };
        self.at += 1;
        let mut owner = block.owner;
        if block.slashdashed {
            if owner.phase == Phase::Entries {
                owner.phase = Phase::Blocks;
            }
            return Ok(owner);
        }
        owner.phase = Phase::AfterBlock;
        // A block left out holds no node kept: `pending` holds none of its.
        if !owner.discarded {
            let first_child = self.nodes.len();
            self.nodes.extend(self.pending.drain(block.first_pending..));
            owner.data.children = narrow(first_child)..narrow(self.nodes.len());
            owner.data.span.end = narrow(self.at);
        }
        Ok(owner)
    }

    /// Counts a node or an entry toward [`Limits::items`], at `at`. Slashdashed ones count too:
    /// each is read before it is left out, and a slashdashed node holds a place on the stack of
    /// blocks while its block is open, so that counting them keeps the reader's memory bounded by
    /// the limit, whatever the depth allowed.
    fn count_item(&mut self, at: usize) -> Result<(), Error> {
        self.items += 1;
        if self.items > self.limits.items {
            return Err(self.error(
                at,
                format!(
                    "more nodes and entries than the limit of {}",
                    self.limits.items
                ),
            ));
        }
        Ok(())
    }

    /// Reads KDL's `slashdash`, `/-` and the line space after it, if one comes next.
    fn slashdash(&mut self) -> Result<bool, Error> {
        let start = self.at;
        if !self.rest().starts_with("/-") {
            return Ok(false);
        }
        self.at += 2;
        self.line_space()?;
        if self.rest().is_empty() || self.rest().starts_with([';', '}']) {
            return Err(self.error(
                start,
                "a slashdash, `/-`, with nothing after it to leave out",
            ));
        }
        Ok(true)
    }

    /// Reads KDL's `type`, a type annotation, `(type)`, and the space after it, if one comes next.
    fn annotation(&mut self) -> Result<Option<Token>, Error> {
        if !self.rest().starts_with('(') {
            return Ok(None);
        }
        self.at += 1;
        self.node_space()?;
        let annotation = self.string("a type annotation")?;
        self.node_space()?;
        if !self.rest().starts_with(')') {
            return Err(self.unexpected("`)`, closing the type annotation"));
        }
        self.at += 1;
        self.node_space()?;
        Ok(Some(annotation))
    }

    /// Reads a string, as a name; `what` names what it is, for an error.
    fn string(&mut self, what: &str) -> Result<Token, Error> {
        let start = self.at;
        match self.value(what)? {
            ValueData::String(token) => Ok(token),
            _ => Err(self.error(start, format!("{what} must be a string"))),
        }
    }

    /// Reads a value, without its type annotation: a string, a number, or a keyword, `#true`;
    /// `what` names what it is, for an error.
    fn value(&mut self, what: &str) -> Result<ValueData, Error> {
        match self.peek() {
            Some('"') => self.quoted_string().map(ValueData::String),
            Some('#') => self.hashed(),
            Some(c) if chars::is_identifier_char(c) => self.bare_word(),
            _ => Err(self.unexpected(what)),
        }
    }

    /// Reads a run of identifier characters: an identifier, KDL's `identifier-string`, or a
    /// number.
    fn bare_word(&mut self) -> Result<ValueData, Error> {
        let start = self.at;
        let rest = self.rest();
        let length = rest
            .find(|c| !chars::is_identifier_char(c))
            .unwrap_or(rest.len());
        let word = &rest[..length];
        let span = narrow(start)..narrow(start + length);
        let value = match chars::bare(word) {
            Bare::Identifier => ValueData::String(Token {
                span,
                form: Form::Bare,
            }),
            Bare::Number if number::written(word).is_some() => ValueData::Number(span),
            Bare::Number => {
                return Err(self.error(
                    start,
                    "a malformed number: an identifier may not start like a number either",
                ));
            }
            Bare::PointNumber => {
                return Err(self.error(
                    start,
                    "a number with no digit before its point: an identifier may not start like one \
                     either",
                ));
            }
            Bare::Keyword => {
                return Err(self.error(
                    start,
                    format!("a keyword without its `#`: write `#{word}`, or quote it as a string"),
                ));
            }
        };
        self.at += length;
        Ok(value)
    }

    /// Reads what starts with `#`: a raw string, KDL's `raw-string`, or a keyword, `#true`,
    /// `#false`, `#null`, `#inf`, `#-inf` or `#nan`.
    fn hashed(&mut self) -> Result<ValueData, Error> {
        let start = self.at;
        let rest = self.rest();
        let hashes = rest.bytes().take_while(|&byte| byte == b'#').count();
        if rest[hashes..].starts_with('"') {
            return self.raw_string(hashes).map(ValueData::String);
        }
        let length = rest[1..]
            .find(|c| !chars::is_identifier_char(c))
            .map_or(rest.len(), |length| 1 + length);
        let span = narrow(start)..narrow(start + length);
        let value = match &rest[..length] {
            "#true" => ValueData::Bool(true),
            "#false" => ValueData::Bool(false),
            "#null" => ValueData::Null,
            "#inf" | "#-inf" | "#nan" => ValueData::Number(span),
            _ => {
                return Err(self.error(
                    start,
                    "a keyword KDL does not know: its keywords are #true, #false, #null, #inf, \
                     #-inf and #nan",
                ));
            }
        };
        self.at += length;
        Ok(value)
    }

    /// Reads KDL's `quoted-string`: `"…"`, or multi-line, `"""…"""`.
    fn quoted_string(&mut self) -> Result<Token, Error> {
        if self.rest().starts_with(r#"""""#) {
            self.multi_line_string(0)
        } else {
            self.single_line_string(0)
        }
    }

    /// Reads KDL's `raw-string` with `hashes` hashes: `#"…"#`, or multi-line, `#"""…"""#`.
    fn raw_string(&mut self, hashes: usize) -> Result<Token, Error> {
        if self.rest()[hashes..].starts_with(r#"""""#) {
            self.multi_line_string(hashes)
        } else {
            self.single_line_string(hashes)
        }
    }

    /// Reads a string on one line, quoted, `"…"`, or raw, `#"…"#`, with `hashes` hashes.
    fn single_line_string(&mut self, hashes: usize) -> Result<Token, Error> {
        let start = self.at;
        let escapes = hashes == 0;
        let mut form = if escapes { Form::Plain } else { Form::Raw };
        self.at += hashes + 1;
        loop {
            let rest = self.rest();
            if closes(rest, 1, hashes) {
                self.at += 1 + hashes;
                break;
            }
            if rest.is_empty() {
                return Err(self.error(self.at, "a string not closed"));
            }
            let (piece, length) =
                string::piece(rest, escapes).map_err(|message| self.error(self.at, message))?;
            match piece {
                Piece::Newline => {
                    return Err(self.error(
                        self.at,
                        "a string not closed on its line: a string holds a new line as `\\n`, or \
                         is multi-line, `\"\"\"`",
                    ));
                }
                Piece::Escaped(_) | Piece::WhitespaceEscape => form = Form::Escaped,
                Piece::Char(_) => {}
            }
            self.at += length;
        }
        Ok(Token {
            span: narrow(start)..narrow(self.at),
            form,
        })
    }

    /// Reads a multi-line string, quoted, `"""…"""`, or raw, `#"""…"""#`, with `hashes` hashes.
    fn multi_line_string(&mut self, hashes: usize) -> Result<Token, Error> {
        let start = self.at;
        let escapes = hashes == 0;
        let body_start = start + hashes + 3;
        self.at = body_start;
        if chars::newline_len(self.rest()).is_none() {
            return Err(self.error(
                self.at,
                "a multi-line string starts a new line right after its opening quotes",
            ));
        }
        while !closes(self.rest(), 3, hashes) {
            if self.rest().is_empty() {
                return Err(self.error(self.at, "a multi-line string not closed"));
            }
            let (_, length) = string::piece(self.rest(), escapes)
                .map_err(|message| self.error(self.at, message))?;
            self.at += length;
        }
        let body_end = self.at;
        self.at += 3 + hashes;
        // Its lines' indentation is checked once the last line is known, as decoding does: a wrong
        // escape or character is therefore reported before a wrong indentation on an earlier line.
        string::multi_line(
            &self.text[body_start..body_end],
            escapes,
            &mut String::new(),
        )
        .map_err(|fault| self.error(body_start + fault.at, fault.message))?;
        Ok(Token {
            span: narrow(start)..narrow(self.at),
            form: if escapes {
                Form::MultiLine
            } else {
                Form::RawMultiLine
            },
        })
    }

    /// Reads KDL's `line-space*`: node space, newlines and single-line comments.
    fn line_space(&mut self) -> Result<(), Error> {
        loop {
            self.node_space()?;
            if let Some(length) = chars::newline_len(self.rest()) {
                self.at += length;
            } else if self.rest().starts_with("//") {
                self.single_line_comment()?;
            } else {
                return Ok(());
            }
        }
    }

    /// Reads KDL's `node-space*`: whitespace, multi-line comments and line continuations. Tells
    /// whether there was any.
    fn node_space(&mut self) -> Result<bool, Error> {
        let start = self.at;
        loop {
            match self.peek() {
                Some('\\') => self.escline()?,
                Some('/') if self.rest().starts_with("/*") => self.multi_line_comment()?,
                Some(c) if chars::is_space(c) => self.at += c.len_utf8(),
                _ => return Ok(self.at > start),
            }
        }
    }

    /// Reads KDL's `escline`, a line continuation: `\`, whitespace and comments, then a newline or
    /// the end of the text.
    fn escline(&mut self) -> Result<(), Error> {
        self.at += 1;
        loop {
            match self.peek() {
                Some('/') if self.rest().starts_with("/*") => self.multi_line_comment()?,
                Some(c) if chars::is_space(c) => self.at += c.len_utf8(),
                _ => break,
            }
        }
        if let Some(length) = chars::newline_len(self.rest()) {
            self.at += length;
            Ok(())
        } else if self.rest().starts_with("//") {
            self.single_line_comment()
        } else if self.rest().is_empty() {
            Ok(())
        } else {
            Err(self.unexpected(
                "the end of the line after `\\`, which continues a node on the next line",
            ))
        }
    }

    /// Reads KDL's `single-line-comment`, `//`, up to its newline, which it includes, or the end
    /// of the text.
    fn single_line_comment(&mut self) -> Result<(), Error> {
        self.at += 2;
        while let Some(c) = self.peek() {
            if let Some(length) = chars::newline_len(self.rest()) {
                self.at += length;
                break;
            }
            self.allowed(c)?;
            self.at += c.len_utf8();
        }
        Ok(())
    }

    /// Reads KDL's `multi-line-comment`, `/* … */`, the comments nested in it included.
    fn multi_line_comment(&mut self) -> Result<(), Error> {
        let mut depth = 0_usize;
        loop {
            let rest = self.rest();
            if rest.starts_with("/*") {
                depth += 1;
                self.at += 2;
            } else if rest.starts_with("*/") {
                depth -= 1;
                self.at += 2;
                if depth == 0 {
                    return Ok(());
                }
            } else if let Some(c) = rest.chars().next() {
                self.allowed(c)?;
                self.at += c.len_utf8();
            } else {
                return Err(self.error(self.at, "a comment not closed"));
            }
        }
    }

    /// Refuses `c`, read at the current byte, if a document may not hold it.
    fn allowed(&self, c: char) -> Result<(), Error> {
        if chars::is_disallowed(c) {
            return Err(self.error(self.at, chars::not_allowed(c)));
        }
        Ok(())
    }

    /// The text not read yet.
    fn rest(&self) -> &'t str {
        &self.text[self.at..]
    }

    /// The character being read, none at the end of the text.
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// An error at `offset`.
    fn error(&self, offset: usize, message: impl Into<String>) -> Error {
        error_at(self.text, offset, message)
    }

    /// An error at the character being read, which is not the `expected` one.
    fn unexpected(&self, expected: &str) -> Error {
        let found = match self.peek() {
            None => "the end of the text".to_owned(),
            Some(c) if chars::is_disallowed(c) => {
                return self.error(self.at, chars::not_allowed(c));
            }
            Some(c) if chars::is_newline(c) => "a new line".to_owned(),
            Some(c) if chars::is_space(c) => "a space".to_owned(),
            Some(c) => format!("`{c}`"),
        };
        self.error(self.at, format!("expected {expected}, found {found}"))
    }
}

/// Whether `c` starts an entry: a string, a number, a keyword or a type annotation.
fn starts_entry(c: char) -> bool {
    matches!(c, '"' | '#' | '(') || chars::is_identifier_char(c)
}

/// Whether `rest` starts with a string's closing delimiter: `quotes` quotes then `hashes` hashes.
fn closes(rest: &str, quotes: usize, hashes: usize) -> bool {
    let bytes = rest.as_bytes();
    bytes.len() >= quotes + hashes
        && bytes[..quotes].iter().all(|&byte| byte == b'"')
        && bytes[quotes..quotes + hashes]
            .iter()
            .all(|&byte| byte == b'#')
}

/// An error at `offset` in `text`.
fn error_at(text: &str, offset: usize, message: impl Into<String>) -> Error {
    let (line, column) = line_and_column(&text[..offset]);
    Error {
        line,
        column,
        offset,
        message: message.into(),
    }
}

/// The line and the column, from 1, of the character after `before`: each of KDL's newlines ends a
/// line, `\r\n` once; a column counts characters.
fn line_and_column(before: &str) -> (u32, u32) {
    let mut line = 1_u32;
    let mut column = 1_u32;
    let mut characters = before.chars().peekable();
    while let Some(c) = characters.next() {
        let carriage_return_of_crlf = c == '\r' && characters.peek() == Some(&'\n');
        if chars::is_newline(c) && !carriage_return_of_crlf {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }
    (line, column)
}

/// `offset`, a byte of the text or a count of its nodes or entries, as the `u32` that spans and
/// ranges keep: [`document`] refuses a text beyond `u32::MAX` bytes, and each node and entry takes
/// one byte at least.
fn narrow(offset: usize) -> u32 {
    debug_assert!(u32::try_from(offset).is_ok(), "{offset} is beyond u32");
    offset as u32
}

#[cfg(test)]
mod tests {
    use crate::{Limits, read, read_within};

    #[test]
    fn slashdashed_nodes_and_entries_count_toward_the_limits() {
        let two_items = Limits {
            items: 2,
            ..Limits::DEFAULT
        };
        assert!(read_within("/-a 1\n", &two_items).is_ok());
        assert!(read_within("/-a 1 2\n", &two_items).is_err());
        let one_level = Limits {
            depth: 1,
            ..Limits::DEFAULT
        };
        let error = read_within("a { /-b { c } }", &one_level).expect_err("too deep");
        assert_eq!(error.offset, 10);
    }

    #[test]
    fn a_character_a_document_may_not_hold_is_refused_in_comments_too() {
        let error = read("node // \u{202E}\n").expect_err("refused");
        assert_eq!((error.line, error.column), (1, 9));
        assert!(read("/* \u{7F} */ node").is_err());
        assert!(read("\u{FEFF}node").is_ok());
    }

    #[test]
    fn a_text_larger_than_its_limit_is_refused_at_a_character_s_start() {
        let three_bytes = Limits {
            size: 2,
            ..Limits::DEFAULT
        };
        let error = read_within("aéé", &three_bytes).expect_err("too large");
        assert_eq!((error.offset, error.column), (1, 2));
    }

    #[test]
    fn an_entry_s_span_runs_from_its_name_or_annotation_to_its_value_s_end() {
        let text = "node key = (t)\"v\" (u)1";
        let document = read(text).expect("read");
        let spans: Vec<_> = document
            .nodes()
            .flat_map(|node| node.entries())
            .map(|entry| &text[entry.span()])
            .collect();
        assert_eq!(spans, ["key = (t)\"v\"", "(u)1"]);
    }
}
