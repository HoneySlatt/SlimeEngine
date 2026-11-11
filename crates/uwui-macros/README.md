# uwui-macros

uwui's derive macros. Today it holds one, `Declare`, which makes the typed *declarations* of uwui's
styles: for a struct of a style, an enum with one variant per field, an array's by element, which
is what a rule of a style sheet, or an element's own style, sets of it
([ADR-0229](../../docs/adr/0229-styles-typed-as-declarations.md),
[ADR-0231](../../docs/adr/0231-an-elements-own-style-written-in-code.md)). The cascade applies a
rule's declarations in order to a `Style`; no reflection runs when an element is styled. It numbers
a style's leaves, for the cascade's mask order, and says which lay out, which a hover or a press
written in code may not set. It lists a style's colors, which transitions move.

Its name is provisional: the names of uwui's crates are settled at M6d.10
([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)).

## Role

- `#[derive(Declare)]` on a struct of named fields; `#[declare(nested)]` on a field whose type is
  a struct that derives it too; `#[declare(lays_out)]` on a field, nested or not, that lays out
  what it styles; `#[declare(color)]` on a field, not nested, that holds a color, which a field
  whose type names `LinearColor` may not lack. The rustdoc of `derive_declare` lists what it
  generates and what it refuses.
- The generated code names `crate::style::Same`, `crate::style::Colors`,
  `slime_color::LinearColor` and `slime_reflect::Value`: it serves uwui's own styles alone, and its
  items are `pub(crate)`.

## Dependencies

`proc-macro2`, `quote` and `syn`, which `slime-reflect-derive` already takes
([`docs/dependencies.md`](../../docs/dependencies.md)).

## What it must not do

- Depend on any crate of the engine: it moves with uwui (M6d.11).
- Generate code that reads documents beyond `S::named`, which goes when uwui reads its documents
  with `serde` (M6d.3); `serde`'s `Deserialize` for the declarations is generated then.
