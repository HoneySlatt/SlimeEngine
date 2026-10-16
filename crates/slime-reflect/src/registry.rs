//! The registry: the types a program describes, by name, and what reads and sets their values.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::error::{Problem, ReflectError, RegistryError};
use crate::info::{FieldInfo, Kind, Range, TypeInfo};
use crate::path::{Path, Step};
use crate::reflect::Reflect;
use crate::value::Value;

/// The types a program describes, by name: their descriptions, and for those that a Rust type is
/// behind, what turns their values into [`Value`]s and back.
///
/// Types are registered explicitly ([`Registry::register`]), and a registration registers the
/// types that the type's values hold; a type never registered is found by no name. A type with no
/// Rust type behind it is registered by its description alone ([`Registry::register_dynamic`]),
/// as scripts and plugins will (Stage 8).
///
/// It checks values against their types' descriptions ([`Registry::check`]), sets their parts by
/// path ([`Registry::set`]), which respects the fields' ranges, and lists their leaves
/// ([`Registry::leaves`]): all three work on plain values, whether a Rust type is behind them or
/// not.
#[derive(Default)]
pub struct Registry {
    registrations: Vec<Registration>,
    /// The index of each type's registration, by its name and its aliases.
    names: HashMap<String, usize>,
    /// The index of each Rust type's registration.
    rust: HashMap<TypeId, usize>,
}

/// A registered type: its description, and, if a Rust type is behind it, what turns its values
/// into [`Value`]s and back.
pub struct Registration {
    info: TypeInfo,
    rust: Option<RustType>,
}

/// What a registration knows of the Rust type behind it.
#[derive(Clone, Copy)]
struct RustType {
    id: TypeId,
    to_value: fn(&dyn Any) -> Option<Value>,
    from_value: fn(&Value) -> Result<Box<dyn Any + Send + Sync>, ReflectError>,
}

impl RustType {
    fn of<T: Reflect>() -> Self {
        Self {
            id: TypeId::of::<T>(),
            to_value: |value| value.downcast_ref::<T>().map(Reflect::to_value),
            from_value: |value| Ok(Box::new(T::from_value(value)?)),
        }
    }
}

impl Registration {
    /// The type's description.
    pub fn info(&self) -> &TypeInfo {
        &self.info
    }

    /// The Rust type behind it, if there is one.
    pub fn type_id(&self) -> Option<TypeId> {
        self.rust.map(|rust| rust.id)
    }

    /// The value of `value`, if a Rust type is behind the registration and `value` is of it.
    pub fn to_value(&self, value: &dyn Any) -> Option<Value> {
        (self.rust?.to_value)(value)
    }

    /// The Rust value that `value` describes, boxed, to downcast to the Rust type behind the
    /// registration.
    ///
    /// # Errors
    ///
    /// As the type's [`Reflect::from_value`]; [`Problem::Invalid`] when no Rust type is behind the
    /// registration.
    pub fn from_value(&self, value: &Value) -> Result<Box<dyn Any + Send + Sync>, ReflectError> {
        let Some(rust) = self.rust else {
            return Err(ReflectError::new(Problem::Invalid(format!(
                "no Rust type is behind `{}`",
                self.info.name
            ))));
        };
        (rust.from_value)(value)
    }
}

/// A leaf of a value: a part that holds no other parts, which an edit sets whole. Numbers,
/// booleans, text, options, assets, entities, empty lists, and enums whose variant holds no data.
#[derive(Clone, Debug, PartialEq)]
pub struct Leaf {
    /// Where it is in the value.
    pub path: Path,
    /// Its type, by name.
    pub ty: String,
    /// The field it is, or whose element it is, with its attributes; `None` for the value itself.
    pub field: Option<FieldInfo>,
}

impl Registry {
    /// A registry that holds no type.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `T`, and the types that its values hold, unless it is registered already.
    ///
    /// # Panics
    ///
    /// If another type has `T`'s name, or one of its names has: two types under one name would
    /// read each other's values.
    pub fn register<T: Reflect>(&mut self) {
        if self.rust.contains_key(&TypeId::of::<T>()) {
            return;
        }
        let index = self
            .add(T::type_info(), Some(RustType::of::<T>()))
            .unwrap_or_else(|error| panic!("`{}` cannot be registered: {error}", T::type_name()));
        self.rust.insert(TypeId::of::<T>(), index);
        T::register_dependencies(self);
    }

    /// Registers a type with no Rust type behind it, which `info` describes: its values are
    /// [`Value`]s only, which the registry checks, sets and lists as any other.
    ///
    /// # Errors
    ///
    /// When another type has its name or one of its aliases, or when it holds values of a type
    /// that the registry does not hold. Nothing is registered then.
    pub fn register_dynamic(&mut self, info: TypeInfo) -> Result<(), RegistryError> {
        let named: Vec<&str> = match &info.kind {
            Kind::Struct(fields) => fields.iter().map(|field| field.ty.as_str()).collect(),
            Kind::Enum(variants) => variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .map(|field| field.ty.as_str())
                .collect(),
            Kind::Option(ty) | Kind::List { element: ty, .. } => vec![ty.as_str()],
            _ => Vec::new(),
        };
        if let Some(missing) = named.iter().find(|name| self.get(name).is_none()) {
            return Err(RegistryError::UnknownType {
                ty: info.name.clone(),
                names: (*missing).to_owned(),
            });
        }
        self.add(info, None).map(|_| ())
    }

    /// Adds a registration under its name and aliases.
    fn add(&mut self, info: TypeInfo, rust: Option<RustType>) -> Result<usize, RegistryError> {
        for name in std::iter::once(&info.name).chain(&info.aliases) {
            if let Some(&taken) = self.names.get(name) {
                return Err(RegistryError::NameTaken {
                    name: name.clone(),
                    by: self.registrations[taken].info.name.clone(),
                });
            }
        }
        let index = self.registrations.len();
        for name in std::iter::once(&info.name).chain(&info.aliases) {
            self.names.insert(name.clone(), index);
        }
        self.registrations.push(Registration { info, rust });
        Ok(index)
    }

    /// The type named `name`, by its name or one of its aliases, if it is registered.
    pub fn get(&self, name: &str) -> Option<&Registration> {
        self.names
            .get(name)
            .map(|&index| &self.registrations[index])
    }

    /// The registration of the Rust type `T`, if it is registered.
    pub fn of<T: 'static>(&self) -> Option<&Registration> {
        self.of_type(TypeId::of::<T>())
    }

    /// The registration of the Rust type whose ID is `id`, if it is registered.
    pub fn of_type(&self, id: TypeId) -> Option<&Registration> {
        self.rust.get(&id).map(|&index| &self.registrations[index])
    }

    /// Every registered type, in the order they were registered.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Registration> {
        self.registrations.iter()
    }

    /// How many types are registered.
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    /// Whether no type is registered.
    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }

    /// The description of the type named `name`.
    fn info(&self, name: &str) -> Result<&TypeInfo, ReflectError> {
        self.get(name)
            .map(Registration::info)
            .ok_or_else(|| ReflectError::new(Problem::NotRegistered(name.to_owned())))
    }

    /// Checks that `value` is a value of the type named `ty`: its kind, its fields, its variant,
    /// the width of its integers, the length of its arrays, under the names the types have now.
    /// Ranges are left to edits ([`Registry::set`]).
    ///
    /// # Errors
    ///
    /// When it is not, with the path of the part at fault; [`Problem::NotRegistered`] for a type
    /// that the registry does not hold.
    pub fn check(&self, ty: &str, value: &Value) -> Result<(), ReflectError> {
        self.verify(ty, value, None, false)
    }

    /// Sets the part of `value`, a value of the type named `ty`, that `path` leads to, to `new`,
    /// once `new` is checked as a value of that part's type ([`Registry::check`]) whose numbers lie
    /// within the ranges of the fields they are in. A field of an enum is one of the variant that
    /// the value holds; the empty path sets the whole value.
    ///
    /// ```
    /// use slime_reflect::{Problem, Reflect, Registry, Value};
    ///
    /// #[derive(Reflect)]
    /// #[reflect(name = "Lamp")]
    /// struct Lamp {
    ///     #[reflect(range(min = 0.0))]
    ///     intensity: f32,
    /// }
    ///
    /// let mut registry = Registry::new();
    /// registry.register::<Lamp>();
    /// let mut lamp = Lamp { intensity: 30.0 }.to_value();
    /// let intensity = "intensity".parse().unwrap();
    /// registry.set("Lamp", &mut lamp, &intensity, Value::F32(12.0)).expect("in range");
    /// assert_eq!(lamp.get(&intensity), Some(&Value::F32(12.0)));
    /// let refused = registry.set("Lamp", &mut lamp, &intensity, Value::F32(-1.0)).unwrap_err();
    /// assert!(matches!(refused.problem(), Problem::OutOfRange { .. }));
    /// assert_eq!(refused.to_string(), "`intensity`: -1 is not ≥ 0");
    /// ```
    ///
    /// # Errors
    ///
    /// When `path` names no part of `value` ([`Problem::NoSuchPath`]), when `new` is not a value
    /// of the part's type, or when a number of it lies out of its field's range
    /// ([`Problem::OutOfRange`]), with the path of the part at fault, from `value`. `value` is
    /// left as it was then.
    pub fn set(
        &self,
        ty: &str,
        value: &mut Value,
        path: &Path,
        new: Value,
    ) -> Result<(), ReflectError> {
        let (part, field) = self.resolve(ty, value, path)?;
        self.verify(&part, &new, field.as_ref().and_then(|f| f.range), true)
            .map_err(|error| {
                // From the part, where the error was found, to `value`.
                let at = error
                    .path()
                    .steps()
                    .iter()
                    .cloned()
                    .fold(path.clone(), Path::then);
                ReflectError::at(at, error.problem().clone())
            })?;
        let part = value
            .get_mut(path)
            .ok_or_else(|| ReflectError::at(path.clone(), Problem::NoSuchPath))?;
        *part = new;
        Ok(())
    }

    /// The leaves of `value`, a value of the type named `ty`, in the order of its fields and
    /// elements: what an editor lists, and edits one at a time ([`Registry::set`]).
    ///
    /// # Errors
    ///
    /// When `value` is not a value of `ty`, as [`Registry::check`] says.
    pub fn leaves(&self, ty: &str, value: &Value) -> Result<Vec<Leaf>, ReflectError> {
        self.check(ty, value)?;
        let mut leaves = Vec::new();
        self.collect(ty, value, Path::default(), None, &mut leaves)?;
        Ok(leaves)
    }

    fn collect(
        &self,
        ty: &str,
        value: &Value,
        path: Path,
        field: Option<&FieldInfo>,
        leaves: &mut Vec<Leaf>,
    ) -> Result<(), ReflectError> {
        let fields = match (&self.info(ty)?.kind, value) {
            (Kind::Struct(fields), Value::Struct(_)) => Some(fields),
            (Kind::Enum(variants), Value::Enum(name, _)) => variants
                .iter()
                .find(|variant| &variant.name == name)
                .map(|variant| &variant.fields)
                .filter(|fields| !fields.is_empty()),
            (Kind::List { element, .. }, Value::List(elements)) if !elements.is_empty() => {
                for (index, element_value) in elements.iter().enumerate() {
                    self.collect(
                        element,
                        element_value,
                        path.clone().index(index),
                        field,
                        leaves,
                    )?;
                }
                return Ok(());
            }
            _ => None,
        };
        match fields {
            Some(fields) => {
                for inner in fields {
                    let inner_value = value
                        .child(&Step::Field(inner.name.clone()))
                        .expect("a checked value holds its fields");
                    self.collect(
                        &inner.ty,
                        inner_value,
                        path.clone().field(inner.name.clone()),
                        Some(inner),
                        leaves,
                    )?;
                }
            }
            // A unit struct has no leaf: there is nothing in it to set.
            None if matches!(value, Value::Struct(_)) => {}
            None => leaves.push(Leaf {
                path,
                ty: ty.to_owned(),
                field: field.cloned(),
            }),
        }
        Ok(())
    }

    /// The type of the part of `value` that `path` leads to, and the field it is in.
    fn resolve(
        &self,
        ty: &str,
        value: &Value,
        path: &Path,
    ) -> Result<(String, Option<FieldInfo>), ReflectError> {
        let nothing = || ReflectError::at(path.clone(), Problem::NoSuchPath);
        let mut ty = ty.to_owned();
        let mut field: Option<FieldInfo> = None;
        let mut part = value;
        for step in path.steps() {
            let next = part.child(step).ok_or_else(nothing)?;
            let fields = match (&self.info(&ty)?.kind, part, step) {
                (Kind::Struct(fields), _, Step::Field(_)) => fields,
                (Kind::Enum(variants), Value::Enum(variant, _), Step::Field(_)) => {
                    &variants
                        .iter()
                        .find(|candidate| &candidate.name == variant)
                        .ok_or_else(nothing)?
                        .fields
                }
                (Kind::List { element, .. }, _, Step::Index(_)) => {
                    // The list's field, whose range its elements keep.
                    ty = element.clone();
                    part = next;
                    continue;
                }
                _ => return Err(nothing()),
            };
            let Step::Field(name) = step else {
                return Err(nothing());
            };
            let found = fields
                .iter()
                .find(|f| &f.name == name)
                .ok_or_else(nothing)?;
            ty = found.ty.clone();
            field = Some(found.clone());
            part = next;
        }
        Ok((ty, field))
    }

    /// Checks `value` against the type named `ty`, and, if `ranges`, its numbers against `range`
    /// and the ranges of the fields they are in.
    fn verify(
        &self,
        ty: &str,
        value: &Value,
        range: Option<Range>,
        ranges: bool,
    ) -> Result<(), ReflectError> {
        let info = self.info(ty)?;
        let wrong = || {
            ReflectError::new(Problem::WrongKind {
                expected: info.name.clone(),
                found: value.kind(),
            })
        };
        if let (true, Some(range), Some(number)) = (ranges, range, value.number())
            && !range.contains(number)
        {
            return Err(ReflectError::new(Problem::OutOfRange {
                value: number,
                range,
            }));
        }
        match (&info.kind, value) {
            (Kind::Bool, Value::Bool(_))
            | (Kind::F32, Value::F32(_))
            | (Kind::F64, Value::F64(_))
            | (Kind::Text, Value::Text(_))
            | (Kind::Asset, Value::Asset(_))
            | (Kind::Entity, Value::Entity(_)) => Ok(()),
            (&Kind::Int { bits }, &Value::Int(number)) => within_width(number, bits, true),
            (&Kind::UInt { bits }, &Value::UInt(number)) => within_width(number, bits, false),
            (Kind::Struct(fields), Value::Struct(values)) => {
                self.verify_fields(fields, values, ranges)
            }
            (Kind::Enum(variants), Value::Enum(name, values)) => {
                let Some(variant) = variants.iter().find(|variant| &variant.name == name) else {
                    return Err(ReflectError::new(Problem::UnknownVariant {
                        variant: name.clone(),
                        known: variants
                            .iter()
                            .map(|variant| variant.name.clone())
                            .collect(),
                    }));
                };
                self.verify_fields(&variant.fields, values, ranges)
            }
            (Kind::Option(inner), Value::Option(inner_value)) => match inner_value {
                Some(inner_value) => self.verify(inner, inner_value, range, ranges),
                None => Ok(()),
            },
            (Kind::List { element, len }, Value::List(elements)) => {
                if let Some(len) = *len
                    && len != elements.len()
                {
                    return Err(ReflectError::new(Problem::WrongLength {
                        expected: len,
                        found: elements.len(),
                    }));
                }
                for (index, element_value) in elements.iter().enumerate() {
                    self.verify(element, element_value, range, ranges)
                        .map_err(|error| error.within(Step::Index(index)))?;
                }
                Ok(())
            }
            _ => Err(wrong()),
        }
    }

    /// Checks that `values` are exactly `fields`, each a value of its type.
    fn verify_fields(
        &self,
        fields: &[FieldInfo],
        values: &[(String, Value)],
        ranges: bool,
    ) -> Result<(), ReflectError> {
        if let Some((unknown, _)) = values
            .iter()
            .find(|(name, _)| !fields.iter().any(|field| &field.name == name))
        {
            return Err(ReflectError::new(Problem::UnknownField {
                field: unknown.clone(),
                known: fields.iter().map(|field| field.name.clone()).collect(),
            }));
        }
        for field in fields {
            let Some((_, value)) = values.iter().find(|(name, _)| name == &field.name) else {
                return Err(ReflectError::new(Problem::MissingField(field.name.clone())));
            };
            self.verify(&field.ty, value, field.range, ranges)
                .map_err(|error| error.within(Step::Field(field.name.clone())))?;
        }
        Ok(())
    }
}

/// Checks that `number` fits in an integer of `bits` bits, signed or not.
fn within_width<N: Into<i128> + Copy>(
    number: N,
    bits: u32,
    signed: bool,
) -> Result<(), ReflectError> {
    let (min, max): (i128, i128) = if signed {
        (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
    } else {
        (0, (1 << bits) - 1)
    };
    let number: i128 = number.into();
    if (min..=max).contains(&number) {
        Ok(())
    } else {
        Err(ReflectError::new(Problem::OutOfRange {
            value: number as f64,
            range: Range {
                min: min as f64,
                min_excluded: false,
                max: max as f64,
            },
        }))
    }
}
