//! KDL's classes of characters: whitespace, newlines, the characters a document may not hold, and
//! those an identifier, a string written without quotes, is made of.

/// Whether `c` is whitespace that ends no line: KDL's `unicode-space`.
pub(crate) fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    ) || ('\u{2000}'..='\u{200A}').contains(&c)
}

/// Whether `c` ends a line: KDL's `newline`, of which `\r\n` is one too.
pub(crate) fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\r' | '\n' | '\u{85}' | '\u{B}' | '\u{C}' | '\u{2028}' | '\u{2029}'
    )
}

/// The length in bytes of the newline that starts `rest`, if one does: `\r\n` is one newline.
pub(crate) fn newline_len(rest: &str) -> Option<usize> {
    if rest.starts_with("\r\n") {
        return Some(2);
    }
    rest.chars()
        .next()
        .filter(|&c| is_newline(c))
        .map(char::len_utf8)
}

/// Whether `c` may not appear as itself anywhere in a document: control characters, the controls
/// of the text's direction, and the byte order mark, which only the document's first character
/// may be. A quoted string holds them as escapes, `\u{7F}`.
pub(crate) fn is_disallowed(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}'
            | '\u{E}'..='\u{1F}'
            | '\u{7F}'
            | '\u{200E}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// The error's message for `c`, which [`is_disallowed`].
pub(crate) fn not_allowed(c: char) -> String {
    format!(
        "the character U+{:04X} is not allowed in a document",
        u32::from(c)
    )
}

/// Whether an identifier may hold `c`: KDL's `identifier-char`.
pub(crate) fn is_identifier_char(c: char) -> bool {
    !is_space(c)
        && !is_newline(c)
        && !is_disallowed(c)
        && !matches!(
            c,
            '\\' | '/' | '(' | ')' | '{' | '}' | ';' | '[' | ']' | '"' | '#' | '='
        )
}

/// What a bare word, a run of identifier characters, is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bare {
    /// An identifier: a string.
    Identifier,
    /// A number, or nothing valid: it starts with a digit, or a sign and a digit.
    Number,
    /// Nothing valid: it starts with a point and a digit, `.5` or `-.5`, which is no number, since
    /// a number needs a digit before its point, and which an identifier may not look like.
    PointNumber,
    /// Nothing valid: a keyword without its `#`, `true` or `nan`.
    Keyword,
}

/// What `word`, a run of identifier characters, is, by KDL's `identifier-string`: an identifier
/// may not start like a number, nor be a keyword.
pub(crate) fn bare(word: &str) -> Bare {
    let unsigned = word.strip_prefix(['+', '-']).unwrap_or(word);
    let starts_with_digit = |text: &str| text.starts_with(|c: char| c.is_ascii_digit());
    if starts_with_digit(unsigned) {
        Bare::Number
    } else if unsigned.strip_prefix('.').is_some_and(starts_with_digit) {
        Bare::PointNumber
    } else if matches!(word, "true" | "false" | "null" | "inf" | "-inf" | "nan") {
        Bare::Keyword
    } else {
        Bare::Identifier
    }
}

/// Whether `text` may be written as an identifier, without quotes.
pub(crate) fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_identifier_char) && bare(text) == Bare::Identifier
}
