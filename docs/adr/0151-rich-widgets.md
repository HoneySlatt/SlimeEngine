# 0151. Rich widgets

- Status: Accepted; amends [0149](0149-the-looks-foundations.md) (M6b.7d cut in two: rich fields,
  then what the editor tells and finds, M6b.7e)
- Date: 2026-10-27

## Context

[ADR-0149](0149-the-looks-foundations.md) cut the rest of M6b.7 in three: the look's foundations
(M6b.7b), the layers (M6b.7c, [ADR-0150](0150-layers.md)), then the rich widgets (M6b.7d), what
makes editing fast in the editor that the owner drew: fields of vectors with the axes' colors and
units, numbers changed by a drag on their label and given as expressions, sections whose header
holds more than a title, buttons that toggle and controls of segments, fields of search, texts that
empty fields show, help over a field from the documentation of what it edits, notifications and bars
of progress, counts on tabs, the palette of commands and a sheet of the shortcuts.

That is more than any milestone of Stage 6b held, and several of its parts held a choice that was
the owner's: how large the language of expressions is, which units the fields know, who builds a
section's header, who keeps the notifications and the palette. This record was first proposed, the
parts that held no open choice built meanwhile; the owner took the choices on 2026-10-27, among the
options that "Alternatives considered" lists.

## Decision

**M6b.7d in two.** M6b.7d, *rich fields*: what a panel's fields and buttons need, every part of
which stands on the elements that exist. M6b.7e, *what the editor tells and finds*: notifications
and their bars of progress, the palette of commands and the sheet of shortcuts, which stand on
layers and on the program's commands. Each ends with its example, its image tests and an agent's
session, and CI green.

**Built first, without an open choice, in M6b.7d:**

- *Texts that empty fields show* (`Element::placeholder`): a field of text or of numbers that holds
  nothing shows a text in its quieter color, which screen readers read as its placeholder.
- *Buttons that toggle* (`Kind::ToggleButton`): a button that stays pressed, its state the program's
  under a check box's rule, read as a toggled button, selected by the state `Checked`.
- *Controls of segments* (`Kind::Segments`): a row of buttons of which one is chosen, as a bar of
  tabs is, moved through by the arrows, read as a group of radio buttons; the look draws them as one
  rounded box, the chosen segment raised.
- *Counts on tabs*: a tab's count, a number drawn after its title in a small rounded box, read as
  part of its name.
- *Bars of progress* (`Kind::Progress`): a track filled to a share, or moving without one while the
  share is unknown, read as a progress indicator with its value.
- *Fields of search*: a field of text with a magnifier before its text and a mark that empties it
  after, once it holds something; its shortcut is the program's, a command of its tables that gives
  it the focus.
- *A field's prefix*: a field of numbers may show a short text before its number, `X`, which the
  pointer drags to change the number, whether or not the field holds the focus; the look colors the
  prefixes of the classes `axis-x`, `axis-y` and `axis-z` red, green and blue. A field of a vector
  is a row of such fields, which the program builds as a group named by the vector, each field
  committing its component.

**Expressions in fields of numbers.** A field of numbers reads what is typed as an expression: the
four operations, parentheses, a unary minus, `pi`, and the functions `sqrt`, `sin`, `cos`, `abs`,
`min` and `max`, read by a parser of the engine's own, the field committing the result and showing
it. A text that reads as no expression is refused as a text that reads as no number is today.

**Drags finer or coarser.** A drag of a field of numbers, on its prefix or on the field, changes its
number by a tenth of its step for each logical pixel while Shift is held, and by ten steps while
Control is held, as editors of three dimensions do.

**Units, by families.** A field of numbers may hold a family of units, length (millimeters,
centimeters, meters, kilometers), angle (degrees, radians), time (milliseconds, seconds) or none,
and its base unit, meters, degrees or seconds. It shows its unit after its number, and reads a
number typed with any unit of its family, `10cm` in a field of meters, converted, within
expressions too, `1m + 20cm`; the program's value is always in the base unit.

**A section's header holds a slot.** After a section's title, a slot that the program builds, as a
list's rows are built, holding what it likes: an icon, the type of what it shows, a button of a menu
or of reset, each element of it taking the pointer without folding the section.

**Help from the documentation, by the program.** The program gives a field's tooltip from the
documentation that reflection keeps of a field (`FieldInfo::doc`); the framework draws a tooltip of
two parts, a title and a body, and reads nothing of reflection. M6b.8's editors of reflected values
give these tooltips.

**Notifications, kept by the framework (M6b.7e).** The program gives a notification
(`Ui::notify`), of a kind, information, success, warning or error, its text, and a bar of progress
and buttons if it has them; the framework stacks them at the window's bottom end, takes each away
after a time that its kind sets unless the pointer rests on it, and has screen readers read them as
they come.

**The palette of commands, opened by the framework (M6b.7e).** The framework opens it
(`Ui::open_palette`): a modal layer of a field of search and a list of the commands that the
program names, each with the chord that the tables of shortcuts give it, ranked as the user types
by a match of the engine's own, letters in order, the starts of words first; the program reads the
command chosen as it reads a shortcut's.

**The sheet of shortcuts, built by the framework (M6b.7e).** A modal layer that the framework builds
from the tables of shortcuts, by scope, each chord and its command's name as the program names it.

## Alternatives considered

- **M6b.7d whole**: one milestone, but more than twice the largest milestone of Stage 6b, where the
  plan cuts a milestone that outgrows itself at its start (`docs/PLAN.md`, "The size of the
  framework").
- **A kind for a field of a vector**: one element that commits a vector whole, but a second way to
  hold numbers beside the field of numbers, and a vector of four for colors and rotations, which
  M6b.8 edits as their own.
- **Expressions of the four operations alone**: a smaller parser, but `sqrt(2)/2` and `sin(30)`,
  which an editor's users write, refused.
- **A crate of expressions**, such as `evalexpr`: variables and many functions, but one more
  dependency for much more than an editor's fields need.
- **A unit shown alone, typed units refused**, or **no units at Stage 6b**: less now, the
  conversions that the editor wants paid later.
- **Fixed parts in a section's header**, an icon, a subtitle, a menu and a reset, each an action of
  its own: simpler for that use, but every other header asking the framework to change.
- **The framework reading reflection for help**: automatic, but the interface's framework tied to
  reflection's types for values, against its role.
- **Notifications, the palette and the sheet built by each program**: freer, but every program
  writing again the stack and its times, the ranking, the list of chords, and their accessibility.
- **A crate that ranks commands**, such as `nucleo`: one more dependency for a ranking of about a
  hundred lines.

## Consequences

- `slime-ui` gains the kinds and fields above, expressions and units in fields of numbers, a slot
  in a section's header and tooltips of two parts in M6b.7d; notifications, the palette and the
  sheet of shortcuts in M6b.7e. The look's sheet styles them; the `widgets` example shows them, its
  images drawn again; an agent's session uses each.
- `docs/PLAN.md` holds M6b.7d, rich fields, and M6b.7e, what the editor tells and finds.
