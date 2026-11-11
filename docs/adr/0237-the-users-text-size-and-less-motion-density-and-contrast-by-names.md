# 0237. The user's layer: the size of text and less motion as preferences, density and contrast by names

- Status: Accepted; amends [0223](0223-styles-written-in-code-through-tokens.md) (density and
  more contrast leave the lengths and colours that code writes as literals; the size of text
  spares icons and reaches a list's rows), [0149](0149-the-looks-foundations.md) (the same, against
  0223's amendment), [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)
  (the curve of the size of text given by stops; the core takes the user's preferences before the
  reader that gives them), [0231](0231-an-elements-own-style-written-in-code.md) (the fourth step of
  the mask order is the size of text alone) and
  [0232](0232-transitions-read-the-derives-colours-and-share-what-they-show.md) (the derive lists
  no lengths)
- Date: 2026-11-11

## Context

[ADR-0223](0223-styles-written-in-code-through-tokens.md) put a *user's layer* last in the order
of styling: the size of text scales every text; the density scales the lengths that code writes
as literals, by a factor that the density's theme gives; under high contrast, the colours that
code writes give way to the rules'. [ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)
made the size of text a curve, and left the system's preferences to a reader built at uwui's first
stage, with forced colours. [ADR-0232](0232-transitions-read-the-derives-colours-and-share-what-they-show.md)
left the user's layer to M6d.2b2b2, with the derive's list of lengths and their classes.

Today a program sets less motion with `Ui::set_reduced_motion`; the look's
`themes(Colors, Density)` picks its theme of colours, high contrast among them, and its compact
theme. The look has one set of high contrast, dark.

Two attacks of the lead's first draft found, before M6d.2b2b2 started, that one factor or two miss
the look's compact theme on 52 of its 128 uses, which is no factor; that a list's rows, whose
height is read in six places, overlap under a factor; that at 200 % text 13 of the `widgets`
example's 19 boxes of fixed width that hold a text overflow; and that colours of code giving way
only where a rule sets the same field part a text from its background, 9.4:1 falling to 1.9:1.
The lead's second draft was attacked in turn. Giving way to the kind's base erases a box or a ring
drawn in code, and leaves a clip's name, its colour computed in code, white over the clip's own
colour. At 200 % three texts of the `widgets` example are cut, one by an ancestor, one by uwui's
own sheet of shortcuts. The editor's icons outgrow their boxes of 18 pixels from 139 %. The frame
after a change of the size of text allocates 71.6 times for each element, under no target.

The facts, read on 2026-11-10 and 11:

- No toolkit found scales an application's literal lengths under density. Flutter's
  `VisualDensity` "does not affect text sizes, icon sizes, or padding values"; WinUI's compact
  density is a dictionary of resources merged so that it "overrides the default size values" by
  their keys. A screen across the room scales everything: XAML's apps "are scaled at a default of
  200%" on a TV.
- Under more contrast, no reference replaces an application's literal colours: CSS's
  `prefers-contrast` is a query; on Apple's platforms "color objects you create using fixed
  component values do not adapt"; libadwaita's applications "that use custom drawing or styles may
  need to support it manually". Only forced colours replace them.
- GNOME's text factor is linear, from 0.5 to 3.0. Windows' goes from 1 to 2.25, larger text less
  affected, by no published formula. Android 14 interpolates sorted stops. Microsoft: "Don't scale
  font-based icons or symbols". WCAG's failure F69: text "clipped, truncated, or obscured", a
  container "set to a fixed pixel size".

The owner decided on 2026-11-11, one question at a time.

## Decision

**Density reaches lengths by their names.** The rules read the density's theme, as today; tokens
will, at M6d.2b2b3. A length that code writes as a number is the author's, exact under every
density. The *couch* density, for a screen across the room, scales the root, which grows
everything, numbers included, with a theme for the named values; it is built with its first
reader. The derive lists no lengths.

**Under more contrast, the colours that code writes stay.** The look takes its values of high
contrast by their names: the rules today, tokens at M6d.2b2b3. A colour written in code as a
number is kept, as every reference keeps it; the themes' test kit flags it
([ADR-0223](0223-styles-written-in-code-through-tokens.md)). Colours of code give way under forced
colours, built at uwui's first stage: by property, each to the role that pairs with it, never to
nothing, with an opt-out for content that the elements below it inherit.

**The size of text reaches every text but icons, and a list's rows.**

- `TextSize` is a curve: stops, each a size of text and the size it becomes, sorted by size and
  never shrinking, interpolated linearly between them and from zero below the first; beyond the
  last, each size grows by the last stop's ratio. `TextSize::factor` makes the linear curve of a
  factor; 1.0 is the default. A stop or a factor that is not finite, a ratio outside 0.5 to 3, or
  stops out of order are refused.
- The cascade applies it in its fourth step, after the own states, to the size of every text,
  whoever wrote it, the rules or the code. A text whose first font is its style's font of icons
  (`icons.font`) keeps its size: icons stay in their boxes. A control's marks take its control's
  size, as before.
- A list's rows grow by the curve's ratio at the list's own size of text, their height whole in
  pixels of the screen, read through one accessor: rows never overlap, at any scale.
- Other lengths do not grow: a box of fixed size that holds a text overflows. uwui's own and the
  `widgets` example's become least sizes. A test at 200 % finds no text of the `widgets` example,
  its layers open, cut by its box or by any ancestor up to the first that scrolls.
- The frame after a change of the size of text lays every text out again, as the first frame
  does: it allocates no more for each element than the first frame of the same tree, a counter of
  `ui.tree.look` that CI checks.

**A program gives the user's layer as one value,** `Preferences`, in uwui's core: the size of text
and less motion, which the interface reads. `Ui::set_preferences` replaces
`Ui::set_reduced_motion` and styles every element again when the size of text changes.
`Preferences` takes forced colours and less transparency with their first reader, at uwui's first
stage; light or dark and more contrast join it when the look becomes a value, the interface then
choosing the look's set itself (ADR-0226). Until then the look's `themes(Colors, Density)` stays
the one source of contrast.

## Alternatives considered

Density:

- **A factor on the lengths written in code,** given by the density's theme, rounded to the pixel:
  0223's letter, the user's choice over the author's numbers. But a pixel off beside the rules'
  values on 52 of 128 uses, a list of lengths with their classes, and a target that may shrink
  under WCAG 2.2's 24 pixels.
- **A table of steps,** each number taking the nearest step's value: closer to the theme, 14 uses
  missed. But implicit, two neighbouring numbers falling into different steps, and no reference.

Contrast:

- **Colours of code giving way by property,** to the rule's value, else to the role of their
  property: 0223's letter, nothing parted. But stronger than every reference, an opt-out for
  content with no reader in the repository, and a milestone cut in two.
- **Every colour of code giving way to the kind's base:** the simplest. But a box, a border and a
  ring drawn in code erased, and a clip's name unreadable over its colour.

The size of text:

- **Icons growing too,** as SF Symbols do: an icon beside a text keeps its scale. But their boxes
  of fixed size, in the editor too, become least sizes, against Microsoft's advice.
- **Text alone,** rows and boxes left to authors: the least work. But rows that overlap and cut
  their text, and WCAG 1.4.4 failed on the `widgets` example.

How a program gives it:

- **A setter for each preference,** `set_text_size` beside `set_reduced_motion`: the least change.
  But each preference added grows the API, and the reader calls several at each change.
- **`Preferences` with light or dark and contrast,** the look's `themes` reading it: the final
  shape now. But two calls that may disagree, since the interface keeps no themes, fields that it
  does not read, and a third enum of light or dark beside `slime-platform`'s.

## Consequences

- ADR-0223's second and third points of the user's layer are replaced: what code writes as a
  number follows neither density nor more contrast. An application that wants both writes tokens,
  M6d.2b2b3's, and gives its own themes for each density and for high contrast.
- Magnolia's components follow density and contrast through their tokens; a duration never grows
  with density; a clip's colour and its name, written in code, keep their contrast. Forced colours'
  inherited opt-out serves a clip.
- The fourth step of ADR-0231's mask order scales the size of text; the styles made are made again
  when it changes.
- `slime-editor`, `slime-devui`, the examples and the tests call `Ui::set_preferences`.
- The look has no light set of high contrast: debt, until the look is a value.
- WCAG 1.4.4 is checked on the `widgets` example only; the editor at 200 % is not checked.
