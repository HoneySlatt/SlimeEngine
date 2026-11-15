# uwui-desktop

uwui's desktop host: an application on uwui alone, in a window of its own, its frames drawn on the
platform's signal and none at rest
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0271](../../docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)).
It speaks each platform itself, not through winit: Wayland alone yet, through `wayland-client`.
Its name is provisional, until M6d.10 names uwui's crates; it leaves with uwui at M6d.11, under
MIT OR Apache-2.0.

## Role

- Gives an application one call, `run(look, window, app)`, which never returns
  ([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)):
  the look first, its fonts, style sheets and themes; the window's settings; the application, an
  `App`, whose `start` builds its roots and may fail, whose `step` runs after the events and before
  each frame, whose `close_requested` keeps the window open or lets it close, and whose `stop`
  runs as the run ends.
- Holds a *core* that knows no platform, tested without a window:
  - `pacing`: nothing runs at rest; the first frame after a rest starts at once; while frames run,
    one per refresh, each on the signal that the image before asked; a window whose signal does
    not come holds its frames, an event of the window or a wake from another thread running one
    once the signal is 100 ms late ([ADR-0187](../../docs/adr/0187-a-hidden-window-at-rest.md)); the time given to the
    interface on the grid of the signals (`FrameSource::OnGrid`), its period measured from their
    gaps.
  - `anchor`: the platform's times of its events, in another clock, placed on the host's clock by
    the fastest reception of a sliding window (`EventSource::Anchored`,
    [ADR-0227](../../docs/adr/0227-the-time-that-motion-reads.md)).
  - `wake`: the interface's waker and the termination signals, SIGTERM, SIGINT and SIGHUP, which
    wake the loop once however many come before it steps; a signal asks the application to close.
  - `session`: the application's run, its interface and its window, in the order of its calls.
- Holds Wayland's layer, on Linux (`wayland`): its loop on `rustix`, one `poll` on the
  compositor's socket, an eventfd that wakes it and a timerfd on `CLOCK_BOOTTIME`, the host's clock
  (`clock`); an xdg-shell window, its title, its app id, its minimum size and fullscreen without
  borders, its closing told to the application; the compositor's bar through xdg-decoration, where
  it gives one; frames on the compositor's frame callback, asked just before each present so that
  the commit of Vulkan's driver carries it; the pointer's moves, buttons and wheel with their
  times.
- Opens its own device (`gpu`), with wgpu's hint `MemoryHints::MemoryUsage`, the adapter that uses
  the least power, no optional feature, Mailbox where the surface offers it and Fifo elsewhere, and
  draws each frame's display list with `uwui-render` into the window's image.

What waits for its milestone: the keyboard through libxkbcommon, the cursor's shapes, the
fractional scale, activation, the presentations' times and frames drawn just in time (M6d.7a2);
X11 (M6d.7b), Windows (M6d.7c) and macOS (M6d.7d), where `run` says that it has no layer and ends;
a bar drawn by uwui where the compositor draws none, Weston and Mutter (M6d.7e); the clipboard,
input methods and screen readers (M6d.8). [`debt.md`](../../docs/debt.md) holds the rest.

## Entry points

- `run(look, window, app)`, its exit statuses `EXIT_HOST_FAILED` and `EXIT_START_FAILED`.
- `App`, `Close`; `Look`; `WindowSettings`, how the window opens; `Window`, its state and the
  application's requests.
- `examples/application.rs`: buttons, a field, a list and a dialog, at rest between inputs.

## Tests

`cargo test -p uwui-desktop` runs the core's tests, on every platform, without a window. The
`rest` test, `test = false`, runs the example in the Wayland compositor that `WAYLAND_DISPLAY`
names: at rest after its start, woken by the pointer where the series gives input and at rest
again, then ended by SIGTERM through its closing. `scripts/check.sh windowed` runs it in a headless
Weston, and `scripts/compositors.sh` in Weston, sway, KWin and Mutter
([`compositors.md`](../../docs/compositors.md)). `scripts/check.sh lint` checks that its tree
holds no engine crate but `slime-color` and `slime-input`, that no other crate of the workspace
depends on it, and that `uwui` pulls no windowing library nor wgpu.

## Dependencies

`uwui`, the interface it hosts; `uwui-render` and wgpu, which draw it; `raw-window-handle`, the
window's pointers given to wgpu; `pollster`, which waits for wgpu's requests; `slime-input`, the
pointer's buttons and wheel; `ctrlc`, the termination signals; `tracing`. On Linux,
`wayland-client` on libwayland's display, loaded at run time, `wayland-protocols`' xdg-shell and
xdg-decoration, and `rustix`, the loop's poll, eventfd and timerfd and the clocks.

## Must not

- Depend on an engine crate besides `slime-color` and `slime-input`, which leave with uwui, nor be
  depended on by one; `scripts/check.sh` checks both.
- Show a type of a platform's library in its public API.
- Run, poll or draw while nothing happens.
- Use `unsafe` but where it gives wgpu the window's raw handles, each with its `// SAFETY:`.
