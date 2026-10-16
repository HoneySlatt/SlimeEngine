//! The descriptions of types: their names, kinds, fields, variants and attributes.

use std::fmt;

use crate::value::Value;

/// The description of a reflected type: its name, the names it had before, its version, its
/// documentation, and its kind, with its fields or its variants.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeInfo {
    /// Its stable name, unique in a registry: bare for the engine's types (`Transform`),
    /// `<crate>::<Type>` for a game's or a plugin's, never a module path; composed for the
    /// standard types (`Option<f32>`, `Vec<AssetId>`, `[f32; 4]`, `(String, Extra)`).
    pub name: String,
    /// The names it had before, which a registry finds it by too.
    pub aliases: Vec<String>,
    /// The version of its values' shape, from 1: a change of meaning takes a new one, and an
    /// upgrade from the one before (`docs/adr/0080-reflection-and-serialization.md`).
    pub version: u32,
    /// Its documentation, from its doc comments.
    pub doc: String,
    /// The value that data which leaves a value of the type out gives it, if the type declares
    /// one: its [`Reflect::default_value`](crate::Reflect::default_value). A struct that declares
    /// one gives its fields, too, the values of its default when data leaves them out, whatever
    /// their own [`FieldInfo::default`] says.
    pub default: Option<Value>,
    /// What its values are.
    pub kind: Kind,
}

/// What the values of a type are.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Kind {
    /// Booleans.
    Bool,
    /// Signed integers of `bits` bits.
    Int {
        /// 8, 16, 32 or 64.
        bits: u32,
    },
    /// Unsigned integers of `bits` bits.
    UInt {
        /// 8, 16, 32 or 64.
        bits: u32,
    },
    /// 32-bit floats.
    F32,
    /// 64-bit floats.
    F64,
    /// Text.
    Text,
    /// A struct, a tuple struct or a tuple: its fields, in order. A unit struct has none.
    Struct(Vec<FieldInfo>),
    /// An enum: its variants, in order.
    Enum(Vec<VariantInfo>),
    /// An optional value of the type named.
    Option(String),
    /// A list of values of the type `element` names, of any length, or an array of `len`.
    List {
        /// The type of its elements, by name.
        element: String,
        /// The length of an array; `None` for a list.
        len: Option<usize>,
    },
    /// References to assets.
    Asset,
    /// References to entities.
    Entity,
}

/// A field of a struct, or of an enum's variant: its name, its type, and its attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    /// Its name: `0`, `1`, ... in a tuple.
    pub name: String,
    /// Its type, by name.
    pub ty: String,
    /// The names it had before.
    pub aliases: Vec<String>,
    /// Its documentation, from its doc comments.
    pub doc: String,
    /// The value that data which leaves the field out gives it, if the field declares one: its
    /// type's `Default` value, for a field that derives with `#[reflect(default)]`.
    pub default: Option<Value>,
    /// Whether it is left out of saves: what a game computes again, such as a value that changes
    /// at every frame. It is described, read and set as any other. A transient field has a default,
    /// its own, its struct's or its type's, which a save that leaves it out gives it: the registry
    /// refuses one without.
    pub transient: bool,
    /// The numbers it may hold, which edits through reflection respect
    /// ([`Registry::set`](crate::Registry::set)): its own, or each of its elements' for a list.
    pub range: Option<Range>,
    /// The unit of its numbers, as a symbol: `m`, `rad`, `lx`, `cd`, ...
    pub unit: Option<String>,
}

impl FieldInfo {
    /// The field `name`, of the type named `ty`, without attributes.
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            aliases: Vec::new(),
            doc: String::new(),
            default: None,
            transient: false,
            range: None,
            unit: None,
        }
    }
}

/// A variant of an enum: its name, and its fields, as a struct's.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantInfo {
    /// Its name.
    pub name: String,
    /// The names it had before.
    pub aliases: Vec<String>,
    /// Its documentation, from its doc comments.
    pub doc: String,
    /// Its fields; none for a variant without data.
    pub fields: Vec<FieldInfo>,
}

/// The finite numbers a field may hold: from `min` to `max`, `min` itself included unless
/// `min_excluded`, `max` included. A range without a lower or upper bound has an infinite one; it
/// holds no infinity and no `NaN` either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    /// Its lower bound, `-∞` for none.
    pub min: f64,
    /// Whether `min` itself is left out: a positive number.
    pub min_excluded: bool,
    /// Its upper bound, included, `+∞` for none.
    pub max: f64,
}

impl Range {
    /// Every number: no bound.
    pub const ALL: Self = Self {
        min: f64::NEG_INFINITY,
        min_excluded: false,
        max: f64::INFINITY,
    };

    /// Whether `number` lies within it. A range holds finite numbers only: an infinity or a `NaN`
    /// lies within none, whatever its bounds.
    pub fn contains(&self, number: f64) -> bool {
        if !number.is_finite() {
            return false;
        }
        let above = if self.min_excluded {
            number > self.min
        } else {
            number >= self.min
        };
        above && number <= self.max
    }
}

/// `≥ 0`, `> 0`, `from 0 to 1.5`, `≤ 1`: each bound as the `f64` it is.
impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let low = self.min > f64::NEG_INFINITY;
        let high = self.max < f64::INFINITY;
        match (low, high, self.min_excluded) {
            (true, true, false) => write!(f, "from {} to {}", self.min, self.max),
            (true, true, true) => write!(f, "> {} and ≤ {}", self.min, self.max),
            (true, false, false) => write!(f, "≥ {}", self.min),
            (true, false, true) => write!(f, "> {}", self.min),
            (false, true, _) => write!(f, "≤ {}", self.max),
            (false, false, _) => write!(f, "a finite number"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_holds_what_lies_between_its_bounds() {
        let positive = Range {
            min: 0.0,
            min_excluded: true,
            ..Range::ALL
        };
        assert!(!positive.contains(0.0) && positive.contains(1e-30));
        let cone = Range {
            min: 0.0,
            max: 1.5,
            ..Range::ALL
        };
        assert!(cone.contains(0.0) && cone.contains(1.5) && !cone.contains(1.6));
        assert!(!Range::ALL.contains(f64::NAN));
        assert!(!positive.contains(f64::INFINITY) && !Range::ALL.contains(f64::NEG_INFINITY));
        assert_eq!(positive.to_string(), "> 0");
        assert_eq!(cone.to_string(), "from 0 to 1.5");
    }
}
