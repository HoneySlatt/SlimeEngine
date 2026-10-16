# 0086. Serialization

- Status: Accepted; amends [0080](0080-reflection-and-serialization.md) (where serialization lives,
  and the conventions of its documents) and [0085](0085-reflection.md) (defaults described by
  their values, and upgrades)
- Date: 2026-10-16

## Context

[ADR-0080](0080-reflection-and-serialization.md) decides that serialization is driven by
reflection, through its one value type; that text is TOML, read with `toml_edit` and written by the
engine's own writer, the same value giving the same bytes on every platform; that a file states the
version of its format and of each type it holds, an earlier version brought forward by upgrades on
plain values that accept partial ones; that sources are read strictly and saves leniently; and that
reading never panics, errors naming the file, the line, the column and the property path. It leaves
two questions to the milestone that builds it, M5a.7: where serialization lives, and how TOML holds
what it has no notion of: enum variants, absent values, integers past 64 signed bits.

Where the engine stood: `slime-reflect` described types and turned values into `Value`s and back,
with no file format. A description said whether data that leaves a value out gives it a default,
but not which value: only the Rust type knew it, in `from_value`. A type had a version and no way
to bring earlier values forward. The cooker read and wrote metadata files itself, a one-key subset
of TOML, `id = "<uuid>"`; 68 of them are committed, each exactly `id = "<uuid>"` and a line feed.
`toml_edit` 0.25.17, `toml_parser`, `winnow` and `toml_datetime` were in the lock file already, for
winit on Android, and built for neither Linux nor Windows.

## Decision

The owner took the first two decisions on 2026-10-16.

**Serialization is a crate of its own, `slime-serialize`**, above `slime-reflect`: only it depends
on TOML. The crates that describe their types, and a game that writes no file, do not build a
parser; the binary encoding of Stage 5b has its place beside the text one.

**The conventions of documents** (the rustdoc of `slime_serialize::format` is their home):

- A document holds one value, whose type the reader names: a value written as a table is the root
  table, any other the root's `$value`.
- A type past version 1 has its version under `$versions`, a table at the top, by name, once per
  document; a type left out is at version 1, so that metadata files keep their bytes. `$format`
  gives the format's version, 1 if left out. Keys that start with `$` are the format's: no Rust
  field is named so, and a type with no Rust type behind it, or an alias, may not be.
- A struct is a table of its fields, a tuple struct an array of them, a tuple struct of one field
  that field's value, a unit struct an empty table.
- An option in a field that holds none is left out, and an option field left out holds none,
  whatever its default; one that holds a value is that value. An option elsewhere is an empty array
  or an array of its value.
- A variant without fields is its name, as text; one with fields is a table of one key, its name,
  holding its fields as a struct's: `[light.Spot]`.
- Unsigned integers past `i64::MAX` are text. Floats come back to their bits: their shortest digits,
  of 32 bits for an `f32` unless TOML's parser, which reads 64 bits, would then round it to
  another, and `inf`, `-inf`, `nan`, `-nan`; a NaN's payload is not kept.
- References to assets are their IDs as text. References to entities are neither written nor read:
  a handle holds within one run only, and the world's documents of M5a.8 are to write persistent
  identities in their place.
- The writer puts a table of numbers, booleans and text on its key's line, and a larger table under
  a header of its own; keys bare where TOML allows; basic strings; LF line endings.

**Upgrades work on the values of documents.** A type past version 1 names an upgrade,
`#[reflect(upgrade = path)]`, a `fn(u32, Value, bool) -> Result<Value, String>` from each earlier
version to the next, told whether the value is partial, so that it adds the fields a whole value
lacks and nothing to a part; the derive refuses a type past version 1 without one, and an upgrade
at version 1. The reader applies them, one version at a time, to the value as the document holds
it, tables and arrays as `Value::Struct` and `Value::List`, before the type's description reads
it: a type whose shape changed is read from its earlier shape, a part of a value as well as a
whole one.

**Defaults are described by their values.** `TypeInfo::default` and `FieldInfo::default` hold the
value that data which leaves the value out gives it, not whether there is one: a document that
leaves out a field with a default reads into a whole value, which reflection and the world accept,
and a type with no Rust type behind it declares defaults too. The registry checks them, within
their fields' ranges, when it registers the type.

**Reading and writing.** `read` gives a whole value, `read_partial` a part of one, its missing
fields left out, `read_file` a file's value; `write` and `write_partial` write them, refusing what
a reader would refuse, a number out of its field's range included, so that what is written can be
read; `read_as` and `write_as` read and write a Rust type's value through a registry of its own.
Strict reading refuses a field that the type does not have; lenient reading leaves it out with a
warning, as it does a type it does not know under `$versions`. Documents are refused past 16 MiB,
at which the densest one took 1.8 GB and 3.7 s to read in a release build, most of it the
parser's, and past 64 levels of nesting, under the 80 at which the parser stops.
The place of a part in the text is looked up only when an error needs it: an error is placed at
the key of a field unknown or given twice, and at the value otherwise.

**Metadata files** are the type `Metadata { id: AssetId }`, read strictly and written through
`slime-serialize`; the cooker's own reader is removed once a test has read every committed metadata
file through both readers and found the same ID, and the writer has written each back to the same
bytes. `*.toml` files are kept with LF endings by `.gitattributes`, as metadata files are.

## Alternatives rejected

- **Serialization in `slime-reflect`**: one crate fewer, but every crate that describes its types,
  and every game, would build a TOML parser it does not use, and the formats would mix with the
  descriptions.
- **Writing every version, 1 included**: more explicit, but every metadata file would change, and
  small files would grow a table that says nothing.
- **A variant named by a field beside its own** (`variant = "Spot"`): one level shorter, but the
  name would be reserved in every type for good, and nested variants would be ambiguous.
- **Values out of their fields' ranges read leniently, with a warning**: a save would load values
  that its types refuse; the writer refuses them instead, so that what it writes can be read.
- **Upgrades on Rust types or on typed values**: an earlier shape cannot be read by the type's
  description now, nor by a Rust type that no longer has it.
- **Writing an entity's handle**: tests would pass, but files would hold numbers that mean nothing
  in the next run, which later versions would have to migrate.
- **`toml_edit`'s writer, or the `toml_writer` crate**: their output may change with their versions;
  the bytes of a document must not.

## Consequences

- A save, a scene file or a setting is a value of a registered type, read and written by one path,
  whose conventions are documented once; upgrades are written against documents' values.
- Defaults are values in descriptions: editors and tools can start a value from them without its
  Rust type.
- References to entities wait for the world's documents (M5a.8); a document that holds one is
  refused.
- The checks build four more crates, `toml_edit`, `toml_parser`, `winnow` and `toml_datetime`; the
  lock file gains no third-party crate.
