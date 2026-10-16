//! Why a value cannot be read, set or registered.

use std::fmt;

use crate::info::Range;
use crate::path::{Path, Step};

/// Why reflection refuses a value or a path, and where: the path of the part at fault, from the
/// value that was given.
#[derive(Clone, Debug, PartialEq)]
pub struct ReflectError {
    path: Path,
    problem: Problem,
}

/// What is wrong, in a [`ReflectError`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Problem {
    /// The part is not a value of the type it should be: a text where a number goes.
    WrongKind {
        /// The type the part has, by name.
        expected: String,
        /// The kind of value found ([`Value::kind`](crate::Value::kind)).
        found: &'static str,
    },
    /// A field that the value leaves out, which neither the field nor its type gives a default
    /// to.
    MissingField(String),
    /// A field that the value gives twice, or under its name and one of its aliases.
    DuplicateField(String),
    /// A field that the type does not have.
    UnknownField {
        /// The field found.
        field: String,
        /// The fields the type has.
        known: Vec<String>,
    },
    /// A variant that the enum does not have.
    UnknownVariant {
        /// The variant found.
        variant: String,
        /// The variants the enum has.
        known: Vec<String>,
    },
    /// An array of another length than its type's.
    WrongLength {
        /// The length of the type.
        expected: usize,
        /// The length found.
        found: usize,
    },
    /// A number outside the range of the field it is set in, or of its type's width.
    OutOfRange {
        /// The number.
        value: f64,
        /// The values it may take.
        range: Range,
    },
    /// The path names no part of the value.
    NoSuchPath,
    /// A type that the registry does not hold, by name.
    NotRegistered(String),
    /// A value that its type refuses for a reason of its own, such as an entity handle without a
    /// generation.
    Invalid(String),
    /// A value of a version of its type newer than the one the program knows.
    NewerVersion {
        /// The version the value is of.
        found: u32,
        /// The type's version in the program.
        known: u32,
    },
    /// A value of an earlier version of its type that cannot be brought to the next one: the type
    /// has no upgrade from it, or its upgrade refuses the value, for `reason`.
    Upgrade {
        /// The version the value is of.
        from: u32,
        /// Why it cannot be brought to the next.
        reason: String,
    },
}

impl ReflectError {
    /// The error `problem`, at the value itself; [`ReflectError::within`] moves it into the part
    /// it belongs to.
    pub fn new(problem: Problem) -> Self {
        Self {
            path: Path::default(),
            problem,
        }
    }

    /// The error `problem` at `path`.
    pub fn at(path: Path, problem: Problem) -> Self {
        Self { path, problem }
    }

    /// The same error, in the part that `step` leads to: what a type's code returns when one of
    /// its fields refuses a value.
    #[must_use]
    pub fn within(self, step: Step) -> Self {
        Self {
            path: self.path.after(step),
            problem: self.problem,
        }
    }

    /// Where the error is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What is wrong.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "the value: {}", self.problem)
        } else {
            write!(f, "`{}`: {}", self.path, self.problem)
        }
    }
}

/// What is wrong, without where: `ReflectError` says where.
impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::WrongKind { expected, found } => {
                write!(f, "a value of `{expected}` is expected, not {found}")
            }
            Problem::MissingField(field) => write!(f, "the field `{field}` is missing"),
            Problem::DuplicateField(field) => write!(f, "the field `{field}` is given twice"),
            Problem::UnknownField { field, known } => {
                write!(f, "no field `{field}`, where there are {}", list(known))
            }
            Problem::UnknownVariant { variant, known } => {
                write!(f, "no variant `{variant}`, where there are {}", list(known))
            }
            Problem::WrongLength { expected, found } => {
                write!(f, "{found} elements, where there are {expected}")
            }
            Problem::OutOfRange { value, .. } if !value.is_finite() => {
                write!(f, "{value} is not a finite number")
            }
            Problem::OutOfRange { value, range } => write!(f, "{value} is not {range}"),
            Problem::NoSuchPath => write!(f, "names nothing"),
            Problem::NotRegistered(name) => write!(f, "no type `{name}` is registered"),
            Problem::Invalid(reason) => write!(f, "{reason}"),
            Problem::NewerVersion { found, known } => write!(
                f,
                "a value of version {found} of its type, newer than version {known}, which this \
                 program knows"
            ),
            Problem::Upgrade { from, reason } => {
                write!(
                    f,
                    "cannot be brought from version {from} of its type: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ReflectError {}

/// `a`, `b` and `c`, or `none`.
fn list(names: &[String]) -> String {
    match names {
        [] => "none".to_owned(),
        [only] => format!("`{only}`"),
        [first @ .., last] => {
            let first: Vec<String> = first.iter().map(|name| format!("`{name}`")).collect();
            format!("{} and `{last}`", first.join(", "))
        }
    }
}

/// Why a registry refuses a type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistryError {
    /// Another type has this name, or this alias.
    NameTaken {
        /// The name, or the alias.
        name: String,
        /// The type that has it.
        by: String,
    },
    /// The description is not one: integers of another width than 8, 16, 32 or 64 bits, two
    /// fields or variants of one name.
    Invalid {
        /// The type described.
        ty: String,
        /// What is wrong with its description.
        reason: String,
    },
    /// The type names a type that the registry does not hold.
    UnknownType {
        /// The type described.
        ty: String,
        /// The type it names.
        names: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTaken { name, by } => write!(f, "the type `{by}` has the name `{name}`"),
            Self::Invalid { ty, reason } => write!(f, "`{ty}` is not described: {reason}"),
            Self::UnknownType { ty, names } => {
                write!(
                    f,
                    "`{ty}` holds values of `{names}`, which is not registered"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_say_where_and_what() {
        let error = ReflectError::new(Problem::WrongKind {
            expected: "f32".to_owned(),
            found: "text",
        })
        .within(Step::Field("intensity".to_owned()))
        .within(Step::Field("light".to_owned()));
        assert_eq!(
            error.to_string(),
            "`light.intensity`: a value of `f32` is expected, not text"
        );
        let unknown = ReflectError::new(Problem::UnknownField {
            field: "colour".to_owned(),
            known: vec![
                "color".to_owned(),
                "range".to_owned(),
                "intensity".to_owned(),
            ],
        });
        assert_eq!(
            unknown.to_string(),
            "the value: no field `colour`, where there are `color`, `range` and `intensity`"
        );
    }
}
