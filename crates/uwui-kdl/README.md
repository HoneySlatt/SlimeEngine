# uwui-kdl

uwui's reader of KDL 2.0, the notation of every document of uwui: layouts, a theme's parts, values
and rules, shortcuts, docks' layouts and settings
([ADR-0241](../../docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md),
[ADR-0225](../../docs/adr/0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)).

Its name is provisional: the names of uwui's crates are settled at M6d.10
([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

## Role

- `read` and `read_within` read a text into a `Document`: a flat tree of nodes and entries, each
  placed in the text by its span, strings of one line without escapes borrowed. The reader checks
  the syntax alone; what a node means is the typed layer's work above it, in `uwui`, which reads
  its themes, style sheets, tables of shortcuts and docks' layouts through it, and in `slime-cook`, which checks
  the syntax of the documents it cooks.
- `Limits` bound what content from outside may hold: its size, its depth, its count of nodes and
  entries. The reader never recurses; its time grows with its text alone.
- An `Error` names its line, its column in characters, and what is wrong; `Error::at` places one
  that the typed layer above finds at an item's span, as the reader places its own.
- `write_string`, `write_f32` and `write_integer` write KDL's values, the one place that writes
  them: the normal form shares them, and `uwui::dock` writes a layout with them. A number written
  by `write_f32` reads back to its bits through `Number::to_f32`, which rounds once from the text.
- `Document::normal_form` writes a document in the normal form of KDL's official tests.

The crate's rustdoc holds each item's contract.

## Dependencies

None. It lies below `uwui` and `uwui-macros`, so that the macro of layouts reads files when a
program is built (ADR-0225).

## What it must not do

- Depend on any crate, of the engine or not: it moves with uwui (M6d.11).
- Recurse on its input, or hold more than its limits allow.
- Know what uwui's documents mean.

## Tests

- `tests/suite.rs` runs KDL's official tests, kept in `tests/kdl-suite` with their licence.
- `tests/reader.rs` checks a document's contents, its errors' places and its limits.
- `tests/hostile.rs` gives the reader damaged documents, through the engine's shared helper of
  damaged files ([ADR-0102](../../docs/adr/0102-a-damaged-file-gives-an-error.md)), and random
  input from a seed printed on failure, which `UWUI_KDL_SEED` replays; the helper moves into the
  crate with uwui.
