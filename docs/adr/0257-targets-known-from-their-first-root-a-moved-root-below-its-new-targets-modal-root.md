# 0257. Targets known from their first root; a root moved stands below its new target's modal root

- Status: Accepted; amends
  [0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) (an event for a
  target that the interface does not know) and
  [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)
  (an event for a target without roots; a root and a layer moved to another target)
- Date: 2026-11-14

## Context

M6d.5c3a built [ADR-0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md).
Building it, and its closing review, showed three rules that the ADR got wrong or left out.

- ADR-0256 makes a target's record with its first root or its first event, and takes nothing on a
  target without roots but a move, a leaving and a release. Yet an interface without roots took the
  keys of its shortcuts that need no focus, on `TargetId::MAIN`, as two tests of uwui's shortcuts
  check. The rule would have given these keys to the game.
- ADR-0256 does not say where a root moved to another target stands among that target's roots.
  Roots stand in the order they were added. A root added after a modal dialog, then moved to the
  dialog's target, stood above the dialog and took the input that the dialog keeps from its window.
- ADR-0256 moves a layer to the target of the root that it opens over at each frame. A modal layer
  that the program placed over another target's root moved there without leaving the old target's
  stack of modal roots: the old target stayed blocked, the new one was not. `Ui::set_root` changed a
  layer's target until the next frame.

## Decision

The lead's, within ADR-0256's decisions.

**A target is known from its first root**, and `TargetId::MAIN` from the start, until the host
removes it at M6d.5e.

- An event for a target that the interface does not know is not taken, and changes nothing but
  the time; its outputs are empty.
- A known target whose roots are all gone keeps its input, as `TargetId::MAIN` does before its first
  root: its events reach no element, a release whose press it took is taken, and a key reaches the
  shortcuts that need no focus.
- An answer of the clipboard names its read, and is taken on any target.

**One move to another target**, for a root that `Ui::set_root` moves and for a layer placed over
another target's root, with the layers over each:

- what the old target's input held in them lets go first, as ADR-0256 says;
- then their target changes;
- then each modal root among them leaves the old target's stack and joins the new one's. The focus
  that it took goes back on the old target, which an element that left cannot take.

**A root moved without a modal root of its own stands below the new target's lowest modal root.**
A dialog keeps its window, as Qt's window-modal dialogs do.

**`Ui::set_root` keeps a layer's target.** A layer follows the root that it opens over.

## Alternatives considered

- **A record for every target that an event names**, as ADR-0256 wrote, its pointer kept before
  its first root. Keys on a target never seen would reach the shortcuts that need no focus. A host
  gives the pointer again as it moves, and the interface would keep a record for every number that
  a host names.
- **Taking nothing on a known target without roots.** `TargetId::MAIN` before its first root
  would lose the shortcuts that it took before ADR-0256.
- **A moved root on top of its new target's roots**, as a root added then: it would take the input
  over a dialog open there. **By the order of adding, as before**: whether a dialog keeps its window
  would depend on when each root was made.
- **Two moves, one for roots and one for layers**: the case of a modal layer shows that they must
  do the same.

## Consequences

- A move for a target that the interface does not know keeps no pointer: the first move after
  its first root gives it.
- `Ui::active_target` reads the active target, which ADR-0256 names.
- uwui's tests hold each rule: a key and a release on a known target without roots, a root moved
  below a modal root, a modal layer placed over another target's root, a root moved with its modal
  layer.
