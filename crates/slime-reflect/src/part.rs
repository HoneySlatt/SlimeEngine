//! Parts of values: what changed of a value, as a save holds it, and those changes set.

use crate::error::{Problem, ReflectError};
use crate::info::{FieldInfo, Kind};
use crate::path::{Path, Step};
use crate::registry::Registry;
use crate::value::Value;

/// Whether `fields` are a tuple's: named `0`, `1`, ... in order. A part holds a tuple whole.
pub fn is_tuple(fields: &[FieldInfo]) -> bool {
    !fields.is_empty()
        && fields
            .iter()
            .enumerate()
            .all(|(index, field)| field.name == index.to_string())
}

impl Registry {
    /// What changed from `from` to `to`, values of the type named `ty`, as a *part* of a value:
    /// the fields of its structs, and of its enums when both hold the same variant, that differ,
    /// at any depth; whole, whatever else differs: numbers, text, tuples, lists, options, a variant
    /// that another replaced. `None` when nothing differs, floats compared by their bits. Fields
    /// marked transient are left out unless `transient`: what a save leaves out.
    ///
    /// ```
    /// use slime_reflect::{Reflect, Registry, Value};
    ///
    /// #[derive(Reflect)]
    /// #[reflect(name = "Lamp")]
    /// struct Lamp {
    ///     intensity: f32,
    ///     lit: bool,
    /// }
    ///
    /// let mut registry = Registry::new();
    /// registry.register::<Lamp>();
    /// let (on, dim) = (
    ///     Lamp { intensity: 30.0, lit: true }.to_value(),
    ///     Lamp { intensity: 12.0, lit: true }.to_value(),
    /// );
    /// let part = registry.difference("Lamp", &on, &dim, false).expect("lamps");
    /// assert_eq!(part, Some(Value::Struct(vec![("intensity".to_owned(), Value::F32(12.0))])));
    /// let mut lamp = on.clone();
    /// registry.overlay("Lamp", &mut lamp, &part.expect("a change")).expect("a part of a lamp");
    /// assert_eq!(lamp, dim);
    /// ```
    ///
    /// # Errors
    ///
    /// [`Problem::NotRegistered`] for a type that the registry does not hold, with its path.
    pub fn difference(
        &self,
        ty: &str,
        from: &Value,
        to: &Value,
        transient: bool,
    ) -> Result<Option<Value>, ReflectError> {
        let kind = &self
            .get(ty)
            .ok_or_else(|| ReflectError::new(Problem::NotRegistered(ty.to_owned())))?
            .info()
            .kind;
        let changed = match (kind, from, to) {
            (Kind::Struct(fields), Value::Struct(from_fields), Value::Struct(to_fields))
                if !is_tuple(fields) =>
            {
                self.changed_fields(fields, from_fields, to_fields, transient)?
                    .map(Value::Struct)
            }
            (
                Kind::Enum(variants),
                Value::Enum(from_name, from_fields),
                Value::Enum(to_name, to_fields),
            ) if from_name == to_name => {
                match variants.iter().find(|variant| &variant.name == to_name) {
                    Some(variant) if !is_tuple(&variant.fields) => self
                        .changed_fields(&variant.fields, from_fields, to_fields, transient)?
                        .map(|fields| Value::Enum(to_name.clone(), fields)),
                    _ => (!from.same(to)).then(|| to.clone()),
                }
            }
            _ => (!from.same(to)).then(|| to.clone()),
        };
        Ok(changed)
    }

    /// The fields of `to` that differ from those of `from`, which `fields` describes, each as a part.
    fn changed_fields(
        &self,
        fields: &[FieldInfo],
        from: &[(String, Value)],
        to: &[(String, Value)],
        transient: bool,
    ) -> Result<Option<Vec<(String, Value)>>, ReflectError> {
        let field_value = |values: &[(String, Value)], name: &str| {
            values
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value.clone())
        };
        let mut changed = Vec::new();
        for field in fields.iter().filter(|field| transient || !field.transient) {
            let part = match (field_value(from, &field.name), field_value(to, &field.name)) {
                (Some(from), Some(to)) => self
                    .difference(&field.ty, &from, &to, transient)
                    .map_err(|error| error.within(Step::Field(field.name.clone())))?,
                (_, to) => to,
            };
            if let Some(part) = part {
                changed.push((field.name.clone(), part));
            }
        }
        Ok((!changed.is_empty()).then_some(changed))
    }

    /// Sets in `value`, a value of the type named `ty`, what `part` holds, a part of a value as
    /// [`Registry::difference`] makes it: each of its fields in place of `value`'s, at any depth, and
    /// whole what it holds whole, as [`Registry::set`] sets it, within the fields' ranges. A variant
    /// other than the one that `value` holds is set whole: the part must hold all of its fields.
    ///
    /// # Errors
    ///
    /// As [`Registry::set`], with the path of the part at fault: a field that the type does not
    /// have, a value of another kind, a number out of its field's range. `value` may be changed in
    /// part then.
    pub fn overlay(&self, ty: &str, value: &mut Value, part: &Value) -> Result<(), ReflectError> {
        let mut leaves = Vec::new();
        self.part_leaves(ty, value, part, Path::default(), &mut leaves)?;
        for (path, leaf) in leaves {
            self.set(ty, value, &path, leaf)?;
        }
        Ok(())
    }

    /// The values that `part`, a part of `value`, sets whole, with their paths: the fields of its
    /// structs, and of its enums that hold the variant that `value` holds, gone through.
    fn part_leaves(
        &self,
        ty: &str,
        value: &Value,
        part: &Value,
        path: Path,
        leaves: &mut Vec<(Path, Value)>,
    ) -> Result<(), ReflectError> {
        let kind = &self
            .get(ty)
            .ok_or_else(|| ReflectError::at(path.clone(), Problem::NotRegistered(ty.to_owned())))?
            .info()
            .kind;
        let fields = match (kind, value, part) {
            (Kind::Struct(fields), Value::Struct(values), Value::Struct(parts))
                if !is_tuple(fields) =>
            {
                Some((fields, values, parts))
            }
            (Kind::Enum(variants), Value::Enum(name, values), Value::Enum(part_name, parts))
                if name == part_name =>
            {
                variants
                    .iter()
                    .find(|variant| &variant.name == name)
                    .filter(|variant| !is_tuple(&variant.fields))
                    .map(|variant| (&variant.fields, values, parts))
            }
            _ => None,
        };
        let Some((fields, values, parts)) = fields else {
            leaves.push((path, part.clone()));
            return Ok(());
        };
        for (name, part_value) in parts {
            let at = path.clone().field(name.clone());
            let field = fields
                .iter()
                .find(|field| &field.name == name)
                .ok_or_else(|| {
                    ReflectError::at(
                        at.clone(),
                        Problem::UnknownField {
                            field: name.clone(),
                            known: fields.iter().map(|field| field.name.clone()).collect(),
                        },
                    )
                })?;
            match values.iter().find(|(field_name, _)| field_name == name) {
                Some((_, held)) => self.part_leaves(&field.ty, held, part_value, at, leaves)?,
                None => leaves.push((at, part_value.clone())),
            }
        }
        Ok(())
    }
}
