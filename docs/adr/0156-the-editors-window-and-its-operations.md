# 0156. The editor's window and its operations

- Status: Accepted; amends [0155](0155-the-editors-design.md) (an operation's shortcut lives in
  the editor's table of shortcuts; the cache directory waits for its first user; the loop's wake is
  built on a user event of winit 0.30's, the one way it has),
  [0135](0135-the-editors-program.md) (the program opens a window, and its run without a window
  waits for requests), [0021](0021-frame-pacing.md) (a program may run its frames on demand) and
  [0143](0143-what-agents-see-of-the-interface.md) (roots are named in paths by their keys)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided the editor's design: the window a presentation that
reads the core through a value they share and asks it through typed requests (D1), every action a
named operation in one registry (D2), frames on demand (D4), quitting at once (D11). Milestone
M6c.1a builds the window and its operations; the owner split M6c.1 in two at its start and took
three decisions on 2026-10-28: the window's center stays empty, with a word, until M6c.2's
viewport; a level is opened from the window, by a path, with the recent levels in the File menu,
and saving under another name waits for M6c.6, where a copy of a level gets an asset ID and
identities of its own; and the program's run without a window runs its frames on demand too.

What the code held before this milestone:

- The editor's program ran without a window, its frames paced at 60 per second
  (`AppConfig::pace_headless`), and answered the development channel's `Edit` requests in
  `Simulation::edit`.
- `slime-app` ran a windowed program's frames on a schedule, waking at each one even when nothing
  changed; nothing woke the loop from another thread but what screen readers ask.
- `slime-ui` gave the program `ActionKind::Command` for a shortcut's chord and the palette's
  choice, but a menu's item gave only a press of its element. Its palette showed 12 commands at
  most. A root was named in an agent's path by its place among the roots, which a raised floating
  panel or a closed layer changes. `FrameStats::animating` said that the interface needed another
  frame, and nothing read it.
- The window's title could not change once the window existed.

## Decision

**The core and the window share a value, and the window asks through typed requests.** The editor's
program builds its simulation, the core's session, and its presentation, the window, around one
`Rc<RefCell<Shared>>`: `slime-app` runs both on one thread, and a simulation that moves to another
thread would need `Send`, which the compiler would then ask for. The core writes what the window
shows, the level's file, the steps done and undone, how many steps are not written, the selection,
the recent levels and the notices to show; the window reads it, and pushes `Request`s into it,
which the core drains at its next update, each through the one handler that also answers the
development channel's `Edit` requests, converted into the same `Request`s. A request's failure
reaches the window as a notice, and an agent as the reply's failure. The window asks for a frame
whenever it pushes a request, so that the core answers it at once.

**Operations.** The registry, in `slime-editor`'s core, is a table of every operation: its name,
`file.save`, its label, `Save`, and its parameters, each a name and what it holds, `path`. Each
operation is of one kind: one that changes the level (a step), one that writes or opens a file,
one that changes the selection, or one that changes the window alone; the last two never enter the
history. The core runs every operation by its name and its arguments, given as text; the window's
operations it hands to the window through the shared value, and refuses without a window. The
menus, the palette, the shortcuts, the notifications' buttons and agents name operations:

- An operation's shortcut lives in the editor's table of shortcuts, `editor.keys`, which the
  window gives its interface, and from which the menus and the palette show it
  (`Ui::command_chord`): one home, where ADR-0155 listed the shortcut among the registry's fields.
- A menu is a list of operations, data that the window builds its items from, each item's
  element mapped to its operation. The recent levels' items run `file.open` with their path.
- The palette lists the operations without parameters. One with a parameter waits for what fills
  it: `file.open` comes to a person with M6c.1b's dialog, and to agents and the recent levels now.
- A test finds every operation of the menus and every command of `editor.keys` in the registry.

The development channel's protocol goes to version 10: `Edit::Operations` lists the registry,
`Edit::Run` runs an operation by its name with its arguments. The MCP server gains two tools,
`list_operations` and `run_operation`, so that an operation added to the registry reaches agents
with no new tool.

**Frames on demand.** `AppConfig::on_demand` replaces `pace_headless`, whose only user was the
editor: with it, a run, windowed or not, runs a frame only when something asks for one, and paces
the frames asked one after another at `max_frame_rate`. What asks:

- an event of the window, which a redraw that the system asks for is too;
- a request of the development channel, whose listener wakes the loop as it queues one, and its
  work that waits for a frame, a capture or steps;
- the presentation's interface: while `Ui::needs_frame` holds, a transition, a progress bar
  without a share, or a list whose rows its builder must build, each frame; and once the time
  that `Ui::next_change` gives has passed, a tooltip's or a submenu's rest, a notification that
  ends, which need no frame meanwhile; the interface's clock moves on as each event comes, so that
  what an event starts after a long wait starts then;
- the program, through `Context::request_frame`, and from another thread through a `Waker`
  (`Context::waker`), which a load or a cook that ends will use;
- a termination signal and the end of the tool that started the program, which stop it.

The loop waits without a deadline when nothing asks, or until the interface's next change. With a window, `slime-platform` gives the
loop a `Waker` built on winit's event loop proxy with a user event of its own, the one way that
winit 0.30 has to wake its loop; winit 0.31's `wake_up` replaces it when the engine moves to it.
Without a window, the loop waits on a condition variable that the `Waker` notifies. Games keep
their scheduled frames.

**The window.** A presentation of the editor's program, on `slime-ui` in the editor's look:

- A menu bar, File (the recent levels, Save, Quit), Edit (Undo, Redo, Select nothing) and Help
  (the palette, the shortcuts); the palette at Ctrl+Shift+P, the sheet of shortcuts at F1.
- Fixed regions: on the left, the dev overlay's world panel, as the hierarchy and the inspector
  until M6c.5, built into a slot of the region (`WorldPanel::build`); in the center, a region that
  says that there is no viewport yet, which M6c.2 fills; a status bar below, which names the level's file, how
  many steps are not written and how many entities are selected. The world panel's changes are
  `Request`s, each a step; its selection and the core's follow each other.
- The window's title names the level's file, how many steps are not written, and the program:
  `slime-platform`'s `Window::set_title` changes it as it changes.
- Closing the window quits at once (D11): the journal keeps what is not written, which the next
  start recovers.
- How many steps are not written is the distance in the history between the level as it is and
  as it was last written: the steps done since, the steps undone below it, and, once a step is
  made after undoing below it, both sides of the fork.
- The interface draws only in frames that run, which run on demand.

**Roots are named by their keys.** `Ui::add_root` and `Ui::open_layer` take a key, which names the
root in an agent's path, `editor/window/menus/File`, in place of its number among the roots, which
a raised floating panel or a closed layer changed. The interface's own layers are `palette`,
`shortcuts`, `tooltip`, `notifications`, `choices` and `colors`; the dev overlay names a panel's
layer by the command that opens it. A key that an open root holds already gets a variant, `menu-2`,
so that two roots never share one, and an interface whose program opens the same layer twice goes
on. The palette's list scrolls past 12 commands within most of the window's height, the chosen
one kept in sight.

**Opening a level, the recent levels and the first run.** `file.open` opens another level in the
same window: the level open is unloaded, its journal keeping what is not written, and the new
level opened, as the program opens its first, recovering its journal if it holds steps that the
file does not. The program's level opens at once at the first run. The recent levels, the last 10
opened, are kept in `slime-editor/<program>.recent` in the user's state directory, which
`slime-app` gains (`state_directory`: `$XDG_STATE_HOME`, else `$HOME/.local/state`, and
`%LOCALAPPDATA%` on Windows); the cache directory waits for its first user, M6c.7's thumbnails. A
first run, which no file of recent levels shows, gives one notice: the palette finds every
operation by its name. A program whose cooked data is missing and which names its sources cooks
them first: the `editor` example cooks the examples' assets.

**The program.** `slime_editor::run` opens the window, and `--headless` runs without it, as CI's
agents drive it; both run their frames on demand. `slime_editor::window_image` draws the window
into an image without a window of the system, as it shows once the level is open: the image
tests compare it at scales 1 and 2, and the `editor` example's `--output` writes it.

## Alternatives considered

- **`Arc<Mutex<Shared>>`** for the shared value: what a simulation on another thread needs, which
  nothing runs today; it would make every read lock for nothing, and the compiler says when it is
  needed.
- **The window calling the core directly** rather than through a queue of requests: the window
  holds no access to the simulation in `slime-app`, and a queue is what the channel's requests
  already are; one handler answers both.
- **The world panel's changes through `Context::send`**, as the dev overlay sends them in a game,
  reaching the core as `Simulation::apply`: a second path into the core, which D1 asks to avoid.
- **The shortcut in the registry**, as ADR-0155 listed it, beside the table of shortcuts that binds
  the keys: two homes for one fact, which drift.
- **A frame on a deadline while waiting**, as the windowed loop did, so that a signal is noticed:
  the signal's handler and the stdin's watcher wake the loop instead, and an idle editor runs no
  frame.
- **Naming roots by a number that does not change**, their order of creation: stable, but a path
  that a person or an agent reads says nothing, where a key says what the root is.
- **Panicking on a key that an open root holds**: it finds a program's mistake at once, but a
  menu opened while another closes, in an order that the user's input decides, would end the
  program.
- **Counting a tooltip's rest and a notification's time with frames**, as a game's interface
  does, every frame of it: an idle editor that shows a notification would draw for its five
  seconds; a deadline wakes it once.
- **Keeping the cache directory beside the state directory now**, as ADR-0155 said: nothing uses
  it before the thumbnails.

## Consequences

- A person and an agent run the same operations, by the same names, through the same handler; a
  new operation reaches the menus, the palette and agents at once.
- An idle editor runs no frame, with a window or without; a test checks that it runs none for
  10 s. A program that turns `on_demand` on must ask for the frames that its own work needs, which
  `Context::request_frame` and `Waker` give.
- Every program and test that adds a root names it.
- `editor.keys` is the editor's table of shortcuts; a game's editor package shares it.
- Saving under another name, a dialog to open a level, the cursor held and the mouse's raw motion
  come with M6c.1b and M6c.6.
