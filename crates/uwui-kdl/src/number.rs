//! The text of KDL's numbers: its grammar, which the reader checks, and its value as an `i64` or an
//! `f64`, read when it is asked for.

/// A number's text, cut into its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Written<'a> {
    /// `#inf`.
    Infinity,
    /// `#-inf`.
    NegativeInfinity,
    /// `#nan`.
    NotANumber,
    /// Digits, in a base, with a sign.
    Digits(Digits<'a>),
}

/// A number written with digits. Each part keeps its underscores; each holds at least one digit,
/// first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Digits<'a> {
    /// Whether it starts with `-`.
    pub(crate) negative: bool,
    /// 2, 8, 10 or 16.
    pub(crate) radix: u32,
    /// Its whole part, after its sign and its base's prefix, `0x`.
    pub(crate) whole: &'a str,
    /// Its fraction, after the point, in a decimal number.
    pub(crate) fraction: Option<&'a str>,
    /// Its exponent, after `e` or `E`, its sign included, in a decimal number.
    pub(crate) exponent: Option<&'a str>,
}

impl Digits<'_> {
    /// Whether it is written as a float: with a fraction or an exponent.
    pub(crate) fn is_float(&self) -> bool {
        self.fraction.is_some() || self.exponent.is_some()
    }
}

/// `text` cut into a number's parts, if it is a number by KDL's grammar: `#inf`, `#-inf`, `#nan`,
/// or `decimal`, `hex`, `octal` or `binary`.
pub(crate) fn written(text: &str) -> Option<Written<'_>> {
    match text {
        "#inf" => return Some(Written::Infinity),
        "#-inf" => return Some(Written::NegativeInfinity),
        "#nan" => return Some(Written::NotANumber),
        _ => {}
    }
    let negative = text.starts_with('-');
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (radix, rest) = [("0x", 16), ("0o", 8), ("0b", 2)]
        .into_iter()
        .find_map(|(prefix, radix)| Some((radix, unsigned.strip_prefix(prefix)?)))
        .unwrap_or((10, unsigned));
    let (whole, mut rest) = digits(rest, radix)?;
    let mut fraction = None;
    let mut exponent = None;
    if radix == 10 {
        if let Some(after_point) = rest.strip_prefix('.') {
            let (digits, after) = digits(after_point, 10)?;
            fraction = Some(digits);
            rest = after;
        }
        if let Some(after_e) = rest.strip_prefix(['e', 'E']) {
            let sign = usize::from(after_e.starts_with(['+', '-']));
            let (digits, after) = digits(&after_e[sign..], 10)?;
            exponent = Some(&after_e[..sign + digits.len()]);
            rest = after;
        }
    }
    rest.is_empty().then_some(Written::Digits(Digits {
        negative,
        radix,
        whole,
        fraction,
        exponent,
    }))
}

/// Digits of `radix` that start `text`, with the underscores among and after them, and the text
/// after them; none if `text` does not start with a digit.
fn digits(text: &str, radix: u32) -> Option<(&str, &str)> {
    let is_digit = |byte: u8| char::from(byte).is_digit(radix);
    if !text.bytes().next().is_some_and(is_digit) {
        return None;
    }
    let length = text
        .bytes()
        .position(|byte| byte != b'_' && !is_digit(byte))
        .unwrap_or(text.len());
    Some(text.split_at(length))
}

/// The value of the number written `text`, which the reader checked, if it is whole and an `i64`
/// holds it.
pub(crate) fn integer(text: &str) -> Option<i64> {
    let Some(Written::Digits(number)) = written(text) else {
        return None;
    };
    if number.is_float() {
        return None;
    }
    let mut magnitude: u64 = 0;
    for digit in number
        .whole
        .chars()
        .filter_map(|c| c.to_digit(number.radix))
    {
        magnitude = magnitude
            .checked_mul(u64::from(number.radix))?
            .checked_add(u64::from(digit))?;
    }
    if number.negative {
        0_i64.checked_sub_unsigned(magnitude)
    } else {
        i64::try_from(magnitude).ok()
    }
}

/// The value of the number written `text`, which the reader checked, as the nearest `f64`.
pub(crate) fn float(text: &str) -> f64 {
    match written(text) {
        Some(Written::Infinity) => f64::INFINITY,
        Some(Written::NegativeInfinity) => f64::NEG_INFINITY,
        // The reader keeps only numbers that `written` reads: none is `None`.
        Some(Written::NotANumber) | None => f64::NAN,
        Some(Written::Digits(number)) if number.radix == 10 => {
            // The standard library rounds a decimal text to the nearest float, at any length.
            let without_underscores: String = text.chars().filter(|&c| c != '_').collect();
            without_underscores.parse().unwrap_or(f64::NAN)
        }
        Some(Written::Digits(number)) => {
            let magnitude = whole_in_power_of_two_radix(number.whole, number.radix);
            if number.negative {
                -magnitude
            } else {
                magnitude
            }
        }
    }
}

/// The whole number written `digits` in `radix`, 2, 8 or 16, as the nearest `f64`, at any length.
///
/// Its leading 64 bits are rounded by the conversion of a `u64`, the bits beyond them kept as one
/// bit, set if any of them is, below the bits the conversion keeps: a tie between two floats is
/// only one if every bit beyond them is clear. The float is then scaled by a power of two, which
/// is exact.
fn whole_in_power_of_two_radix(digits: &str, radix: u32) -> f64 {
    let bits_per_digit = radix.trailing_zeros();
    let mut leading: u64 = 0;
    let mut leading_bits: u32 = 0;
    let mut bits_beyond: u64 = 0;
    let mut any_beyond_set = false;
    for digit in digits.chars().filter_map(|c| c.to_digit(radix)) {
        for shift in (0..bits_per_digit).rev() {
            let bit = (digit >> shift) & 1;
            if leading_bits == 64 {
                bits_beyond += 1;
                any_beyond_set |= bit == 1;
            } else if leading_bits > 0 || bit == 1 {
                leading = leading << 1 | u64::from(bit);
                leading_bits += 1;
            }
        }
    }
    if any_beyond_set {
        leading |= 1;
    }
    // A `u64` converts to the nearest float, ties to even.
    let rounded = leading as f64;
    rounded * power_of_two(bits_beyond)
}

/// 2 to the power `exponent`, exactly, or infinite beyond the largest float.
fn power_of_two(exponent: u64) -> f64 {
    const BIAS: u64 = 1023;
    const MANTISSA_BITS: u32 = 52;
    if exponent > BIAS {
        f64::INFINITY
    } else {
        f64::from_bits((exponent + BIAS) << MANTISSA_BITS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_read_by_kdl_s_grammar_alone() {
        for valid in [
            "0",
            "-0",
            "+1_",
            "1_2.3_4e-5_6",
            "1E+10",
            "0x0_",
            "0xA_f",
            "0o7",
            "0b1_0",
        ] {
            assert!(written(valid).is_some(), "{valid}");
        }
        for invalid in [
            "0x", "0x_1", "0X1", "1.", "1._5", "1.5.5", "1e", "1e_5", "0b2", "+-1",
        ] {
            assert!(written(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn a_whole_number_reads_as_an_i64_up_to_its_bounds() {
        assert_eq!(integer("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(integer("-0x8000000000000000"), Some(i64::MIN));
        assert_eq!(integer("9223372036854775808"), None);
        assert_eq!(integer("1e3"), None);
        assert_eq!(integer("0xe"), Some(14));
    }

    #[test]
    fn a_number_beyond_64_bits_rounds_to_the_nearest_float_ties_to_even() {
        let zeros = "0".repeat(16);
        // 2^53 + 1, shifted by 64 bits: a tie, which rounds to the even 2^53.
        let tie = format!("0x20000000000001{zeros}");
        assert_eq!(float(&tie), 2f64.powi(53 + 64));
        // One more bit, far below: above the tie, rounded up.
        let above = format!("0x20000000000001{}1", &zeros[1..]);
        assert_eq!(float(&above), (2f64.powi(53) + 2.0) * 2f64.powi(64));
        assert_eq!(float(&format!("-0x{}", "f".repeat(300))), f64::NEG_INFINITY);
        assert_eq!(float("0b0"), 0.0);
    }
}
