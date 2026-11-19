# uwui-windowing

The platform layer of uwui's desktop host: a window, a person's input with its times, and the
platform's signal of frames, given to the host above it, `uwui-desktop`. It knows neither uwui's
interface nor the GPU
([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)).
It speaks each platform itself, not through winit: Wayland through `wayland-client`, and X11
through `x11rb`
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0311](../../docs/adr/0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md)).
Its name is provisional, until M6d.10a names uwui's crates; it leaves with uwui at M6d.11, under
MIT OR Apache-2.0, and becomes a crate that others use after uwui's 1.0, when the engine moves
onto it.

## Role

- Runs the platform's loop and calls the host back
  ([ADR-0302](../../docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)):
  `run(opening, handler)` opens the window, calls `Handler::opened` with its raw handles once the
  platform sized it, `Handler::turn` at each turn with what `Happened`, and `Handler::closing` as
  the window closes; the host answers each turn with a `Control`, when to call it again, and makes
  its requests on the `Window`: its title, minimum size, fullscreen, activation, cursor, and the
  platform's signal asked before each present, which numbers the image; `Happened` tells when
  each image showed (`Presentation`), with the display's period, where the platform says it.
- Gives a person's input as `Input`, in the window's physical pixels: each pointer named, the mouse
  pointer 0, each finger of a touch screen a pointer of its own, named anew at each touch; the
  wheel; the keys, modifiers and focus. Each event's time is the platform's, anchored on the host's
  clock, or its reception's (`uwui-time`).
- Holds a *core* that knows no platform, tested without a window on every platform: `running`,
  the run that every layer shares, which drops the host before the window, so that what it made
  on the window's handles goes first, and calls `Handler::closing` however the run ends but by a
  panic, checked on a fake platform; `anchor`, the
  platform's times of events placed on the host's clock by the fastest reception of a sliding
  window ([ADR-0227](../../docs/adr/0227-the-time-that-motion-reads.md)); `wake`, the waker that
  turns the loop from any thread and the termination signals, SIGTERM, SIGINT and SIGHUP, each
  turning it once however many come before it turns; a second signal, half a second or more after
  the first, ends the process at once with `EXIT_TERMINATED`, for a host that no longer ends
  ([ADR-0306](../../docs/adr/0306-uwuis-crash-reports-a-marker-held-by-a-lock-hours-counted-a-panic-ends-with-101.md)).
- Chooses its layer on Linux as it starts: Wayland's where `WAYLAND_DISPLAY` or `WAYLAND_SOCKET`
  names a compositor that opens the window, else X11's where `DISPLAY` names a server.
- Holds Wayland's layer, on Linux (`wayland`): its loop on `rustix`, one `poll` on the compositor's
  socket, an eventfd that wakes it and a timerfd on `CLOCK_BOOTTIME`, the host's clock (`wait`,
  `clock`), which X11's layer shares;
  an xdg-shell window, its bar through xdg-decoration where the compositor draws one; the frame
  callback as the signal; when each image showed, through `wp_presentation`, its time placed on
  the host's clock from the compositor's (`clock`); the pointer, `wl_touch`'s fingers and the keyboard with its repeat,
  which Wayland leaves to its clients; the cursor's shapes through `wp_cursor_shape_v1`; the scale,
  fractional through a viewport, else an integer one; activation through `xdg_activation_v1`.
- Holds X11's layer, on Linux (`x11`): two connections of libxcb, its own, which makes the window
  and reads every event, and Vulkan's driver's, which the raw handle names and the loop never
  reads, so that no other thread drains the socket that it waits on; the window and its requests
  through ICCCM's and the extended window manager hints' properties and messages; its frames on
  Present's notices of the window's CRTC, or a timer of its own at the display's rate where the
  server gives no Present; when each image showed, where the driver presents through Present; the
  display's period from RandR; the signal held while the window is unmapped or covered whole, and
  a frame asked when the server says that it lost the window's image; the pointer and its wheel
  through XInput2, with the server's times.
- Holds the keyboard's code of libxkbcommon, Linux's (`xkb`): the compositor's keymap, its state,
  the compose tables of the person's locale, each key turned into `slime-input`'s `KeyEvent`.

What waits for its milestone: X11's keyboard, cursor, scale and touch (M6d.7b2); the system's
notice of a resume on Linux (M6d.7b3); Windows (M6d.7c) and macOS (M6d.7d), where `run` says that
it has no layer; a stylus, through `zwp_tablet_v2`,
with uwui's first stage; the clipboard and input methods (M6d.8).

## Entry points

`run`, `Handler`, `Window`, `Happened`, `Input` and `Opening`: the crate's rustdoc.

## Tests

`cargo test -p uwui-windowing` runs the core's tests on every platform, and on Linux the
keyboard's, on layouts that libxkbcommon compiles from its rules, and the translation of the
compositor's pointer, touch and keyboard events, without a compositor; X11's pointer, its wheel,
its frames' signal and the display's period, without a server. The window itself is tested
through the host above it, by `uwui-desktop`'s `rest` tests in the series of compositors, X11's
under Xvfb. There too, `cargo test -p uwui-windowing --test x11 -- --test-threads=1`, which needs
the X server that `DISPLAY` names, watches X11's window as another client sees it: its fullscreen
hint without a window manager, its pings, its start's end, its signal held while hidden, and the
fallback from a Wayland compositor that does not answer.

## Depends on

`slime-input`, the input's vocabulary; `uwui-time`, the times; `raw-window-handle`, the window's
pointers given to the host; `ctrlc`, the termination signals; `tracing`. On Linux,
`wayland-client` on libwayland's display, loaded at run time, `wayland-protocols`' xdg-shell,
xdg-decoration, the cursor's shapes, the fractional scale, the viewporter and activation;
`xkbcommon-dl`, libxkbcommon loaded at run time; `x11rb` on libxcb loaded at run time, with
XInput2, RandR and Present; and `rustix`, the loop's poll, eventfd and timerfd, the clocks, the
keymap's read and the machine's name.

## Must not

- Depend on `uwui`, `uwui-render`, wgpu or its GPU's libraries, nor on an engine crate but
  `slime-input`, nor be depended on by a crate but `uwui-desktop`: `scripts/check.sh` checks all
  three.
- Show a type of a platform's library in its API.
- Run, poll or wake while nothing happens.
- Use `unsafe` but where it calls libxkbcommon, each with its `// SAFETY:`; libxcb is reached
  through `x11rb`'s safe interface.
