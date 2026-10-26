# 0149. The look's foundations

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) and
  [0148](0148-widgets.md) (the rest of M6b.7 cut in three: the look's foundations, then layers,
  then rich widgets; transitions in Stage 6b), [0132](0132-the-engines-ui-framework.md) (Stage 7
  grows the transitions that Stage 6b declares), [0141](0141-style-sheets-and-themes.md) (a theme's
  value names others and computes colors; themes stacked)
- Date: 2026-10-27

## Context

M6b.7a ([ADR-0148](0148-widgets.md)) gave the framework the widgets of a panel and the editor's
*look*, its fonts and its style, in one dark theme whose every color was written out by hand. The
owner then drew the editor that the engine aims at, and asked for the most beautiful and modern look
the engine can give without a line of a project's own, and one that a project or a user changes
easily. A proposal set the editor's screen in the look, with what it lacked: colors that follow one
accent, light and high-contrast themes, transitions, a focus shown to the keyboard alone, crisp
lines at every scale, digits of one width, density; and widgets and features beyond them, rich
fields, a palette of commands, notifications.

[ADR-0141](0141-style-sheets-and-themes.md) had decided that a theme's value names no other, and
[ADR-0132](0132-the-engines-ui-framework.md) that Stage 7 builds animation and transitions. The owner
decided on 2026-10-26 and 2026-10-27, among the options that "Alternatives considered" lists, what
changes, and in which order.

## Decision

**The rest of M6b.7 in three, the look first.** M6b.7b builds the look's foundations; M6b.7c the
layers that M6b.7a left, popups, tooltips, menus, lists to choose from, floating panels and modal
layers, with Lucide's icons; M6b.7d the rich widgets, fields of vectors with their units, numbers
changed by a drag and given as expressions, sections with rich headers, buttons that toggle, fields
of search, notifications and the palette of commands. Each later widget is built on the look's
tokens and transitions from its first line.

**A theme's value names others, and computes colors.** A value of a theme may be `"$name"`, the
value of another name, or a call of a color function: `mix($a, $b, 20%)`, a color between two,
`lighten($a, 0.08)` and `darken($a, 0.08)`, a color lighter or darker, and `alpha($a, 40%)`, a color
of another opacity. The functions work in OKLCH, a space of lightness, chroma and hue in which equal
steps look equal: a hover that lightens a color by a step looks one step lighter whatever the color.
The values are worked out when the interface reads its themes, each once, in the order that they
name each other: a value that names itself through others, a name that no theme gives, a function
given the wrong values, refuse the themes, by the value at fault.

**Themes stack.** An interface takes its themes in order (`Ui::set_style`): a later theme's value
replaces an earlier one's of the same name, and the values that name it follow it. The engine's look
comes first, then a project's theme, then a user's: a theme of one value, `accent`, changes every
color that the look computes from it.

**The look on two levels.** The look's themes hold a palette, its neutrals tinted toward the accent,
the accent and the colors of states, then the roles that the style sheet names, surfaces, borders,
texts, fields, the states of each, worked out from the palette. A base theme holds the scales that
every theme shares: spacings on a grid of 4 pixels, radii, sizes of text, shadows and durations.
Three themes of colors, dark, light and high contrast, stack on it, and a compact theme stacks on
them for a denser layout.

**Transitions are declared in the rules.** A style holds a transition (`style.transition`): how long
a change of its values takes and along which curve. When an element's style changes, as its state
does under the pointer, the values that a transition covers, its colors and its opacity, go from
what they showed to what the new style gives over that time; a section that opens or folds grows or
shrinks to its height the same way. The look declares transitions where they help; a project's
sheet changes or removes them. The interface takes the time of each frame from its program
(`Ui::advance`), says whether a transition is under way, and drops them all when the user asks for
less motion. Stage 7 builds animation of the games' interfaces on this: what it adds, keyframes,
plays alongside.

**An element has an opacity.** A style's paint gains an opacity, which multiplies the alpha of what
the element and its descendants draw, transitions moving it as they move colors.

**The focus shows to the keyboard.** An element that the keyboard, an agent or the program focused
is in the state `FocusVisible` as well as `Focused`; one that the pointer focused is not. The look
draws its rings on `FocusVisible`, and keeps a field's border on `Focused`, as browsers do.

**Crisp at every scale, numbers in columns.** Boxes, borders and clips are placed on whole pixels
of the screen, at every scale, so that a border of one pixel stays one pixel. A text's style names
OpenType features (`TextStyle::features`), which parley applies: the look sets Geist's digits of one
width, `tnum`, in fields of numbers and in text of fixed width.

**The system's colors where the platform tells them.** `slime-platform` gives the scheme of colors
that the system prefers, dark or light, and its changes, where winit reports it: on Windows. On
Linux winit reports none; a program follows the user's choice.

## Alternatives considered

The order of the rest of M6b.7:

- **Layers first**, as the plan had it, then the look's foundations: the plan's order kept, but the
  layers styled again once the tokens came.
- **The look and the rich widgets in one milestone**, after the layers: fewer milestones, but a long
  one before its check.

Transitions:

- **Automatic**, every change of a color and of an opacity moving over one duration of the theme:
  nothing to write, but no setting per element, and the declaration left for Stage 7 to add.
- **Left to Stage 7**, as ADR-0132 had it: Stage 6b smaller, but the editor's look without motion
  until 0.8, and Stage 7 designing it without a user.

Colors:

- **Every color written out**, as M6b.7a's theme did: nothing to work out, but a project that changes
  its accent rewrites a dozen colors, and their contrasts by eye.
- **Functions in sRGB**, as most tools of the web mixed them: familiar, but a lightening that looks
  stronger on blue than on yellow, and mixes that go grey between far hues.

## Consequences

- A theme document may name other values and call color functions; ADR-0141's rule that a theme's
  value names no other no longer holds. `Ui::set_style` takes a list of themes.
- `slime-color` converts colors to and from OKLCH.
- The look's style sheet and themes are written again on the palette and the roles; the `widgets`
  example's reference images are drawn again, and gain one per theme.
- `Style` gains `transition`, a style's paint `opacity`, a text's style `features`, the element's
  states `FocusVisible`; `Ui` gains `advance` and the setting of less motion, and a frame says
  whether a transition is under way, so that a program that draws on demand keeps drawing then.
- An element's opacity multiplies what each of its primitives draws, rather than drawing the element
  apart and blending it whole: overlapping parts of a translucent element show through each other,
  until Stage 7's layers draw groups apart.
- On Linux, following the system's dark or light preference waits for a reader of the desktop's
  settings portal, a new dependency to decide.
