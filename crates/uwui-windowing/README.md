# uwui-windowing

The platform layer of uwui's desktop host: a window, a person's input with its times, and the
platform's signal of frames, given to the host above it, `uwui-desktop`. It knows neither uwui's
interface nor the GPU
([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)).
It speaks each platform itself, not through winit: Wayland alone yet, through `wayland-client`
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
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
  platform's signal asked before each present.
- Gives a person's input as `Input`, in the window's physical pixels: each pointer named, the mouse
  pointer 0, each finger of a touch screen a pointer of its own, named anew at each touch; the
  wheel; the keys, modifiers and focus. Each event's time is the platform's, anchored on the host's
  clock, or its reception's (`uwui-time`).
- Holds a *core* that knows no platform, tested without a window on every platform: `anchor`, the
  platform's times of events placed on the host's clock by the fastest reception of a sliding
  window ([ADR-0227](../../docs/adr/0227-the-time-that-motion-reads.md)); `wake`, the waker that
  turns the loop from any thread and the termination signals, SIGTERM, SIGINT and SIGHUP, each
  turning it once however many come before it turns.
- Holds Wayland's layer, on Linux (`wayland`): its loop on `rustix`, one `poll` on the compositor's
  socket, an eventfd that wakes it and a timerfd on `CLOCK_BOOTTIME`, the host's clock (`clock`);
  an xdg-shell window, its bar through xdg-decoration where the compositor draws one; the frame
  callback as the signal; the pointer, `wl_touch`'s fingers and the keyboard with its repeat,
  which Wayland leaves to its clients; the cursor's shapes through `wp_cursor_shape_v1`; the scale,
  fractional through a viewport, else an integer one; activation through `xdg_activation_v1`.
- Holds the keyboard's code of libxkbcommon, Linux's (`xkb`): the compositor's keymap, its state,
  the compose tables of the person's locale, each key turned into `slime-input`'s `KeyEvent`.

What waits for its milestone: the presentations' times (M6d.7a2c); X11 (M6d.7b), Windows (M6d.7c)
and macOS (M6d.7d), where `run` says that it has no layer; a stylus, through `zwp_tablet_v2`,
with uwui's first stage; the clipboard and input methods (M6d.8).

## Entry points

`run`, `Handler`, `Window`, `Happened`, `Input` and `Opening`: the crate's rustdoc.

## Tests

`cargo test -p uwui-windowing` runs the core's tests on every platform, and on Linux the
keyboard's, on layouts that libxkbcommon compiles from its rules, and the translation of the
compositor's pointer, touch and keyboard events, without a compositor. The window itself is tested
through the host above it, by `uwui-desktop`'s `rest` tests in the series of compositors.

## Depends on

`slime-input`, the input's vocabulary; `uwui-time`, the times; `raw-window-handle`, the window's
pointers given to the host; `ctrlc`, the termination signals; `tracing`. On Linux,
`wayland-client` on libwayland's display, loaded at run time, `wayland-protocols`' xdg-shell,
xdg-decoration, the cursor's shapes, the fractional scale, the viewporter and activation;
`xkbcommon-dl`, libxkbcommon loaded at run time; and `rustix`, the loop's poll, eventfd and
timerfd, the clocks and the keymap's read.

## Must not

- Depend on `uwui`, `uwui-render`, wgpu or its GPU's libraries, nor on an engine crate but
  `slime-input`, nor be depended on by a crate but `uwui-desktop`: `scripts/check.sh` checks all
  three.
- Show a type of a platform's library in its API.
- Run, poll or wake while nothing happens.
- Use `unsafe` but where it calls libxkbcommon, each with its `// SAFETY:`.
