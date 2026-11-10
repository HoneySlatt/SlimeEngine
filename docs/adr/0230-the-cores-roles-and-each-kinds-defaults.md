# 0230. The core's roles, and each kind's defaults from its look

- Status: Accepted; amends [0141](0141-style-sheets-and-themes.md) (an element's style starts from
  its kind's defaults, not from one default style; a theme that gives a role a value of another type
  is refused), [0223](0223-styles-written-in-code-through-tokens.md) (a token is resolved when the
  interface takes its themes and read when an element is styled; the core declares the roles that
  its components' defaults read) and [0229](0229-styles-typed-as-declarations.md) (M6d.2b cut in
  two; the derive's list of colours and a declaration's payload move to its second half)
- Date: 2026-11-10

## Context

[ADR-0229](0229-styles-typed-as-declarations.md) left to M6d.2b the core's roles and their
fallbacks, tokens, the themes' values typed, each kind's defaults from its theme, a declaration's
payload, styles written in code with the user's layer, and the call sites that move. That is about
twelve deliveries in two groups: what a theme gives the components, and what code writes.

Today every element's style starts from one default style, `Style::default()`, whatever its kind.
It holds colours of its own: a control's track grey, its fill blue and its mark white, a text white
and its selection blue at 40 % (`uwui::style`). A rule's `$name` is replaced by its theme's value
when the interface takes its sheets and themes; a value is typed only where a rule names it. The
colour picker draws a checkerboard of two greys and rings of black and white of its own. The
editor's look sets every colour that each kind draws, and its themes name some thirty colours of
its own palette. The hosts of the repository clear their windows to black; nothing in uwui paints
a root's background unless a rule does.

[ADR-0223](0223-styles-written-in-code-through-tokens.md) has the core declare the roles that every
theme gives, each with a fallback, and says that a token is resolved when the element is styled.
[ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) gives every root a
look, with no default, and lists the roles of forced colours.

The lead's first draft was attacked before it reached the owner. The attack found that fallbacks in
black on white would draw black text on the hosts' black, nothing painting a canvas. It found that
a field's selected text has no colour of its own, so a selection opaque in the text's colour would
hide it. It found that the derive's list of colours, and the token as a public type, depend on a
declaration's payload, which only code reads, and that a draft that resolved roles at two moments
amended ADR-0223 without saying so. The owner decided on 2026-11-10, one question at a time.

## Decision

**M6d.2b is cut in two,** the theme's side first, in the order of the decisions' dependencies:

- M6d.2b1, the roles and each kind's defaults, here;
- M6d.2b2, styles written in code: a declaration's payload (`Spec<T>`, and whether it records the
  state that set it), the token as a public type and an application's own tokens, the derive's
  list of colours and lengths with transitions reading it, the builder with its states,
  ADR-0223's order and the call's place, the user's layer, the debts of an own style, a field of
  text named by a theme, the call sites that move, and `subsecond`. Its questions are put at its
  start.

**The core declares seven roles,** the colours that its components' defaults read, named after
CSS's system colours where one exists. Each has a fallback, taken when the themes do not give it:

| Role | Fallback | What reads it by default |
|---|---|---|
| `canvas_text` | white | every text; a slider's and a scroll bar's thumb, the chevrons of a section, a menu's item and a list to choose from, a menu's tick, a tab's count's digits, a field's prefix and marks |
| `gray_text` | grey, 0.3 in linear light | a field's placeholder, a menu item's shortcut |
| `button_face` | grey, 0.3 in linear light | a check box unchecked, the tracks of a slider, a bar of progress and a scroll bar, a section's box, a tab's count's box, a split's handle |
| `accent` | blue, (0.2, 0.4, 0.9) in linear light | a check box checked, the part of a slider and of a bar of progress up to its value, a split's handle focused or dragged |
| `highlight` | `accent` | a field's selection, at 40 % of its opacity |
| `highlight_text` | `canvas_text` | a check box's tick |
| `focus` | `accent` | the ring of a part of the colour picker |

- The fallbacks are today's colours: an interface given no theme draws as it does today.
- A fallback serves a look that lacks a role. Every root is given a look (ADR-0226); the fallbacks
  are not a look.
- `canvas`, `button_text` and `link_text` come with their first reader: Plain and forced colours,
  at uwui's first stage. So does a colour of selected text, which forced colours need; until then,
  a field's selected text keeps the text's colour, over the selection drawn at 40 %.
- The roles stay the core's own until M6d.2b2 makes the token a public type.

**Each kind's defaults are written in Rust,** in one table with what each kind reads
(`docs/adr/0229-styles-typed-as-declarations.md`):

- a function gives each kind its base style from the roles resolved: the colours above, and
  `Style::default()` for everything else;
- they are the bottom of the cascade, under every rule;
- they set no background and no border: a component without a look draws no box, as today;
- `Style::default()` holds no colour but transparent; an element's own style still sets the fields
  whose bits differ from `Style::default()`'s;
- the styles made are shared by kind as well as by rules and own style;
- uwui's own layers that the look styles by their classes, the tooltip, the palette, the
  notifications and the colour picker, are containers and take no defaults beyond a container's:
  debt until they are written as parts, at uwui's first stage
  ([ADR-0222](0222-what-a-theme-is.md));
- the colour picker's checkerboard is its content, and its rings of black and white are markers
  seen on any colour: both stay literals, which the test of colours names.

**Roles and names are resolved once, when the interface takes its themes,** and read when an
element is styled. Each role is read once, as its type, into a table of resolved values; each
kind's base style is built from it; each rule's names are replaced by their values, as today. In
M6d.2b2, an own style's tokens read the same table. A change of themes styles every element again
and runs no view, as ADR-0223 asks; a role that the themes lack is found when they are taken.

**A role's value of another type refuses the themes.** `Ui::set_style` returns an error that names
the theme and the value, and the interface keeps the style it had, as for any document at fault
([ADR-0141](0141-style-sheets-and-themes.md)). A role that the themes lack takes its fallback, with
one warning for each role and interface, given only when themes are given. A name that no role
declares, a palette's `neutral_3`, stays typed where a rule names it.

## Alternatives considered

The cut:

- **One milestone,** as the card had it: one review. But twelve deliveries and two groups of
  decisions, a long session before its first check.
- **Three now,** the migration of the call sites apart: each smaller. But the second's cut decided
  before its builder exists.

The roles:

- **The eight roles of forced colours, `accent` and `focus`,** their fallbacks Plain's black on
  white: Plain's whole vocabulary at once. But three roles with no reader, black text on the hosts'
  black, and a slider's track and fill of one colour.
- **The same seven roles, their fallbacks Plain's white on black:** readable on the hosts. But
  a track and its fill, a box and its tick, of one colour; and an interface without a theme changes.
- **The editor look's thirty roles:** richer defaults. But an editor's palette in the core, which
  forced colours would map onto eight.

Where the defaults are written:

- **A style sheet of the core,** read when an interface is made: data, as ADR-0222 wants the core's
  own look. But read through reflection at each interface until M6d.3, and the core's look, the
  first stage's, mixed with the defaults under every look.
- **Kinds of their own for uwui's layers,** so that the tooltip and the others take defaults: no
  colour missing anywhere. But four new kinds, their rules and the look's sheet changed, made again
  when they become parts.

When names resolve:

- **When each element is styled,** ADR-0223's letter: no amendment. But a lookup at each leaf of
  each style made, a declared type for every leaf, and a missing role found late.
- **Lazily, cached by role and themes:** nothing resolved that is not used. But a second structure
  to keep right, for seven roles.

A role of another type:

- **Its fallback, with a warning:** never a refusal. But a slip in a theme hidden in a warning, and
  two rules for faults of type.

## Consequences

- An interface given no theme draws as before; under the editor's look nothing that draws changes,
  its themes giving each role from their palette.
- The fixtures of the styles, which record every leaf, change where a kind's base differs from the
  old default on a leaf that the kind does not draw; the images prove that nothing drawn changed.
- An own style that sets a colour equal to the old default, white text, now wins over the rules;
  code still cannot set a colour to transparent against a rule (`docs/debt.md`, followed at
  M6d.2b2).
- Reading the look's documents reads the roles too, and allocates more until M6d.3.
- A theme outside the repository that gave a role's name a value of another type is refused.
- A test draws every kind, in each configuration of the test of what each kind reads, under a theme
  that gives each role a colour of its own, and finds every colour drawn a role's, transparent, or
  a literal that the test names.
- M6d.2b2 decides the payload and the derive's list of colours, which transitions then read in place
  of their list kept by hand.
