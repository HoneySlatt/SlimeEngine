//! Reflection for the standard types the engine uses, and for glam's: by hand, here, where the
//! trait is.

use glam::{DVec3, Quat, Vec3};

use crate::error::{Problem, ReflectError};
use crate::info::{FieldInfo, Kind, Range, TypeInfo};
use crate::path::Step;
use crate::reflect::Reflect;
use crate::registry::Registry;
use crate::support::{field, fields, known, missing, wrong};
use crate::value::Value;

/// The description of a type without fields, aliases or default, in version 1.
fn described(name: String, doc: &str, kind: Kind) -> TypeInfo {
    TypeInfo {
        name,
        aliases: Vec::new(),
        version: 1,
        doc: doc.to_owned(),
        default: None,
        kind,
    }
}

impl Reflect for bool {
    fn type_name() -> String {
        "bool".to_owned()
    }

    fn type_info() -> TypeInfo {
        described(Self::type_name(), "A boolean.", Kind::Bool)
    }

    fn register_dependencies(_: &mut Registry) {}

    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        match *value {
            Value::Bool(value) => Ok(value),
            _ => Err(wrong("bool", value)),
        }
    }
}

/// Integers: a signed one reads `Value::Int`, an unsigned one `Value::UInt`, within its width.
macro_rules! integer {
    ($ty:ty, $name:literal, $variant:ident, $kind:ident, $bits:literal, $doc:literal) => {
        impl Reflect for $ty {
            fn type_name() -> String {
                $name.to_owned()
            }

            fn type_info() -> TypeInfo {
                described(Self::type_name(), $doc, Kind::$kind { bits: $bits })
            }

            fn register_dependencies(_: &mut Registry) {}

            fn to_value(&self) -> Value {
                Value::$variant((*self).into())
            }

            fn from_value(value: &Value) -> Result<Self, ReflectError> {
                match *value {
                    Value::$variant(number) => <$ty>::try_from(number).map_err(|_| {
                        ReflectError::new(Problem::OutOfRange {
                            value: number as f64,
                            range: Range {
                                min: <$ty>::MIN as f64,
                                min_excluded: false,
                                max: <$ty>::MAX as f64,
                            },
                        })
                    }),
                    _ => Err(wrong($name, value)),
                }
            }
        }
    };
}

integer!(i8, "i8", Int, Int, 8, "A signed integer of 8 bits.");
integer!(i16, "i16", Int, Int, 16, "A signed integer of 16 bits.");
integer!(i32, "i32", Int, Int, 32, "A signed integer of 32 bits.");
integer!(i64, "i64", Int, Int, 64, "A signed integer of 64 bits.");
integer!(u8, "u8", UInt, UInt, 8, "An unsigned integer of 8 bits.");
integer!(
    u16,
    "u16",
    UInt,
    UInt,
    16,
    "An unsigned integer of 16 bits."
);
integer!(
    u32,
    "u32",
    UInt,
    UInt,
    32,
    "An unsigned integer of 32 bits."
);
integer!(
    u64,
    "u64",
    UInt,
    UInt,
    64,
    "An unsigned integer of 64 bits."
);

/// Floats, which keep their width.
macro_rules! float {
    ($ty:ty, $name:literal, $variant:ident, $doc:literal) => {
        impl Reflect for $ty {
            fn type_name() -> String {
                $name.to_owned()
            }

            fn type_info() -> TypeInfo {
                described(Self::type_name(), $doc, Kind::$variant)
            }

            fn register_dependencies(_: &mut Registry) {}

            fn to_value(&self) -> Value {
                Value::$variant(*self)
            }

            fn from_value(value: &Value) -> Result<Self, ReflectError> {
                match *value {
                    Value::$variant(number) => Ok(number),
                    _ => Err(wrong($name, value)),
                }
            }
        }
    };
}

float!(f32, "f32", F32, "A 32-bit float.");
float!(f64, "f64", F64, "A 64-bit float.");

impl Reflect for String {
    fn type_name() -> String {
        "String".to_owned()
    }

    fn type_info() -> TypeInfo {
        described(Self::type_name(), "Text.", Kind::Text)
    }

    fn register_dependencies(_: &mut Registry) {}

    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        match value {
            Value::Text(text) => Ok(text.clone()),
            _ => Err(wrong("String", value)),
        }
    }
}

impl<T: Reflect> Reflect for Option<T> {
    fn type_name() -> String {
        format!("Option<{}>", T::type_name())
    }

    /// Data that leaves it out gives `None`.
    fn type_info() -> TypeInfo {
        TypeInfo {
            default: Some(Value::Option(None)),
            ..described(
                Self::type_name(),
                "A value that may be absent.",
                Kind::Option(T::type_name()),
            )
        }
    }

    fn register_dependencies(registry: &mut Registry) {
        registry.register::<T>();
    }

    fn default_value() -> Option<Self> {
        Some(None)
    }

    fn to_value(&self) -> Value {
        Value::Option(self.as_ref().map(|value| Box::new(value.to_value())))
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        match value {
            Value::Option(None) => Ok(None),
            Value::Option(Some(value)) => T::from_value(value).map(Some),
            _ => Err(wrong(&Self::type_name(), value)),
        }
    }
}

/// The elements of `value`, a list's, each read as a `T`.
fn elements<T: Reflect>(ty: &str, value: &Value) -> Result<Vec<T>, ReflectError> {
    let Value::List(elements) = value else {
        return Err(wrong(ty, value));
    };
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            T::from_value(element).map_err(|error| error.within(Step::Index(index)))
        })
        .collect()
}

impl<T: Reflect> Reflect for Vec<T> {
    fn type_name() -> String {
        format!("Vec<{}>", T::type_name())
    }

    fn type_info() -> TypeInfo {
        described(
            Self::type_name(),
            "A list of values, of any length.",
            Kind::List {
                element: T::type_name(),
                len: None,
            },
        )
    }

    fn register_dependencies(registry: &mut Registry) {
        registry.register::<T>();
    }

    fn to_value(&self) -> Value {
        Value::List(self.iter().map(Reflect::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        elements(&Self::type_name(), value)
    }
}

impl<T: Reflect, const N: usize> Reflect for [T; N] {
    fn type_name() -> String {
        format!("[{}; {N}]", T::type_name())
    }

    fn type_info() -> TypeInfo {
        described(
            Self::type_name(),
            "An array of values, of one length.",
            Kind::List {
                element: T::type_name(),
                len: Some(N),
            },
        )
    }

    fn register_dependencies(registry: &mut Registry) {
        registry.register::<T>();
    }

    fn to_value(&self) -> Value {
        Value::List(self.iter().map(Reflect::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        let elements: Vec<T> = elements(&Self::type_name(), value)?;
        let found = elements.len();
        elements
            .try_into()
            .map_err(|_| ReflectError::new(Problem::WrongLength { expected: N, found }))
    }
}

impl<A: Reflect, B: Reflect> Reflect for (A, B) {
    fn type_name() -> String {
        format!("({}, {})", A::type_name(), B::type_name())
    }

    fn type_info() -> TypeInfo {
        described(
            Self::type_name(),
            "A pair of values.",
            Kind::Struct(vec![
                FieldInfo::new("0", A::type_name()),
                FieldInfo::new("1", B::type_name()),
            ]),
        )
    }

    fn register_dependencies(registry: &mut Registry) {
        registry.register::<A>();
        registry.register::<B>();
    }

    fn to_value(&self) -> Value {
        Value::Struct(vec![
            ("0".to_owned(), self.0.to_value()),
            ("1".to_owned(), self.1.to_value()),
        ])
    }

    fn from_value(value: &Value) -> Result<Self, ReflectError> {
        let fields = fields(value, &Self::type_name())?;
        known(fields, &[&["0"], &["1"]])?;
        Ok((
            field(fields, "0", &[])?.map_or_else(|| missing("0"), Ok)?,
            field(fields, "1", &[])?.map_or_else(|| missing("1"), Ok)?,
        ))
    }
}

/// glam's vectors and quaternions: structs of their coordinates.
macro_rules! coordinates {
    ($ty:ty, $name:literal, $number:ty, $doc:literal, $($axis:ident),+) => {
        impl Reflect for $ty {
            fn type_name() -> String {
                $name.to_owned()
            }

            fn type_info() -> TypeInfo {
                described(
                    Self::type_name(),
                    $doc,
                    Kind::Struct(vec![
                        $(FieldInfo::new(stringify!($axis), <$number>::type_name())),+
                    ]),
                )
            }

            fn register_dependencies(registry: &mut Registry) {
                registry.register::<$number>();
            }

            fn to_value(&self) -> Value {
                Value::Struct(vec![
                    $((stringify!($axis).to_owned(), self.$axis.to_value())),+
                ])
            }

            fn from_value(value: &Value) -> Result<Self, ReflectError> {
                let fields = fields(value, $name)?;
                known(fields, &[$(&[stringify!($axis)]),+])?;
                Ok(Self::from_array([$(
                    field::<$number>(fields, stringify!($axis), &[])?
                        .map_or_else(|| missing(stringify!($axis)), Ok)?
                ),+]))
            }
        }
    };
}

coordinates!(Vec3, "Vec3", f32, "A 3D vector of 32-bit floats.", x, y, z);
coordinates!(
    DVec3,
    "DVec3",
    f64,
    "A 3D vector of 64-bit floats.",
    x,
    y,
    z
);
coordinates!(
    Quat,
    "Quat",
    f32,
    "A rotation, as a quaternion of 32-bit floats, of length 1 to rotate without scaling.",
    x,
    y,
    z,
    w
);

#[cfg(test)]
mod tests {
    use super::*;

    /// `value` read back as a `T` from its value.
    fn round_trip<T: Reflect + PartialEq + std::fmt::Debug>(value: T) {
        assert_eq!(T::from_value(&value.to_value()).as_ref(), Ok(&value));
    }

    #[test]
    fn the_standard_types_and_glam_s_round_trip() {
        round_trip(true);
        round_trip(-5_i8);
        round_trip(i64::MIN);
        round_trip(u64::MAX);
        round_trip(1.5_f32);
        round_trip(-2.25_f64);
        round_trip("lantern".to_owned());
        round_trip(Some(4.0_f32));
        round_trip(None::<f32>);
        round_trip(vec![1_u32, 2, 3]);
        round_trip([0.5_f32; 4]);
        round_trip(("radius".to_owned(), 2.0_f64));
        round_trip(Vec3::new(1.0, 2.0, 3.0));
        round_trip(DVec3::new(1.0e7, 2.0, 3.0));
        round_trip(Quat::from_rotation_y(0.5));
    }

    #[test]
    fn a_value_of_another_kind_or_width_is_refused_where_it_is() {
        let error = u8::from_value(&Value::UInt(300)).expect_err("too wide");
        assert!(
            matches!(error.problem(), Problem::OutOfRange { .. }),
            "{error}"
        );
        assert!(
            f32::from_value(&Value::F64(1.0)).is_err(),
            "floats keep their width"
        );
        let error = Vec::<f32>::from_value(&Value::List(vec![Value::F32(1.0), Value::Bool(true)]))
            .expect_err("a bool in the list");
        assert_eq!(error.path().to_string(), "[1]");
        let error =
            <[f32; 3]>::from_value(&Value::List(vec![Value::F32(1.0)])).expect_err("too short");
        assert_eq!(
            *error.problem(),
            Problem::WrongLength {
                expected: 3,
                found: 1
            }
        );
        let error = Vec3::from_value(&Value::Struct(vec![
            ("x".to_owned(), Value::F32(1.0)),
            ("y".to_owned(), Value::F32(1.0)),
        ]))
        .expect_err("no z");
        assert_eq!(*error.problem(), Problem::MissingField("z".to_owned()));
    }

    #[test]
    fn composed_names_say_what_they_hold() {
        assert_eq!(Option::<f32>::type_name(), "Option<f32>");
        assert_eq!(Vec::<Option<u8>>::type_name(), "Vec<Option<u8>>");
        assert_eq!(<[f32; 4]>::type_name(), "[f32; 4]");
        assert_eq!(<(String, f64)>::type_name(), "(String, f64)");
    }
}
