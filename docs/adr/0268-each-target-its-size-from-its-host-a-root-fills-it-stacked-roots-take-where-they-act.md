# 0268. Each target its size from its host; a root fills it; stacked roots take where they act

- Status: Accepted; amends [0140](0140-the-tree-of-elements.md) (a root's size and scale are its
  target's), [0142](0142-input-and-focus.md) (no root's origin; which root the pointer
  reaches), [0249](0249-the-hosts-api-its-items-and-their-table.md) (the target's size a method,
  its units, the engine's column),
  [0251](0251-the-engines-host-clock-and-its-times.md) (a layer takes its target's scale),
  [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)
  (a root moved takes its new target's size) and
  [0258](0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md) (the
  table of targets keeps their sizes)
- Date: 2026-11-14

## Context

M6d.5e2, the last part of the host's API
([ADR-0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)),
makes each root take its size and scale from its target, which its host gives, so that no
presentation reads the window for its interface.

The facts, read in the repository on 2026-11-14:

- `RootSettings` held each root's size in logical pixels, its scale, its direction, its origin on
  its surface, whether it is modal, and its target. Each presentation read the window's size and
  scale at each frame and wrote them into its roots: the editor, the dev overlay
  (`Overlay::set_window`), *Hunna Wood* through it, the examples; the benchmarks and the editor's
  images without a window wrote fixed ones. 108 literals in 52 files built a `RootSettings`, most in
  uwui's tests.
- uwui kept a layer's computed place in the layer's own `RootSettings::origin`, and gave each
  layer a size that folded its target's roots' boxes.
- The hit test stopped at the top root whose box held the point: a root smaller than its target,
  or placed in it, let the pointer reach those below. No program stacks two roots that are not
  layers on one target, but for the `interface` example's modal dialog; some 16 tests did.
- Placing a box again after a scroll mirrored it from right to left across the window's width,
  while its layout had mirrored it across the layer's own: a palette scrolled in a right-to-left
  window shifted its rows out of the layer.
- The window's images may be smaller than the window: `slime-gfx` limits each side to the device's
  largest. A window minimized on Windows has a size of zero, and frames still run while it is, for
  what asks from outside.
- In the engine's frame, the development channel answers agents and the game runs before the
  presentation lays out its interface in `extract`.
- ADR-0249 put "Target" in the events channel. It built "Frame" and "Root's clock", of the same
  table, as methods (`Ui::set_frame_shown`, `Ui::set_root_clock`): an event carries an answer and
  a time, and an event for a target that the interface does not know is not taken (ADR-0257).

The design was attacked by a fresh agent before it went to the owner. Its rule for stacked roots
tested what a root draws alone: a button that draws nothing, on a root over another that draws,
would have lost its press. Its scale was read as given, not as the boxes were laid out at. Its
type let a host give a scale that is not a number. It found the two mirrors that disagreed.

## Decision

The owner's decisions of 2026-11-14: the target's size given by a method; stacked roots take the
pointer where they act; the go for the one mechanical change of track A's two files. Then the
lead's within them.

**A target's size, given by a method.** `uwui::host::TargetSize` is a target's image's width and
height in physical pixels and its scale. `TargetSize::new` refuses a side of zero
(`TargetSizeError::Empty`) and a scale that is not a finite number above zero
(`TargetSizeError::Scale`), as `TextSize::factor` refuses a bad factor: nothing that a host gives
can panic. `Ui::set_target_size(target, size)` gives it; `Ui::target_size(target)` reads what the
host last gave.

- Kept in the table of targets' frames (ADR-0258), apart from the target's input: a size given
  before the target's first root is kept, without making the target known to input (ADR-0257).
- The interface keeps the size given and the size its roots were last laid out at. The frame of
  the target makes the given one the laid-out one, and lays each of its roots out again, drawn
  again whole for another scale. Between the two, what the interface reads of its boxes, the
  questions and an agent's actions, reads the scale they were laid out at.
- A size given again unchanged does nothing, and asks for no frame: a host gives it at every frame.
- Positions stay in the target's physical pixels. ADR-0249's rule "a root's size is in logical
  pixels, with the scale that the host gives" becomes: a target's size is in physical pixels, with
  its scale, which its host gives; each root on it fills it.

**`RootSettings` keeps what is the root's own:** its direction, whether it is modal, as it is
added, and its target. `RootSettings::on(target)` is a root on a target, left to right, not modal;
no default hides the target (ADR-0256). A root's own zoom, ADR-0237's density for a couch, waits
for its reader: a factor of the root's over its target's scale then.

**A layer's place kept apart.** A layer keeps the place that the interface computes for it on the
layer's own record; it lays out within its target's size, its box its content's. The size that
every layer lies within is its target's, not a fold of the roots' boxes. Laying out and placing
again after a scroll mirror across one line, the root's own box.

**Stacked roots take where they act**, the owner's choice. Every root that is no layer fills its
target. One that is neither a layer nor modal takes the point where it would take a press: where it
draws, its scroll bars included, or where an element under the point takes the pointer or is a
surface, the test that `Ui::covers` already makes. Elsewhere the point goes on to the roots below;
where none takes it, the deepest box of the top root that it went through holds it, as a lone root
holds it today. A layer takes the point wherever its box lies; a modal root everywhere on its
target. A game's HUD over its pause menu lets the menu's buttons through, without a setting.

**The edge cases.**

- Never given: a target lays its roots out at one pixel, at scale 1, as every presentation's
  placeholder did before its first frame. `target_size` answers none.
- Zero: a host whose window has no area gives nothing, `TargetSize::new` refusing it. The target
  keeps its size and its roots their layout; a window restored to the same size lays nothing out.
  A window started minimized leaves its target never given until it shows.
- Larger than the image: a host gives the size of the image that it draws into. The engine gives
  the surface's images' (`Surface::size`), smaller than the window past the device's largest side;
  a position beyond falls outside every root.
- A root moved to another target takes that target's size and scale.

**The engine.** `slime-app` gives `TargetId::MAIN` its surface's images' size and the window's
scale at each frame, before the frame's phases, when the presentation has an interface: unchanged,
it costs a comparison. A scale that `TargetSize` refuses is logged and the last kept. The editor,
the dev overlay, *Hunna Wood* and the examples read their target's size from their interface; the
overlay opens a panel asked before the size is known at its next `take_actions`. The editor's images
without a window, the examples' images and the benchmarks give their targets' sizes themselves, as
the hosts they are.

**ADR-0249's column of the engine**, the cells that change, with M6d.5e2 built:

| Row | The engine |
|---|---|
| Made, resized | `Event::Resized`, which resizes the surface; `slime-app` gives the target its images' size at each frame (`Ui::set_target_size`) |
| Scale | `Event::ScaleFactorChanged`; the run reads the window's scale at each frame and gives it with the target's size |
| Covered | the window holds its frames when hidden on Wayland, of size zero, or covered on X11 without a compositing manager and on macOS; under a compositing manager, Xwayland included, X11 reports every window visible and nothing is held; uwui is told nothing |

The other cells hold. ADR-0249's "Target" row of events becomes the method: its addition, loss,
making again and removal wait for their readers, as ADR-0264 says.

## Alternatives considered

On the shape:

- **An event, `Input::TargetResized`**: ADR-0249's letter. But an event answers taken or not, which
  means nothing here; an event for a target unknown is not taken, while its size must be kept; and a
  question asked while an event is handled sees its edits laid out, which would lay out on demand.
- **`RootSettings` keeping a size of its own, optional**: a root smaller than its target stays
  possible. Two ways to size a root, and no program reads it: a panel placed over a root is a layer.
- **`set_target_size(target, pixels, scale)` panicking on a bad scale**, as `add_root` did: the
  contract unchanged. But a host reads its scale from its environment, and a panic would follow a
  platform's bad value.
- **`RootSettings::default()`**: shorter literals; but it hides `MAIN`, which ADR-0256 refused.

On stacked roots:

- **A root takes the point where it draws alone**: simpler to state. But a button without a fill,
  most of the editor's look at rest, would lose its press to a root below that draws.
- **A setting per root that lets the pointer through**: explicit. An option that nothing reads; by
  default the top root would take every point.
- **One root per target takes the pointer, the rest layers**: the simplest rule. But a HUD and a
  menu become layers, which a root's element anchors, against the roots of their own that ADR-0251
  and ADR-0226 open for a game.

On a size of zero:

- **Laid out at zero**: what the editor did when a frame ran minimized. Its layout then collapses,
  and is laid out again whole as the window comes back.

On when the engine gives it:

- **At the window's event**: a size known between frames. Nothing lays out between frames, and the
  surface's size is set at that event already.

## Consequences

- `uwui::host` gains `TargetSize` and `TargetSizeError`; `Ui` gains `set_target_size` and
  `target_size`. `RootSettings` loses `size`, `scale` and `origin`, gains `on` and `Eq`; `add_root`
  and `set_root` no longer panic. `slime-gfx`'s `Surface` gains `size`.
- `Overlay::set_window` goes; `slime-devui` no longer depends on `slime-platform`. The editor's
  `frame`, *Hunna Wood*'s overlay and the examples' drawing lose the window's size and scale.
- No presentation reads the window's size or scale for its interface. *Hunna Wood*'s and the
  scene example's statistics show the image's size, which the device may draw smaller than the
  window.
- A right-to-left layer that scrolls keeps its rows within it.
- The tests that stacked smaller or placed roots now pad a root's content to its old place, give a
  modal layer, or use targets of their own; a modal root's empty area is now its own, not none.
- The editor's window, moved between screens of other scales, a scale changed in the system's
  settings, and minimized then restored, are checked by hand in the Windows virtual machine
  ([`PLAN.md`](../PLAN.md#checks-by-hand-waiting-for-the-owner)).
