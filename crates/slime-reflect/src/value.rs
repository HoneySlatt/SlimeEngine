//! The value type: what a reflected value is, as plain data.

use crate::path::{Path, Step};

/// A value of a reflected type, as plain data: what passes between reflection, files, saves, the
/// inspector, and later scripts and the C ABI (`docs/adr/0080-reflection-and-serialization.md`).
/// It holds no Rust type: a value of a type with no Rust type behind it is one too.
///
/// References to assets and to entities are kinds of their own, and floats keep their width. A
/// struct's fields keep the order its type declares them in; a tuple's and a tuple struct's are
/// named `0`, `1`, ...
///
/// Equality compares floats as numbers: `NaN` equals nothing, and `0.0` equals `-0.0`.
/// [`diff`](crate::diff) compares them by their bits.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A boolean.
    Bool(bool),
    /// A signed integer, of any width up to 64 bits.
    Int(i64),
    /// An unsigned integer, of any width up to 64 bits.
    UInt(u64),
    /// A 32-bit float.
    F32(f32),
    /// A 64-bit float.
    F64(f64),
    /// Text.
    Text(String),
    /// A struct's fields, by name, in the order its type declares them.
    Struct(Vec<(String, Value)>),
    /// An enum's variant, by name, and its fields, as a struct's: none for a variant without data.
    Enum(String, Vec<(String, Value)>),
    /// A value that may be absent.
    Option(Option<Box<Value>>),
    /// The elements of a list or of an array.
    List(Vec<Value>),
    /// A reference to an asset: the 128 bits of its ID.
    Asset(u128),
    /// A reference to an entity, while its world runs: the bits of its handle, which a file never
    /// holds (`docs/adr/0079-the-engines-own-world.md`).
    Entity(u64),
}

impl Value {
    /// What kind of value it is, for messages: `a bool`, `text`, `a struct`, ...
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bool(_) => "a bool",
            Self::Int(_) => "a signed integer",
            Self::UInt(_) => "an unsigned integer",
            Self::F32(_) => "an f32",
            Self::F64(_) => "an f64",
            Self::Text(_) => "text",
            Self::Struct(_) => "a struct",
            Self::Enum(..) => "an enum",
            Self::Option(_) => "an option",
            Self::List(_) => "a list",
            Self::Asset(_) => "an asset",
            Self::Entity(_) => "an entity",
        }
    }

    /// The part of the value that `path` leads to, if it leads to one. An option that holds a value
    /// is gone through, to the value: `color.r` in an optional color that holds one.
    ///
    /// ```
    /// use slime_reflect::Value;
    ///
    /// let light = Value::Struct(vec![
    ///     ("intensity".to_owned(), Value::F32(30.0)),
    ///     ("ids".to_owned(), Value::List(vec![Value::Asset(7)])),
    /// ]);
    /// assert_eq!(light.get(&"intensity".parse().unwrap()), Some(&Value::F32(30.0)));
    /// assert_eq!(light.get(&"ids[0]".parse().unwrap()), Some(&Value::Asset(7)));
    /// assert_eq!(light.get(&"range".parse().unwrap()), None);
    /// ```
    pub fn get(&self, path: &Path) -> Option<&Value> {
        path.steps()
            .iter()
            .try_fold(self, |value, step| value.child(step))
    }

    /// The part of the value that `path` leads to, to change, if it leads to one.
    pub fn get_mut(&mut self, path: &Path) -> Option<&mut Value> {
        path.steps()
            .iter()
            .try_fold(self, |value, step| value.child_mut(step))
    }

    /// The part that `step` leads to: through an option that holds a value, the value's.
    pub(crate) fn child(&self, step: &Step) -> Option<&Value> {
        match (self, step) {
            (Self::Option(Some(value)), _) => value.child(step),
            (Self::Struct(fields) | Self::Enum(_, fields), Step::Field(name)) => fields
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value),
            (Self::List(elements), Step::Index(index)) => elements.get(*index),
            _ => None,
        }
    }

    fn child_mut(&mut self, step: &Step) -> Option<&mut Value> {
        match (self, step) {
            (Self::Option(Some(value)), _) => value.child_mut(step),
            (Self::Struct(fields) | Self::Enum(_, fields), Step::Field(name)) => fields
                .iter_mut()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value),
            (Self::List(elements), Step::Index(index)) => elements.get_mut(*index),
            _ => None,
        }
    }

    /// Its number, if it is one, for a range to check.
    pub(crate) fn number(&self) -> Option<f64> {
        match *self {
            Self::Int(number) => Some(number as f64),
            Self::UInt(number) => Some(number as f64),
            Self::F32(number) => Some(f64::from(number)),
            Self::F64(number) => Some(number),
            _ => None,
        }
    }

    /// Whether it is `other`, floats compared by their bits.
    pub(crate) fn same(&self, other: &Value) -> bool {
        match (self, other) {
            (Self::F32(a), Self::F32(b)) => a.to_bits() == b.to_bits(),
            (Self::F64(a), Self::F64(b)) => a.to_bits() == b.to_bits(),
            (Self::Struct(a), Self::Struct(b)) => same_fields(a, b),
            (Self::Enum(a, a_fields), Self::Enum(b, b_fields)) => {
                a == b && same_fields(a_fields, b_fields)
            }
            (Self::Option(a), Self::Option(b)) => match (a, b) {
                (Some(a), Some(b)) => a.same(b),
                (None, None) => true,
                _ => false,
            },
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.same(b))
            }
            _ => self == other,
        }
    }
}

/// Whether two lists of fields have the same names, in order, and the same values.
fn same_fields(a: &[(String, Value)], b: &[(String, Value)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|((a_name, a), (b_name, b))| a_name == b_name && a.same(b))
}
