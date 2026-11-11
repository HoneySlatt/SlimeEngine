//! The bodies of KDL's strings, cut into pieces: characters, newlines and escapes. The reader checks
//! a string's body with them while reading it; [`decode`] gives its text when it is asked for.

use std::borrow::Cow;
use std::ops::Range;

use crate::Form;
use crate::chars;

/// What is wrong in a string's body, and where: a byte from the body's start.
#[derive(Clone, Debug)]
pub(crate) struct Fault {
    pub(crate) at: usize,
    pub(crate) message: String,
}

/// A piece of a string's body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    /// A character written as itself, which is not a newline.
    Char(char),
    /// A newline written as itself; `\r\n` is one.
    Newline,
    /// A character written as an escape, `\n` or `\u{E06C}`.
    Escaped(char),
    /// `\` and the whitespace and newlines after it, which the string's text leaves out.
    WhitespaceEscape,
}

/// The piece that starts `rest`, and its length in bytes. Escapes are read where `escapes`, as in
/// a quoted string; a raw string has none.
///
/// # Errors
///
/// What is wrong: an escape KDL does not know, a character a document may not hold, or an empty
/// `rest`, which holds no piece.
pub(crate) fn piece(rest: &str, escapes: bool) -> Result<(Piece, usize), String> {
    if let Some(length) = chars::newline_len(rest) {
        return Ok((Piece::Newline, length));
    }
    match rest.chars().next() {
        None => Err("a string cut short".to_owned()),
        Some('\\') if escapes => escape(rest),
        Some(c) if chars::is_disallowed(c) => Err(chars::not_allowed(c)),
        Some(c) => Ok((Piece::Char(c), c.len_utf8())),
    }
}

/// The escape that starts `rest`, at its `\`, and its length in bytes: KDL's `string-character`
/// that starts with `\`.
fn escape(rest: &str) -> Result<(Piece, usize), String> {
    let after = &rest[1..];
    let escaped = match after.chars().next() {
        // A `\` that ends the text: the string never closes.
        None => return Err("a string not closed: it ends after a `\\`".to_owned()),
        Some('"') => '"',
        Some('\\') => '\\',
        Some('b') => '\u{8}',
        Some('f') => '\u{C}',
        Some('n') => '\n',
        Some('r') => '\r',
        Some('t') => '\t',
        Some('s') => ' ',
        Some('u') => return unicode_escape(rest),
        Some(c) if is_whitespace(c) => {
            let length = after.find(|c| !is_whitespace(c)).unwrap_or(after.len());
            return Ok((Piece::WhitespaceEscape, 1 + length));
        }
        _ => {
            return Err(
                "an escape KDL does not know: a string's escapes are \\n, \\r, \\t, \\\\, \\\", \
                 \\b, \\f, \\s, \\u{…}, and \\ before whitespace"
                    .to_owned(),
            );
        }
    };
    Ok((Piece::Escaped(escaped), 2))
}

/// Whether a whitespace escape, `\` before whitespace, leaves `c` out.
fn is_whitespace(c: char) -> bool {
    chars::is_space(c) || chars::is_newline(c)
}

/// The escape `\u{…}` that starts `rest`, and its length in bytes.
fn unicode_escape(rest: &str) -> Result<(Piece, usize), String> {
    let wrong_form = || "a \\u{…} escape holds one to six hexadecimal digits in braces".to_owned();
    let digits = rest.strip_prefix("\\u{").ok_or_else(wrong_form)?;
    let length = digits
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(digits.len());
    if !(1..=6).contains(&length) || !digits[length..].starts_with('}') {
        return Err(wrong_form());
    }
    let value = u32::from_str_radix(&digits[..length], 16).map_err(|_| wrong_form())?;
    let c = char::from_u32(value).ok_or_else(|| {
        "a \\u{…} escape names a Unicode scalar value: not a surrogate, D800 to DFFF, and at most \
         10FFFF"
            .to_owned()
    })?;
    Ok((Piece::Escaped(c), "\\u{".len() + length + "}".len()))
}

/// The pieces of `body`, from the byte `from`, each with where it starts and its length; a fault
/// ends them.
fn pieces(
    body: &str,
    from: usize,
    escapes: bool,
) -> impl Iterator<Item = Result<(usize, Piece, usize), Fault>> + '_ {
    let mut at = from;
    std::iter::from_fn(move || {
        if at >= body.len() {
            return None;
        }
        let start = at;
        match piece(&body[start..], escapes) {
            Ok((piece, length)) => {
                at += length;
                Some(Ok((start, piece, length)))
            }
            Err(message) => {
                at = body.len();
                Some(Err(Fault { at: start, message }))
            }
        }
    })
}

/// The text of the string of `form` that lies at `span` in `text`, which the reader checked.
pub(crate) fn decode<'t>(text: &'t str, span: &Range<u32>, form: Form) -> Cow<'t, str> {
    let written = &text[span.start as usize..span.end as usize];
    // A raw string's hashes, before and after its quotes.
    let hashes = written.bytes().take_while(|&byte| byte == b'#').count();
    let between = |quotes: usize| &written[hashes + quotes..written.len() - hashes - quotes];
    match form {
        Form::Bare => Cow::Borrowed(written),
        Form::Plain | Form::Raw => Cow::Borrowed(between(1)),
        Form::Escaped => decoded(between(1), quoted),
        Form::MultiLine => decoded(between(3), |body, text| multi_line(body, true, text)),
        Form::RawMultiLine => decoded(between(3), |body, text| multi_line(body, false, text)),
    }
}

/// `body` decoded by `decode`.
fn decoded<'t>(
    body: &'t str,
    decode: impl FnOnce(&str, &mut String) -> Result<(), Fault>,
) -> Cow<'t, str> {
    let mut text = String::with_capacity(body.len());
    let decoding = decode(body, &mut text);
    debug_assert!(
        decoding.is_ok(),
        "the reader checks every string it keeps: {decoding:?}"
    );
    match decoding {
        Ok(()) => Cow::Owned(text),
        Err(_) => Cow::Borrowed(body),
    }
}

/// Decodes `body`, a quoted string's between its quotes, into `text`.
///
/// # Errors
///
/// The first fault of `body`: an escape KDL does not know, or a newline.
fn quoted(body: &str, text: &mut String) -> Result<(), Fault> {
    for piece in pieces(body, 0, true) {
        match piece? {
            (_, Piece::Char(c) | Piece::Escaped(c), _) => text.push(c),
            (_, Piece::WhitespaceEscape, _) => {}
            (at, Piece::Newline, _) => {
                return Err(Fault {
                    at,
                    message: "a quoted string holds no newline".to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// Decodes `body`, a multi-line string's between its quotes, into `text`; it reads escapes where
/// `escapes`, as a raw string does not.
///
/// The body starts with a newline. Its last line, after its last newline, holds the closing quotes'
/// indentation, and nothing else. Each line between, unless it holds only whitespace, starts with
/// that indentation, which its text leaves out; a line of whitespace alone is empty. Whitespace
/// escapes are left out before the lines are cut, and the others decoded after, as the
/// specification of KDL asks. The lines are joined by `\n`, whatever newline they end with.
///
/// # Errors
///
/// The first fault of `body`: no newline after the opening quotes, something other than whitespace
/// before the closing quotes, a line that does not start with their indentation, an escape KDL
/// does not know.
pub(crate) fn multi_line(body: &str, escapes: bool, text: &mut String) -> Result<(), Fault> {
    let Some(first_newline) = chars::newline_len(body) else {
        return Err(Fault {
            at: 0,
            message: "a multi-line string starts a new line right after its opening quotes"
                .to_owned(),
        });
    };
    let mut last_line = first_newline;
    for piece in pieces(body, first_newline, escapes) {
        if let (at, Piece::Newline, length) = piece? {
            last_line = at + length;
        }
    }
    let indentation = indentation(body, last_line, escapes)?;
    let mut line = first_newline;
    while line < last_line {
        let (end, next, blank) = line_end(body, line, escapes)?;
        if line > first_newline {
            text.push('\n');
        }
        if !blank {
            if !body[line..end].starts_with(indentation) {
                return Err(Fault {
                    at: line,
                    message: "a line of a multi-line string does not start with the whitespace \
                              before its closing quotes"
                        .to_owned(),
                });
            }
            for piece in pieces(&body[..end], line + indentation.len(), escapes) {
                if let (_, Piece::Char(c) | Piece::Escaped(c), _) = piece? {
                    text.push(c);
                }
            }
        }
        line = next;
    }
    Ok(())
}

/// The indentation of a multi-line string's closing quotes: the whitespace of its last line, which
/// starts at `last_line` in `body`, and which holds nothing else.
fn indentation(body: &str, last_line: usize, escapes: bool) -> Result<&str, Fault> {
    // A whitespace escape takes all the whitespace after it: none is left after one.
    let mut end = body.len();
    for piece in pieces(body, last_line, escapes) {
        match piece? {
            (_, Piece::Char(c), _) if chars::is_space(c) => {}
            (at, Piece::WhitespaceEscape, _) => end = end.min(at),
            (at, ..) => {
                return Err(Fault {
                    at,
                    message: "a multi-line string's closing quotes are on a line of their own, \
                              after whitespace alone"
                        .to_owned(),
                });
            }
        }
    }
    Ok(&body[last_line..end])
}

/// Where the line that starts at `line` in `body` ends: the start of its newline, and the start of
/// the next line; and whether it holds whitespace alone.
fn line_end(body: &str, line: usize, escapes: bool) -> Result<(usize, usize, bool), Fault> {
    let mut blank = true;
    for piece in pieces(body, line, escapes) {
        match piece? {
            (at, Piece::Newline, length) => return Ok((at, at + length, blank)),
            (_, Piece::Char(c), _) if chars::is_space(c) => {}
            (_, Piece::WhitespaceEscape, _) => {}
            _ => blank = false,
        }
    }
    Ok((body.len(), body.len(), blank))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `body` of a multi-line string decoded.
    fn decoded(body: &str, escapes: bool) -> Result<String, usize> {
        let mut text = String::new();
        multi_line(body, escapes, &mut text).map_err(|fault| fault.at)?;
        Ok(text)
    }

    #[test]
    fn a_multi_line_string_s_lines_lose_the_last_line_s_indentation() {
        assert_eq!(
            decoded("\r\n  a\r\n\r\n   b\\t\r\n  ", true),
            Ok("a\n\n b\t".into())
        );
        assert_eq!(decoded("\n  a\\\n  b\n  \\  ", true), Ok("ab".into()));
        assert_eq!(decoded("\n  a\\\n  ", false), Ok("a\\".into()));
        assert_eq!(decoded("\n", true), Ok(String::new()));
        // A line that does not start with the indentation, and a last line that holds text.
        assert_eq!(decoded("\n a\n  ", true), Err(1));
        assert_eq!(decoded("\n  a\n  b", true), Err(7));
    }

    #[test]
    fn an_escape_names_a_unicode_scalar_value_in_one_to_six_digits() {
        assert_eq!(
            piece("\\u{10FFFF}", true),
            Ok((Piece::Escaped('\u{10FFFF}'), 10))
        );
        for invalid in [
            "\\u{}",
            "\\u{0000041}",
            "\\u{D800}",
            "\\u{110000}",
            "\\u{41",
            "\\/",
        ] {
            assert!(piece(invalid, true).is_err(), "{invalid}");
        }
        assert_eq!(piece("\\/", false), Ok((Piece::Char('\\'), 1)));
    }
}
