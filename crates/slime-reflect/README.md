# slime-reflect

The engine's reflection: types described at run time, their values as plain data, read and
changed by path through a registry (`docs/adr/0080-reflection-and-serialization.md`).

## Status

Reflection describes, reads and changes values; files and saves come with serialization, which
works through the same values. Maps, generic types and functions come when something needs them.

## Role

- `Reflect`, the trait of a type whose values reflection handles, and its derive,
  `#[derive(Reflect)]`, which requires a stable name: bare for the engine's types (`Transform`),
  `<crate>::<Type>` for a game's (`hunnawood::Collider`). Attributes give aliases, a version,
  defaults, skipped and transient fields, ranges and units. The standard types the engine uses
  (booleans, integers, floats, `String`, `Option`, `Vec`, arrays, pairs) and glam's `Vec3`,
  `DVec3` and `Quat` are described here, by hand: the orphan rule allows it nowhere else.
- `Value`, plain data, which passes between reflection, files, saves and the editors: references to
  assets and to entities are kinds of their own, and floats keep their width.
- `TypeInfo`, the description of a type: its kind, its fields or variants, with their names, types
  and attributes.
- `Registry`, the types a program describes, by name and alias, registered explicitly, each
  registration registering the types its values hold; it can hold a type with no Rust type behind
  it. It checks values against their types, sets their parts by `Path` within the fields' ranges,
  and lists their leaves.
- `diff` and `apply`: what differs between two values, path by path, and those differences set.

A crate that owns a value type implements reflection where the type is defined: `slime-assets`
for `AssetId` and the custom values of nodes, `slime-color` for `LinearColor`, `slime-world` for
its entities and components.

## Entry points

```rust
use slime_reflect::{Reflect, Registry, Value, apply, diff};

/// A lantern on a post.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "hunnawood::Lantern")]
struct Lantern {
    /// How strong its light is.
    #[reflect(range(min = 0.0), unit = "cd")]
    intensity: f32,
    /// Whether it is lit.
    lit: bool,
}

let mut registry = Registry::new();
registry.register::<Lantern>();
let lit = Lantern { intensity: 30.0, lit: true }.to_value();
let mut dimmed = lit.clone();
registry
    .set("hunnawood::Lantern", &mut dimmed, &"intensity".parse().unwrap(), Value::F32(12.0))
    .expect("in its range");
assert_eq!(Lantern::from_value(&dimmed), Ok(Lantern { intensity: 12.0, lit: true }));
let mut again = lit.clone();
apply(&mut again, &diff(&lit, &dimmed)).expect("the paths are in the value");
assert_eq!(again, dimmed);
```

The rustdoc holds the contracts: what the derive takes and refuses, what each operation checks,
and the errors, which name the path of the part at fault.

## Dependencies

`glam`, whose types it describes, and `slime-reflect-derive`, its derive macro, which it
re-exports. No other crate of the engine: those that own a value type depend on it.

## Must not

- Depend on another crate of the engine, or on a GPU, a window or a file format.
- Name a type by its module path: names are stable, and files and saves hold them.
- Register a type before `main` runs, by code that runs on its own: a program registers what it
  uses.
