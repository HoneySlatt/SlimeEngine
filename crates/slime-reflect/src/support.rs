//! What the code that `#[derive(Reflect)]` writes calls, and the implementations by hand: not an
//! API of its own.

use crate::error::{Problem, ReflectError};
use crate::path::Step;
use crate::reflect::Reflect;
use crate::value::Value;

/// The fields of a struct's value, or of a variant's, by name.
pub type Fields<'a> = &'a [(String, Value)];

/// The error of `value`, which is not a value of the type named `ty`.
pub fn wrong(ty: &str, value: &Value) -> ReflectError {
    ReflectError::new(Problem::WrongKind {
        expected: ty.to_owned(),
        found: value.kind(),
    })
}

/// The fields of `value`, a struct's, for the type named `ty`.
///
/// # Errors
///
/// When `value` is not a struct's.
pub fn fields<'a>(value: &'a Value, ty: &str) -> Result<Fields<'a>, ReflectError> {
    match value {
        Value::Struct(fields) => Ok(fields),
        _ => Err(wrong(ty, value)),
    }
}

/// The variant of `value`, an enum's, and its fields, for the type named `ty`.
///
/// # Errors
///
/// When `value` is not an enum's.
pub fn variant<'a>(value: &'a Value, ty: &str) -> Result<(&'a str, Fields<'a>), ReflectError> {
    match value {
        Value::Enum(variant, fields) => Ok((variant, fields)),
        _ => Err(wrong(ty, value)),
    }
}

/// Refuses a field of `fields` that is none of `known`, the names and aliases of a type's fields;
/// the message lists `names`, their names.
///
/// # Errors
///
/// [`Problem::UnknownField`] for the first field that is unknown.
pub fn known(
    fields: &[(String, Value)],
    known: &[&str],
    names: &[&str],
) -> Result<(), ReflectError> {
    match fields
        .iter()
        .find(|(name, _)| !known.contains(&name.as_str()))
    {
        Some((field, _)) => Err(ReflectError::new(Problem::UnknownField {
            field: field.clone(),
            known: names.iter().map(|&name| name.to_owned()).collect(),
        })),
        None => Ok(()),
    }
}

/// The field of `fields` named `name`, or one of `aliases`, read as a `T`, if there is one.
///
/// # Errors
///
/// As [`Reflect::from_value`], within the field.
pub fn field<T: Reflect>(
    fields: &[(String, Value)],
    name: &str,
    aliases: &[&str],
) -> Result<Option<T>, ReflectError> {
    let Some((_, value)) = fields
        .iter()
        .find(|(field, _)| field == name || aliases.contains(&field.as_str()))
    else {
        return Ok(None);
    };
    T::from_value(value)
        .map(Some)
        .map_err(|error| error.within(Step::Field(name.to_owned())))
}

/// The value of the field `name`, which data left out: its type's default, if the type declares
/// one ([`Reflect::default_value`]).
///
/// # Errors
///
/// [`Problem::MissingField`] if the type declares none.
pub fn missing<T: Reflect>(name: &str) -> Result<T, ReflectError> {
    T::default_value().ok_or_else(|| ReflectError::new(Problem::MissingField(name.to_owned())))
}

/// The error of the variant `variant`, which an enum whose variants are `known` does not have.
pub fn unknown_variant(variant: &str, known: &[&str]) -> ReflectError {
    ReflectError::new(Problem::UnknownVariant {
        variant: variant.to_owned(),
        known: known.iter().map(|&name| name.to_owned()).collect(),
    })
}

/// A type whose values are numbers, or hold numbers that a range applies to: what a field given a
/// `range` must be.
#[diagnostic::on_unimplemented(
    message = "a range applies to numbers: `{Self}` holds none",
    note = "give `range` to fields of integers, floats, or options, lists and arrays of them"
)]
pub trait Ranged {}

macro_rules! ranged {
    ($($ty:ty),*) => { $(impl Ranged for $ty {})* };
}
ranged!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
impl<T: Ranged> Ranged for Option<T> {}
impl<T: Ranged> Ranged for Vec<T> {}
impl<T: Ranged, const N: usize> Ranged for [T; N] {}

/// What fails to compile when `T` is not [`Ranged`].
pub fn ranged<T: Ranged>() {}
