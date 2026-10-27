//! What a field of numbers reads (`docs/adr/0151-rich-widgets.md`): an expression of numbers, the
//! four operations, parentheses, a sign, `pi` and a few functions, a number, `pi`, a call or a part
//! in parentheses written with a unit of the field's family, converted to its base unit.

use super::Units;

/// The functions that an expression calls, by their names, and how many arguments each takes at
/// least: `min` and `max` take two or more.
const FUNCTIONS: [(&str, usize); 6] = [
    ("sqrt", 1),
    ("sin", 1),
    ("cos", 1),
    ("abs", 1),
    ("min", 2),
    ("max", 2),
];

/// How deep parentheses, signs and calls nest at most: a text nested deeper reads as nothing,
/// rather than reading it at the cost of the stack.
const DEPTH: usize = 64;

/// The number that `text` reads as, for a field of numbers of the family `units`: an expression of
/// numbers, `+`, `-`, `*`, `/`, parentheses, a sign, `pi`, and the functions `sqrt`, `sin`, `cos`,
/// `abs`, `min` and `max`. A number, `pi`, a call or a part in parentheses may be followed by a unit
/// of the field's family, `10cm`, `90°`, `(pi/2) rad`, `250ms`, which converts it to the family's
/// base unit, meters, degrees or seconds; a field of no family takes no unit. Every number of an
/// expression is in the field's base unit: `sin` and `cos` take degrees in a field of angles, and
/// radians elsewhere, as Rust's do. None for a text that reads as no expression, nested more than
/// 64 deep, a unit of another family, or a part or a result that is not a finite number.
pub(crate) fn evaluate(text: &str, units: Units) -> Option<f64> {
    let mut reader = Reader {
        text: text.as_bytes(),
        at: 0,
        units,
        depth: 0,
    };
    let value = reader.sum()?;
    reader.blanks();
    (reader.at == reader.text.len() && value.is_finite()).then_some(value)
}

/// The symbol of the base unit of `units`, as a field shows it after its number; none for no family.
pub(crate) fn symbol(units: Units) -> Option<&'static str> {
    match units {
        Units::None => None,
        Units::Length => Some("m"),
        Units::Angle => Some("°"),
        Units::Time => Some("s"),
    }
}

/// `value`, if it is a finite number.
fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

/// How many of the base unit of `units` one `name` makes; none for a unit of another family.
fn factor(units: Units, name: &str) -> Option<f64> {
    Some(match (units, name) {
        (Units::Length, "mm") => 0.001,
        (Units::Length, "cm") => 0.01,
        (Units::Length, "m") => 1.0,
        (Units::Length, "km") => 1000.0,
        (Units::Angle, "°" | "deg") => 1.0,
        (Units::Angle, "rad") => 180.0 / std::f64::consts::PI,
        (Units::Time, "ms") => 0.001,
        (Units::Time, "s") => 1.0,
        _ => return None,
    })
}

/// A text read from its start, by a recursive descent of its grammar.
struct Reader<'a> {
    text: &'a [u8],
    at: usize,
    units: Units,
    /// How deep the reading nests now.
    depth: usize,
}

impl Reader<'_> {
    /// Terms added and taken away.
    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.at += 1;
                    value = finite(value + self.product()?)?;
                }
                Some(b'-') => {
                    self.at += 1;
                    value = finite(value - self.product()?)?;
                }
                _ => return Some(value),
            }
        }
    }

    /// Factors multiplied and divided.
    fn product(&mut self) -> Option<f64> {
        let mut value = self.signed()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.at += 1;
                    value = finite(value * self.signed()?)?;
                }
                Some(b'/') => {
                    self.at += 1;
                    value = finite(value / self.signed()?)?;
                }
                _ => return Some(value),
            }
        }
    }

    /// A factor and its unit, after any signs, nested no deeper than [`DEPTH`].
    fn signed(&mut self) -> Option<f64> {
        self.depth += 1;
        if self.depth > DEPTH {
            return None;
        }
        let value = match self.peek() {
            Some(b'-') => {
                self.at += 1;
                self.signed().map(|value| -value)
            }
            Some(b'+') => {
                self.at += 1;
                self.signed()
            }
            _ => self.factor().and_then(|value| finite(value * self.unit()?)),
        };
        self.depth -= 1;
        value
    }

    /// A number and its unit, `pi`, a call of a function, or an expression in parentheses.
    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            b'(' => {
                self.at += 1;
                let value = self.sum()?;
                self.expect(b')')?;
                Some(value)
            }
            byte if byte.is_ascii_digit() || byte == b'.' => self.number(),
            byte if byte.is_ascii_alphabetic() => {
                let name = self.word();
                if name == "pi" {
                    return Some(std::f64::consts::PI);
                }
                let &(_, least) = FUNCTIONS.iter().find(|(function, _)| *function == name)?;
                self.expect(b'(')?;
                let mut arguments = vec![self.sum()?];
                while self.peek() == Some(b',') {
                    self.at += 1;
                    arguments.push(self.sum()?);
                }
                self.expect(b')')?;
                if arguments.len() < least || (least == 1 && arguments.len() > 1) {
                    return None;
                }
                let first = arguments[0];
                // The angles of a field of angles are in degrees.
                let angle = match self.units {
                    Units::Angle => first.to_radians(),
                    _ => first,
                };
                finite(match name.as_str() {
                    "sqrt" => first.sqrt(),
                    "sin" => angle.sin(),
                    "cos" => angle.cos(),
                    "abs" => first.abs(),
                    "min" => arguments.into_iter().fold(f64::INFINITY, f64::min),
                    _ => arguments.into_iter().fold(f64::NEG_INFINITY, f64::max),
                })
            }
            _ => None,
        }
    }

    /// A number as Rust writes it: digits, a decimal point, an exponent.
    fn number(&mut self) -> Option<f64> {
        let start = self.at;
        let digits = |reader: &mut Self| {
            while reader
                .text
                .get(reader.at)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                reader.at += 1;
            }
        };
        digits(self);
        if self.text.get(self.at) == Some(&b'.') {
            self.at += 1;
            digits(self);
        }
        // An exponent, `1e3`, where digits follow the `e`, which no unit starts with.
        if matches!(self.text.get(self.at), Some(b'e' | b'E')) {
            let mut after = self.at + 1;
            if matches!(self.text.get(after), Some(b'+' | b'-')) {
                after += 1;
            }
            if self
                .text
                .get(after)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.at = after;
                digits(self);
            }
        }
        std::str::from_utf8(&self.text[start..self.at])
            .ok()?
            .parse()
            .ok()
    }

    /// The factor of the unit after a factor: 1 for none, none for a unit that the field's family
    /// does not hold.
    fn unit(&mut self) -> Option<f64> {
        let start = self.at;
        self.blanks();
        if self.text[self.at..].starts_with("°".as_bytes()) {
            self.at += "°".len();
            return factor(self.units, "°");
        }
        if self
            .text
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_alphabetic())
        {
            let name = self.word();
            return factor(self.units, &name);
        }
        self.at = start;
        Some(1.0)
    }

    /// The letters from here.
    fn word(&mut self) -> String {
        let start = self.at;
        while self
            .text
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_alphabetic())
        {
            self.at += 1;
        }
        String::from_utf8_lossy(&self.text[start..self.at]).into_owned()
    }

    /// The next byte that is not a blank, left to read.
    fn peek(&mut self) -> Option<u8> {
        self.blanks();
        self.text.get(self.at).copied()
    }

    /// Reads `byte`, after blanks; none where another comes.
    fn expect(&mut self, byte: u8) -> Option<()> {
        (self.peek() == Some(byte)).then(|| self.at += 1)
    }

    /// Steps over blanks.
    fn blanks(&mut self) {
        while self
            .text
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.at += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Units, evaluate};

    fn near(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn numbers_and_operations_read_as_rust_and_arithmetic_do() {
        assert_eq!(evaluate("1.5", Units::None), Some(1.5));
        assert_eq!(evaluate(" -4.2 ", Units::None), Some(-4.2));
        assert_eq!(evaluate("1e3", Units::None), Some(1000.0));
        assert_eq!(evaluate(".5", Units::None), Some(0.5));
        assert_eq!(evaluate("1 + 2 * 3", Units::None), Some(7.0));
        assert_eq!(evaluate("(1 + 2) * 3", Units::None), Some(9.0));
        assert_eq!(evaluate("8 / 2 / 2", Units::None), Some(2.0));
        assert_eq!(evaluate("-(2 - 5)", Units::None), Some(3.0));
        assert_eq!(evaluate("2 * -3", Units::None), Some(-6.0));
    }

    #[test]
    fn pi_and_the_functions_read_their_arguments() {
        assert!(near(evaluate("sqrt(2)/2", Units::None), 2f64.sqrt() / 2.0));
        assert!(near(evaluate("sin(pi/6)", Units::None), 0.5));
        assert!(near(evaluate("cos(0)", Units::None), 1.0));
        assert_eq!(evaluate("abs(-3)", Units::None), Some(3.0));
        assert_eq!(evaluate("min(4, 2, 8)", Units::None), Some(2.0));
        assert_eq!(evaluate("max(1, 2*3)", Units::None), Some(6.0));
        assert_eq!(evaluate("min(4)", Units::None), None, "min takes two");
        assert_eq!(evaluate("sqrt(1, 2)", Units::None), None, "sqrt takes one");
        assert_eq!(evaluate("tan(1)", Units::None), None, "no such function");
    }

    #[test]
    fn units_of_the_fields_family_convert_to_its_base() {
        assert_eq!(evaluate("10cm", Units::Length), Some(0.1));
        assert!(near(evaluate("1m + 20cm", Units::Length), 1.2));
        assert_eq!(evaluate("2 km", Units::Length), Some(2000.0));
        assert_eq!(evaluate("90°", Units::Angle), Some(90.0));
        assert_eq!(evaluate("45deg", Units::Angle), Some(45.0));
        assert!(near(
            evaluate("1rad", Units::Angle),
            180.0 / std::f64::consts::PI
        ));
        assert_eq!(evaluate("250ms", Units::Time), Some(0.25));
        // A unit of another family, or any unit in a field of none, reads as nothing.
        assert_eq!(evaluate("10cm", Units::Angle), None);
        assert_eq!(evaluate("10cm", Units::None), None);
    }

    #[test]
    fn what_reads_as_no_expression_or_no_finite_number_gives_none() {
        for text in [
            "",
            "2,5",
            "1 +",
            "(1",
            "1)",
            "abc",
            "1/0",
            "sqrt(-1)",
            "1 2",
            "2e",
            "min(sqrt(-1), 2)",
            "1/(1/0)",
            "1\u{a0}+ 2",
            "é",
        ] {
            assert_eq!(evaluate(text, Units::None), None, "{text:?}");
        }
        // Nested past its depth, it reads as nothing, the stack spared.
        let deep = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
        assert_eq!(evaluate(&deep, Units::None), None);
        assert_eq!(evaluate(&"-".repeat(100_000), Units::None), None);
    }

    #[test]
    fn a_field_of_angles_reads_degrees_and_a_unit_after_any_factor() {
        assert!(near(evaluate("sin(30)", Units::Angle), 0.5));
        assert!(near(evaluate("sin(pi/6)", Units::None), 0.5));
        assert!(near(evaluate("(pi/2) rad", Units::Angle), 90.0));
        assert!(near(evaluate("pi rad", Units::Angle), 180.0));
        assert_eq!(evaluate("90 °", Units::Angle), Some(90.0));
        assert_eq!(evaluate("1e3cm", Units::Length), Some(10.0));
        assert_eq!(evaluate("(1 + 1) m", Units::Length), Some(2.0));
    }
}
