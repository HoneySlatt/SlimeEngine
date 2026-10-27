# 0151. Rich widgets

- Status: Proposed
- Date: 2026-10-27

## Context

[ADR-0149](0149-the-looks-foundations.md) cut the rest of M6b.7 in three: the look's foundations
(M6b.7b), the layers (M6b.7c, [ADR-0150](0150-layers.md)), then the rich widgets (M6b.7d), what
makes editing fast in the editor that the owner drew: fields of vectors with the axes' colors and
units, numbers changed by a drag on their label and given as expressions, sections whose header
holds more than a title, buttons that toggle and controls of segments, fields of search, texts that
empty fields show, help over a field from the documentation of what it edits, notifications and bars
of progress, counts on tabs, the palette of commands and a sheet of the shortcuts.

That is more than any milestone of Stage 6b held, and several of its parts hold a choice that is
the owner's: how large the language of expressions is, which units the fields know, who builds a
section's header, who keeps the notifications and the palette. The owner asked on 2026-10-27 that
this record propose the cut and the choices before any of it is built, and that the parts that hold
no open choice be built meanwhile.

## Decision (proposed)

**M6b.7d in two.** M6b.7d, *rich fields*: what a panel's fields and buttons need, every part of
which stands on the elements that exist. M6b.7e, *what the editor tells and finds*: notifications,
bars of progress, the palette of commands and the sheet of shortcuts, which stand on layers and on
the program's commands. Each ends with its example, its image tests and an agent's session, and CI
green.

**Built without an open choice, in M6b.7d:**

- *Texts that empty fields show* (`placeholder`): a field of text or of numbers that holds nothing
  shows a text in its quieter color, which screen readers read as its placeholder.
- *Buttons that toggle* (`Kind::ToggleButton`): a button that stays pressed, its state the program's
  under a check box's rule, read as a toggle button, selected by the state `Checked`.
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
  pointer drags to change the number, as it drags a field that does not hold the focus today; the
  look colors the prefixes of the classes `axis-x`, `axis-y` and `axis-z` red, green and blue. A
  field of a vector is a row of such fields, which the program builds as a group named by the
  vector, each field committing its component.

**Open choices, for the owner** (each with the recommendation first):

1. *Expressions in fields of numbers.* (a) The four operations, parentheses, a unary minus and `pi`,
   read by a parser of the engine's own, about 150 lines, the field committing the result and
   showing it; (b) the same and the functions of `f64`, `sqrt`, `sin`, `cos`, `abs`, `min`, `max`;
   (c) a crate of expressions, such as `evalexpr`, more functions and variables, one more dependency
   whose license is to be checked. Recommended: (b), for an editor whose users write `sqrt(2)/2` and
   angles.
2. *Units.* (a) A field of numbers holds a unit of one family, length, angle, time or none, shows it
   after its number, and reads a number typed with another unit of the family, `10cm` in a field of
   meters, converting it; (b) a unit shown alone, typed units refused; (c) no units at Stage 6b.
   Recommended: (a), its families those of the engine's components: meters, degrees and seconds,
   the program's value always in the family's base unit.
3. *A section's header.* (a) A slot that the program builds after the title, as a list's rows are
   built, holding what it likes, an icon, the type, a button of a menu or of reset, each an element
   that takes the pointer without folding the section; (b) fixed parts, an icon, a subtitle, a menu
   and a reset, each told to the program as an action of its own. Recommended: (a), one way for
   every header, the editor's inspector building its own.
4. *Help from the documentation.* (a) The program gives a field's tooltip from the documentation
   that reflection keeps of a field (`FieldInfo::doc`), the tooltip showing a title and a body; the
   framework only draws a tooltip of two parts; (b) the framework reads reflection itself. Recommended:
   (a): `slime-ui` stays free of reflection's types for values, M6b.8's editors of reflected values
   give the tooltips.
5. *Notifications (M6b.7e).* (a) The framework keeps a stack of them at the window's bottom end
   (`Ui::notify`), each of a kind, information, success, warning or error, its text, a bar of
   progress and buttons if the program gives them, gone after a time that its kind sets unless the
   pointer rests on it, read by screen readers as they come; (b) the program builds them as layers.
   Recommended: (a), as tooltips are kept, every program alike.
6. *The palette of commands (M6b.7e).* (a) The framework opens it (`Ui::open_palette`), a modal
   layer of a field of search and a list of the commands that the program names, each with the chord
   that the tables give it, ranked as the user types by a match of the engine's own, letters in
   order, the starts of words first, the program reading the command chosen as it reads a
   shortcut's; (b) the program builds it from a field and a list, the framework ranking alone;
   (c) a crate that ranks, such as `nucleo`, one more dependency. Recommended: (a), the ranking
   about 100 lines.
7. *The sheet of shortcuts (M6b.7e).* (a) A modal layer that the framework builds from the tables
   of shortcuts, by scope, each chord and its command's name as the program names it; (b) the
   program's. Recommended: (a).

## Alternatives considered

- **M6b.7d whole**: one milestone, but more than twice the largest milestone of Stage 6b, where the
  plan cuts a milestone that outgrows itself at its start (`docs/PLAN.md`, "The size of the
  framework").
- **A kind for a field of a vector**: one element that commits a vector whole, but a second way to
  hold numbers beside the field of numbers, and a vector of four for colors and rotations, which
  M6b.8 edits as their own.

## Consequences

- `slime-ui` gains the kinds and fields above; the look's sheet styles them; the `widgets` example
  shows them, its images drawn again; an agent's session uses each.
- The open choices decide the rest of M6b.7d, then M6b.7e, which `docs/PLAN.md` adds once they are
  taken; this record is accepted then, its choices written in place of the options.
