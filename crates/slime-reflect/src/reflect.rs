//! The trait of reflected types.

use crate::error::ReflectError;
use crate::info::TypeInfo;
use crate::registry::Registry;
use crate::value::Value;

/// A function that brings a value of a type from the version it is given, of the type's versions
/// before its last, to the next: `upgrade(version, value, partial)`, or a reason why it cannot.
///
/// It works on the value as a document holds it, before any Rust type reads it, so that the type's
/// shape may have changed since: tables, arrays, text, booleans, and numbers, a float as `Value::F64`
/// or, written without a fraction, `Value::Int`; `slime-serialize` says how a document holds each
/// kind of value. When `partial` is true, the value leaves out the fields that did not change, as a
/// save's differences do: the upgrade changes what is there, and adds nothing. A whole value holds
/// every field of its version, and its upgrade adds the fields that the next version adds.
pub type Upgrade = fn(u32, Value, bool) -> Result<Value, String>;

/// A type whose values reflection describes, reads and changes: one that turns into a [`Value`]
/// and back, under a stable name, with a description of its shape ([`TypeInfo`]).
///
/// The engine's types and a game's derive it, `#[derive(Reflect)]`, which requires a stable name
/// (`docs/adr/0080-reflection-and-serialization.md`); the standard types the engine uses and
/// glam's vectors and quaternions are described here, by hand, with the helpers of
/// [`support`](crate::support). A type whose values follow rules beyond the ranges of their fields
/// declares a check, which refuses the values that break them (`docs/adr/0085-reflection.md`).
///
/// ```
/// use slime_reflect::{Reflect, Registry, Value};
///
/// /// A lantern on a post.
/// #[derive(Debug, PartialEq, Reflect)]
/// #[reflect(name = "hunnawood::Lantern")]
/// struct Lantern {
///     /// How strong its light is.
///     #[reflect(range(min = 0.0), unit = "cd")]
///     intensity: f32,
///     /// Whether it is lit.
///     lit: bool,
/// }
///
/// let lantern = Lantern { intensity: 30.0, lit: true };
/// let value = lantern.to_value();
/// assert_eq!(value.get(&"intensity".parse().unwrap()), Some(&Value::F32(30.0)));
/// assert_eq!(Lantern::from_value(&value), Ok(lantern));
///
/// let mut registry = Registry::new();
/// registry.register::<Lantern>();
/// assert!(registry.get("bool").is_some(), "a registration registers its fields' types");
/// ```
///
/// # The derive
///
/// `#[derive(Reflect)]` describes a struct, a tuple struct, a unit struct, or an enum whose
/// variants hold fields or not. Doc comments become the documentation of the type, its fields and
/// its variants. A tuple's fields are named `0`, `1`, ... It takes, in `#[reflect(...)]`:
///
/// - on the type: `name = "..."`, its stable name, which it requires; `alias = "..."`, a name it
///   had before, as many as it had; `version = 2`, the version of its values' shape, 1 if
///   unsaid; `default`, which makes data that leaves a value of the type out give its `Default`
///   value, and, on a struct, data that leaves a field out give that field's default value;
///   `check = path::to::check`, a function `fn(&Self) -> Result<(), String>` that accepts a value
///   once it is read, or refuses it with a reason ([`Problem::Invalid`](crate::Problem)): the
///   rules between fields, or on numbers without a range, such as a cone narrower than another;
///   `upgrade = path::to::upgrade`, an [`Upgrade`] from each of its earlier versions to the next,
///   which a type past version 1 declares, and only such a type;
/// - on a field: `alias = "..."`; `default`, which makes data that leaves the field out give its
///   type's `Default` value; `skip`, which leaves the field out of the description and the value,
///   and gives it its `Default` value when read, as an edit through reflection reads the whole
///   value again; `transient`, which leaves it out of saves only;
///   `range(min = 0.0, max = 1.0)`, or `above = 0.0` for a lower bound left out, each bound a
///   number or a constant, which edits through reflection respect, and which holds finite numbers
///   only; `unit = "m"`, the symbol of
///   its numbers' unit;
/// - on a variant: `alias = "..."`.
///
/// A field that data leaves out is refused, unless the field or its type gives a default: an
/// [`Option`] gives `None`.
///
/// The derive refuses a type without a name:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// struct Lantern {
///     lit: bool,
/// }
/// ```
///
/// a generic type, which is described by hand:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Pair")]
/// struct Pair<T> {
///     first: T,
/// }
/// ```
///
/// a union, which no description says which field holds:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Bits")]
/// union Bits {
///     float: f32,
///     integer: u32,
/// }
/// ```
///
/// a field whose type is not reflected:
///
/// ```compile_fail,E0277
/// struct Wick;
///
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp")]
/// struct Lamp {
///     wick: Wick,
/// }
/// ```
///
/// a range on a field that holds no number:
///
/// ```compile_fail,E0277
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp")]
/// struct Lamp {
///     #[reflect(range(min = 0.0))]
///     lit: bool,
/// }
/// ```
///
/// an attribute it does not know, or a skipped field given another:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp", color = "red")]
/// struct Lamp {
///     lit: bool,
/// }
/// ```
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp")]
/// struct Lamp {
///     #[reflect(skip, unit = "cd")]
///     intensity: f32,
/// }
/// ```
///
/// a version 0, which no type has, or an alias that starts with `$`, which documents keep for
/// themselves:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp", version = 0)]
/// struct Lamp {
///     lit: bool,
/// }
/// ```
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp")]
/// struct Lamp {
///     #[reflect(alias = "$value")]
///     lit: bool,
/// }
/// ```
///
/// a type past version 1 without an upgrade, which could not read its earlier values, or an upgrade
/// on a type at version 1, which has no earlier values:
///
/// ```compile_fail
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp", version = 2)]
/// struct Lamp {
///     lit: bool,
/// }
/// ```
///
/// ```compile_fail
/// fn upgrade(_: u32, value: slime_reflect::Value, _: bool) -> Result<slime_reflect::Value, String> {
///     Ok(value)
/// }
///
/// #[derive(slime_reflect::Reflect)]
/// #[reflect(name = "Lamp", upgrade = upgrade)]
/// struct Lamp {
///     lit: bool,
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not reflected",
    note = "derive `Reflect` for it, with `#[reflect(name = \"...\")]`, or implement it by hand"
)]
pub trait Reflect: Send + Sync + 'static {
    /// The stable name of the type, which a registry finds it by: the name of its [`TypeInfo`].
    fn type_name() -> String
    where
        Self: Sized;

    /// The description of the type.
    fn type_info() -> TypeInfo
    where
        Self: Sized;

    /// Registers the types that its values hold, its fields' ([`Registry::register`]), but not
    /// itself.
    fn register_dependencies(registry: &mut Registry)
    where
        Self: Sized;

    /// The value that data which leaves a value of this type out gives it, if the type declares
    /// one: `None` for an [`Option`], the `Default` value of a type that derives `Reflect` with
    /// `#[reflect(default)]`. `None` by default: data must give the value.
    fn default_value() -> Option<Self>
    where
        Self: Sized,
    {
        None
    }

    /// How a value of this type is brought from one version of the type to the next: the function
    /// that `#[reflect(upgrade = ...)]` names, which the type declares once past version 1.
    /// `None` by default.
    fn upgrade() -> Option<Upgrade>
    where
        Self: Sized,
    {
        None
    }

    /// Its value, as plain data.
    fn to_value(&self) -> Value;

    /// The value of this type that `value` describes.
    ///
    /// # Errors
    ///
    /// When `value` is not one, with the path of the part at fault: a [`ReflectError`] whose
    /// problem says what is wrong.
    fn from_value(value: &Value) -> Result<Self, ReflectError>
    where
        Self: Sized;
}
