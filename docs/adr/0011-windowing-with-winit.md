# 0011. Windowing and input with winit, behind `slime-platform`

- Status: Accepted; amended by [0020](0020-rendering-in-the-main-loop.md) (frames on redraw requests)
- Date: 2026-10-12

## Context

The engine needs windows and keyboard and mouse input, first on Linux (Wayland, and X11 through
Xwayland or an X server), later on Windows. The roadmap left the choice open until Stage 1:
use winit behind the engine's platform layer, or talk to Wayland and X11 directly.

Doing it directly means implementing, per windowing system, keyboard layouts and text input
(xkbcommon, compose sequences, input methods), display scaling, window decorations drawn by the
client when the compositor does not draw them, cursor themes, clipboard and drag and drop, then
all of it again for Windows. None of this is what makes the engine valuable (roadmap,
principle 7).

## Decision

- `slime-platform` uses [winit](https://github.com/rust-windowing/winit) 0.30, the current stable
  release, with the Wayland and X11 backends, both loaded at run time, and client-side
  decorations for compositors that do not draw window decorations themselves (GNOME, for
  example). The decorations are built without title text (feature
  `wayland-csd-adwaita-notitle`): the text renderer depends on `ttf-parser`, which is
  unmaintained (RUSTSEC-2026-0192) and would fail the dependency checks (ADR-0006). On such
  compositors the title bar shows no text; the title still appears in task switchers.
- winit is used only inside `slime-platform`, and none of its types appears in that crate's public
  API. The engine exposes its own types: `Window`, `WindowConfig`, `PhysicalSize`, `Event`,
  `EventHandler`, `LoopControl`, `PlatformError`. winit's errors are kept as the `source()` of a
  `PlatformError` (ADR-0008).
- `Window` implements the `raw-window-handle` 0.6 traits. They are the standard interface through
  which graphics libraries (softbuffer now, wgpu in Stage 2) draw into a window created by another
  library, so this is a deliberate exception to the rule of not exposing a dependency's types.
- winit owns the event loop (ADR-0010). `slime-platform` adapts winit's callbacks to its own
  `EventHandler` trait, which `slime-app`'s windowed driver implements. The window is created when
  winit reports that the application has started (`resumed`), as winit recommends.
- Frames run when winit has delivered all pending events (`about_to_wait`), and the driver asks
  winit to wake it at the time of the next frame. This is the frame limiter of Stage 1.
- Gamepads are not covered by winit; a separate library (gilrs, for example) will be chosen when
  input mapping needs it (Stage 7).
- winit 0.31 is in beta; moving to it is a change to `slime-platform` only, done once it is
  released and stable.

It was checked on niri, before writing the sample, that a Wayland window without content is not
shown (ADR-0002): winit created the window and received its size, but the compositor did not list
it until the sample filled it with pixels.

## Alternatives considered

- **Wayland (smithay-client-toolkit) and X11 (x11rb) directly.** Full control, at the cost of
  months of work outside the engine's purpose, to be repeated for every new platform.
- **SDL3.** Mature and used by many games, with gamepad support. But it is a C library, adds a
  native build dependency, and has an event model of its own to adapt to.
- **winit 0.31 beta.** A newer API, but a pre-release dependency at the base of the engine.

## Consequences

- Wayland and X11 work today, and Windows support later is mostly a matter of testing and of the
  platform-specific details the engine adds itself.
- The windowing system's libraries are loaded at run time, so a headless run, which never creates
  an event loop, does not need them. The Nix shell puts them on the loader path; other systems
  need them installed (see `CONTRIBUTING.md`).
- winit allows one attempt at creating an event loop per process, even a failed one, so a process
  can run only one windowed application and cannot retry after failing to connect.
- Replacing winit means rewriting `slime-platform`'s event loop and translation code; no other
  crate changes.
- The examples use `softbuffer` to fill the window until the GPU layer exists (ADR-0002).
