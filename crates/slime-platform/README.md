# slime-platform

The engine's link to the operating system's windowing and input services.

## Status

Partial. It opens one window on Wayland or X11 and reports its window events (close requested,
resized, scale factor changed, focus). Keyboard and mouse input arrive in milestone M1.5 (see
`docs/PLAN.md`). Linux only.

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
  the main thread, and only once per process.
- `EventHandler` is what the event loop calls back: `window_created` once, then `event` for each
  event and `about_to_wait` whenever all pending events are handled, then `exiting` once.
  `about_to_wait` returns a `LoopControl`: wait until a given instant, or exit.
- `Window` is a cheap, clonable handle to the window: its size in physical pixels, its scale
  factor, and the `raw-window-handle` traits through which a graphics library draws into it.
- `Event` is what happened to the window. It is `#[non_exhaustive]`: new kinds of events will be
  added.
- `PlatformError` says why the event loop could not run; winit's own error is its `source()`.

The windowing system is Wayland when `WAYLAND_DISPLAY` points to a compositor, X11 otherwise. Its
libraries (libwayland-client, libxkbcommon, libX11, libXcursor, libXi, libxcb) are loaded at run
time, so a program that never creates an event loop runs without them.

## Dependencies

- `winit`: windows, events and input on each windowing system.
- `raw-window-handle`: the standard traits for passing a window to a graphics library.
- `thiserror`: derives `PlatformError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Depend on `slime-app` or on any crate above it: dependencies point from `slime-app` to
  `slime-platform`, never the reverse.
- Expose winit's types in its public API.
