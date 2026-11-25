# uwui-desktop

uwui's desktop host: an application on uwui alone, in a window of its own, its frames drawn on the
platform's signal and none at rest
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0271](../../docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)).
It speaks each platform itself, not through winit, through its platform layer, `uwui-windowing`:
Wayland, X11 and Windows yet.
Its name is provisional, until M6d.10a names uwui's crates; it leaves with uwui at M6d.11, under
MIT OR Apache-2.0.

## Role

- Gives an application one call, `run(look, window, app)`, which never returns
  ([ADR-0226](../../docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md)):
  the look first, its fonts, style sheets and themes; the window's settings; the application, an
  `App`, whose `start` builds its roots and may fail, whose `step` runs after the events and before
  each frame, whose `close_requested` keeps the window open or lets it close, and whose `stop`
  runs as the run ends, after a failure of the host too, so that it may keep its work; never after
  a panic, nor after a second termination signal, which ends the process at once.
- Stands on a platform layer, `uwui-windowing`, which runs the platform's loop and calls the host
  back at each turn, and knows neither the interface nor the GPU
  ([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md),
  [ADR-0302](../../docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)).
  Wayland's window, X11's and Windows' yet; its input, its times, its scale and its signal of frames are the
  layer's ([its README](../uwui-windowing/README.md)). A window whose image the platform lost, as
  X11 does with a covered one where no compositor keeps it, is drawn again.
- Holds a *core* that knows no platform, tested without a window on every platform:
  - `pacing`: nothing runs at rest; while frames run, one per refresh, each on the signal that the
    image before asked, drawn just in time where the layer tells when images show: it starts at the
    signal plus the period, less the frame's work from its turn's start and a margin that a frame
    drawn just in time widens when its image's signal says that it missed its refresh, or at once
    when that time has passed, as for the first frame after a rest
    ([ADR-0309](../../docs/adr/0309-input-to-shown-measured-beside-input-to-present-in-motion-an-input-reaches-the-next-image.md)); a window whose signal does
    not come holds its frames, an event of the window or a wake from another thread running one
    once the signal is late ([ADR-0187](../../docs/adr/0187-a-hidden-window-at-rest.md)); the time given to the
    interface predicted from the presentations that the layer tells (`FrameSource::Predicted`):
    the first refresh after the frame's usual work, on the grid of the last image shown and the
    period, plus the usual lag of recent frames that came as it does, in step with the signal or
    after a rest; where the layer tells none, on
    the grid of the signals (`FrameSource::OnGrid`), its period measured from their gaps
    ([ADR-0227](../../docs/adr/0227-the-time-that-motion-reads.md)).
  - `recovery`: the budgets within which it makes a lost device and a lost surface again, the
    engine's, adapted
    ([ADR-0264](../../docs/adr/0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md),
    [ADR-0305](../../docs/adr/0305-uwuis-desktop-host-makes-its-device-and-surface-again-a-programs-images-made-again-from-their-source.md)):
    a device replaced at once, a surface given up made again after a wait or at once when the
    window changes; past the budgets, the run ends.
  - `session`: the application's run, its interface and its window, in the order of its calls.
  - `crash`: what it keeps of the runs that did not end well, in the application's crash
    directory under the user's state directory
    ([ADR-0306](../../docs/adr/0306-uwuis-crash-reports-a-marker-held-by-a-lock-hours-counted-a-panic-ends-with-101.md)):
    a crash report written by its panic hook for a panic on the host's thread; a session's marker
    that each run holds by a file's lock and removes at its end, which the next start finds when
    the run did not end and tells; the hours of use, the runs and the crashes counted.
- A panic in any call of its layer to it, the application's code, uwui's or its own, and in
  `stop`, is caught before it leaves the call: the application is called no more and never
  dropped, the device and the window still let go of, and the process ends with `EXIT_PANICKED`
  ([ADR-0298](../../docs/adr/0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md)).
- `host`, at each turn of the layer: a closing asked or a termination signal given to the
  application; the window's size and scale given to the device; a resume told to the program,
  logged and counted as asked from outside
  ([ADR-0315](../../docs/adr/0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md));
  the person's input given to the interface, the pointer's moves gathered, each event's time the
  layer's; the application's step and its requests made of the window; a lost device replaced and
  a surface given up made again, the window holding its frames meanwhile; a frame drawn when the
  pacing says so, the platform's signal asked just before its present, and an image drawn in part
  not presented, the frame drawn again after a wait that doubles.
- Opens its own device (`gpu`) on the window's raw handles, with wgpu's hint
  `MemoryHints::MemoryUsage`, Direct3D 12 on Windows, as the engine's
  ([ADR-0317](../../docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md)),
  where `WGPU_BACKEND` names no other, its shaders compiled there by the `dxcompiler.dll` beside the
  program, which a program on the host ships, else by FXC with a warning (`instance`), the adapter that uses the least power, no optional feature, Mailbox
  where the surface offers it and Fifo elsewhere, and draws each frame's display list with
  `uwui-render` into the window's image. A lost device is replaced through the same wgpu
  instance, every object of the old one gone first.
- Writes a trace of its frames, of when each image showed, of the frames that missed their
  refresh, and of each input to present where `UWUI_TRACE` names a file
  (`trace`; its contract in `TRACE_ENV_VAR`'s rustdoc,
  [ADR-0307](../../docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md)),
  which `scripts/host-cost.py` reads.

What waits for its milestone: on Windows, the keys, focus and touch (M6d.7c2), the images'
times, a covered window and the resume's notice (M6d.7c3); macOS (M6d.7d), where `run` says that
it has no layer and ends; a bar drawn by uwui where the compositor draws none, Weston and Mutter
(M6d.7e); the clipboard, input methods and screen readers (M6d.8).
[`debt.md`](../../docs/debt.md) holds the rest.

## Entry points

- `run(look, window, app)`, its exit statuses `EXIT_HOST_FAILED`, `EXIT_START_FAILED`,
  `EXIT_PANICKED` and `EXIT_TERMINATED`; its rustdoc says where the crash directory lies and what
  it holds.
- `App`, `Close`; `Look`; `WindowSettings`, how the window opens and the application's version;
  `Window`, its state and the
  application's requests, `bring_to_front` and `request_attention` among them, and the doors of
  the tests `lose_device` and `lose_surface`.
- `examples/application.rs`: buttons, a field, a list and a dialog, at rest between inputs, which
  writes `resumed after <seconds> s` when the system resumes; with
  `--commands`, the losses, a panic and a window kept open taken on its standard input.
- `examples/blank.rs`: a *blank window* on the platform layer alone and wgpu, which clears its
  window and presents it again at each input, and nothing at rest; it writes the same trace, with
  the host's `trace.rs`.
  `scripts/host-cost.py` measures it, the same program on winit (`uwui-blank-winit`) and
  `application` in one compositor: the host's own cost is what `application` adds above it.

## Tests

`cargo test -p uwui-desktop` runs the core's tests, on every platform, without a window. The
`rest` tests, `test = false`, run the example on the display that the environment names, the Wayland
compositor of `WAYLAND_DISPLAY`, else the X server of `DISPLAY`. The first:
an image presented, in its trace too with when it showed, and at rest after its start; where the series gives input,
the pointer put on its button *Add*, whose hover draws frames, an input to present in its trace,
and at rest again; its field clicked, `s` and `a`
typed, and the row that *Add* makes named `sa`, at rest again; then ended by SIGTERM through its
closing. Weston takes no input: there the input's part does not run. The second: its device lost
on purpose, then its surface, each followed by a new swap chain's image, then at rest; two devices
more replaced, and the fourth ending the run with `EXIT_HOST_FAILED`, the application stopped. The
third: the window drawn at the scale that the compositor prefers, and the start's token of
activation passed on. The fourth: a panic in its step ending the run with `EXIT_PANICKED`, neither
`stop` called nor the application dropped, its crash report written and its marker left; the next
start finding that marker, then killed; the start after finding the killed run's, ending cleanly,
and the tally counting three runs and two crashes. The fifth: a window kept open when asked to
close, ended by a second termination signal with `EXIT_TERMINATED`. The last: the blank window's
first image in its trace, then its end on SIGTERM with status 0. Each run has a state directory
of its own, never the user's. `scripts/check.sh windowed` runs them in a headless Weston, and
`scripts/compositors.sh` the first three in Weston, sway, KWin and Mutter, the third on an output
at 1.5, at 2 in Weston, and the first two under Xvfb, where the window's images' presentations
are not told
([`compositors.md`](../../docs/compositors.md)). `scripts/check.sh lint` checks that its tree
holds no engine crate but `slime-color` and `slime-input`, that no other crate of the workspace
depends on it, that `uwui` pulls no windowing library nor wgpu, and that its platform layer
pulls neither `uwui`, `uwui-render` nor wgpu, and serves no other crate.

On Windows, the `windows` test, `test = false`, runs the examples on the desktop of the session,
the examples built first, DXC beside them, one test at a time: the application at rest after its
start, by each thread's cycles on the processor, every thread named where one runs or ends; its
title; the mouse moved onto *Add* through `SendInput`, its hover drawn, the arrow shown, an input to
present in its trace, at rest again, the I-beam over its field; its right border dragged, frames
drawn while the button is held, from within Win32's modal loop of the resize, down to its smallest
size and no smaller; Ctrl+Break during such a drag ending the run, the button still held; closed
by `WM_CLOSE`, as the close button asks, and by Ctrl+Break, each through its closing; its device
then its surface lost, each drawn again, then at rest; the blank window's first frame in its trace,
then its end with status 0 when asked to close. The resize needs Windows to show a window's content
while it is dragged: the test turns that setting on for the session where
`UWUI_TEST_SETS_DRAG_FULL_WINDOWS=1` asks, as on CI's throwaway runner, and fails elsewhere where it
is off. Each run keeps its state in a directory of its own, which `LOCALAPPDATA` names. CI's
`windows (host)` job runs it on its runner's desktop; `scripts/check.sh lint` lints it for
Windows.

## Dependencies

`uwui`, the interface it hosts; `uwui-windowing`, the platform layer below it; `uwui-render` and
wgpu, which draw it; `raw-window-handle`, the window's pointers given to wgpu; `pollster`, which
waits for wgpu's requests; `tracing`; its tests, `slime-input`'s pointers.

## Must not

- Depend on an engine crate besides `slime-color` and `slime-input`, which leave with uwui, nor be
  depended on by one; `scripts/check.sh` checks both.
- Speak a platform itself: its layer does, below it. `scripts/check.sh` checks that it names none
  of the platforms' libraries.
- Show a type of a platform's library in its public API.
- Run, poll or draw while nothing happens.
- Use `unsafe` but where it gives wgpu the window's raw handles, with its `// SAFETY:`.
