# slime-platform

The engine's link to the operating system's windowing and input services.

## Status

Partial. It opens one window, on Wayland or X11 (Linux) and on Windows, reports its events (close
requested, resized, scale factor changed, focus, keys, the modifier keys held, mouse buttons, cursor
motion, the cursor entering and leaving, scrolling, and the text that input methods compose, for
Chinese, Japanese or Korean) and its redraw requests; it allows and places the input method, sets
the pointer's shape, and reads and writes the system's clipboard of text
(`docs/adr/0145-text-edited.md`). With its `accessibility` feature, the window holds AccessKit's
adapter, through which screen readers read what it shows (`docs/adr/0146-screen-readers.md`). It
does not report yet: touch screens, touchpad gestures, gamepads, or files dropped on the window.

## Role

- Owns every interaction with the windowing system: connecting to it, creating the window,
  running the event loop, and translating what happens into engine-owned events.
- It is the only crate that uses the windowing library (winit), and none of winit's types appears
  in its public API (`docs/adr/0011-windowing-with-winit.md`).
- With its `accessibility` feature, holds AccessKit's adapter of the window, the only crate that
  uses it: AT-SPI on Linux, through D-Bus, and UI Automation on Windows. AccessKit's tree and the
  actions that screen readers ask cross its API, which `slime-app` translates the interface's
  meaning into and out of (`docs/adr/0146-screen-readers.md`).

## Entry points

Applications normally do not call this crate directly: `slime-app` drives it when
`AppConfig::window` is set, and passes the window and the events to the application.

- `run_event_loop(&config, &mut handler)` connects to the windowing system, creates the window
  described by a `WindowConfig`, and runs the event loop until the handler ends it. It must run on
  the main thread (it panics otherwise), and only once per process: the windowing library allows
  one attempt per process, even a failed one. It panics on an invalid configuration (a zero size,
  a size above 16 384 logical pixels, a title with a NUL byte) before touching the windowing
  system.
- `WindowConfig`: the window's title (`"SlimeEngine"` by default) and the requested width and
  height of its content area, in logical pixels (1280 × 720 by default); compositors may choose
  another size. On X11, the size in physical pixels, the logical size times the scale factor, is
  limited to 65 535 per side, the most the X11 protocol allows.
- `EventHandler` is what the event loop calls back: `window_created` once, then `event` for each
  event, `redraw_requested` when the window should be drawn, and `about_to_wait` whenever all
  pending events are handled, then `exiting` once. `about_to_wait` returns a `LoopControl`: wait
  until a given instant, or exit. On Windows, while the user resizes or moves the window, only
  `event` and `redraw_requested` are called, because the system runs a loop of its own.
- `Window` is a cheap, clonable handle to the window: its size in physical pixels, its scale
  factor, `request_redraw` to ask for a chance to draw, `set_ime_allowed` and `set_ime_area` for
  the input method, `set_cursor` for the pointer's shape, and the `raw-window-handle` traits
  through which a graphics library draws into it.
- With the `accessibility` feature, the window is made hidden, AccessKit's adapter is made for it,
  then it shows, so that screen readers see it from the start; Wayland, which shows a window with
  its first frame, ignores the first step. A screen reader's requests arrive as
  `Event::Accessibility` (`Accessibility`): the whole tree asked for, an action on one of its
  nodes, or no screen reader any more. `Window::update_accessibility` gives the adapter an update
  of the tree, which the event loop applies once the handler's call returns, on its own thread,
  where the adapter stays. Without a screen reader, the adapter does nothing; without a session
  bus, as in CI, it does not connect.
- `Clipboard` reads and writes the system's clipboard of text, through arboard: Wayland's
  protocol of the clipboard's managers where the compositor gives it, X11's otherwise, and
  Windows' own.
- `Event` is what happened to the window, or input it received. It is `#[non_exhaustive]`: new
  kinds of events will be added. Key events pair up the same way on every platform: a key still
  down when the window loses the keyboard focus is released just before `Focused(false)`, and a
  key already down when the window gains the focus is not reported until it is pressed again. On
  X11 and Windows, "already down" is judged when the event loop handles the focus gain, which on
  X11 can come after the application has initialized, so a key pressed in between is not
  reported either.
  winit differs here from one platform to the next, so the event loop keeps track of the keys
  that are down. On Windows, Print Screen gives no event: Windows reports only its release; and
  while both Shift keys are down, Windows reports no release until both are up, so the one let
  go of first is released only then, with the other (Windows 11 reports the second Shift's press
  as an auto-repeat, so only the first Shift's press and release are reported).
  Mouse button events pair up too: a button still down when the window loses the keyboard focus
  is released just before `Focused(false)`, since on Windows its real release goes to another
  window, and a later real release is not reported.
  `ModifiersChanged` reports the modifier keys held as a whole (`Modifiers`: Shift, Control, Alt,
  Super), once per change: those held when the window gains the focus are reported, and those still
  held when it loses the focus are reported released, before the keys. A shortcut such as Ctrl+S
  is a press of S while the modifiers last reported hold Control.
- Input types, which `slime-input` holds and this crate re-exports and translates winit's events
  into (`docs/adr/0142-input-and-focus.md`): `KeyEvent` describes a key twice: by `physical`
  position, whatever the layout (`KeyCode`, named after the W3C UI Events `code` values such as
  `KeyQ` or `Escape`, with a few exceptions listed in its rustdoc), and by `logical` meaning with
  the current layout (`Key`: a character such as `"a"`, a `NamedKey` such as `Enter`, a dead key, or
  `Unidentified`; `KeyCode` has `Unidentified` too, for keys without a code the engine knows). It
  also gives the `text` the key produces and whether it is a repeat. Use the physical code for
  movement keys and the logical key for shortcuts and text. The press of a key right after a dead
  key reports what the two keys make together (on Wayland and X11, `Unidentified` and no text when
  they do not combine) and its release does not, so a press and its release can have different
  logical values: track which keys are down by physical code. `KeyCode::name` and
  `KeyCode::from_name` give a key's code as text and back, as tools name keys. `MouseButton`,
  `ButtonState` and `ScrollDelta` describe the mouse.
- `Input` is the input as data: the keys and the mouse buttons held, and those pressed and released
  since the last frame (`Buttons`), which `Input::handle` follows from the events and
  `Input::end_frame` ends, or that a test presses and releases itself. `slime-app`'s host keeps it
  for a game's simulation (`docs/adr/0107-the-execution-model.md`).
- `PlatformError` says why the event loop could not run: `Connect` (no windowing system can be
  reached), `SecondEventLoop` (one was already requested in this process), `CreateWindow` or
  `EventLoop`; winit's own error is its `source()`. When the window could not be created, no
  `EventHandler` method is called. A Wayland compositor that dies while the loop runs ends it
  cleanly (`exiting` is called) with `PlatformError::EventLoop`, whose source says only
  `Exit Failure: <code>`; the cause appears in lines that the Wayland library prints to standard
  error itself. An X server that dies or closes the connection ends the process instead: the X11
  library (Xlib) prints that the X connection is broken and exits with status 1, so neither
  `exiting` nor anything after it runs.

On Linux, the windowing system is chosen from the environment, with no fallback from one to the
other: Wayland if `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set to a non-empty value, otherwise X11
if `DISPLAY` is. A `WAYLAND_SOCKET` that does not name an open file descriptor aborts debug builds,
since the Wayland library takes ownership of the descriptor before checking it, and gives
`PlatformError::Connect` in release builds. Its libraries are loaded at run time, so a program that
never creates an event loop runs without them: libxkbcommon for both; libwayland-client for
Wayland; libX11, libX11-xcb, libXcursor, libXi, libxcb and libxkbcommon-x11 for X11. Without the
Wayland or X11 libraries, `run_event_loop` returns `PlatformError::Connect`; without libxkbcommon,
or libxkbcommon-x11 on X11, the windowing library panics instead, on Wayland only when the
compositor first reports a keyboard, which can happen while the loop runs.

## Dependencies

- `winit`: windows, events and input on each windowing system.
- `arboard`: the system's clipboard, without its images, with its Wayland feature.
- `accesskit` and `accesskit_winit`, with the `accessibility` feature only: the tree that screen
  readers read, and its adapter for winit's windows.
- `slime-input`: the engine's vocabulary of input, which the events carry.
- `raw-window-handle`: the standard traits for passing a window to a graphics library.
- `thiserror`: derives `PlatformError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Depend on `slime-app` or on any crate above it: dependencies point from `slime-app` to
  `slime-platform`, never the reverse.
- Expose winit's types in its public API.
