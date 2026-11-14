# uwui-accesskit

What a uwui interface means, given to screen readers as AccessKit's tree, and what they ask of it
given back to the interface ([ADR-0146](../../docs/adr/0146-screen-readers.md),
[ADR-0261](../../docs/adr/0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md),
[ADR-0262](../../docs/adr/0262-uwui-accesskits-api-one-tree-per-window-the-hosts-node-made-when-needed.md)).
Any host of uwui uses it beside its own AccessKit adapter; the engine's is `slime-app`, with its
`accessibility` feature. MIT OR Apache-2.0, as uwui's crates.

## Role

- Translates what one target's part of the interface means into AccessKit's tree: each element
  that a person reads or uses, its role, name, value, states and actions, a field's text by runs
  of characters with its caret, below the window's node. It reads the interface through its own
  *reader of meaning* (`uwui::tree::Ui::add_meaning_reader`): the whole tree when a screen reader
  asks for it, then what changed alone, each update after the target's frame.
- Does what a screen reader asks through the interface's calls, as a person's input does: a press,
  the focus, a value, a selection, a scroll, a section opened. A request is done only on the
  window's tree, on an element of its target, and on an action that the element takes now: a
  screen reader may ask on a tree older than the last.
- Re-exports `accesskit`, so that a host builds its window's node and hands the updates to its
  adapter with the crate's own version.

## Entry points

- `ScreenReaderTree::new(target)`: the tree of one window, which shows `target`.
- Its methods follow the three events of an AccessKit adapter: `tree_requested()`,
  `act(&mut ui, &request)`, `deactivated(ui)`.
- `update(ui, window_changed, window)` after each frame of the target: the update to give the
  adapter, none while no screen reader reads. `window` makes the window's node, called only when
  the update holds it.

The crate's rustdoc holds the contracts and an example of a host.

## Tests

`src/tests.rs` builds interfaces in uwui's test font, translates them, and builds AccessKit's tree
from the updates with `accesskit_consumer`, as the adapters do, which panics on an update that is
not one. They cover the widgets' roles, states and values, the requests, a modal root, an
interface that comes and goes, the window's reader let go, and random changes whose updates keep
the tree whole. `scripts/screen-reader-check.sh` reads the `interface` example's window through
AT-SPI, as a screen reader does, in `scripts/check.sh windowed`.

## Dependencies

`uwui`, whose meaning it reads; `accesskit`, re-exported; tracing, a warning when the interface
refuses a reader, each request at the debug level. Its tests: `accesskit_consumer`, and
`slime-color` for a color well's color, which uwui names without re-exporting.

## Must not

- Hold the platform's adapter, or know the window: its host does, and makes the window's node.
- Depend on an engine crate; its tests on none but `slime-color`, which leaves with uwui.
  `scripts/check.sh lint` checks it.
- Be built into a game: games leave screen readers out until Stage 7, which `scripts/check.sh lint`
  checks.
