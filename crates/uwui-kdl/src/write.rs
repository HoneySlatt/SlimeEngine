//! KDL's values written as text: the one place that writes them, which the normal form and uwui's
//! writers of documents share (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

use std::fmt::Write as _;

use crate::chars;

/// Writes `string`, a node's or a property's name or a text, as KDL reads it back: an identifier
/// where it may be one, `hierarchy`; quoted otherwise, `"editor 1"`, `"true"`, `""`, a character
/// that a quoted string may not hold as itself written as an escape, `"\n"`, `"\u{7f}"`.
///
/// ```
/// let mut text = String::new();
/// uwui_kdl::write_string(&mut text, "editor 1");
/// text.push(' ');
/// uwui_kdl::write_string(&mut text, "viewport");
/// assert_eq!(text, r#""editor 1" viewport"#);
/// ```
pub fn write_string(text: &mut String, string: &str) {
    if chars::is_identifier(string) {
        text.push_str(string);
        return;
    }
    text.push('"');
    for c in string.chars() {
        match c {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            '\u{8}' => text.push_str("\\b"),
            '\u{C}' => text.push_str("\\f"),
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            c if chars::is_newline(c) || chars::is_disallowed(c) => {
                // Writing to a `String` cannot fail.
                let _ = write!(text, "\\u{{{:x}}}", u32::from(c));
            }
            c => text.push(c),
        }
    }
    text.push('"');
}

/// Writes `value` so that [`Number::to_f32`](crate::Number::to_f32) reads back the same bits: a
/// finite value in the shortest decimal that does, `0.25`, `1`, `-0`; `#inf`, `#-inf`, and `#nan`
/// for every value that is not a number, whose bits are not kept.
///
/// ```
/// let mut text = String::new();
/// uwui_kdl::write_f32(&mut text, 0.55);
/// assert_eq!(text, "0.55");
/// ```
pub fn write_f32(text: &mut String, value: f32) {
    if value.is_nan() {
        text.push_str(NAN);
    } else if value == f32::INFINITY {
        text.push_str(INFINITY);
    } else if value == f32::NEG_INFINITY {
        text.push_str(NEGATIVE_INFINITY);
    } else {
        // `Display` writes the shortest decimal that parses back to the same float, never with an
        // exponent: a decimal of KDL's grammar. The tests check a sample of every `f32`; every
        // finite one was checked once (`docs/measurements.md`, M6d.3c2). Writing to a `String`
        // cannot fail.
        let _ = write!(text, "{value}");
    }
}

/// Writes `value`, a whole number, in decimal: `-12`.
pub fn write_integer(text: &mut String, value: i64) {
    // Writing to a `String` cannot fail.
    let _ = write!(text, "{value}");
}

/// How KDL writes infinity.
pub(crate) const INFINITY: &str = "#inf";

/// How KDL writes negative infinity.
pub(crate) const NEGATIVE_INFINITY: &str = "#-inf";

/// How KDL writes what is not a number.
pub(crate) const NAN: &str = "#nan";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::{self, Written};

    /// `value` as [`write_f32`] writes it.
    fn written_f32(value: f32) -> String {
        let mut text = String::new();
        write_f32(&mut text, value);
        text
    }

    /// The two decimals that an `f32`'s shortest text and a read through an `f64` round twice to
    /// another `f32`, the least positive `f32`, the largest, a negative zero, and a sample of every
    /// pattern of bits: each is a decimal of KDL, read back to its bits.
    #[test]
    fn an_f32_written_reads_back_to_its_bits() {
        let special = [
            0x15ae_43fd,
            0x95ae_43fd,
            1,
            0x8000_0001,
            f32::MAX.to_bits(),
            f32::MIN.to_bits(),
            (-0.0_f32).to_bits(),
            0,
        ];
        let sample = (0..u32::MAX).step_by(9_973);
        for bits in special.into_iter().chain(sample) {
            let value = f32::from_bits(bits);
            let text = written_f32(value);
            if !value.is_finite() {
                continue;
            }
            assert!(
                matches!(number::written(&text), Some(Written::Digits(_))),
                "{text} is not a decimal of KDL"
            );
            assert_eq!(number::float32(&text).to_bits(), bits, "{text}");
        }
        // Read through an `f64`, the first rounds twice, to the next `f32`.
        let text = written_f32(f32::from_bits(0x15ae_43fd));
        assert_eq!(text, "0.00000000000000000000000007038531");
        assert_ne!((number::float(&text) as f32).to_bits(), 0x15ae_43fd);
    }

    #[test]
    fn an_f32_is_written_in_its_shortest_decimal_and_what_is_no_number_by_its_keyword() {
        assert_eq!(written_f32(0.55), "0.55");
        assert_eq!(written_f32(0.25), "0.25");
        assert_eq!(written_f32(1.0), "1");
        assert_eq!(written_f32(-0.0), "-0");
        assert_eq!(written_f32(f32::INFINITY), "#inf");
        assert_eq!(written_f32(f32::NEG_INFINITY), "#-inf");
        assert_eq!(written_f32(f32::NAN), "#nan");
        assert_eq!(written_f32(-f32::NAN), "#nan");
    }

    #[test]
    fn a_string_is_an_identifier_where_it_may_be_and_quoted_otherwise() {
        let written = |string: &str| {
            let mut text = String::new();
            write_string(&mut text, string);
            text
        };
        assert_eq!(written("hierarchy"), "hierarchy");
        for (string, quoted) in [
            ("editor 1", r#""editor 1""#),
            ("", r#""""#),
            ("true", r#""true""#),
            ("-1", r#""-1""#),
            ("a#b", r##""a#b""##),
            ("a\"b\\c", r#""a\"b\\c""#),
            ("\u{7f}", r#""\u{7f}""#),
        ] {
            assert_eq!(written(string), quoted);
        }
        assert_eq!(written("é"), "é");
    }
}
