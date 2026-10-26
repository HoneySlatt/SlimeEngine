# 0148. Widgets

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) (M6b.7 cut in two: the widgets
  of a panel, then layers) and [0132](0132-the-engines-ui-framework.md) (each widget is an element
  kind of `slime-ui`)
- Date: 2026-10-26

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided the framework: a retained tree of elements of
its own, its actions turned into commands, its meaning read by agents and screen readers, styles
typed and themes read from files. M6b.7 was to give it every widget that the dev overlay and the
editor are made of: check boxes, sliders, lists to choose from, sections that fold, tabs, tooltips,
context menus, popups and modal layers, floating panels, splitters and scroll bars, with the
editor's theme and fonts. Fields of text and of numbers ([ADR-0145](0145-text-edited.md)) had shown
one way to make an element that keeps what the user gives: the element keeps it, the program reads
it as actions, and the program's value replaces the user's only when it changes.

The owner decided on 2026-10-26, among the options that "Alternatives considered" lists, how M6b.7
is cut, how a widget is made, where the editor's look lives and which example shows the widgets.

## Decision

**M6b.7 cut in two, along what the widgets stand on.** M6b.7a brings what a panel holds and needs
nothing over the tree: check boxes, sliders, sections that fold, tabs, two panels and the handle
between them, scroll bars, the editor's theme and its fonts of text and of fixed width. M6b.7b
brings what opens over a panel and needs layers: popups, tooltips, context menus, lists to choose
from, floating panels and modal layers, with the font of icons.

**Each widget is an element kind of `slime-ui`.** As a field, a widget is a `Kind` of the tree: its
state is in the tree, its input, its meaning and its actions are its own, and rules style it by its
kind and select it by its states. What its builder gives it is under the rule of fields: the
program's value replaces what the user left only when it differs from the value that the program
gave before, so that a builder that runs again keeps what the user did.

- **A check box** (`Kind::CheckBox`) toggles on a press, on Enter or Space while it holds the focus,
  and on an agent's or a screen reader's press; the program reads `FieldValue::Checked` changed then
  committed. Its state is `ElementState::checked`, which rules select as `Checked`.
- **A slider** (`Kind::Slider`) keeps a value within its bounds and on its steps: the pointer drags
  its thumb or presses its track, its keys move it by a step, by ten, or to its bounds; the program
  reads each change, and the commit as the drag ends or after each key. Agents and screen readers
  set its value (`Ui::set_value`), which its meaning gives with its bounds (`ValueRange`).
- **A section that folds** (`Kind::Section`) draws a header, a mark and its title, above its
  children, room for which its layout makes within its padding. Folded, what it holds takes a
  computed style of no display, which every walk of the tree already reads: it is not laid out,
  drawn, reached, focused nor meant. Whether it is open is the interface's alone, as a scroll is, and
  tells the program nothing; its meaning is AccessKit's `Details`, expanded or not.
- **Tabs** follow the structure that screen readers know, a bar of tabs and the panel of the chosen
  one: a bar (`Kind::TabBar`) whose children are its tabs (`Kind::Tab`), each a title, the chosen
  one in the state `Selected`, and a panel that the program builds for the chosen tab
  (`Children::chosen`), which it names as such (`Role::TabPanel`). A choice runs again the builder
  that made the bar, as a list that shows other rows does, so that the panel follows without the
  program moving its revision. Screen readers see every tab, and the chosen panel.
- **Two panels and a handle** (`Kind::Split`) lay out its first two children side by side or one
  above the other, the first taking its share, the second the rest less the handle, which the
  pointer drags and the arrows move; children past the second are folded as a section's. Its
  meaning is a splitter whose value is the share.
- **Scroll bars** belong to a box that scrolls and to a list, drawn over their content while it does
  not fit: a thumb as long as the share that shows, which the pointer drags, and a track that a press
  pages along. They take the presses that fall on them before the elements below.
- **A control's parts** are styled apart from its box (`style::ControlStyle`): the size of what the
  pointer takes, a radius, and three colors, the track, the fill and the mark, which a check box, a
  slider, a section's mark, a handle and the scroll bars share. A check box's label and a section's
  title lie against their box or their mark, from either side, whatever the direction of their text,
  which aligns it to its own start.

**The editor's look lives in `slime-ui`, behind a feature.** The fonts and the style made for the
editor and the dev overlay, their *look*, are `slime-ui`'s module `look`, built only with the
feature `editor-look`, so that a game that draws neither carries none of it. The overlay draws in it
once M6b.8 moves it onto the engine's interface, and the editor once Stage 6c gives it a window. Its
fonts are Geist, which [ADR-0139](0139-text-drawn-and-the-display-list.md) chose, and Geist Mono,
for text of fixed width, numbers and code: the variable fonts of their upright faces, of release
1.7.0, each one file for every weight, under the OFL with their license beside them
([ADR-0133](0133-font-files-under-the-open-font-license.md)). Its style sheet styles labels,
buttons, fields and widgets by their kind and their states, and a few classes, `window`, `bar`,
`panel`, `muted` and `fixed`, the last for Geist Mono; its theme, dark, gives the values that the
sheet names. Both are documents of the repository, `crates/slime-ui/look/`, built into the program
as text and read as any style sheet and theme are ([ADR-0141](0141-style-sheets-and-themes.md)): a
program stacks its own sheet after the look's. The elements that take the focus show it by the color
of a border of one pixel that they always keep, transparent until then for those that show none
otherwise, so that the focus moves nothing; the handle between two panels shows it by its fill.

**An example of its own shows the widgets.** The `widgets` example draws a panel of the editor's
kind in the editor's look, every widget of M6b.7a in it: a bar of tabs; an inspector of two panels
side by side, a list of entities and a box of sections that fold, of sliders and check boxes; a
console of two panels one above the other, the lines of a log and check boxes that choose its
levels. Its image tests compare it with reference images, and an agent's session through the MCP
server uses each widget through its actions.

## Alternatives considered

How M6b.7 is cut:

- **One milestone**, as planned: one pass, but a long one before the first check.
- **Three milestones**, simple widgets, layers and popups, floating panels and splitters: smaller
  steps, but one ADR and one run of CI more, and splitters apart from the panels that they divide.

How a widget is made:

- **Widgets composed by functions** from the elements that exist, boxes, buttons and texts, with a
  few behaviours of their own: fewer kinds in the tree, but each widget's meaning and actions
  composed again by hand, and its state held by the program, which a builder that runs again would
  lose.
- **A design study first**, by several agents, before any code: a better-informed design, but the
  milestone held until the owner's return, the fields having shown the way already.

Where the editor's look lives:

- **A crate of its own**: a home that says what it is, but a crate for three documents, two fonts
  and three functions, and one more dependency for the editor and the overlay.
- **Source assets that each program cooks**, as the examples' themes are: the look read as a game's
  UI is, but every game that ships the overlay cooking the editor's documents and fonts, which the
  asset pipeline does not cook yet.

Which example shows the widgets:

- **The `interface` example, extended**: one example of the framework, but one that already shows
  text in eight writing systems, a view, a list of 100 000 rows, a dialog and fields, in a theme of
  its own, which the widgets would crowd, and in another look than the editor's.

How tabs are made:

- **A container of pages**, its tabs drawn by the container from its pages' names, as some engines
  do: one element, but unchosen pages hidden, so that screen readers see no tab but the chosen one.
- **Tabs that hold their panels**, each tab's children drawn in the bar's content area: every tab
  seen, but a layout of their own outside the tab's box, as only lists have.

## Consequences

- `slime-ui` gains `Kind::CheckBox`, `Kind::Slider`, `Kind::Section`, `Kind::TabBar`, `Kind::Tab`
  and `Kind::Split`, their builders on `Element`, `ElementState::checked`, `expanded` and
  `selected`, `FieldValue::Checked`, `Meaning::range`, the roles `CheckBox`, `Slider`, `Details`,
  `TabList`, `Tab`, `TabPanel` and `Splitter`, and `style::ControlStyle`; `slime-input`'s
  `CursorShape` gains `ResizeVertical`. A program that matches on `FieldValue` reads the new value.
- The development channel and `slime-app`'s translation for screen readers give the new states,
  roles, values and actions; agents use the widgets through the actions that exist, a press or a
  value set.
- A box that scrolls and a list draw two items more in their display lists while their content does
  not fit.
- `slime-ui` gains the feature `editor-look`, its module `look` and the directory `look/`, some 340
  KiB of fonts; a program that builds the look in ships `Geist.OFL.txt` beside it, as the Windows
  artifact of CI does for `widgets.exe`. The workspace's documentation leaves the module out, which
  only the examples turn on: `scripts/check.sh` documents `slime-ui` with the feature apart.
- The box that a text's glyphs lie in reaches as far as their advances, without the spaces that end
  a line, where it reached across the whole width that its lines were aligned in: a label takes the
  presses that fall on its glyphs ([ADR-0145](0145-text-edited.md)), whatever its alignment, not
  those across the rest of its box, nor, centred or aligned to its end, those past it.
- A share of two panels reads, in their meaning, as its own type writes it: `0.32`; a slider's value
  on its steps has no more decimals than its step and its least value are written with, so that
  steps of `0.1` give `0.6`, not the `0.6000000000000001` that they add up to in binary; a field of
  numbers dragged keeps the decimals of its step and of the number that it started from, by the
  same rounding, so that `1.25` dragged four tenths up gives `1.65`, where it gave `1.7`. A slider
  whose bounds are no numbers is unbounded from their side, where its least bound made it panic.
- The examples build the look in; `examples/tests/reference/` holds three images more, and
  `scripts/check.sh` draws the example into an image, runs it in a headless Weston, and runs the
  agent's session there.
