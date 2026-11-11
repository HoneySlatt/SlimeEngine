# 0232. Transitions read the derive's colours and share what they show; M6d.2b2b cut in three

- Status: Accepted; amends [0231](0231-an-elements-own-style-written-in-code.md) (M6d.2b2b cut in
  three: transitions first, then the user's layer, then tokens) and
  [0229](0229-styles-typed-as-declarations.md) (the derive lists a style's colours by a mark on each
  field; lengths are listed with the user's layer)
- Date: 2026-11-11

## Context

[ADR-0231](0231-an-elements-own-style-written-in-code.md) left to M6d.2b2b a declaration's payload,
tokens, the derive's list of colours and lengths with transitions reading it, the user's layer,
a field of text named by a theme, `subsecond`, and a target for the frame after a change of theme.

Transitions keep their own list of the colours that they move (`Shown`, in `uwui::tree::motion`):
eight colours and the opacity, the backdrop of a layer's root left out. For each element moving,
at the frame that starts its transition and at each frame after, they clone the element's whole
target style and write the colours shown into it. A style holds seven texts of icons, a list of
fonts and other lists: a clone is some eleven allocations.

Counted on 2026-11-11, in a debug build, by a copy of `ui.tree.look`'s benchmark that matches CI's
counter: the frame after a change of the look's colours, 12 487 elements styled again, makes
393 706 allocations, 31.5 an element. The cascade makes 232 in all; drawing again makes 18.5 an
element; transitions 13. Each of the 28 following frames of the transition makes 30.5 an element
moving, and takes 21 to 29 ms in a release build. No budget bounds either frame
([`debt.md`](../debt.md)).

The lead's draft of M6d.2b2b was attacked twice before it reached the owner. The first attack
measured the figures above. It found that one factor of density on the lengths that code writes
disagrees with the look's compact values, that the size of text leaves boxes of fixed size
overflowing at 200 %, and that the user's layer would read the contrast from two sources. The
second attack found that a unit of length relative to the text would change the sheets' format,
that colours of code giving way only where a rule sets the same field would part a text from its
background (a contrast of 9.4:1 falling to 1.9:1 under the editor's look), and that the first half
still held twelve deliveries, of which transitions, which no image test sees, were a group apart.
The owner decided on 2026-11-11, one question at a time.

## Decision

**M6d.2b2b is cut in three,** each abstraction with its first reader:

- M6d.2b2b1, transitions and the frame of a change of theme: the derive's list of colours, which
  transitions read; the styles shown shared; the frame's targets. Nothing drawn changes but the
  backdrop, below.
- M6d.2b2b2, the user's layer over what code writes: how a program gives it, the density on the
  lengths that code writes, the size of text, the colours of code under more contrast. Its
  questions are put at its start, with the two attacks' findings.
- M6d.2b2b3, tokens: `Token<T>`, the core's roles as tokens, an application's own tokens and their
  lists, a declaration's payload, a field of text named by a theme, `subsecond`. Its questions are
  put at its start, an application's needs among them.

**The derive lists a style's colours.** A field of a style marked `#[declare(color)]` holds a colour
that transitions move; the derive gives each struct of a style the colours of its marked and nested
fields, in a fixed order and number (`Colors`). A field whose type is `LinearColor` without the mark
fails to compile, so that no colour is forgotten. A shadow, an option, gives its colour,
transparent where it casts none. Transitions read that list in place of their own; the backdrop of a
layer's root joins it, and a modal layer's scrim now fades with a change of theme.

**Elements that move alike share what they show.** Elements whose transitions start at the same
frame, from the same colours shown, towards the same style, show one style, made once at each frame
of the transition and shared. Each element keeps its own transition: an element styled again
during its transition starts another, from what it shows, and leaves the others.

**Targets,** checked in CI by `ui.tree.look`'s counters, in a debug build:

- the frame after a change of the look's colours: at most 20 allocations for each element styled
  again;
- each frame of the transition that it starts: at most 19 allocations for each element moving, a
  counter of its own.

Drawing an element again, when the cache of texts already holds its text, no longer copies the
text's style nor the cache's key. What drawing again still allocates is uwui's debt, followed in
its paint.

## Alternatives considered

The cut:

- **Two halves, the user's layer with transitions:** one review. But twelve deliveries, two groups
  of questions, and transitions, which no image test sees, checked with changes that the images do.
- **Tokens first:** an application's tokens sooner. But the payload, which tells a literal from a
  token, has no reader before the user's layer, the reason ADR-0231 gave for its own cut.

The styles shown:

- **A style cloned for each element moving,** as today, the colours from the derive's list: the
  least change. But some eleven allocations an element at each frame.
- **Drawing reads the colours shown beside the style:** no style made during a transition. But 55
  lines in five files read a style's colours to draw, and `Ui::style` would give the target, not
  what shows.
- **A style that clones cheaply,** its texts and lists shared (`Arc<str>`): simpler transitions. But
  `Style`'s public types change, and reflection reads them until M6d.3.

The targets:

- **Figures recorded, no target:** nothing checked. But a frame that no budget bounds, which grew
  unseen before.

The backdrop:

- **Left out, as today:** transitions move exactly what they moved. But a list kept apart from the
  derive's, and a scrim that jumps while the rest fades.

## Consequences

- Transitions move every colour that a style holds, and any colour added to a style later.
- `benches/` counts a frame of the transition; CI checks both targets.
- The look's images, the two fixtures of styles and the frames' times are unchanged; the counters of
  `ui.tree.look` fall, and are recorded again.
- M6d.2b2b2 and M6d.2b2b3 start from the two attacks' findings, kept with the lead's notes; the
  derive's list of lengths, with each length's class, comes with M6d.2b2b2, its first reader.
