# slime-reflect-derive

The derive macro of the engine's reflection, `#[derive(Reflect)]`
(`docs/adr/0080-reflection-and-serialization.md`), which `slime-reflect` re-exports: a crate
derives through `slime_reflect::Reflect`, and depends on `slime-reflect` only.

## Role

It reads a struct or an enum, its `#[reflect(...)]` attributes and its doc comments, and writes
the type's implementation of `slime_reflect::Reflect`: its stable name, its description, the
registration of its fields' types, and its conversions to and from `slime_reflect::Value`. The
code it writes names `::slime_reflect`, and calls `slime_reflect`'s hidden `support` module, so
that it stays short. The attributes it takes, and what it refuses, are documented with the trait,
in `slime-reflect`, where its misuses are tested as doc tests that fail to compile.

## Entry points

- `Reflect`, the derive.

## Dependencies

`proc-macro2`, `quote` and `syn` (its default features), with which derive macros are written,
and which the workspace builds already, for `thiserror`'s derive.

## Must not

- Be used without `slime-reflect`: the code it writes calls it.
- Describe a generic type, a type with lifetimes or a union: those are described by hand.
