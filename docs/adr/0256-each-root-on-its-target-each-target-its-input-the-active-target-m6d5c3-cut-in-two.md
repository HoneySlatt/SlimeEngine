# 0256. Each root on its target, each target its input and outputs, the active target: M6d.5c3 cut in two

- Status: Accepted; amends [0249](0249-the-hosts-api-its-items-and-their-table.md) (a modal root
  and a layer's rules by target),
  [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (M6d.9a1b3 follows M6d.5c3a),
  [0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) (an event for
  a target without roots; the engine's one move) and
  [0255](0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md) (the program's
  clipboard on the active target); amended by
  [0257](0257-targets-known-from-their-first-root-a-moved-root-below-its-new-targets-modal-root.md) (targets known from their first root; a root and a layer moved),
  [0258](0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md) (each
  target's own frame) and
  [0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md) (a target removed waits for its reader)
- Date: 2026-11-14

## Context

M6d.5c3's card ([`stage-6d.md`](../stage-6d.md#m6d5c3--the-targets)) gives each root its target,
and each target its hit test, pointer, hover, capture, focus, modal root, layers, display list,
frame and frame's time, and its outputs. [ADR-0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md)
named the targets and served one, `TargetId::MAIN`.
[ADR-0255](0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md) left this milestone
to say where the program's own writes and reads of the clipboard go.

The facts, read in the repository on 2026-11-14:

- `Ui::frame` is called 666 times; roots are made from 93 literal `RootSettings`; `Ui::focus` is
  called 145 times.
- What the interface keeps of a person's input, `Interaction`, holds one pointer, hover, capture,
  set of held buttons, focus, stack of modal roots, set of modifiers, rest, tooltip and drag. About
  160 accesses read or write them, and 35 walks go over every root or every layer.
- A modal root stops the hit test of every root below it. Layers are sized by every root's extent
  and closed by a press on any root.
- The interface's own layers stand over "the window's root", the first root that is no layer: the
  notifications and the palette of commands.
- Track A's `slime-app/src/channel.rs`, which ADR-0248 keeps from track B, holds a literal
  `RootSettings`, two calls of `Ui::frame` and a call of `Ui::meaning_at`.
- `slime-app` has one window. It gives every event on `TargetId::MAIN`, and holds one move of the
  pointer.

The platforms and toolkits, read on 2026-11-14 (`.delegation/m6d5c3-design-challenge/`, local;
sources below):

- Each keeps one active window and a focus in each window. Qt gives key events to its focus
  window, and a widget focused in a window that is not active takes the keys when its window
  becomes active. egui keeps one focus for each viewport.
- Modality has three reaches:
  - only the dialog's own document: the web's modal `<dialog>` and macOS's sheets;
  - the parent window: Qt's `WindowModal`;
  - every window of the application: Qt's `ApplicationModal`, GTK 4's modal windows, Win32's
    `MB_TASKMODAL`.

  On Wayland, the client itself filters the parent's events under a dialog of its own
  (`xdg-dialog`).
- The clipboard is the application's, not a window's. Yet Wayland's selection needs the serial of
  the event that triggered it, and the web writes only inside the handler of the person's event.

A challenge of a first design found the faults that the alternatives below record.

## Decision

The owner's decisions of 2026-11-14:

- the cut in two;
- a mechanical change of track A's `channel.rs`;
- a modal root that blocks its own target;
- the program's clipboard on the active target;
- the engine's one move.

Then the lead's decisions within them.

**M6d.5c3 is cut in two**, each part in a session of its own:

- M6d.5c3a, input and outputs per target. Each root on its target. Each target's hit test,
  pointer, hover, capture, focus, modal roots, layers and outputs. The active target. A display
  list for each target, from one frame.
- M6d.5c3b, frames and times per target. `Ui::frame` for one target, its frame's time, and its
  next frame due. What a frame settles narrowed to its target. Input to present measured.

M6d.9a1b3, which waits for the methods of input that an agent's action calls, may start once
M6d.5c3a is done. M6d.5c3b then edits shared files after a `git fetch`.

**Track A's file.** The owner allows the mechanical change of `channel.rs` that each part's change
of API needs:

- `target: TargetId::MAIN` in its `RootSettings`, and the target given to `meaning_at`, at
  M6d.5c3a;
- its two frames, at M6d.5c3b.

Each change goes in the commit that changes the API, after a `git fetch`. uwui keeps one way to add
a root and one way to draw a frame.

**Each root names its target.**

- `RootSettings::target`.
- A layer stands on the target of the root that it opens over, taken as it is added and again at
  each frame, as it takes that root's scale and direction.
- `Ui::set_root` with another target moves a root, and the layers over it at once. What the old
  target's input held in it lets go, as if the input had left:
  - a held button is released, a surface told so;
  - a slider, a splitter or a part of a color picker held is committed;
  - a field that held the focus commits, its input ends, and its reads of the clipboard are dropped;
  - its hover and its pressed state clear;
  - a modal root leaves the old target's stack, giving back the focus that it took, and joins the
    new target's stack.

**Each target keeps its own input, explicitly.**

- Shared by every target: the actions that wait for the program, the shortcuts and the platform.
- Kept for each target:
  - the pointer, the hover, the capture and the held buttons;
  - the focus, and whether a press gave it;
  - the modal roots;
  - the modifiers;
  - the rest, the tooltip, the drag, the floating panel or splitter held, and the surface held;
  - what the host was last told of its outputs.
- Every reader names the target that it reads: the event's, the element's, or the active
  target's. No target is "loaded".
- A target's record is made with its first root or its first event. It is kept until the host
  removes the target, at M6d.5e.
- The removal of an element clears it from every target.

**The active target.**

- It is the target of the last key or button that the interface was given: `TargetId::MAIN` until
  then, and the target that holds the system's focus from M6d.5e.
- What names no element follows it:
  - `Ui::focus`, and `Ui::set_focus(None)`;
  - the modifiers that the program reads;
  - the palette of commands and the sheet of shortcuts, opened over its first root that is no
    layer;
  - an agent's or the program's action without an element;
  - the program's clipboard;
  - the focus told with the meaning's changes, until M6d.5d gives each window its tree.
- `Ui::focus_in(target)` reads any target's focus. `Ui::set_focus(Some(id))` gives the focus on
  the element's target, without making that target active.

**The hit test, modal roots and layers, by target.**

- `Ui::element_at`, `Ui::meaning_at` and `Ui::covers` take a target, and walk that target's roots
  alone.
- A modal root stops its own target, takes its focus, and keeps the focus among its elements. The
  owner's choice: blocking a parent window, or every window, waits for its first reader, a dialog
  in a window of its own or panels torn off into windows.
- A layer is sized by its own target's roots. A press on a target closes the layers of that target
  alone; Escape, the top layer of the key's target.
- Tab without a focus, F6 and the bar of menus's Alt walk the key's target's roots.
- The notifications stand over the first root of `TargetId::MAIN` that is no layer, and read that
  target's rest.
- A layer's rules follow the target of the root that it opens over. When a layer has a target of
  its own (ADR-0249's row "A layer's own target"), they still follow that root, and the layer
  keeps its own target.

**An event for a target without roots.**

- A move or the pointer's leaving updates that target's pointer.
- A release of a button whose press the interface took is taken.
- Everything else is not taken, and changes nothing but the time.

**Outputs for each target.**

- The cursor is that target's pointer's.
- The input method is that target's focused field's.
- A field's clipboard writes and reads go to the field's target. A read is dropped when the focus
  of its field's own target leaves the field. Its answer pastes if that field still holds that
  focus, whatever target the answer comes on.
- The program's writes and reads go to the active target's outputs, the owner's choice: that is
  where the person's gesture was, whose serial Wayland asks for and whose handler the web writes
  in.

**One frame in M6d.5c3a.** `Ui::frame` lays out, draws and settles every target, as before. It
gives `TargetId::MAIN`'s display list. `Ui::display_list(target)` gathers another target's, as
the last frame drew it. M6d.5c3b gives each target its own frame.

**The engine** holds one move of the pointer, without a target, the owner's choice. A host with
several windows holds one move for each; the engine has one window.

## Alternatives considered

- **A "loaded" target**, as M6d.5a's prototype did: the interface's input holds one target's state
  at a time and parks the others, so the code that reads it changes little. The challenge found it
  wrong:
  - uwui's own inner calls, a layer closed or a picker replaced, would load another target in the
    middle of an event;
  - actions, the scope of modal roots, the meaning and the questions would read the target last
    loaded;
  - queries that take `&self` cannot load at all, which would need a second way to reach the
    parked state.

  The prototype swapped only the pointer and the hover.
- **Focus for each target, without an active target.** Every call that names no element would
  need a target: the palette, the program's clipboard, `set_focus(None)`, an agent's action.
  Desktop toolkits keep one active window for this.
- **`set_focus(None)` clearing every target's focus.** The dev overlay's "back to the game" would
  also drop a focus in another window that it did not mean to touch.
- **The target set after the root is added** (`Ui::set_target`), keeping `channel.rs` untouched. A
  modal root added first on `TargetId::MAIN` would take that target's focus.
- **The API bent around track A's file**: a second way to add a root and a second way to draw a
  frame, each with a hidden default.
- **A modal root that blocks every window**, as GTK 4 does: a game's pause menu would freeze the
  editor's other windows. **Every reach now**: nothing in Stage 6d reads it.
- **The program's clipboard on a target named at each call**: the editor would name its main
  window, wrong once its panels are torn off. **An output of the whole application**: truest to
  the platforms, but it amends ADR-0248's and ADR-0249's outputs by target, and the web and Wayland
  still want the gesture's window.
- **The engine's move held for each target**: code that no path of the engine reaches.
- **One part**, as the card read: a change about twice M6d.5c2's, and track A waiting for both
  halves.

## Consequences

- `RootSettings` gains `target`. `Ui` gains `focus_in` and `display_list`. `element_at`,
  `meaning_at` and `covers` take a target.
- Until M6d.5c3b, one frame settles every target, and one frame's time serves every root. Each
  target's spinner and next frame due wait for it. A host of two targets gathers the second
  target's display list apart from the frame.
- The program's clipboard follows the active target. A write that the program makes without a
  gesture goes to the window that the person last touched.
- A dialog in a window of its own, and panels torn off into windows, wait for a modal root that
  blocks across targets ([`debt.md`](../debt.md)).
- Each target's element in focus keeps its `focused` state, so the meaning's tree may hold one for
  each target until M6d.5d gives each window its own tree.
- The engine's input and its outputs stay on `TargetId::MAIN`, as before.

## Sources

Read on 2026-11-14.

- Qt: [`QGuiApplication::focusWindow`](https://doc.qt.io/qt-6/qguiapplication.html#focusWindow),
  [`QWidget::setFocus`](https://doc.qt.io/qt-6/qwidget.html#setFocus),
  [`Qt::WindowModality`](https://doc.qt.io/qt-6/qt.html#WindowModality-enum).
- GTK 4: [`gtk_window_set_modal`](https://docs.gtk.org/gtk4/method.Window.set_modal.html).
- egui: its `Memory`'s focus for each viewport, in [its sources](https://github.com/emilk/egui/blob/main/crates/egui/src/memory/mod.rs).
- AppKit: [`beginSheet`](https://developer.apple.com/documentation/appkit/nswindow/beginsheet(_:completionhandler:)),
  [`runModal(for:)`](https://developer.apple.com/documentation/appkit/nsapplication/runmodal(for:)).
- Win32: [`MessageBox`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messagebox).
- The web: [MDN's `HTMLDialogElement.showModal`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLDialogElement/showModal).
- Wayland: [`xdg-dialog`](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/staging/xdg-dialog/xdg-dialog-v1.xml),
  [the core protocol's data device](https://gitlab.freedesktop.org/wayland/wayland/-/raw/main/protocol/wayland.xml).
