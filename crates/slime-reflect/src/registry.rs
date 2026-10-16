//! The registry: the types a program describes, by name, and what reads and sets their values.

use std::any::TypeId;
use std::collections::{HashMap, HashSet};

use crate::error::{Problem, ReflectError, RegistryError};
use crate::info::{FieldInfo, Kind, Range, TypeInfo};
use crate::path::{Path, Step};
use crate::reflect::{Reflect, Upgrade};
use crate::value::Value;

/// The types a program describes, by name: their descriptions, and the Rust type behind each, if
/// one is.
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

/// A registered type: its description, and the Rust type behind it, if one is.
pub struct Registration {
    info: TypeInfo,
    type_id: Option<TypeId>,
    upgrade: Option<Upgrade>,
}

impl Registration {
    /// The type's description.
    pub fn info(&self) -> &TypeInfo {
        &self.info
    }

    /// The Rust type behind it, if there is one.
    pub fn type_id(&self) -> Option<TypeId> {
        self.type_id
    }
}

/// A leaf of a value: a part that holds no other parts, which an edit sets whole. Numbers,
/// booleans, text, assets, entities, options that hold none or hold a leaf, empty lists, and
/// enums whose variant holds no data. An option that holds a value with parts is gone through:
/// its leaves are that value's.
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
    /// read each other's values. If a default that `T` declares is not a value of its type within
    /// its field's range, or a transient field has no default: a bug in the type.
    pub fn register<T: Reflect>(&mut self) {
        if self.rust.contains_key(&TypeId::of::<T>()) {
            return;
        }
        let index = self
            .add(T::type_info(), Some(TypeId::of::<T>()), T::upgrade())
            .unwrap_or_else(|error| panic!("`{}` cannot be registered: {error}", T::type_name()));
        self.rust.insert(TypeId::of::<T>(), index);
        T::register_dependencies(self);
        if let Err(reason) = self.verify_defaults(&T::type_name()) {
            panic!("`{}` cannot be registered: {reason}", T::type_name());
        }
    }

    /// Registers a type with no Rust type behind it, which `info` describes: its values are
    /// [`Value`]s only, which the registry checks, sets and lists as any other. The types it holds
    /// values of must be registered first, so that such a type cannot hold values of itself.
    ///
    /// # Errors
    ///
    /// When another type has its name or one of its aliases, when it holds values of a type that
    /// the registry does not hold, or when its description is not one: integers of another width
    /// than 8, 16, 32 or 64 bits, two fields or variants of one name, a field or a variant whose name
    /// or alias starts with `$`, which documents keep (`slime-serialize`), a default that is not a
    /// value of its type within its field's range, a transient field without a default. Nothing is
    /// registered then.
    pub fn register_dynamic(&mut self, info: TypeInfo) -> Result<(), RegistryError> {
        let invalid = |reason: String| RegistryError::Invalid {
            ty: info.name.clone(),
            reason,
        };
        let unique = |names: Vec<&String>, what: &str| match first_repeated(names) {
            Some(twice) => Err(invalid(format!("two {what} are named `{twice}`"))),
            None => Ok(()),
        };
        let named: Vec<&str> = match &info.kind {
            &Kind::Int { bits } | &Kind::UInt { bits } if ![8, 16, 32, 64].contains(&bits) => {
                return Err(invalid(format!("integers of {bits} bits")));
            }
            Kind::Struct(fields) => {
                unique(fields.iter().map(|field| &field.name).collect(), "fields")?;
                fields.iter().map(|field| field.ty.as_str()).collect()
            }
            Kind::Enum(variants) => {
                unique(
                    variants.iter().map(|variant| &variant.name).collect(),
                    "variants",
                )?;
                for variant in variants {
                    unique(
                        variant.fields.iter().map(|field| &field.name).collect(),
                        "fields",
                    )?;
                }
                variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .map(|field| field.ty.as_str())
                    .collect()
            }
            Kind::Option(ty) | Kind::List { element: ty, .. } => vec![ty.as_str()],
            _ => Vec::new(),
        };
        if let Some(missing) = named.iter().find(|name| self.get(name).is_none()) {
            return Err(RegistryError::UnknownType {
                ty: info.name.clone(),
                names: (*missing).to_owned(),
            });
        }
        let parts: Vec<(&String, &Vec<String>)> = match &info.kind {
            Kind::Struct(fields) => fields.iter().map(|f| (&f.name, &f.aliases)).collect(),
            Kind::Enum(variants) => variants
                .iter()
                .map(|v| (&v.name, &v.aliases))
                .chain(
                    variants
                        .iter()
                        .flat_map(|v| &v.fields)
                        .map(|f| (&f.name, &f.aliases)),
                )
                .collect(),
            _ => Vec::new(),
        };
        if let Some(reserved) = parts
            .into_iter()
            .flat_map(|(name, aliases)| std::iter::once(name).chain(aliases))
            .find(|name| name.starts_with('$'))
        {
            return Err(invalid(format!(
                "`{reserved}` starts with `$`, which documents keep for themselves"
            )));
        }
        let name = info.name.clone();
        let names: Vec<String> = std::iter::once(&info.name)
            .chain(&info.aliases)
            .cloned()
            .collect();
        let index = self.add(info, None, None)?;
        // The defaults are values of the type, and of its fields' types, which the registry checks
        // once it holds the type.
        if let Err(reason) = self.verify_defaults(&name) {
            self.registrations.truncate(index);
            for name in &names {
                self.names.remove(name);
            }
            return Err(RegistryError::Invalid { ty: name, reason });
        }
        Ok(())
    }

    /// Checks the defaults that the type named `name` declares, its own and its fields', as values
    /// of their types whose numbers lie within their fields' ranges, and that each transient field
    /// has one: the struct's, its own or its type's.
    fn verify_defaults(&self, name: &str) -> Result<(), String> {
        let Some(registration) = self.get(name) else {
            return Ok(());
        };
        let info = &registration.info;
        if let Some(default) = &info.default {
            self.verify(name, default, None, true)
                .map_err(|error| format!("its default: {}", within_default(&error)))?;
        }
        let fields: Vec<&FieldInfo> = match &info.kind {
            Kind::Struct(fields) => fields.iter().collect(),
            Kind::Enum(variants) => variants.iter().flat_map(|v| &v.fields).collect(),
            _ => Vec::new(),
        };
        for field in fields {
            if let Some(default) = &field.default {
                self.verify(&field.ty, default, field.range, true)
                    .map_err(|error| {
                        let reason = within_default(&error);
                        format!("the default of the field `{}`: {reason}", field.name)
                    })?;
            }
            // A save leaves the field out, and a reader gives it its default.
            let of_owner = matches!(&info.default, Some(Value::Struct(defaults))
                if defaults.iter().any(|(name, _)| name == &field.name));
            let of_type = self
                .get(&field.ty)
                .is_some_and(|field_type| field_type.info.default.is_some());
            if field.transient && !of_owner && field.default.is_none() && !of_type {
                return Err(format!(
                    "the field `{}` is transient but has no default, which a save that leaves it \
                     out would give it",
                    field.name
                ));
            }
        }
        Ok(())
    }

    /// Adds a registration under its name and aliases.
    fn add(
        &mut self,
        info: TypeInfo,
        type_id: Option<TypeId>,
        upgrade: Option<Upgrade>,
    ) -> Result<usize, RegistryError> {
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
        self.registrations.push(Registration {
            info,
            type_id,
            upgrade,
        });
        Ok(index)
    }

    /// The type named `name`, by its name or one of its aliases, if it is registered.
    pub fn get(&self, name: &str) -> Option<&Registration> {
        self.names
            .get(name)
            .map(|&index| &self.registrations[index])
    }

    /// The registration of the Rust type whose ID is `id`, if it is registered.
    pub fn of_type(&self, id: TypeId) -> Option<&Registration> {
        self.rust.get(&id).map(|&index| &self.registrations[index])
    }

    /// Every registered type, in the order they were registered.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Registration> {
        self.registrations.iter()
    }

    /// Brings `value`, a value of the type named `ty` at its `version` as a document holds it, to
    /// the type's version now, through the type's upgrade ([`Reflect::upgrade`]), one version at a
    /// time; a value of the type's version comes back as it is. `partial` says whether the value
    /// leaves out the fields that did not change ([`Upgrade`](crate::Upgrade)).
    ///
    /// ```
    /// use slime_reflect::{Reflect, Registry, Value};
    ///
    /// /// A lantern, whose `power` was called `watts` at version 1.
    /// #[derive(Reflect)]
    /// #[reflect(name = "Lantern", version = 2, upgrade = upgrade)]
    /// struct Lantern {
    ///     power: f32,
    /// }
    ///
    /// fn upgrade(version: u32, value: Value, _partial: bool) -> Result<Value, String> {
    ///     match (version, value) {
    ///         (1, Value::Struct(fields)) => Ok(Value::Struct(
    ///             fields
    ///                 .into_iter()
    ///                 .map(|(name, value)| match name.as_str() {
    ///                     "watts" => ("power".to_owned(), value),
    ///                     _ => (name, value),
    ///                 })
    ///                 .collect(),
    ///         )),
    ///         (version, _) => Err(format!("no lantern of version {version} was a table")),
    ///     }
    /// }
    ///
    /// let mut registry = Registry::new();
    /// registry.register::<Lantern>();
    /// let old = Value::Struct(vec![("watts".to_owned(), Value::F64(60.0))]);
    /// let new = registry.upgrade("Lantern", 1, old, false).expect("an upgrade from version 1");
    /// assert_eq!(new, Value::Struct(vec![("power".to_owned(), Value::F64(60.0))]));
    /// ```
    ///
    /// # Errors
    ///
    /// [`Problem::NotRegistered`] for a type that the registry does not hold;
    /// [`Problem::NewerVersion`] for a version past the type's; [`Problem::Upgrade`] for version
    /// 0, which no type has, when the type has no upgrade, or when its upgrade refuses the value.
    pub fn upgrade(
        &self,
        ty: &str,
        version: u32,
        value: Value,
        partial: bool,
    ) -> Result<Value, ReflectError> {
        let registration = self
            .get(ty)
            .ok_or_else(|| ReflectError::new(Problem::NotRegistered(ty.to_owned())))?;
        let known = registration.info.version;
        if version > known {
            return Err(ReflectError::new(Problem::NewerVersion {
                found: version,
                known,
            }));
        }
        let refused = |from, reason: &str| {
            ReflectError::new(Problem::Upgrade {
                from,
                reason: reason.to_owned(),
            })
        };
        if version == 0 {
            return Err(refused(0, "versions start at 1"));
        }
        let mut value = value;
        for from in version..known {
            let upgrade = registration
                .upgrade
                .ok_or_else(|| refused(from, "the type has no upgrade"))?;
            value = upgrade(from, value, partial).map_err(|reason| refused(from, &reason))?;
        }
        Ok(value)
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
            // A unit struct's fields are none: it has no leaf, nothing in it to set.
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
            (Kind::Option(inner), Value::Option(Some(inner_value)))
                if self.has_parts(inner, inner_value)? =>
            {
                return self.collect(inner, inner_value, path, field, leaves);
            }
            _ => None,
        };
        match fields {
            Some(fields) => {
                for inner in fields {
                    let Some(inner_value) = value.child(&Step::Field(inner.name.clone())) else {
                        continue;
                    };
                    self.collect(
                        &inner.ty,
                        inner_value,
                        path.clone().field(inner.name.clone()),
                        Some(inner),
                        leaves,
                    )?;
                }
            }
            None => leaves.push(Leaf {
                path,
                ty: ty.to_owned(),
                field: field.cloned(),
            }),
        }
        Ok(())
    }

    /// Whether `value`, of the type named `ty`, holds parts that a path reaches: fields, a
    /// variant's fields, elements, or an option's value that does.
    fn has_parts(&self, ty: &str, value: &Value) -> Result<bool, ReflectError> {
        Ok(match (&self.info(ty)?.kind, value) {
            (Kind::Struct(fields), _) => !fields.is_empty(),
            (Kind::Enum(variants), Value::Enum(name, _)) => variants
                .iter()
                .any(|variant| &variant.name == name && !variant.fields.is_empty()),
            (Kind::List { .. }, Value::List(elements)) => !elements.is_empty(),
            (Kind::Option(inner), Value::Option(Some(inner_value))) => {
                self.has_parts(inner, inner_value)?
            }
            _ => false,
        })
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
            // An option that holds a value is gone through, to the value: the option's field, and
            // its range, stay.
            while let (Kind::Option(inner), Value::Option(Some(inner_value))) =
                (&self.info(&ty)?.kind, part)
            {
                ty = inner.clone();
                part = inner_value;
            }
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
        // Its kind first, then its range: a number of another kind is of the wrong kind.
        let in_range = || match (ranges, range, value.number()) {
            (true, Some(range), Some(number)) if !range.contains(number) => {
                Err(ReflectError::new(Problem::OutOfRange {
                    value: number,
                    range,
                }))
            }
            _ => Ok(()),
        };
        match (&info.kind, value) {
            (Kind::Bool, Value::Bool(_))
            | (Kind::Text, Value::Text(_))
            | (Kind::Asset, Value::Asset(_))
            | (Kind::Entity, Value::Entity(_)) => Ok(()),
            (Kind::F32, Value::F32(_)) | (Kind::F64, Value::F64(_)) => in_range(),
            (&Kind::Int { bits }, &Value::Int(number)) => {
                within_width(number, bits, true).and_then(|()| in_range())
            }
            (&Kind::UInt { bits }, &Value::UInt(number)) => {
                within_width(number, bits, false).and_then(|()| in_range())
            }
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

    /// Checks that `values` are exactly `fields`, each once and a value of its type.
    fn verify_fields(
        &self,
        fields: &[FieldInfo],
        values: &[(String, Value)],
        ranges: bool,
    ) -> Result<(), ReflectError> {
        if let Some(twice) = first_repeated(values.iter().map(|(name, _)| name)) {
            return Err(ReflectError::new(Problem::DuplicateField(twice.clone())));
        }
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

/// What `error`, found in a default, says: its path within the default, if it is in a part of it.
fn within_default(error: &ReflectError) -> String {
    if error.path().is_empty() {
        error.problem().to_string()
    } else {
        error.to_string()
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

/// The first of `names` that comes twice, if one does.
fn first_repeated<'a>(names: impl IntoIterator<Item = &'a String>) -> Option<&'a String> {
    let mut seen = HashSet::new();
    names.into_iter().find(|&name| !seen.insert(name))
}
