# 0264. The lifecycle before each target's size; a lost surface made again within a budget; M6d.5e cut in two

- Status: Accepted; amends [0025](0025-giving-up-a-lost-surface.md) (a surface given up no longer
  ends the run at once), [0187](0187-a-hidden-window-at-rest.md) (a covered window holds its
  frames where the system says so),
  [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (M6d.5e cut in two),
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (the lifecycle's rows, the preferences'
  row and the engine's column) and
  [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)
  (a target removed waits for its reader)
- Date: 2026-11-14

## Context

M6d.5e, the last part of the host's API
([ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md)), makes `slime-app` reach uwui
through that API alone. Its card asks for: the lifecycle given to uwui, focus, suspend and resume,
occlusion and low memory, where winit gives them; the text's size and less motion given through
the API; the surface lost and made again rather than ending the run; each root's size and scale
given by the host, the presentations no longer reading the window; and ADR-0249's column of the
engine checked against the code.

The facts, read in the repository on 2026-11-14:

- `slime-app` already reaches uwui through the API for input, outputs, times, the waker and the
  screen readers. What it does not give: each root's size and scale, which each presentation reads
  from the window and writes in its roots' `RootSettings`; the window's focus, which never reaches
  uwui; its visibility.
- Giving each target its size reshapes `RootSettings`: some 106 literals in 52 files, two of them
  in track A's files, and tests whose roots are smaller than their target, which the hit test reads
  (the topmost root whose box holds the point takes it). uwui stores a layer's place in its
  `RootSettings` too. It is a change the size of M6d.5c3a.
- winit 0.30 gives `suspended`, `resumed` after the start and `memory_warning` on Android, iOS and
  the web alone; `Occluded` on X11 and macOS, not on Wayland nor Windows. The engine runs on Linux
  and Windows ([ADR-0015](0015-platforms-and-ci.md)). `slime-platform`
  translates none of them.
- Nothing in uwui reads whether a target is covered: a covered target draws the same. The engine
  holds a hidden window's frames itself ([ADR-0187](0187-a-hidden-window-at-rest.md)), and ignores
  `Occluded`: a covered window keeps drawing on X11 ([`debt.md`](../debt.md)).
- When the window loses the focus, `slime-platform` releases the keys, buttons and modifiers still
  held, then reports the loss. uwui gives its bar of menus the focus when Alt is let go alone, so
  Alt and Tab from the editor gives the bar the focus through Alt's release; a drag in progress
  drops where the pointer is, through the button's release.
- `slime-gfx` already makes a lost surface again, and gives it up at its fourth loss in a row
  without an image between, or when it cannot be created: the run then ends
  ([ADR-0025](0025-giving-up-a-lost-surface.md)). Four losses may come within a few frames. A
  device's loss has its own budget, three new devices within five minutes
  ([ADR-0075](0075-recovering-from-a-lost-gpu-device.md)); its path rebuilds every GPU object of
  the program, and refuses a surface lost for good.
- `Ui::set_preferences` is the one door of the text's size and less motion
  ([ADR-0237](0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)). No host
  of the engine reads the system's: winit 0.30 gives light or dark alone, and the reader of the
  system's preferences is uwui's first stage's
  ([ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)). The editor
  sets none in a run.
- No presentation gives another interface during a run; ADR-0262 says that `slime-app` gives one
  interface for a run. In the engine, a target goes away only as the run ends.
- Windows sends a mouse's messages for touch and the pen too, marked by `GetMessageExtraInfo`;
  winit 0.30 registers for `WM_TOUCH` and does not tell the marked messages apart.

The design was attacked by a fresh agent before it went to the owner. Its first cut put each
target's size first; it would have edited track A's files mid-milestone, and needed the second
half's focus. Its first answer to a lost surface went through the device's restore, which panics
on a device still valid, and rebuilds the program's objects. Its first focus gave uwui the loss
after the releases, too late to keep Alt's from opening the bar.

## Decision

The owner's decisions of 2026-11-14: the cut, the lifecycle first; a lost surface made again
within a budget of time; the engine rests while covered, uwui told nothing; `Ui::set_preferences`
documented as the host's door. Then the lead's within them.

**M6d.5e is cut in two**, each part in a session of its own:

- M6d.5e1, the lifecycle: the window's focus given to uwui; a lost surface made again within a
  budget; a covered window at rest; the preferences' door; what is not built, written down;
  ADR-0249's column of the engine corrected; rest and input to present measured.
- M6d.5e2, each target's size and scale given by its host: roots sized by their target, the
  presentations no longer reading the window for their interface; designed at its start, run when
  track A is between milestones, with the owner's go on track A's two files; the editor's start
  measured; M6d.5 closed.

**The window's focus, as an event.** `Input::Focus(bool)` gives a target the system's focus or takes
it. It is an event, ordered among the keys and buttons whose meaning it changes.

- Given, it makes its target the active one
  ([ADR-0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)).
  A key or a button on another target still makes that one active.
- Taken, it ends what a key or a button let go in another window would end wrongly: Alt let go
  alone no longer reaches the bar of menus, and a drag in progress is given up, as Escape gives it
  up. The pointer keeps its place, so a press that follows the focus's return, without a move,
  reaches what lies under it. The element in focus keeps the focus, and takes the keys again when
  the target is given the focus back. The active target does not change.
- Neither is taken: `Answer::NotTaken`.
- `slime-platform` reports the loss of the focus before the releases it makes, so that the
  interface hears it first. The presentation and the game see that order too.

**A lost surface is made again, within a budget of time**, the owner's choice.

- `slime-gfx` gives a surface up as before. The run no longer ends then: the window holds its
  frames, as a hidden window does (ADR-0187), and the run makes the surface again, at the next
  event that may change it (the window resized, its scale changed, the focus given, a redraw that
  the system asks), or after a wait that doubles from one second, never at every frame.
- Each surface given up, and each that cannot be made again, counts. The run ends with
  `GfxError::SurfaceLost`, as ADR-0025 says, at the fourth within five minutes, the device's rule.
- `Surface::make_again` makes a surface given up again; `Surface::lose` gives one up on purpose,
  as `Gpu::lose` loses a device, for the tests and the tools; `Context::lose_surface` reaches it
  from a presentation.
- uwui is told nothing: `uwui-render` keeps nothing of a surface, and the frame after it draws the
  whole display list, as every frame does. `Input`'s target lost and made again wait for a host
  whose platform takes the surface while the window lives, Android's.

**A covered window at rest**, the owner's choice. `slime-platform` translates winit's `Occluded`; a
covered window holds its frames, as a hidden one does (ADR-0187), and draws again once uncovered.
winit gives it on X11 and macOS; under a compositing manager, X11 reports every window visible. uwui
is told nothing: nothing in it reads it.

**What is not built**, nothing giving it where the engine runs: suspended, resumed and low memory.
Each waits for its reader: the system's notice of a resume at M6d.7
([ADR-0251](0251-the-engines-host-clock-and-its-times.md)), a host on a phone for the rest. A
target removed waits for M6d.7, the first host whose targets go during a run; ADR-0256's "at
M6d.5e" no longer holds. A presentation still gives one interface for a run, the contract that
[ADR-0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) wrote, until
a presentation that gives another reads more.

**The preferences' door**, the owner's choice. `Ui::set_preferences` is the host's API's item for
the text's size and less motion: a host gives the system's through it, and a program its user's
choice, which comes before the system's (ADR-0226). Combining the two is the reader's, at uwui's
first stage. The engine gives none until then.

**ADR-0249's column of the engine**, read against the code on 2026-11-14. The cells that changed read, with M6d.5e1 built:

| Row | The engine |
|---|---|
| Made, resized | `Event::Resized`, which resizes the surface; each presentation reads the window's size and gives its roots; M6d.5e2 gives it |
| Lost, made again | `slime-gfx` makes the surface again, gives it up at its fourth loss in a row; M6d.5e1 makes it again within a budget |
| Suspend's time | `HostClock` counts a suspend; the time suspended is logged, given to no one |
| Wake at a time | `LoopControl::WaitUntil` for `Ui::next_frame_due` |
| Root's clock | none: a root follows its target's frame's time (ADR-0251) |
| Keyboard | `Event::Key`, `ModifiersChanged`, given as `Input::Key` and `Input::Modifiers` |
| Edit commands | the editor's shortcuts, uwui's own keys; `Input::Copy`, `Cut` and `Paste` exist, no platform of the engine gives them |
| Whose point | `Ui::covers`; `Input::PointerButton`'s answer |
| Touch | none; on Windows, touch and the pen reach the engine as the mouse |
| An element's place | uwui's family for agents, cropped from the target's image |
| Composition, commit in | `Event::Ime`, given as `Input::InputMethod` |
| Deletion around the caret | `Input::DeleteAround`; no platform of the engine gives it |
| Text around the caret, out | `Ui::text_around`; no platform of the engine asks it |
| Range's rectangle | `Outputs::input_area`, `Window::set_ime_area`; `Ui::range_box` |
| Character at a point | `Ui::character_at`; no platform of the engine asks it |
| Field's purpose | `Window::set_ime_allowed`; the purpose waits for its reader (ADR-0253) |
| Clipboard | `slime_platform::Clipboard`, synchronous, behind `Outputs::clipboard_write` and `clipboard_reads`, answered by `Input::ClipboardText` |
| Focus | `Event::Focused`, given as `Input::Focus` |
| Suspended, resumed | none: winit gives neither on the engine's platforms; the system's resume at M6d.7 |
| Covered | the window holds its frames when hidden on Wayland, of size zero, or covered on X11 and macOS; uwui is told nothing |
| Low memory | none: winit gives it on phones alone |
| Light or dark, … text's size | `ColorSchemeChanged`; `Ui::set_preferences`, which the engine does not call; the reader at uwui's first stage |
| Cursor | `Outputs::cursor`, `Window::set_cursor` |

The other cells hold.

## Alternatives considered

On the cut:

- **Each target's size first**: the largest change of API done early. But track A works in `uwui`
  and `slime-app` now, and the change edits two of its files; the interface given again needed the
  second half's focus.
- **One milestone over several sessions**: one ADR and one review, against one milestone per
  session.

On a lost surface:

- **ADR-0025 unchanged**: nothing to write; against the card, and four losses within a few frames
  end the game.
- **Never given up while the window lives**: the game never ends for it; a window may stay black for
  good, without saying why.
- **Through the device's restore**: one path, already bounded. But it panics on a device still
  valid, rebuilds every GPU object of the program, *Hunna Wood* loading its zone again, and the
  surface and the device share one budget.

On a covered window:

- **Nothing read**: no code; X11 keeps drawing a covered window.
- **A visibility event in uwui too**: the card to the letter; nothing in uwui reads it, against
  ADR-0248.

On the preferences:

- **A setting in the engine**, given by the game or the editor: the editor follows a size of text;
  a feature beyond the card, which the reader replaces.
- **The system read now**, the portal on Linux and `UISettings` on Windows: the editor follows the
  system at once; uwui's first stage's work brought forward, with two dependencies.

On the focus:

- **The loss given after the releases**, as the platform orders them today: no change in
  `slime-platform`; but the loss comes too late to keep Alt's release from opening the bar.
- **As if the pointer had left**: the hover and the tooltip cleared; but on Wayland the pointer
  may stay over the window while the keyboard's focus goes and comes back, and the first press,
  with no move, would reach nothing.

## Consequences

- `uwui::host::Input` gains `Focus`. `slime-platform`'s `Event` gains `Occluded`, and reports a lost
  focus before its releases. `slime-gfx`'s `Surface` gains `make_again` and `lose`, `slime-app`'s
  `Context` `lose_surface`.
- The editor's bar of menus no longer takes the focus when the person leaves its window with Alt
  and Tab, and a drag no longer drops when the window loses the focus.
- A run ends for its surface at the fourth given up within five minutes, not at the first. While
  it has none, it holds its frames: a capture asked meanwhile waits, as for a minimized window
  ([`debt.md`](../debt.md)).
- On X11 without a compositing manager and on macOS, a covered window draws nothing. Checked by hand
  on macOS and X11 ([`PLAN.md`](../PLAN.md#checks-by-hand-waiting-for-the-owner)): a surface
  given up cannot be caused there, and is checked through `Context::lose_surface`.
- The editor follows neither the system's size of text nor its less motion until uwui's first
  stage ([`debt.md`](../debt.md)).
- [`stage-6d.md`](../stage-6d.md) holds M6d.5e1 and M6d.5e2 in place of M6d.5e's card.
