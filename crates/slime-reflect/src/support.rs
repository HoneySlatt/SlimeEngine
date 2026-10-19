//! What the code that `#[derive(Reflect)]` writes calls, and what an implementation of
//! [`Reflect`] by hand calls: reading a struct's or an enum's fields from a value, and the errors of
//! a value that is not one. The engine's `AssetId` and `Entity` are described with them.

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

/// Refuses a field of `fields` that is none of a type's fields, each of which `known` gives as its
/// name, then its aliases; and a field given twice, under its name or one of its aliases.
///
/// # Errors
///
/// [`Problem::UnknownField`] for the first field that is unknown, which lists the fields' names;
/// [`Problem::DuplicateField`] for the first given twice.
pub fn known(fields: &[(String, Value)], known: &[&[&str]]) -> Result<(), ReflectError> {
    let mut given = vec![false; known.len()];
    for (name, _) in fields {
        let Some(index) = known
            .iter()
            .position(|names| names.contains(&name.as_str()))
        else {
            return Err(ReflectError::new(Problem::UnknownField {
                field: name.clone(),
                known: known
                    .iter()
                    .filter_map(|names| names.first())
                    .map(|&name| name.to_owned())
                    .collect(),
            }));
        };
        if std::mem::replace(&mut given[index], true) {
            let name = known[index].first().copied().unwrap_or(name);
            return Err(ReflectError::new(Problem::DuplicateField(name.to_owned())));
        }
    }
    Ok(())
}

/// `value`, once `check` accepts it: what a type declares with `#[reflect(check = ...)]`, the
/// rules between its fields that their ranges cannot say. A panic of `check` refuses `value` as its
/// error does, unless the program aborts on panics: a check is a game's code, which a value from a
/// damaged or edited file must not make crash the program
/// (`docs/adr/0096-the-files-the-engine-reads.md`).
///
/// # Errors
///
/// [`Problem::Invalid`], with the reason that `check` gives, if it refuses `value`, or with the
/// message of its panic.
pub fn checked<T>(value: T, check: fn(&T) -> Result<(), String>) -> Result<T, ReflectError> {
    caught(|| check(&value))
        .and_then(|checked| checked)
        .map_err(|reason| ReflectError::new(Problem::Invalid(reason)))?;
    Ok(value)
}

/// What `f` returns, or, if it panics, a reason that says so with the panic's message: how a type's
/// functions that a game writes, its check and its upgrade, are called on a value read.
pub(crate) fn caught<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    // A panic leaves nothing half done that the caller goes on with: the value that `f` read is
    // refused whole.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "without a message".to_owned());
        format!("it panicked: {message}")
    })
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
