# 0229. Styles typed as declarations, a rule's kind checked

- Status: Accepted; amends [0141](0141-style-sheets-and-themes.md) (a rule's part is a list of typed
  declarations; a rule sets a colour whole; a rule that names a kind and sets what that kind does
  not read is refused), [0220](0220-the-roadmap-reshaped-by-the-market.md) (M6d.2 cut in two; the
  part of a style that a theme adds moves to uwui's first stage) and
  [0222](0222-what-a-theme-is.md) (rules are typed for each component at M6d.2, for each of its
  parts at uwui's first stage); amended by [0230](0230-the-cores-roles-and-each-kinds-defaults.md)
  (M6d.2b cut in two; the payload and the derive's list of colours move to its second half), then
  by [0231](0231-an-elements-own-style-written-in-code.md) (an array is one leaf for each element;
  the own style is declarations, not a `Style`), then by
  [0232](0232-transitions-read-the-derives-colours-and-share-what-they-show.md) (the derive lists a
  style's colours by a mark on each field; lengths come with the user's layer)
- Date: 2026-11-10

## Context

uwui's styles cascade through the engine's reflection. A rule's `style` is read by
`slime-serialize` as a reflected `Value`, a part of `UiStyle`. Each time an element is styled, the
default style's `Value` takes each selected rule's part (`Registry::overlay`), then the element's
own style's difference from the default, and is turned back into a `Style`. One frame that styles
again `ui.tree.look`'s 12 487 elements under the editor's look makes 422 651 allocations of
38.1 MB, and reading the look's documents 112 705 more
([`measurements.md`](../measurements.md#m6d2a-the-typed-cascade)). uwui cannot leave the engine with
`slime-reflect` ([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)).

M6d.2's card asked for styles typed per component, each part of a style a type of its own with
every field optional, merged by the cascade; parts of a style that a theme adds; the themes' values
resolved by type; no colour hidden in a component's defaults; and styles written in code as
[ADR-0223](0223-styles-written-in-code-through-tokens.md) decides. A first draft was attacked twice
before it reached the owner. Its property test could not pass: per-kind defaults from tokens,
typed theme values and colours set whole are changes of behaviour that the old cascade cannot be an
oracle for. And an `Option<T>` field holds neither a token nor where its value came from, which
ADR-0223's user's layer and [ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)'s
forced colours need.

The references read on 2026-11-10: GPUI derives a refinement, every field `Option<T>`, merged by
`refine`, its states boxed refinements over the base. Floem keeps a style as a map of typed
properties, declared by a macro anywhere. Iced gives each widget a whole `Style` from a `Catalog`,
with no merge. WPF and WinUI set dependency properties by setters, and attached properties let a
third party add one to a control. CSS keeps declared values apart from computed ones. The owner
decided on 2026-11-10, one question at a time.

## Decision

**M6d.2 is cut in two.**

- M6d.2a, the typed cascade: it behaves as today, but for the two changes of format below, checked
  by a property test against the reflected cascade.
- M6d.2b, tokens, defaults and styles in code: the core's roles and their fallbacks, tokens,
  themes' values typed, each kind's defaults from its theme, styles written in code with ADR-0223's
  order and the user's layer. It is likely cut again at its start.

**A rule's part is a list of typed declarations.** A derive, in a new proc-macro crate of uwui's
whose name M6d.10 settles, reads `Style` and its nested types and generates, for each, an enum of
declarations, nested as `Style` is: `Layout(Padding(Top(12.0)))`, `Paint(Background(colour))`.

- A leaf is what reflection sets whole today: a number, an enum, an array, a list, an option, and
  a colour (below). A nested struct is a level.
- A rule, and an element's own style, hold only what they set, in order.
- The cascade applies the selected rules' declarations, then the element's own, to a `Style`, a
  `match` for each. Everything that reads `Style` is unchanged: layout, paint, transitions,
  controls.
- A declaration's payload is a type that the derive declares. At M6d.2b it holds, at each colour
  and length, whether the value is a literal, a token or a theme's name, and the state that set
  it, for ADR-0223's user's layer, ADR-0226's forced colours and
  [ADR-0227](0227-the-time-that-motion-reads.md)'s two clocks.
- At M6d.2b, the derive also says which leaves are colours and lengths: transitions, today a list
  kept by hand (`uwui::tree::motion`), the user's layer and forced colours then read that one
  list.
- Until M6d.3, a part is read as reflection reads it today, then turned into declarations.

**A rule sets a colour whole.** A colour is one leaf. A part that sets one of its channels alone,
`background = { r = 0.5 }`, is refused, with an error that says so. No document of the repository
does it.

**A rule is typed for its kind.** A table says which parts of a style each kind of element reads.
A rule that names a kind and sets a leaf that the kind does not read is refused when the interface
reads its sheets, as an error of type is ([ADR-0141](0141-style-sheets-and-themes.md)). A rule that
names no kind may set anything: its elements are of every kind. A test checks the table: changing a
leaf changes a kind's drawing exactly when the table says that the kind reads it. The look's rules
that set what their kind does not read are trimmed, eleven: among them, `[scroll]` and `[list]` set
`fill`, which a scroll bar does not read, and `[split]` sets `radius` and `mark`. Typing a rule for each of a
component's parts ([ADR-0222](0222-what-a-theme-is.md)) comes with the parts, at uwui's first
stage.

**The part of a style that a theme adds waits for uwui's first stage,** where Glass or Material 3
Expressive reads one, attached to the look that each root is given (ADR-0226). It adds without a
redesign: a declaration of its own kind, which the readers of sheets accept once the theme names
its type, whose colours and lengths the derive lists like the core's.

## Alternatives considered

The part's type:

- **A refinement**, a struct with every field `Option<T>`, as GPUI's: the simplest to read. But a
  rule and an element's own style each carry a whole `Style`, 640 bytes, nearly empty; an option
  cannot say "empty" without `Option<Option<T>>`; and each field would need its origin at M6d.2b.
- **A map of typed properties**, as Floem's: the most open. But each read becomes a lookup, and
  every reader of styles is written again.
- **Two types by hand, or by `macro_rules!`**: what ADR-0141 rejected, two types kept alike, and
  no list of colours and lengths.

Colours:

- **Channels kept**, merged one by one: no document can break. But a colour becomes four
  declarations, which transitions and forced colours put back together, and code and sheets set it
  two ways.

A rule's kind:

- **A warning in a debug build**, recorded as debt: no document that read before is refused. But
  it does not type rules, and a warning is lost.
- **Nothing in M6d.2a**: a shorter milestone, and the table built later anyway.

The cut:

- **Three milestones now**, the tokens and the styles in code apart: each of a milestone's size,
  but cut before M6d.2a teaches anything.
- **One milestone**: one review, but a long session before its first check.

The theme's part:

- **Built at M6d.2a**, read by a test alone: the card as written. But an abstraction with no
  reader, touching the sheets' reader, the detection of changes, transitions and the test of
  colours, that the first stage would attach again to the look of each root.

## Consequences

- M6d.2a's property test compares the reflected cascade and the typed one, floats by their bits,
  over random rules and own styles of every leaf; the styles of the editor's and the `widgets`
  example's trees under the look are kept as fixtures, which outlive the oracle when reflection
  leaves at M6d.3.
- The counters stay the same, but for the allocations and bytes of a frame that styles again,
  which fall and are recorded.
- A document outside the repository that sets a colour's channel alone, or a leaf that its kind
  does not read, is refused, by the rule and the field at fault.
- M6d.3 reads declarations with `serde`: the derive generates their `Deserialize`, refusing unknown
  fields as reflection does.
- uwui's first stage builds the part of a style that a theme adds, and types rules for each of a
  component's parts.
- `slime-reflect` stays in uwui until M6d.3, for reading documents alone.
