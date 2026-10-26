# 0146. Screen readers

- Status: Accepted; amends [0122](0122-the-development-channel.md) (version 9 of the protocol: a
  field's value read, and set), [0132](0132-the-engines-ui-framework.md) (AccessKit's types stay out
  of `slime-ui`: `slime-app` translates the interface's meaning into AccessKit's tree, and
  `slime-platform` holds the adapter), [0140](0140-the-tree-of-elements.md) (each element carries
  the mark of its meaning) and [0143](0143-what-agents-see-of-the-interface.md) (an element's
  value, an agent's action that sets it, and the changes of the meaning, which the marks give)
- Date: 2026-10-26

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that the tree says what it means, to agents
and to screen readers; that screen readers read it through AccessKit's adapters in
`slime-platform`, behind a feature built into the editor and the examples and left out of games
until Stage 7; and that AccessKit's types stay inside `slime-ui` and `slime-platform`, how its tree
crosses between them decided by the milestone that builds the adapter.
[ADR-0143](0143-what-agents-see-of-the-interface.md) made the meaning when asked, without marks, and
left an element's value, and the mark of the meaning that changed, to this milestone, M6b.6b, whose
fields of text and of numbers ([ADR-0145](0145-text-edited.md)) are the first elements that hold a
value.

AccessKit's adapter must exist before its window first shows, and serves the window from threads of
its own: on Linux, AT-SPI's D-Bus; on Windows, UI Automation, whose adapter stays on the window's
thread. It asks for the whole tree when a screen reader begins to read, then takes updates that hold
only the nodes that changed, each node whole. On Linux, a field of text gives a screen reader its
text only through runs of text, nodes of their own that give each character's length, place and
width and where its words start: without them, Orca names the field and reads none of what it holds.

The owner decided two questions on 2026-10-26, among the options that "Alternatives considered"
lists: how the tree crosses from the interface to the window, and whether fields give their runs of
text now. The milestone's checks then found three crates of the adapter on Linux unmaintained, which
the owner decided to accept, below.

## Decision

**The engine's vocabulary between the interface and the window.** `slime-ui` gives what changed in
its meaning in its own types (`Ui::take_meaning_changes`, `MeaningChanges`): each element that
changed, what it means, the elements that mean something nearest below it, and a field's text by
runs. `slime-app` alone translates them into AccessKit's tree, beside the code that connects the
interface to its window, with its `accessibility` feature; `slime-platform`, with its own, holds the
adapter, and AccessKit's tree and the requests of screen readers cross its API
(`Window::update_accessibility`, `Event::Accessibility`). AccessKit's types stay out of `slime-ui`,
whose meaning serves agents, screen readers, and later a game's own narration, alike.

**Marks of the meaning, and numbers.** An element is marked as its role, name, value, states,
actions, bounds or children change: built again otherwise, given another state, moved or cut by a
layout or a scroll, or edited. The element above it that means something is marked too when it
comes, goes, or comes to mean something or nothing, since its children are the elements that mean
something nearest below it; a root's box that changes marks all it holds; a modal root opened or
closed, which changes what every element takes, makes the next changes whole, as are the first. Each
element and each run of a field's text is named by a number that the interface never gives again
(`MeaningNode::number`), so that a node of AccessKit's tree names one element for its life; the
window's node is 0. Changes that no one takes are kept within twice the elements of the tree, past
which the next changes are whole.

**Fields give their runs of text now.** A field's text is given as runs, each a piece of one line of
one direction (`Ui::field_runs`, `TextRun`): its characters, the clusters that the field's selection
moves by, with their lengths, places and widths as the field lays them out, and where its words
start, as the field moves by words, a word holding the spaces after it; and the selection, among
them. A run holds at most 255 characters, and a character at most 255 bytes, as AccessKit counts
them in a byte. A screen reader reads a field's text, follows its caret by characters, words and
lines, hears what is typed, and places the caret and selects (`Ui::select_text`).

**A value, set as a person would.** An element's value is what a field holds, as the user left it.
Agents read it, and an action sets it (`ElementAction::SetValue`, `Ui::set_value`): the field's text
replaced whole, in one edit that its undo takes back, then committed, the program reading the change
and the commit, as when a person replaced the text and pressed Enter. A field that cannot hold the
focus now refuses it, and a field of numbers a text that reads as no finite number. A screen reader
sets a value through the same call; the development channel's protocol, version 9, carries the value
and the action, and the MCP server's `act_on_interface` takes `set_value`.

**The adapter, made before the window shows.** With the feature, `slime-platform` makes the window
hidden, makes AccessKit's adapter for it, then shows it; Wayland, which shows a window with its first
frame, ignores the first step. The adapter sends what screen readers ask to the event loop, which
gives the handler `Event::Accessibility`: the whole tree asked for, an action, or no screen reader
any more. Updates given to the window are applied once the handler's call returns, on the event
loop's thread, where the adapter stays. A screen reader may ask for the tree again while a frame
makes an update of what changed, which reaches the adapter before the handler hears of the ask,
and which the adapter would take as the whole tree: `slime-platform` notes the ask, from the
adapter's own lock, and gives the adapter a tree of the window's node alone in its place, until the
whole tree comes. `slime-app` answers the first with the whole tree at once,
gives an update after each frame drawn while a screen reader reads the window, the window's node
with it when the top of the interface or the window's size changed, and turns the actions into the
interface's calls: a click presses a button and gives a field the focus, the focus, a value, a
selection, and scrolls by an item or by what shows.

**Built into the examples, left out of games.** The examples turn `slime-app`'s `accessibility`
feature on; *Hunna Wood* does not, and `scripts/check.sh` checks that the game holds no adapter. The
editor's program turns it on once it has a window, at Stage 6c. The dependencies are accesskit 0.25
and accesskit_winit 0.34, without its default features; egui's accesskit 0.24 stays beside them
until M6b.8 takes egui out. The log's default filter keeps zbus, through which the adapter speaks
D-Bus, at `warn` ([ADR-0019](0019-default-log-filter.md)). zvariant, zbus_names and
zvariant_derive, which zbus 5 uses, are unmaintained as crates of their own since zbus 6 holds them
(RUSTSEC-2026-0370, RUSTSEC-2026-0371, RUSTSEC-2026-0372): accesskit_unix 0.24.1, the latest, uses
zbus 5, and `deny.toml` ignores the three advisories, with their reason, until it moves to zbus 6.
They are not vulnerabilities, and only a build with the feature holds them.

## Alternatives considered

How the tree crosses:

- **AccessKit's vocabulary**, `slime-ui` making AccessKit's updates and `slime-app` passing them on:
  the least code, but AccessKit's types in `slime-ui`'s API, against ADR-0143, each of AccessKit's
  versions changing three crates' signatures, and a game's own narration, at Stage 7, reading
  AccessKit's tree or doing the work again.
- **The translation in `slime-platform`**, from the interface's types: the platform would depend on
  the interface, or the meaning's types move below both into a crate of their own, for one use.

Fields' text:

- **The value alone now, runs of text later**, at M6b.7 or Stage 7: a shorter milestone, but fields
  that Orca names and does not read until then, and Narrator reading their whole text only.

How the changes are found:

- **The whole meaning compared with what was sent**, at each frame: no mark to keep, but a walk of
  the tree and a node made for each element at every frame while a screen reader reads, and against
  ADR-0140 and ADR-0143, which planned the marks.
- **Nodes named by the element's place and generation**: no number to keep, but no room left in a
  node's 64 bits for the runs of a field's text, which need numbers of their own, and a place
  reused within one update naming two elements at once.

The adapter:

- **The adapter built on Windows alone until AccessKit moves to zbus 6**: no advisory ignored, but
  no screen reader on Linux, against the milestone's goal. **Waiting for that version**: the
  milestone held for a date that no one knows.
- **The adapter's handlers called directly**, from the threads of the platform's service: the whole
  tree returned at once, but the interface, which a presentation owns on the main thread, read from
  another.
- **Every window made hidden, then shown, with the feature or without it**: one way to make a
  window, but a step that a game, which holds no adapter, does not need.

## Consequences

- `Meaning` gains `value`, `ElementAction` gains `SetValue`, and `Ui` gains `take_meaning_changes`,
  `field_runs`, `set_value`, `select_text` and `element_by_number`, and gives `root_settings`;
  marking costs a flag and a push for each change, whether or not a screen reader reads.
- Screen readers cost nothing until one asks for the tree: the adapter asks for none, and
  `slime-app` takes no change. On Windows, which never tells that no client reads any more, a
  window whose tree was asked for once is given an update after each frame for the rest of the
  run.
- The examples give their interfaces to screen readers on Linux and on Windows; the owner checks
  Orca and Narrator by hand. A check of CI cannot run a screen reader, but `slime-app`'s tests build
  AccessKit's tree from the updates with `accesskit_consumer`, as the adapters do, which refuses an
  update that is not one; the owner approved it as a dependency of the tests on 2026-10-26.
- 52 crates of the workspace's build come only through the adapter on Linux, AccessKit's and those
  of AT-SPI and D-Bus, and 4 on Windows, where UI Automation is the system's; a game's build holds
  none of them.
- Not given yet: a modal dialog's mark, the text of labels by runs, a spin button's increments, an
  element scrolled into view, a scroll to an offset, the words of a text that its builder names,
  which give way to its name, a caret's side at the end of a line that breaks within the text, and
  the hover and the press, which AccessKit does not hold.
