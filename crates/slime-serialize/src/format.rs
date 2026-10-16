//! How a document holds values: the format that [`write`](crate::write) writes and
//! [`read`](crate::read) reads, TOML 1.1 (`docs/adr/0086-serialization.md`).
//!
//! A document holds one value of a registered type, whose description leads both ways: the same
//! text is a value of one type or another as the type read says.
//!
//! - **The root.** A value written as a table is the document's root table; any other is the root's
//!   `$value`.
//! - **Versions.** A type past version 1 has its version under `$versions`, a table at the top of
//!   the document, by the type's name, written once whatever how many of its values the document
//!   holds: `"$versions" = { "hunnawood::Door" = 2 }`. A type that the table leaves out is at
//!   version 1, so that a document of types that never changed holds no versions. A value of an
//!   earlier version is brought to its type's version by the type's upgrade
//!   ([`slime_reflect::Upgrade`]), as this document holds it, before the type reads it; a later
//!   version is refused. `$format` gives the version of the format itself, [`VERSION`] if left out:
//!   a later one is refused.
//! - **Structs** are tables of their fields, in the order of their declarations, by their names; a
//!   reader takes their aliases too. A tuple struct is an array of its fields, and a tuple struct of
//!   one field that field's value: `Name("door")` is `"door"`. A unit struct is an empty table,
//!   `{}`.
//! - **Absent values.** A field whose option holds none is left out; one that holds a value is
//!   that value. An option elsewhere, in an array or in another option, is an empty array, `[]`,
//!   or an array of its value, `[1.5]`.
//! - **Enums.** A variant without fields is its name, `"Box"`. A variant with fields is a table of
//!   one key, its name, whose value its fields make as a struct's would: `{ Spot = { ... } }`, or
//!   under a header, `[light.Spot]`.
//! - **Numbers.** Integers are TOML's, but for unsigned ones past `i64::MAX`, which are text:
//!   `"18446744073709551615"`. Floats come back to the same bits: the shortest digits that read
//!   back as the number, of 32 bits for an `f32`, then `inf`, `-inf`, `nan` and `-nan`; a NaN's
//!   payload is not kept. A reader takes an integer where a float goes.
//! - **References** to assets are their IDs as text, `"850ad551-5098-42f9-ae42-26ed6174951a"`.
//!   References to entities are neither written nor read: an entity's handle holds within one run
//!   only, and a world's documents will write the entity's persistent identity in its place.
//! - **Layout.** A table that holds nothing but numbers, booleans and text, and an array that holds
//!   no such tables as the next ones, are written on their key's line: `{ x = 1.0, y = 0.0 }`. A
//!   larger table is under a header of its own, `[a.b]`, and an array of tables, one of them larger,
//!   under `[[a.b]]`, after the values on their keys' lines. Keys are bare where TOML lets them be,
//!   quoted otherwise; text is a basic string, `"..."`, with TOML's escapes. Lines end with LF.
//!
//! A source, which people write, is read strictly: a field that its type does not have is an
//! error. A save is read leniently: such a field is left out, with a warning
//! ([`Mode`](crate::Mode)). A field that a document leaves out takes its default, if its struct,
//! itself or its type declares one ([`slime_reflect::TypeInfo::default`]); a partial value, such as
//! a save's differences, leaves it out ([`read_partial`](crate::read_partial)).

/// The version of the format that this program writes, and the latest it reads.
pub const VERSION: u32 = 1;
