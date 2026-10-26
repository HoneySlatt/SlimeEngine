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
is cut and how a widget is made.

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
  slider, a section's mark, a handle and the scroll bars share.

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
- The editor's theme, its fonts, the example that shows every widget, an agent's session over it and
  its image tests come with the rest of M6b.7a.
