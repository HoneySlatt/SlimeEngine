# 0241. Every document of uwui in KDL, read by a reader of its own

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (uwui's documents are KDL, read by uwui's
  own reader: neither `serde` nor `toml` reads them, and no feature holds the reader),
  [0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md) (every document of uwui
  is KDL, not layouts and parts alone; the reader is built at M6d.3a, in a crate of its own; the
  cooked form is the text), [0229](0229-styles-typed-as-declarations.md) (a rule's declarations
  are read by code that the derive generates from KDL's nodes, not by `serde`),
  [0238](0238-tokens-typed-named-in-code-declared-by-each-crate.md) (a theme writes a value's kind
  by its group, and a text field names a theme's value) and
  [0220](0220-the-roadmap-reshaped-by-the-market.md) (M6d.3 cut in three; the primary modifier is
  named `Primary`)
- Date: 2026-11-11

## Context

M6d.3's card has uwui read and write its themes, style sheets, shortcuts and docks' layouts with
`serde` and `toml`, behind a feature, as [ADR-0181](0181-uwuis-scope.md) decided, so that
`slime-reflect` and `slime-serialize` leave uwui. It leaves to M6d.3's start the cooked form of
these documents, whether a theme's values and rules are written in KDL as its parts are
([ADR-0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)), a field of text
named by a theme and how a theme's file writes a token's kind and unit
([ADR-0238](0238-tokens-typed-named-in-code-declared-by-each-crate.md)), `slime-color`'s home with
its reflection, and a primary modifier, Ctrl or ⌘, in the format of shortcuts
([ADR-0220](0220-the-roadmap-reshaped-by-the-market.md)).

The facts, read in the repository and on the web on 2026-11-11:

- uwui reads its documents through reflection: each value of a theme is read in the type of the
  field that names it, the theme parsed again for each. Reading the editor's look allocates
  127 736 times ([measurements](../measurements.md#m6d2b1-the-roles-and-each-kinds-defaults)).
- The only documents that users saved are the editor's docks' layouts, in its state and in a
  project's `state/editor.layout`. The editor saves no table of shortcuts, only which of its two
  it uses. Only the `interface` example reads cooked documents.
- `slime-color` depends on `slime-reflect` for one derive on `LinearColor`. Scenes and the dev
  overlay read colours through it.
- The `toml` crate 1.2.1 keeps a table's order only with its `preserve_order` feature; TOML's
  specification promises no order of a table's pairs, and the rules of a sheet apply in their
  order. KDL orders its nodes; it allows a property given twice.
- The crate `kdl` 6.7.1 still reads without a bound on depth (kdl-rs issue 203, opened
  2026-11-11).
- KDL's official tests, 581 files of 10.5 kB, are under CC BY-SA 4.0. The branch `main` has moved
  past the tag 2.0.0: 32 cases and fixes of the grammar since, the last on 2026-08-10 (commit
  `f490278`).
- `cargo-fuzz` needs a nightly compiler and does not run on Windows. `bolero` runs random inputs in
  a plain `cargo test` on stable, and the same target later under libFuzzer.
- GTK names the primary modifier `<Primary>`, Zed `secondary`, Electron `CommandOrControl`, egui
  and iced `COMMAND`; Qt and Slint read `Ctrl` as ⌘ on macOS and `Meta` as its Control. Apple
  lists modifiers in the order Control, Option, Shift, Command.

Each recommendation was attacked before it reached the owner. The first cut, the format unchanged
then the notation changed, held almost all of the milestone in its first half, and left after the
reader the notation that shapes it. KDL for every document was found the better destination, but
at about 3,300 lines, not 1,500: a writer, a converter, a migration and the derive's readers
besides the reader; and it reverses ADR-0225's "the other documents stay TOML", decided two days
before. The attack also found that the reader must bound memory, not only depth; that a chain of
names in a theme recurses past any limit of depth; that ⌘C in a text field would fall through to
the program's copy once tables name the primary modifier, since a field reads Control; and that a
macro reading layouts at build time cannot reach a reader inside `uwui`. The owner decided on
2026-11-11, one question at a time.

## Decision

**M6d.3 is cut in three,** the reader first:

- M6d.3a: uwui's reader of KDL, alone, checked against KDL's official tests;
- M6d.3b: themes and style sheets in KDL, read by code that `uwui-macros`'s derive generates, the
  repository's documents converted and checked against today's reflected reader;
- M6d.3c: shortcuts and docks' layouts in KDL, the editor's saved layouts migrated, the cooked
  form, reflection out of uwui and `slime-color`, the primary modifier and its display.

**Every document of uwui is KDL 2.0:** themes' values, style sheets' rules, shortcuts, docks'
layouts and settings, as layouts and a theme's parts already were. Neither `serde` nor `toml`
reads them; no feature is needed, since the reader has no dependency. `serde` still enters uwui at
its first stage, for kept signals ([ADR-0210](0210-signals-named-for-a-reload.md)). The engine's
scenes and saves stay TOML.

**The reader is a crate of uwui's own,** below `uwui` and `uwui-macros`, so that the macro of
layouts reaches it when a program is built; its name is settled at M6d.10.

- It reads a document into a flat tree: nodes and entries in arrays, a node's children a range of
  them, each with its span in the text. A string is borrowed from the text where it holds no
  escape. Entries keep their order and their duplicates: the typed layer above refuses those it
  must.
- Its limits, which the caller may lower: the input's size, a depth of 64, a count of nodes and
  entries. Time grows with the input alone. Its memory for each byte read is measured.
- It stops at the first error of syntax, with its line, its column and what is wrong. The typed
  layer gathers its own errors, up to a cap.
- One writer prints a tree: in the normal form of KDL's tests, and as the documents that a program
  saves. At uwui's first stage, an edit changes the text at a span and keeps the rest, comments and
  formatting included, as ADR-0225 asks.

**KDL's official tests are kept in the repository,** taken once from `main` at `f490278`, in a
folder of their own with their licence, CC BY-SA 4.0, their authors credited, out of uwui's
published package. Where they differ from KDL's specification, the specification wins.

**The reader is tested against hostile input on stable:** a target of `bolero`, a dependency of
the tests, runs random inputs in `cargo test` at each push, on Linux and Windows, a failure
replayed by its seed; beside it, the test of damaged files that every reader gets
([ADR-0102](0102-a-damaged-file-gives-an-error.md)), and a round trip, read, written and read
again to the same tree. Fuzzing guided by coverage, libFuzzer on a nightly compiler on the same
target, is debt, built in uwui's CI at its first stage.

**Types map onto KDL by one set of rules,** which the derive follows:

- A document is a list of nodes. A sheet's rule is a node named by the rule; its children are
  `select` and the style's groups, `layout`, `paint`, `text`, `control`, `transition` and `icons`.
- In a struct, a field that holds a value is a property, `border_width=1`; a nested struct is a
  child node named by its field, `padding start=4 end=4`; an array or a list of values is a child
  node whose arguments are its elements, `radii 4 4 4 4`; a list of structs is one child node for
  each element.
- An enum's unit variant is an identifier, `direction=Column`; a variant that holds a struct is a
  child node named by the variant, `Split side_by_side=#true { … }`.
- An annotation is a unit, `(ms)120`, `(px)12`; an enum whose variants hold a number in a unit
  takes the variant by its unit. A length without a unit is in logical pixels. A duration always
  writes its unit, `(ms)` or `(s)`, in a sheet as in a theme.
- An option left out is left as it is; `#null` empties it. An option of a struct is set whole, as
  today: `shadow color=$shadow` resets its offset, blur and spread.
- A colour is sRGB's `"#rrggbb"` or `"#rrggbbaa"`, a value's name, or a function,
  `"mix($a, $b, 20%)"`. The form by linear channels goes; colours outside sRGB come with uwui's
  first stage, as functions.
- `$name` names a theme's value wherever a value goes, in a field of text too; a text that starts
  with `$` writes `$$`.
- A name, a rule, a scope, a group or a field given twice where it means once is refused.
- A document that a program saves, as a dock's layout, writes its version, and is read leniently:
  what it does not know is left out, with a warning.

**A theme writes each value's kind by its group,** as the design tokens of the W3C's community
group give a group its type: `color { accent "#4C8DFF" }`, `length { space_m 8 }`,
`duration { fast (ms)120 }`, `number { weight_control 500 }`, `text { tick "\u{E06C}" }`. A name is
given once across the groups. The derive marks each number of a style as a length, a duration or a
number, so that a rule naming a value of another kind is refused. The look's icons move from its
sheet to its theme. Code gets no token of text until something reads one.

**The cooked form is the text, checked.** The cooker reads each document with uwui's reader,
checks its syntax, its limits and what it can of its types, and writes it unchanged: an error in a
cooked document keeps its line. A document's file takes two extensions, `dark.theme.kdl`,
`editor.style.kdl`, `editor.keys.kdl`, `editor.layout.kdl`, so that editors read it as KDL and the
cooker knows its kind; its line endings are fixed in `.gitattributes`. This departs from the
roadmap's "binary formats of the engine's own for cooked data" for uwui's documents.

**`slime-reflect` describes `LinearColor`,** by hand, as it describes glam's types; `slime-color`
depends on nothing. The name `LinearColor` and its fields `r`, `g`, `b` and `a` stay, so that a
scene as text, a scene cooked and the dev overlay's values read the same, kept as fixtures.

**A chord names the primary modifier `Primary`,** `Primary+Z`: Ctrl on Linux and Windows, ⌘ on
macOS; `Ctrl` means the Control key everywhere. The repository's tables are converted, `Ctrl`
written `Primary`. A text field's own keys, copy, paste, undo and the others, take `Primary` too.
The platform is a value, from the target when the program is built, which a host (M6d.5) and tests
may set; two chords are the same when they resolve to the same keys on it. A menu shows a shortcut
as its platform writes it: `⇧⌘Z` on macOS, `Ctrl+Shift+Z` on Linux and Windows.

## Alternatives considered

The cut:

- **Two milestones, the format unchanged then the notation changed:** the first checked against
  the reflected reader. But it held almost all of M6d.3, and the notation that shapes the reader
  came after it.
- **Two milestones, the reader with the themes and sheets:** one review fewer. But some 2,500 lines
  before the first check.

The notation:

- **TOML for good,** read by a reader that the derive generates over `toml_edit`, the rules as
  `[[rule]]` so that the specification keeps their order: no file changes, a parser of a billion
  downloads. But two notations and two readers in uwui for good, and no annotation for a value's
  kind.
- **TOML now, KDL at uwui's first stage:** two readers and two rewrites.

The tests:

- **The tag 2.0.0:** the version released. But 32 cases fewer, and the grammar's fixes since.
- **No copy of the tests:** no file under CC BY-SA in the repository. But a conformance that CI does
  not prove.
- **Random inputs of the repository's own generator:** no dependency. But a target written again
  for a fuzzer.
- **libFuzzer now, weekly:** fuzzing guided by coverage at once. But a nightly compiler in the
  flake and a job of CI, files of another track, and Windows left out.

The reader:

- **Inside `uwui`:** one crate fewer. But out of the macro's reach.
- **A lossless tree of every token,** as `toml_edit` or `rowan`: an edit is direct. But two layers
  to keep alike, and more memory and allocations when it reads.

A value's kind:

- **An annotation on each value,** `(px)8`: a theme ordered by subject. But an annotation on every
  length, and a colour guessed from its value, which the W3C's format forbids.
- **No kind written,** as today: the shortest. But a value of the wrong kind found only where it is
  used, and `120` read as two minutes.

The cooked form:

- **The text with today's extensions:** nothing renamed. But no editor reads them as KDL.
- **A binary of uwui's own:** no text to parse. But a second reader, and errors without their line.

`slime-color`'s reflection:

- **A feature of `slime-color`:** the derive kept. But a crate of uwui's repository depending on the
  engine.
- **A type of the engine's wrapping it:** two types of colour, against
  [ADR-0045](0045-one-linear-color-type.md), and conversions wherever the engine speaks to uwui.

The primary modifier:

- **`Ctrl` read as ⌘ on macOS,** `Meta` its Control, as Qt and Slint: no table changes. But `Ctrl`
  lies on a Mac, and `Meta` means Super on Linux.
- **`CmdOrCtrl`,** as Electron: as explicit, longer.

## Consequences

- M6d.3a builds the reader, its writer of the normal form, the tests kept with their licence, the
  target of `bolero`, and `.gitattributes`' rule for `*.kdl`. `bolero` is put in
  [`dependencies.md`](../dependencies.md).
- M6d.3b converts every theme and sheet of the repository, and the 78 documents written in tests,
  benches and examples, checked against the reflected reader while it exists; it resolves a
  theme's names once for each stack, without recursion, a chain bounded.
- M6d.3c migrates the docks' layouts that the editor saved, read a last time through the engine's
  reflection; converts the tables of shortcuts; removes `from_binary` and the cooked binary; moves
  `LinearColor`'s reflection; and leaves `slime-reflect` and `slime-serialize` out of
  `cargo tree -p uwui`.
- The checks of the shortcuts' display on macOS go to the owner's list: CI runs Linux and Windows.
- uwui's first stage builds the edits at a span, fuzzing guided by coverage, colours outside sRGB,
  and a token of text when a reader needs one.
- A program on the web ships the reader whenever it loads a document; its weight is measured
  against [the budget of 1 MB](0181-uwuis-scope.md) at M6d.3b.
