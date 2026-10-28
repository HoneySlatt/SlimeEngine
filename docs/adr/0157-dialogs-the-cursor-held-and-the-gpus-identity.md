# 0157. Dialogs, the cursor held and the GPU's identity

- Status: Accepted; amends [0150](0150-layers.md) (a modal layer's root is of the class `modal`,
  and a layer's root lays the backdrop that its style gives over the window below it)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided that a person opens a level through the system's
dialog, which fills in the path that an agent gives itself, through rfd 0.17
([ADR-0132](0132-the-engines-ui-framework.md) approved it); that a viewport holds the cursor and reads
the mouse's raw motion, locked on Wayland and Windows, confined, hidden and put back on X11, left free
with a word where locking fails; and that each `Gpu` gets an identity that its objects carry, so that
an object used with another `Gpu` panics (G34). Milestone M6c.1b builds them, with a scrim behind a
modal layer, before M6c.2's viewport uses the cursor held.

What the code held before this milestone:

- `slime-platform`'s window could not hold or hide the cursor, nor report the mouse's raw motion,
  which a camera that turns with the mouse needs; it opened no dialog of the system.
- A modal layer of `slime-ui` kept the input from what lies below it, but drew nothing over it: the
  window below looked as usable as before.
- The editor opened another level by its path alone: `file.open`, run by an agent or by an item of
  the recent levels' submenu.
- Each `Gpu::new` opens a wgpu instance of its own, which numbers its objects from zero: a texture
  of one `Gpu` used with another acted on that other `Gpu`'s texture of the same number, without a
  word (G34).

## Decision

**The system's dialog, on a thread of its own.** `Window::choose_file` opens rfd's dialog to choose a
file to open, as a `FileChoice` says (its title, the kinds of files that it offers, the directory
that it opens in), the window its parent, on a thread of its own, and gives the dialog's number
(`DialogId`). The answer reaches the event loop through winit's proxy, as the loop's wakes do, and
the program as `Event::FileChosen { dialog, file }`: the file chosen, or none. The loop goes on
meanwhile: the window draws, and the development channel answers. On Linux, rfd asks the desktop's
portal through D-Bus's library, which it loads as it runs (the development shell gives it); where no
portal answers, it runs `zenity`; where neither does, no dialog shows and the answer is none, as a
cancel's is. The portal's dialog is modal to the window where the compositor gives Wayland's
`xdg-foreign` protocol, which rfd's default features use. On Windows, the system's own dialog shows.
The program cannot close a dialog: only the user does. A panic of rfd on its thread, which a text
holding a NUL character makes, answers as a cancel, so that no one waits for an answer.

**The editor's `file.choose`.** A new operation, *Open a level…*, run by Ctrl+O, the File menu's
first item and the palette, opens a modal layer over the window that says to choose a level in the
system's dialog or to type its path, its field holding the focus, with Open and Cancel; the window
opens the system's dialog at the frame that shows the layer, from the open level's directory,
offering `.scene` files. The dialog's answer runs the operation that an agent runs:

- A file chosen closes the layer and runs `file.open` with its path, through the core's one handler
  of requests ([ADR-0156](0156-the-editors-window-and-its-operations.md)). A path that is not Unicode
  text is told, not opened.
- No file, half a second or more after the dialog opened, is a person's cancel: the layer closes.
- No file sooner than that, before a person could have answered, means that no dialog showed: the
  layer stays, says so, and waits for the path typed.
- The path typed, then Enter or Open, runs `file.open` with it; Escape and Cancel close the layer.
  The answer of a dialog left open after its layer closed is not heard.

Agents never meet a dialog of the system (ADR-0155): a program that a tool started, with a token,
opens none, and *Open a level…*'s layer says to type the level's path, which the agent gives its
field, or it gives `file.open` the path. An agent reaches the operation as a person does, by its
name, the menu, the palette or Ctrl+O, and meets the same layer. `--headless` refuses it as every
operation of the window.

**A backdrop behind a modal layer.** `PaintStyle` gains `backdrop`: the fill that a layer's root
lays over the whole window below it, behind its own box, under the root's opacity, transparent by
default. The display list makes it at each gathering, since the window's size, which every layer
takes, changes without the layer drawing again. A modal layer's root is of the class `modal`, which
a style sheet selects; the editor's look gives it the theme's `scrim`. A modal root that a program
adds itself covers the window already, and draws its own scrim, as the `interface` example's dialog
does.

**The cursor held, and the mouse's raw motion.** `Window::set_cursor_held` holds the cursor, hidden,
where it is, or lets it go, shown again there. On Wayland and Windows, winit locks it; on Wayland,
the position given while locked is a hint of where the cursor shows as it is let go. On X11, which
winit cannot lock, it is confined to the window and put back where it was held after each motion,
whose `CursorMoved` the program does not see. Where the windowing system refuses (on Wayland, a
compositor without the protocol that constrains the pointer; on X11, a pointer that another program
holds), the method returns `PlatformError::HoldCursor`, the cursor stays free, and the program says
so. The window lets the cursor go as it loses the focus,
before `Event::Focused(false)` reports it, as SDL's relative mode does: X11's grab and Windows' clip
would otherwise keep it from the window that the user went to. `Event::RawMotion { delta }` reports
winit's `DeviceEvent::MouseMotion`, the mouse's own motion before the system's acceleration, while
the cursor is held alone. On Windows, winit 0.30 reports a pointer that gives positions rather than
motion, as a virtual machine's tablet or a remote desktop's does, as if they were motion, and
locks it anyway: ADR-0155's word where locking fails cannot be said there, and the motion jumps.
The `window` example holds it while its right button is held, and shows
the motion summed in its title.

**Each `Gpu` with an identity.** Every device that `Gpu::new`, `Gpu::new_for_window`, `Gpu::restore`
or `Surface::restore` opens has a number that no other device of the process has had; every object
that it creates carries it, a readback included, and every use of an object with a `Gpu` (written,
read back, bound, drawn into, drawn with) checks it, panicking with the kind of object at the line
of the caller that mixed them (`#[track_caller]`). A clone of a `Gpu` shares its device and its
identity. A picking asked of a device that was lost goes with it: taken from the device that
replaced it, it panics too.

## Alternatives considered

- **rfd's asynchronous dialogs, polled by the loop**: an executor on the loop's thread, where the
  thread that waits for the blocking call is a few lines; on Windows, rfd runs its dialog on a
  thread of its own for that API too.
- **The dialog run on the loop's thread**: the window stops drawing, which a compositor reports as a
  program that does not answer, and the development channel stops answering.
- **A dialog drawn by the engine, a file browser of `slime-ui`'s**: thousands of lines, without the
  desktop's places, its portals in sandboxes, nor what screen readers know of the system's dialog;
  ADR-0132 chose rfd.
- **The layer that waits closing at every answer without a file**: where no dialog shows, it would
  close at once, and a person could open no level but those in the recent levels' submenu. **The
  layer staying at every such answer**: a person who cancels the system's dialog would cancel the
  editor's too. rfd gives no file for both a cancel and a dialog that never showed, and logs why
  alone: the time of the answer tells them apart.
- **`file.choose` refused through the development channel alone**: an agent reaches it through the
  menu, the palette and Ctrl+O too, and the dialog would show on the desktop of the session's bus,
  which an agent driving the editor in a compositor of its own cannot see nor answer.
- **The scrim as a root of its own below the modal layer**: a root counts in the window's surface,
  which would no longer shrink with the window, in agents' paths and in what screen readers read,
  and closes with the layer by hand. **The scrim as the background of the class given to the layer's
  root**: the root is the layer's box, not the window. **The scrim drawn by each program**: every
  program would draw it again, and the layers of the interface's own would lack it.
- **The cursor confined on every system**: it moves within the window, meets its edges, and shows
  again where the motion left it, not where it was held; Wayland's and Windows' lock is the
  systems' own answer.
- **The raw motion reported whenever the window has the focus**: a camera that read it without
  holding the cursor would turn as the cursor crosses the window toward a panel, and every motion
  would wake a program that runs its frames on demand twice, by `CursorMoved` and by the motion.
- **The program alone letting the cursor go as the window loses the focus**: a program that forgets
  keeps the user's cursor hidden and trapped.
- **One wgpu instance shared by every `Gpu`**, so that wgpu finds the mixed object: every `Gpu`
  would reach a process-wide instance, against the instance that a lost device's replacement and a
  window's surface share today, and wgpu would report the mix to the device's handler of errors, not
  at the line that made it.
- **The check in debug builds alone**: the check is a comparison of two numbers at each use, and a
  mix that it lets through draws another object without a word.

## Consequences

- A person opens any level with Ctrl+O; an agent opens it by its path, as before.
- On Linux, the dialog needs D-Bus's library and a portal or `zenity` at run time; without them, the
  person types the path. Tests cannot show a dialog of the system: what the window does with an
  answer is tested with answers given by number, an agent's session opens a level through the
  layer, its path typed, and the dialog itself is checked by hand on niri, Hyprland and Windows.
- The cursor held and the raw motion are checked by hand on niri, Hyprland, X11 through XWayland and
  Windows, with the `window` example; tests read the motion from the platform's events. In a
  Windows virtual machine whose pointer is a tablet, the motion jumps, as winit 0.30 gives it.
- M6c.2's viewport holds the cursor while a button is held, and turns its camera with the raw motion.
- Every GPU object is 8 bytes larger, and each of its uses compares two numbers.
- Saving a level under another name, M6c.6's, opens rfd's dialog to save the same way.
