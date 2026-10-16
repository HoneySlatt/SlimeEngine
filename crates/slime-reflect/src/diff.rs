//! Values compared, and their differences applied, path by path.

use crate::error::{Problem, ReflectError};
use crate::path::Path;
use crate::value::Value;

/// What differs between `from` and `to`: the parts of `to` to set in `from`, each by its path,
/// that [`apply`] sets, so that `from` becomes `to`. Floats are compared by their bits, so that a
/// `NaN` that stays is no difference and a `-0.0` that was `0.0` is one.
///
/// Fields, variants' fields and elements are compared one by one when both values hold the same
/// ones: two structs or variants with the same fields, two lists of one length. Otherwise the part
/// differs whole: another variant, another length, another kind.
///
/// ```
/// use slime_reflect::{Value, apply, diff};
///
/// let light = |intensity: f32, lit: bool| {
///     Value::Struct(vec![
///         ("intensity".to_owned(), Value::F32(intensity)),
///         ("lit".to_owned(), Value::Bool(lit)),
///     ])
/// };
/// let (before, after) = (light(30.0, true), light(12.0, true));
/// let changes = diff(&before, &after);
/// assert_eq!(changes, [("intensity".parse().unwrap(), Value::F32(12.0))]);
/// let mut changed = before.clone();
/// apply(&mut changed, &changes).expect("every path is in the value");
/// assert_eq!(changed, after);
/// ```
pub fn diff(from: &Value, to: &Value) -> Vec<(Path, Value)> {
    let mut changes = Vec::new();
    compare(from, to, Path::default(), &mut changes);
    changes
}

fn compare(from: &Value, to: &Value, path: Path, changes: &mut Vec<(Path, Value)>) {
    if from.same(to) {
        return;
    }
    let fields = match (from, to) {
        (Value::Struct(a), Value::Struct(b)) => Some((a, b)),
        (Value::Enum(a_variant, a), Value::Enum(b_variant, b)) if a_variant == b_variant => {
            Some((a, b))
        }
        (Value::List(a), Value::List(b)) if a.len() == b.len() => {
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, path.clone().index(index), changes);
            }
            return;
        }
        _ => None,
    };
    match fields {
        Some((a, b)) if a.len() == b.len() && a.iter().zip(b).all(|((a, _), (b, _))| a == b) => {
            for ((name, a), (_, b)) in a.iter().zip(b) {
                compare(a, b, path.clone().field(name.clone()), changes);
            }
        }
        _ => changes.push((path, to.clone())),
    }
}

/// Sets each part of `value` that `changes` names, in order, to the value it gives: what [`diff`]
/// found between a value and another, or a list of edits.
///
/// # Errors
///
/// [`Problem::NoSuchPath`] for the first path that names no part of `value`, as it is once the
/// changes before it are set; they stay set.
pub fn apply(value: &mut Value, changes: &[(Path, Value)]) -> Result<(), ReflectError> {
    for (path, new) in changes {
        let part = value
            .get_mut(path)
            .ok_or_else(|| ReflectError::at(path.clone(), Problem::NoSuchPath))?;
        *part = new.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(intensity: f32, range: Option<f32>) -> Value {
        Value::Enum(
            "Point".to_owned(),
            vec![
                ("intensity".to_owned(), Value::F32(intensity)),
                (
                    "range".to_owned(),
                    Value::Option(range.map(|range| Box::new(Value::F32(range)))),
                ),
            ],
        )
    }

    #[test]
    fn differences_are_found_where_they_are_and_applied_back() {
        let pairs = [
            (point(30.0, None), point(30.0, Some(4.0))),
            (
                point(30.0, None),
                Value::Enum("Directional".to_owned(), Vec::new()),
            ),
            (
                Value::List(vec![Value::Asset(1), Value::Asset(2)]),
                Value::List(vec![Value::Asset(1), Value::Asset(3)]),
            ),
            (
                Value::List(vec![Value::Asset(1)]),
                Value::List(vec![Value::Asset(1), Value::Asset(3)]),
            ),
            (Value::F32(0.0), Value::F32(-0.0)),
            (Value::Text("a".to_owned()), Value::UInt(1)),
        ];
        for (from, to) in pairs {
            let changes = diff(&from, &to);
            assert!(!changes.is_empty());
            let mut changed = from.clone();
            apply(&mut changed, &changes).expect("applied");
            assert!(changed.same(&to), "{changed:?} is not {to:?}");
            assert!(diff(&to, &to).is_empty());
        }
        // Only the field that changed, by its path.
        let changes = diff(&point(30.0, None), &point(12.0, None));
        assert_eq!(changes, [("intensity".parse().unwrap(), Value::F32(12.0))]);
        // A NaN that stays is no difference.
        assert!(diff(&Value::F64(f64::NAN), &Value::F64(f64::NAN)).is_empty());
    }

    #[test]
    fn a_change_whose_path_names_nothing_is_refused() {
        let mut value = point(30.0, None);
        let error = apply(&mut value, &[("color.r".parse().unwrap(), Value::F32(1.0))])
            .expect_err("no color");
        assert_eq!(error.path().to_string(), "color.r");
        assert_eq!(*error.problem(), Problem::NoSuchPath);
    }
}
