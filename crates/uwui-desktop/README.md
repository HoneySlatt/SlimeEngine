# uwui-desktop

uwui's desktop host: an application on uwui alone, in a window of its own, its frames drawn on the
platform's signal and none at rest
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0271](../../docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)).
It speaks each platform itself, not through winit, through its platform layer, `uwui-windowing`:
Wayland alone yet.
Its name is provisional, until M6d.10a names uwui's crates; it leaves with uwui at M6d.11, under
MIT OR Apache-2.0.

## Role

- Gives an application one call, `run(look, window, app)`, which never returns
  ([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)):
  the look first, its fonts, style sheets and themes; the window's settings; the application, an
  `App`, whose `start` builds its roots and may fail, whose `step` runs after the events and before
  each frame, whose `close_requested` keeps the window open or lets it close, and whose `stop`
  runs as the run ends, after a failure of the host too, so that it may keep its work.
- Stands on a platform layer, `uwui-windowing`, which runs the platform's loop and calls the host
  back at each turn, and knows neither the interface nor the GPU
  ([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md),
  [ADR-0302](../../docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)).
  Wayland's window alone yet; its input, its times, its scale and its signal of frames are the
  layer's ([its README](../uwui-windowing/README.md)).
- Holds a *core* that knows no platform, tested without a window on every platform:
  - `pacing`: nothing runs at rest; the first frame after a rest starts at once; while frames run,
    one per refresh, each on the signal that the image before asked; a window whose signal does
    not come holds its frames, an event of the window or a wake from another thread running one
    once the signal is late ([ADR-0187](../../docs/adr/0187-a-hidden-window-at-rest.md)); the time given to the
    interface on the grid of the signals (`FrameSource::OnGrid`), its period measured from their
    gaps.
  - `session`: the application's run, its interface and its window, in the order of its calls.
- `host`, at each turn of the layer: a closing asked or a termination signal given to the
  application; the window's size and scale given to the device; the person's input given to the
  interface, the pointer's moves gathered, each event's time the layer's; the application's step
  and its requests made of the window; a frame drawn when the pacing says so, the platform's
  signal asked just before its present.
- Opens its own device (`gpu`) on the window's raw handles, with wgpu's hint
  `MemoryHints::MemoryUsage`, the adapter that uses the least power, no optional feature, Mailbox
  where the surface offers it and Fifo elsewhere, and draws each frame's display list with
  `uwui-render` into the window's image.

What waits for its milestone: a lost device or surface made again (M6d.7a2b2); the crash report,
the session's marker and the rule of panics (M6d.7a2b3); the presentations' times and frames drawn
just in time (M6d.7a2c); X11 (M6d.7b), Windows (M6d.7c) and macOS (M6d.7d), where `run` says that
it has no layer and ends; a bar drawn by uwui where the compositor draws none, Weston and Mutter
(M6d.7e); the clipboard, input methods and screen readers (M6d.8).
[`debt.md`](../../docs/debt.md) holds the rest.

## Entry points

- `run(look, window, app)`, its exit statuses `EXIT_HOST_FAILED` and `EXIT_START_FAILED`.
- `App`, `Close`; `Look`; `WindowSettings`, how the window opens; `Window`, its state and the
  application's requests, `bring_to_front` and `request_attention` among them.
- `examples/application.rs`: buttons, a field, a list and a dialog, at rest between inputs.

## Tests

`cargo test -p uwui-desktop` runs the core's tests, on every platform, without a window. The
`rest` tests, `test = false`, run the example in the Wayland compositor that `WAYLAND_DISPLAY` names. The first:
an image presented and at rest after its start; where the series gives input, the pointer put on
its button *Add*, whose hover draws frames, and at rest again; its field clicked, `s` and `a`
typed, and the row that *Add* makes named `sa`, at rest again; then ended by SIGTERM through its
closing. Weston takes no input: there the input's part does not run. The second: the window drawn
at the scale that the compositor prefers, and the start's token of activation passed on.
`scripts/check.sh windowed` runs both in a headless Weston, and `scripts/compositors.sh` in Weston,
sway, KWin and Mutter, the second on an output at 1.5, at 2 in Weston
([`compositors.md`](../../docs/compositors.md)). `scripts/check.sh lint` checks that its tree
holds no engine crate but `slime-color` and `slime-input`, that no other crate of the workspace
depends on it, that `uwui` pulls no windowing library nor wgpu, and that its platform layer
pulls neither `uwui`, `uwui-render` nor wgpu, and serves no other crate.

## Dependencies

`uwui`, the interface it hosts; `uwui-windowing`, the platform layer below it; `uwui-render` and
wgpu, which draw it; `raw-window-handle`, the window's pointers given to wgpu; `pollster`, which
waits for wgpu's requests; `slime-input`, the keys, modifiers, pointers, buttons, wheel and
cursor's shapes; `tracing`.

## Must not

- Depend on an engine crate besides `slime-color` and `slime-input`, which leave with uwui, nor be
  depended on by one; `scripts/check.sh` checks both.
- Speak a platform itself: its layer does, below it.
- Show a type of a platform's library in its public API.
- Run, poll or draw while nothing happens.
- Use `unsafe` but where it gives wgpu the window's raw handles, with its `// SAFETY:`.
