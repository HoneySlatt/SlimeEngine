# slime-serialize

The engine's serialization: values of reflected types written to TOML documents and read back,
from earlier versions of their types too (`docs/adr/0080-reflection-and-serialization.md`,
`docs/adr/0086-serialization.md`).

## Status

Text documents only: the cooker's metadata files read through it. Scene files, the world's
documents and saves come with the milestones that need them, and a binary encoding of the same
values with Stage 5b's cooked scenes. References to entities are neither written nor read until
a world's documents write them as persistent identities.

## Role

- `write` and `write_partial`: a value of a registered type, or a part of one, as a document, by
  the engine's own writer: the same value gives the same bytes, with LF line endings, on every
  platform and with every version of the parser, and what could not be read back is refused.
  TOML lacks enum variants, absent values and the largest integers: the module `format` says how
  a document holds every kind of value, and is the home of those conventions.
- `read`, `read_partial` and `read_file`: the value of a registered type that a document holds,
  whole or in part, brought from the version of its type that the document says to the type's
  version through the type's upgrade (`slime_reflect::Upgrade`). A source is read strictly, a save
  leniently (`Mode`). Errors name the file, the line, the column and the path of the part at
  fault; documents larger or deeper than a limit are refused; reading never panics.

## Entry points

```rust
use slime_reflect::{Reflect, Registry};
use slime_serialize::{Mode, read, write};

/// A door, which an earlier version called `closed`.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "my_game::Door")]
struct Door {
    #[reflect(alias = "closed")]
    open: bool,
}

let mut registry = Registry::new();
registry.register::<Door>();
let text = write(&registry, "my_game::Door", &Door { open: true }.to_value()).expect("a door");
assert_eq!(text, "open = true\n");
let read = read(&registry, "my_game::Door", "closed = false", Mode::Strict).expect("by its alias");
assert_eq!(Door::from_value(&read.value), Ok(Door { open: false }));
```

The rustdoc holds the contracts: the format, what each function refuses, and the limits.

## Dependencies

`slime-reflect`, whose descriptions lead both ways; `toml_edit`, its parser only, which brings
`toml_parser`, `winnow` and `toml_datetime`; `thiserror`, for its error type.

## Must not

- Write TOML through another writer: the bytes of a document must not depend on a library's
  version.
- Write or read an entity's handle, which holds within one run only.
- Depend on `serde`: reflection describes every type that it writes
  (`docs/adr/0080-reflection-and-serialization.md`).
