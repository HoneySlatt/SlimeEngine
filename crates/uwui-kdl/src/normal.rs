//! A document written in the normal form of KDL's official tests (`tests/kdl-suite/SUITE-README.md`).
//!
//! The writer reads the tree through the crate's public API, never the text, and never recurses:
//! a stack holds the nodes left to write at each level.

use std::fmt::Write as _;

use crate::number::{self, Written};
use crate::write::{INFINITY, NAN, NEGATIVE_INFINITY, write_string};
use crate::{Document, Entry, Node, Nodes, Value};

/// The spaces of one level of indentation.
const INDENTATION: &str = "    ";

/// `document` in the normal form of KDL's official tests.
pub(crate) fn write(document: &Document<'_>) -> String {
    let mut text = String::new();
    let mut levels: Vec<Nodes<'_, '_>> = vec![document.nodes()];
    while let Some(nodes) = levels.last_mut() {
        let next = nodes.next();
        let depth = levels.len() - 1;
        match next {
            Some(node) => {
                indent(&mut text, depth);
                write_node(&mut text, node);
                let children = node.children();
                if children.len() == 0 {
                    text.push('\n');
                } else {
                    text.push_str(" {\n");
                    levels.push(children);
                }
            }
            None => {
                levels.pop();
                if depth > 0 {
                    indent(&mut text, depth - 1);
                    text.push_str("}\n");
                }
            }
        }
    }
    // A document without nodes is a newline alone, as the suite's empty documents are.
    if text.is_empty() {
        text.push('\n');
    }
    text
}

/// `depth` levels of indentation.
fn indent(text: &mut String, depth: usize) {
    for _ in 0..depth {
        text.push_str(INDENTATION);
    }
}

/// `node`'s line up to its block: its annotation, its name, its arguments, then its properties
/// sorted by name, each name's last alone.
fn write_node(text: &mut String, node: Node<'_, '_>) {
    if let Some(annotation) = node.annotation() {
        write_annotation(text, &annotation.text());
    }
    write_string(text, &node.name().text());
    for argument in node.arguments() {
        text.push(' ');
        write_value(text, &argument);
    }
    let mut properties: Vec<_> = node
        .entries()
        .filter_map(|entry| Some((entry.name()?.text(), entry)))
        .collect();
    // A stable sort keeps the properties of one name in their order: the last of each is kept.
    properties.sort_by(|(one, _), (other, _)| one.cmp(other));
    for (index, (name, entry)) in properties.iter().enumerate() {
        let overridden = properties
            .get(index + 1)
            .is_some_and(|(next, _)| next == name);
        if !overridden {
            text.push(' ');
            write_string(text, name);
            text.push('=');
            write_value(text, entry);
        }
    }
}

/// `entry`'s value, after its annotation.
fn write_value(text: &mut String, entry: &Entry<'_, '_>) {
    if let Some(annotation) = entry.annotation() {
        write_annotation(text, &annotation.text());
    }
    match entry.value() {
        Value::String(string) => write_string(text, &string.text()),
        Value::Number(number) => write_number(text, number.text()),
        Value::Bool(true) => text.push_str("#true"),
        Value::Bool(false) => text.push_str("#false"),
        Value::Null => text.push_str("#null"),
    }
}

/// A type annotation, `(name)`.
fn write_annotation(text: &mut String, name: &str) {
    text.push('(');
    write_string(text, name);
    text.push(')');
}

/// The number written `written`, which the reader checked, in its simplest form: in decimal,
/// without its underscores, its `+` or its leading zeros; a float keeps its digits, its exponent
/// written after `E` with its sign.
fn write_number(text: &mut String, written: &str) {
    let number = match number::written(written) {
        Some(Written::Digits(number)) => number,
        Some(Written::Infinity) => {
            text.push_str(INFINITY);
            return;
        }
        Some(Written::NegativeInfinity) => {
            text.push_str(NEGATIVE_INFINITY);
            return;
        }
        // The reader keeps only numbers that `number::written` reads: none is `None`.
        Some(Written::NotANumber) | None => {
            text.push_str(NAN);
            return;
        }
    };
    let whole = if number.radix == 10 {
        without_leading_zeros(&without_underscores(number.whole)).to_owned()
    } else {
        in_decimal(number.whole, number.radix)
    };
    // A whole number has no negative zero; a float has one.
    if number.negative && (number.is_float() || whole != "0") {
        text.push('-');
    }
    text.push_str(&whole);
    if let Some(fraction) = number.fraction {
        text.push('.');
        text.push_str(&without_underscores(fraction));
    }
    if let Some(exponent) = number.exponent {
        let digits = without_underscores(exponent.trim_start_matches(['+', '-']));
        text.push('E');
        text.push(if exponent.starts_with('-') { '-' } else { '+' });
        text.push_str(without_leading_zeros(&digits));
    }
}

/// `digits` without their underscores.
fn without_underscores(digits: &str) -> String {
    digits.chars().filter(|&c| c != '_').collect()
}

/// `digits` without their leading zeros, `0` for zero.
fn without_leading_zeros(digits: &str) -> &str {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() { "0" } else { trimmed }
}

/// The whole number written `digits` in `radix`, underscores among them, written in decimal,
/// exactly at any length: in time quadratic in its length.
fn in_decimal(digits: &str, radix: u32) -> String {
    /// The base of a limb, nine decimal digits.
    const LIMB: u64 = 1_000_000_000;
    // The number's limbs, the least significant first.
    let mut limbs: Vec<u64> = Vec::new();
    for digit in digits.chars().filter_map(|c| c.to_digit(radix)) {
        let mut carry = u64::from(digit);
        for limb in &mut limbs {
            let value = *limb * u64::from(radix) + carry;
            *limb = value % LIMB;
            carry = value / LIMB;
        }
        if carry > 0 {
            limbs.push(carry);
        }
    }
    let Some((most_significant, rest)) = limbs.split_last() else {
        return "0".to_owned();
    };
    let mut decimal = most_significant.to_string();
    for limb in rest.iter().rev() {
        // Writing to a `String` cannot fail.
        let _ = write!(decimal, "{limb:09}");
    }
    decimal
}

#[cfg(test)]
mod tests {
    /// `text`'s normal form.
    fn normal(text: &str) -> String {
        crate::read(text).expect("read").normal_form()
    }

    #[test]
    fn a_number_is_written_in_decimal_without_what_its_value_does_not_need() {
        assert_eq!(
            normal("n -0 -0.0 -0x0 00.5 1e05 1E-0 +1 0b0_ 0o1_7"),
            "n 0 -0.0 0 0.5 1E+5 1E-0 1 0 15\n"
        );
        assert_eq!(
            normal("n 0xffffffffffffffffffffffffffffffff"),
            "n 340282366920938463463374607431768211455\n"
        );
    }

    #[test]
    fn a_string_is_quoted_only_where_it_must_be_and_escaped_where_it_must() {
        assert_eq!(
            normal(r#"n "a" "10" "-1" ".5" "true" "" "a b" "\u{1}" "\u{85}" "\u{FEFF}" "\t""#),
            "n a \"10\" \"-1\" \".5\" \"true\" \"\" \"a b\" \"\\u{1}\" \"\\u{85}\" \"\\u{feff}\" \"\\t\"\n"
        );
    }

    #[test]
    fn a_tree_is_written_with_its_blocks_closed_at_their_level() {
        assert_eq!(
            normal("a { b { c; d }; e }; f"),
            "a {\n    b {\n        c\n        d\n    }\n    e\n}\nf\n"
        );
    }
}
