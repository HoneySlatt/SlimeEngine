//! Property paths: where a part of a value is, as `light.intensity` or `ids[2]`.

use std::fmt;
use std::str::FromStr;

/// One step of a [`Path`]: a field, by name, or an element, by index.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// A field of a struct, of an enum's variant, or of a tuple, by name: `0`, `1`, ... for a
    /// tuple's.
    Field(String),
    /// An element of a list or of an array, by index.
    Index(usize),
}

/// Where a part of a value is: the fields and elements to go through from the value, in order.
/// It reads as fields joined by `.`, an element as its index in brackets: `color.r`,
/// `ids[2]`, `0[3].1` (the tuple struct's first field, its fourth element, that tuple's second
/// field). The empty path is the value itself.
///
/// A field of an enum names a field of the variant the value holds: `intensity` in a point light.
///
/// ```
/// use slime_reflect::{Path, Step};
///
/// let path: Path = "lights[2].color.r".parse().expect("a path");
/// assert_eq!(path.steps()[1], Step::Index(2));
/// assert_eq!(path.to_string(), "lights[2].color.r");
/// assert_eq!(Path::default().field("ids").index(0), "ids[0]".parse().expect("a path"));
/// assert!("lights..color".parse::<Path>().is_err());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Path(Vec<Step>);

impl Path {
    /// Its steps, from the value.
    pub fn steps(&self) -> &[Step] {
        &self.0
    }

    /// Whether it is the value itself, without steps.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// This path, then the field `name`.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>) -> Self {
        self.0.push(Step::Field(name.into()));
        self
    }

    /// This path, then the element at `index`.
    #[must_use]
    pub fn index(mut self, index: usize) -> Self {
        self.0.push(Step::Index(index));
        self
    }

    /// This path, then `step`.
    #[must_use]
    pub fn then(mut self, step: Step) -> Self {
        self.0.push(step);
        self
    }

    /// `step`, then this path.
    #[must_use]
    pub(crate) fn after(mut self, step: Step) -> Self {
        self.0.insert(0, step);
        self
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, step) in self.0.iter().enumerate() {
            match step {
                Step::Field(name) if index == 0 => write!(f, "{name}")?,
                Step::Field(name) => write!(f, ".{name}")?,
                Step::Index(element) => write!(f, "[{element}]")?,
            }
        }
        Ok(())
    }
}

/// Why a text is not a [`Path`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsePathError {
    path: String,
}

impl fmt::Display for ParsePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a property path: fields of letters, digits and `_` joined by `.`, and \
             indices in brackets",
            self.path
        )
    }
}

impl std::error::Error for ParsePathError {}

impl FromStr for Path {
    type Err = ParsePathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParsePathError {
            path: text.to_owned(),
        };
        let name_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let mut steps = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('[') {
                let end = after.find(']').ok_or_else(error)?;
                let digits = &after[..end];
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(error());
                }
                steps.push(Step::Index(digits.parse().map_err(|_| error())?));
                rest = &after[end + 1..];
                continue;
            }
            // A field: first, or after a `.`.
            let after = if steps.is_empty() {
                rest
            } else {
                rest.strip_prefix('.').ok_or_else(error)?
            };
            let end = after.find(|c| !name_char(c)).unwrap_or(after.len());
            if end == 0 {
                return Err(error());
            }
            steps.push(Step::Field(after[..end].to_owned()));
            rest = &after[end..];
        }
        Ok(Self(steps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_read_and_write_back() {
        for text in ["", "a", "a.b", "ids[0]", "0[3].1", "[2]", "[2][3].x_y"] {
            let path: Path = text.parse().expect(text);
            assert_eq!(path.to_string(), text);
        }
        for bad in [
            ".a", "a.", "a..b", "a[", "a[]", "a[x]", "a b", "a[1]b", "a-b",
        ] {
            assert!(bad.parse::<Path>().is_err(), "{bad}");
        }
        let path: Path = "0[3].1".parse().expect("a path");
        assert_eq!(
            path.steps(),
            [
                Step::Field("0".to_owned()),
                Step::Index(3),
                Step::Field("1".to_owned())
            ]
        );
    }
}
