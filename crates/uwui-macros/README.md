# uwui-macros

uwui's derive macros. `Declare` makes the typed *declarations* of uwui's styles: for a struct of a
style, an enum with one variant per field, an array's by element, which is what a rule of a style
sheet, or an element's own style, sets of it
([ADR-0229](../../docs/adr/0229-styles-typed-as-declarations.md),
[ADR-0231](../../docs/adr/0231-an-elements-own-style-written-in-code.md)), and the code that reads
them from a style sheet's nodes of KDL. The cascade applies a rule's declarations in order to a
`Style`; no reflection runs when an element is styled or a sheet read. It numbers a style's leaves,
for the cascade's mask order, and says which lay out, which a hover or a press written in code may
not set. It lists a style's colors, which transitions move. `Identifiers` reads an enum's variant
from its name.

Its name is provisional: the names of uwui's crates are settled at M6d.10
([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

## Role

- `#[derive(Declare)]` on a struct of named fields; `#[declare(nested)]` on a field whose type is
  a struct that derives it too; `#[declare(lays_out)]` on a field, nested or not, that lays out
  what it styles; `#[declare(color)]` on a field, not nested, that holds a color, which a field
  whose type names `LinearColor` may not lack. The rustdoc of `derive_declare` lists what it
  generates and what it refuses.
- `#[declare(length)]`, `#[declare(duration)]` or `#[declare(number)]` on a field whose type names
  `f32`, which it may not lack: what its numbers measure, as a style sheet writes them.
- `#[derive(Identifiers)]` on an enum of unit variants, which a style sheet names by an
  identifier, `direction=Column`.
- The generated code names `crate::style::Same`, `crate::style::Colors`, `crate::theme::sheet`,
  `uwui_kdl`, `slime_color::LinearColor` and `slime_reflect::Value`: it serves uwui's own styles
  alone, and its items are `pub(crate)`. It reads style sheets in KDL
  ([ADR-0241](../../docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)).

## Dependencies

`proc-macro2`, `quote` and `syn`, which `slime-reflect-derive` already takes
([`docs/dependencies.md`](../../docs/dependencies.md)).

## What it must not do

- Depend on any crate of the engine: it moves with uwui (M6d.11).
- Read a document's text itself: `uwui-kdl` reads it, and the generated code reads its nodes.
  `S::named`, which reads a part read by reflection, serves uwui's tests alone, and goes with
  reflection (M6d.3c).
