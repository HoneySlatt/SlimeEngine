# 0085. Reflection

- Status: Accepted; refines [0080](0080-reflection-and-serialization.md) (when a type is registered)
- Date: 2026-10-16

## Context

[ADR-0080](0080-reflection-and-serialization.md) decides that reflection is the engine's own, in
`slime-reflect` and its derive: stable names, one value type of plain data, property paths,
values compared and their differences applied, an explicit registry that registers the types of
a type's fields and can hold a type with no Rust type behind it, and the attributes of Stage 5.
M5a.5 builds it, and reaches every component of the world and of the game through it. Two
questions were left open: whether a component must be reflected, and where the registry lives.

Where the engine stood: a type became a component by saying so, `impl Component for Position {}`;
the world stored each component type in a storage of its own, found by its Rust type, and could
not list the components an entity holds. Six components were the engine's (`Transform`, `Name`,
`Meshes`, `Hidden`, `Light`, `Extras`) and five *Hunna Wood*'s (`Collider`, `Marker`, `TimeOfDay`,
`Flickers`, `Night`); their fields hold booleans, floats, text, options, lists, a pair, glam's
`DVec3`, `Vec3` and `Quat`, the color type and asset IDs.

## Decision

The owner took the first two decisions on 2026-10-16.

**Every component is reflected.** `Component` requires `Reflect`: a type that is not described
does not compile as a component. The F2 panel (M5a.6) lists every component, and saves (M5a.9)
hold every one; none is forgotten silently.

**Each world keeps its registry.** `World::new` registers the engine's components; the first
component of a type that a world stores registers the type, and the types its values hold. A game
registers ahead, with `World::register`, the types it must find by name before an entity holds
one, as the components of a save it reads; *Hunna Wood* registers all of its components when it
spawns its zone. No code runs before `main`: this refines ADR-0080's "types are registered
explicitly" for components only. Types with no Rust type behind them are registered by their
description, `Registry::register_dynamic`.

The rest follows from ADR-0080:

- **The value type** holds booleans, signed and unsigned integers of up to 64 bits, `f32` and
  `f64` apart, text, structs (fields by name, in order; a tuple's named `0`, `1`, ...), enums (a
  variant by name, with its fields), options, lists, references to assets (their 128 bits) and
  references to entities. A reference to an entity holds the bits of its handle, which mean
  something only in its world while it runs: files will name entities by their persistent identity
  (ADR-0079), which serialization gives in place of the handle (M5a.7, M5a.8).
- **Descriptions** (`TypeInfo`) give a type's name, aliases, version, documentation, whether data
  that leaves it out takes a default, and its kind, with its fields or variants; a field gives its
  name, type, aliases, documentation and attributes. The derive writes them from the type, its
  doc comments and `#[reflect(...)]`: `name`, `alias`, `version` and `default` on a type; `alias`,
  `default`, `skip`, `transient`, `range` and `unit` on a field; `alias` on a variant. A range is
  inclusive, its lower bound left out with `above` for a positive number; a unit is a symbol. The
  derive refuses a generic type, a type with lifetimes, a union, a type without a name, an unknown
  attribute, a skipped field given another attribute, and a range on a field that holds no
  number; the compiler refuses a field whose type is not reflected.
- **Edits work on values.** `Registry::set` follows a path through a value and its type's
  description, checks the new part's kind, fields, variant, widths and lengths, and its numbers
  against the ranges of the fields they are in, then sets it; a field of an enum is one of the
  variant the value holds, and another variant is set whole. The world edits a component by
  taking its value, setting the part, and reading the component back from the value: the derive
  writes only the conversions, and nothing reaches into a Rust value field by field. Errors name
  the path of the part at fault, and the component in the world's.
- **Values are compared** path by path, floats by their bits; two lists of different lengths, or
  two variants, differ whole.
- **The standard types** described by hand are the booleans, integers, floats, `String`, `Option`,
  `Vec`, arrays and pairs; glam's `Vec3`, `DVec3` and `Quat`, the types the components hold. Others
  come when a type needs them. A missing option is `None`.
- `slime-reflect` implements `Display` and `Error` for its errors by hand: ADR-0080 keeps it to
  glam and its derive, and `thiserror` would be a third dependency.

## Alternatives considered

- **Reflection optional for components**, registered apart, as Bevy does: any type could be a
  component, but one never registered would be invisible to the panel and lost from saves without
  an error.
- **A registry apart from the worlds**, which the game makes and gives to the world, the panel and
  saves: one registry for every world and tool, but one more object to pass, and a component
  stored but not registered found by no name until run time. **Strict registration**, an
  unregistered component refused when inserted: explicit, at the cost of a line per component that
  nothing checks at compile time.
- **Typed access field by field**, which the derive would write for each field, as `bevy_reflect`
  does: edits without a round trip through values, at the cost of much more generated code, before
  any measurement asks for it.
- **A range of two optional bounds**, without `above`: a light's range and its shadow distance are
  positive, which an inclusive bound cannot say.
- **Units as an enum**: typed, but closed; a symbol says the same, and the editor shows it as it
  is.

## Consequences

- Every component derives `Reflect` with a stable name, those of the tests included; a crate that
  defines components depends on `slime-reflect`. `slime-world` depends on it, and `slime-assets`
  and `slime-color` do, as ADR-0080 decided.
- A world lists its entities and the names of the components each holds, and reads and sets any
  component, or a part of it, by name and path, without its type (`World::component_value`,
  `World::set_property`). A transform set this way counts as changed.
- An edit costs the conversion of the whole component to a value and back: nothing for an editor,
  which edits one field at a time; a system that changes components at every frame changes them
  typed.
- Ranges are per field. A rule between two fields, a spot light's inner cone narrower than its
  outer cone, is not checked; nor is a rotation's length.
- Serialization (M5a.7) writes and reads these values; it maps references to entities to persistent
  identities, and decides how a save reads a value out of range.
