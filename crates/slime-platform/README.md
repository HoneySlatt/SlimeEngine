# slime-platform

The engine's link to the operating system's windowing and input services.

## Status

Partial. It opens one window, on Wayland or X11 (Linux) and on Windows, reports its events (close
requested, resized, scale factor changed, focus, keys, mouse buttons, cursor motion, the cursor
entering and leaving, and scrolling) and its redraw requests. It does not report yet: the state of
the modifier keys as a whole, text composed by input methods (Chinese, Japanese, Korean), touch
screens, touchpad gestures, gamepads, or files dropped on the window.

## Role

- Owns every interaction with the windowing system: connecting to it, creating the window,
  running the event loop, and translating what happens into engine-owned events.
- It is the only crate that uses the windowing library (winit), and none of winit's types appears
  in its public API (`docs/adr/0011-windowing-with-winit.md`).

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
  factor, `request_redraw` to ask for a chance to draw, and the `raw-window-handle` traits through
  which a graphics library draws into it.
- `Event` is what happened to the window, or input it received. It is `#[non_exhaustive]`: new
  kinds of events will be added. Key events pair up the same way on every platform: a key still
  down when the window loses the keyboard focus is released just before `Focused(false)`, and a
  key already down when the window gains the focus is not reported until it is pressed again.
  winit differs here from one platform to the next, so the event loop keeps track of the keys
  that are down. On Windows, Print Screen gives no event: Windows reports only its release; and
  while both Shift keys are down, Windows reports no release until both are up, so the one let
  go of first is released only then, with the other.
  Mouse button events pair up too: a button still down when the window loses the keyboard focus
  is released just before `Focused(false)`, since on Windows its real release goes to another
  window, and a later real release is not reported.
- Input types: `KeyEvent` describes a key twice: by `physical` position, whatever the layout
  (`KeyCode`, named after the W3C UI Events `code` values such as `KeyQ` or `Escape`, with a few
  exceptions listed in its rustdoc), and by `logical` meaning with the current layout (`Key`: a
  character such as `"a"`, a `NamedKey` such as `Enter`, a dead key, or `Unidentified`; `KeyCode`
  has `Unidentified` too, for keys without a code the engine knows). It also gives the `text` the
  key produces and whether it is a repeat. Use the physical code for movement keys and the
  logical key for shortcuts and text. `MouseButton`, `ButtonState` and `ScrollDelta` describe the
  mouse.
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
- `raw-window-handle`: the standard traits for passing a window to a graphics library.
- `thiserror`: derives `PlatformError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Depend on `slime-app` or on any crate above it: dependencies point from `slime-app` to
  `slime-platform`, never the reverse.
- Expose winit's types in its public API.
