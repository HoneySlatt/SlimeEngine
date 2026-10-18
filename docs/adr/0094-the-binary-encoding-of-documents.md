# 0094. The binary encoding of documents

- Status: Accepted
- Date: 2026-10-18

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides what a cooked scene file is: the engine's own binary
encoding of the document that its text holds, without its types. That encoding is self-describing
and versioned, gives the same bytes on every platform, is read within limits without a panic, and
is read through the same reader as text, strict and lenient alike. It lives in `slime-serialize`,
beside the text, and saves stay text. [ADR-0090](0090-stage-5b-scenes-and-prefabs.md) gives it to
M5b.4, before the cooker uses it in M5b.5.

Where `slime-serialize` stood ([ADR-0086](0086-serialization.md),
[ADR-0087](0087-the-worlds-documents.md)):

- A document was text only. The parser turned it into a tree of tables, arrays, texts, integers,
  floats and booleans; a reader then read each value from that tree, through its type's
  description, upgrades and partial values included.
- An error named the line and the column of the part at fault, which the parser's document finds
  again when an error needs it.
- A document was at most `MAX_SIZE` bytes and nested at most `MAX_DEPTH` levels deep. The writer
  refused what the reader would refuse.

## Decision

**What is encoded: the document's tree, without its types.** The binary encoding holds what parsing
a text gives: tables, arrays, texts, integers, floats and booleans. `$format` and `$versions` are
keys like any other, and are read as a text's are. A reader reads the tree that the bytes give
exactly as it reads the tree that a text gives. The same document read from either gives the same
values, upgrades, partial values, warnings and strictness included. A program that knows no type,
such as the cooker, encodes a text it cannot read the values of.

**How the bytes are laid out.** The module `format` of `slime-serialize` is the home of the layout:

- the mark `SLDB`, then the version of the encoding, one byte;
- a table of strings, each key and each text once, in the order in which a walk of the document
  first meets them;
- the root table, each value a tag of one byte and what it holds;
- integers zigzagged as varints, unsigned LEB128;
- a float in the four bytes of an `f32` when it is one exactly, such as 0.5 or 140, in the eight of
  an `f64` otherwise, NaN with its sign only, as a text keeps it;
- numbers in little-endian order.

The table of strings holds the names of components and fields once, which every entity repeats: the
measurements of `Document::parse_binary`'s rustdoc show what it saves.

**One writer: from a text.** `slime_serialize::encode(text)` encodes the document that a text holds,
without its types, as a cooker can. A document of values that a `Composer` writes is encoded from
its text, so that the same document has one encoding, whose floats read as the text's, at any type.
It refuses an encoding larger than `MAX_SIZE`, which a reader refuses: what is written can be read.

**Who reads it.** `Document::parse_binary(registry, bytes, mode)` gives the `Document` that
`Document::parse` gives for a text: the same keys, tables, values and errors, read through the same
reader.

**Errors name the byte and the place.**

- Bytes that are not the encoding are refused at their byte and with the path of the part being
  read (`ReadProblem::Binary`): cut short; a tag, a varint, a count or a string index that the
  encoding does not have; a string that is not UTF-8, or that comes twice; a key given twice in a
  table; bytes past the root table. A varint longer than it needs, and a string that nothing uses,
  are read: no writer writes them, and they change no value.
- A later version of the encoding is refused (`ReadProblem::NewerEncoding`).
- An error in a value is placed as a text's is, at its key or at its value, but by its byte, from 0
  (`ReadError::byte`), where a text's error gives a line and a column. The reader finds that byte
  when an error needs it, walking the bytes again along the error's path: nothing is kept for
  errors that do not happen, and a document read leniently walks them once for each warning.

**Within limits, never a panic.** What bytes can make a reader hold is bounded as for a text:

- a document is at most `MAX_SIZE` bytes, and nests at most `MAX_DEPTH` levels deep;
- its keys and texts, each counted where it is used, hold at most `MAX_SIZE` bytes, as a text's do,
  so that a short string named many times is not copied without end;
- a count is at most the bytes left, as each thing that it counts takes one at least, and reserves
  at most a thousand places ahead, the rest grown as it is read, so that counts nested within a few
  bytes reserve little.

**What it costs.** Decoding is several times faster than parsing a text; reading the values then
costs the same from either. The encoding holds a fraction of the text's bytes.
`Document::parse_binary`'s rustdoc holds the measurements ([ADR-0053](0053-one-home-per-fact.md)).

## Alternatives considered

- **Keys written in full at each table**: no table of strings to read first, but the names of
  components and fields would take most of a scene's bytes.
- **Every float in eight bytes**: one tag fewer, but twice the bytes for the floats of 32 bits
  that most of a game's values are.
- **Floats as their shortest digits**: the fewest bytes, but reading them back is parsing text,
  which the encoding is there to avoid.
- **The place of every value kept as it is decoded**, for errors: an error placed without walking
  the bytes again, but memory for each value of every document read, for errors that are rare.
- **A larger limit of size for the encoding than for a text**: denser levels in one file, but the
  text that the cooker encodes is limited all the same, and two limits would say two things.
- **A writer of the encoding from values**, beside the one from a text: no text printed and parsed
  again, but two encodings of one document, the floats of 32 bits that a text writes in their
  shortest digits exact in one and as those digits read in the other, which a value read at a
  wider type would tell apart.
- **A reader that refuses all but the encoding that the writer gives**: one form for one document
  checked, but at a cost to every reading, for documents that no writer makes.
- **CBOR or MessagePack, through a crate**: rejected by ADR-0089. They would also leave the
  guarantees of a document, such as a key once per table, to code of the engine all the same.
- **An encoding of typed values**: rejected by ADR-0089, as the cooker would need the game's
  types.

## Consequences

- M5b.5 cooks scene files into this encoding, and a world reads them through
  `Document::parse_binary`, through the code that reads their text.
- Saves stay text, until a measurement asks; the cooked glTF scene keeps its own format, as
  ADR-0089 says.
- A change of the layout raises the version of the encoding, which a reader of an earlier version
  refuses: cooked data is cooked again from its sources.
