# 0080. Reflection and serialization

- Status: Accepted; amends [0069](0069-the-asset-pipeline.md) (what `slime-assets` depends on, and
  who reads metadata files); amended by [0085](0085-reflection.md) (when a component's type is
  registered, and the `check` attribute), [0086](0086-serialization.md) (where serialization
  lives, and the conventions of its documents) and [0101](0101-saves-and-identities-that-never-mislead.md) (aliases that collide)
- Date: 2026-10-16

## Context

The roadmap makes reflection a backbone of the engine: serialization, the editor's inspector,
scripting, replication, undo and redo, and visual scripting all depend on it. Serialization depends
on reflection, and the world on both. Its format is an open decision until Stage 5: "text for
sources, binary for cooked data". Stage 5a builds both ([ADR-0078](0078-stage-5-in-two-halves.md)),
for the world of [ADR-0079](0079-the-engines-own-world.md).

Where the engine stands:

- No crate describes its types at run time. Value types are plain data: `AssetId` is a 128-bit
  number written as a UUID; the color type has four `f32`s; positions and rotations are glam's.
  Rust's orphan rule allows implementing a trait for glam's types only in the crate that defines
  the trait.
- Metadata files hold one key, `id = "<uuid>"`, a subset of TOML that the cooker reads and writes
  itself, meant to grow into the engine's serialization
  ([ADR-0069](0069-the-asset-pipeline.md)). The cooked formats are binary formats written by hand;
  a cooker of another version refuses them, and they are cooked again rather than migrated.
- `syn`, `quote` and `proc-macro2`, with which derive macros are written, are already compiled for
  every engine crate, through `thiserror`'s derive. `serde` is compiled only for the cooker and the
  examples' tests, through `gltf`. No TOML crate is compiled for Linux or Windows.

What later stages ask: an inspector that shows any component (Stage 6); scripts and plugins that
register types at run time, through a C ABI (Stage 8); saves that a game updated for years still
reads; migration tooling for serialized formats (Stage 11).

What the design studies found:

- `bevy_reflect` 0.20 requires glam 0.33, where the engine uses 0.34, and the orphan rule would
  keep the engine from implementing it for glam itself; it makes a breaking release every three to
  five months, registers only Rust types, and names a type by the module it is in, so that moving a
  type breaks the files that name it. `facet` made twelve breaking releases in five months, and
  needs unsafe code for glam's types.
- With `serde`, a type registered at run time has no serializer, a component that needs reflection
  anyway derives both, and types have no versions.
- RON read without its types loses the variants of enums, which upgrading old files needs; the
  `ron` crate made four breaking releases in 2025. JSON has no comments. The YAML crates are
  deprecated or unsound. `bincode` is unmaintained (RUSTSEC-2025-0141); `postcard` and `bitcode`
  are positional, so a game update cannot add or remove a field; `rkyv` ties files to the layout of
  types.

## Decision

The owner took these decisions on 2026-10-16, when Stage 5a was planned.

**Reflection is the engine's own**, in a crate `slime-reflect` and its derive macro.

- `#[derive(Reflect)]` describes a type's values: structs, tuple structs, and enums whose variants
  hold data. The standard types the engine uses (booleans, integers, floats, strings, `Option`,
  `Vec`, arrays), glam's types, the color type, `AssetId` and references to entities are described
  by hand.
- Each type has a stable name that the derive requires, never a module path: bare for the engine's
  types (`Transform`), `<crate>::<Type>` for a game's or a plugin's; former names are aliases. Names
  are unique in a registry.
- One value type, plain data, passes between reflection, files, saves, the inspector and later
  scripts and the C ABI. References to assets and to entities are kinds of their own; floats keep
  their width.
- Property paths read and set the parts of a value (`light.intensity`). Values are compared, and
  their differences applied, path by path.
- Types are registered explicitly, and a registration registers the types of its fields; a type
  never registered fails with its name. A registration can describe a type with no Rust type behind
  it, for Stage 8.
- A field that data leaves out is an error, unless the field or its type declares a default.
- Attributes at Stage 5: documentation, from doc comments; name, alias and version; default; skip;
  transient, not saved; range, which edits through reflection respect; unit.
- `slime-reflect` is a leaf crate: it depends on glam and its derive only. A crate that owns a value
  type implements reflection where the type is defined: `slime-assets` for `AssetId`, and the crate
  that the color type moves to (ADR-0079). This amends ADR-0069, under which `slime-assets` depended
  on no engine crate: it depends on `slime-reflect` only.
- The derive is written with `syn`, `quote` and `proc-macro2`, which the owner approved as direct
  dependencies: they add no crate to the build.

**Serialization is driven by reflection**, through the same value type: one derive and one path,
for compiled types and for types registered at run time. No engine crate depends on `serde`.

- **Text is TOML**, for sources (the scene files of Stage 5b, metadata files, settings) and for
  saves. It is read with `toml_edit`, with `toml_parser`, `toml_writer`, `toml_datetime` and
  `winnow`, which the owner approved (MIT or Apache-2.0; no `serde`, no proc macro), and written by
  the engine's own writer: the same value gives the same bytes on Linux and on Windows, with LF line
  endings, whatever the version of the crate. The milestone that builds it sets the conventions for
  what TOML lacks: enum variants, absent values, integers beyond 64 signed bits.
- **Versions.** A file states the version of its format and the version of each type it holds.
  Renames are aliases. An upgrade from one version of a type to the next works on plain values,
  before any Rust type is involved, and accepts partial ones: a saved difference, an override. A
  source is read strictly, an unknown field being an error; a save leniently, with a warning. A
  version newer than the engine knows is refused. Files of earlier versions are committed, and the
  tests load them.
- Reading never panics: errors name the file, the line, the column and the property path. The sizes
  and depths of what is read are limited, for files that come from elsewhere.
- **Binary** is a self-describing encoding of the same values, the engine's own, built when Stage 5b
  cooks scene files, since cooked data is read without parsing text. Saves stay text until a
  measurement asks for more. The cooked formats of meshes, textures, materials and the manifest
  stay as they are.
- **Metadata files** are read through the same reader, and every existing one stays valid as it is.
  An import setting comes when an importer first needs one. This amends ADR-0069, whose metadata
  files the cooker read with code of its own.

## Alternatives considered

- **`bevy_reflect`**: complete today, but the engine's glam would follow its releases, scripts
  would need a second registry, and moving a type would break files.
- **`facet`**: a design close to a C ABI, but its releases break, and glam would need unsafe code.
- **No derive, every type registered by hand**: each type written twice, and a field forgotten in
  its registration never saved, without an error.
- **Automatic registration** through `inventory` or `linkme`: nothing to forget, but code that runs
  before `main`, registrations that vanish when nothing links their crate, and care needed for the
  static library a C++ SDK may link. It can come later without changing games.
- **Types named by their module path**, which moving a type breaks, or **by a UUID each**, which
  makes files unreadable and which a copied type duplicates.
- **`serde` as the framework**: two derives per component, which can disagree; no versions; nothing
  for types registered at run time; `serde` in every game. **Reflection bridged to `serde`**, as
  Bevy does: `serde` and the format's crate in every game, and the loss of enum variants with RON.
- **RON through the `ron` crate**: the above, its breaking releases, and CRLF line endings by
  default on Windows. **A syntax of the engine's own, close to RON**: typed values and no
  dependency, but a thousand lines of reader to harden against corrupt files, no outside tool to
  read it, and metadata files that would leave TOML. **JSON**: no comments, quotes everywhere.
  **YAML**: deprecated or unsound crates. **KDL**: no arrays as values.
- **`postcard`, `bincode` or its successors, `bitcode`, `rkyv`, MessagePack for saves**:
  positional, unmaintained, tied to the layout of types, or bringing `serde`; none lets a game
  update add or remove a field.

## Consequences

- The engine writes its reflection and its serialization, about three milestones: reflection,
  serialization, and the world's documents. They do less than `bevy_reflect` at first: maps,
  generic types and functions come when something needs them.
- One derive per type: components, a game's state, metadata and settings share it, and Stage 6's
  inspector can show import settings as it shows components.
- `slime-assets` and the color type's crate depend on `slime-reflect`, and the roadmap's dependency
  table gains it.
- The checks compile five more crates, the TOML ones; `syn`, `quote` and `proc-macro2` become
  direct dependencies, without a new crate.
- A type, field or variant renamed keeps old files readable through an alias only; a change of
  meaning takes a version and an upgrade.
- Stage 11's migration tooling builds on the versions in every file, the registered upgrades and
  aliases, and the canonical writer.
