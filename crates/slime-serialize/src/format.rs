//! How a document holds values: the format that [`write`](crate::write) writes and
//! [`read`](crate::read) reads, TOML 1.1 (`docs/adr/0086-serialization.md`).
//!
//! A document holds one value of a registered type, whose description leads both ways: the same
//! text is a value of one type or another as the type read says.
//!
//! - **The root.** A value written as a table is the document's root table; any other is the root's
//!   `$value`. Keys that start with `$` are the format's: no field or variant may be named so, nor
//!   have such an alias.
//! - **Versions.** A type past version 1 has its version under `$versions`, a table at the top of
//!   the document, by the type's name, written once whatever how many of its values the document
//!   holds: `"$versions" = { "hunnawood::Door" = 2 }`. A type that the table leaves out is at
//!   version 1, so that a document of types that never changed holds no versions. A value of an
//!   earlier version is brought to its type's version by the type's upgrade
//!   ([`slime_reflect::Upgrade`]), as this document holds it, before the type reads it; a later
//!   version is refused. `$format` gives the version of the format itself, [`VERSION`] if left out:
//!   a later one is refused.
//! - **Structs** are tables of their fields, in the order of their declarations, by their names; a
//!   reader takes their aliases too. A tuple struct is an array of its fields, and a tuple struct
//!   of one field that field's value: `Name("door")` is `"door"`. A unit struct is an empty table,
//!   `{}`.
//! - **Absent values.** A field whose option holds none is left out, and an option field that a
//!   document leaves out holds none, whatever its default says; one that holds a value is that
//!   value. An option elsewhere, in an array or in another option, is an empty array, `[]`, or an
//!   array of its value, `[1.5]`.
//! - **Enums.** A variant without fields is its name, `"Box"`. A variant with fields is a table of
//!   one key, its name, whose value its fields make as a struct's would: `{ Spot = { ... } }`, or
//!   under a header, `[light.Spot]`.
//! - **Numbers.** Integers are TOML's, but for unsigned ones past `i64::MAX`, which are text:
//!   `"18446744073709551615"`. Floats come back to the same bits: the shortest digits that read
//!   back as the number, of 32 bits for an `f32` unless TOML's parser, which reads 64 bits, would
//!   then round them to another `f32`, those of 64 bits then; and `inf`, `-inf`, `nan` and `-nan`.
//!   A NaN's payload is not kept. A reader takes an integer where a float goes.
//! - **References** to assets are their IDs as text, `"850ad551-5098-42f9-ae42-26ed6174951a"`.
//!   References to entities are the names that the document gives them, as text: a world's
//!   persistent identities, `"zone/door"`, in its saves ([`References`](crate::References)). An
//!   entity's handle holds within one run only: a document that names no entity neither writes nor
//!   reads a reference to one.
//! - **Layout.** A table whose values are all numbers, booleans and text is written on its key's
//!   line, `{ x = 1.0, y = 0.0 }`, and so is an array, unless it holds tables of which one at least
//!   is larger. A larger table is written under a header of its own, `[a.b]`, and an array of
//!   tables of which one is larger under `[[a.b]]`, once each table's values on their keys' lines
//!   are written; a table that holds nothing but tables under headers has no header of its own.
//!   Keys are bare where TOML lets them be, quoted otherwise; text is a basic string, `"..."`, with
//!   TOML's escapes. Lines end with LF.
//!
//! A writer refuses what a reader would refuse, so that what it writes can be read back. A source,
//! which people write, is read strictly: a field that its type does not have is an error. A save
//! is read leniently: such a field is left out, with a warning ([`Mode`](crate::Mode)). A field
//! that a document leaves out takes its default, if its struct, itself or its type declares one
//! ([`slime_reflect::TypeInfo::default`]).
//!
//! A part of a value, such as a save's differences, leaves out the fields of its structs and
//! variants that did not change, at any depth ([`write_partial`](crate::write_partial),
//! [`read_partial`](crate::read_partial)); its tuples, lists and options are whole. Its option
//! fields are written as options elsewhere are, so that it can say that one was emptied:
//! `range = []` for an option that now holds none, `range = [5.0]` for one that holds 5.

/// The version of the format that this program writes, and the latest it reads.
pub const VERSION: u32 = 1;
