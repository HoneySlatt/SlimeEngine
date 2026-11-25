# uwui-windowing

The platform layer of uwui's desktop host: a window, a person's input with its times, and the
platform's signal of frames, given to the host above it, `uwui-desktop`. It knows neither uwui's
interface nor the GPU
([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)).
It speaks each platform itself, not through winit: Wayland through `wayland-client`, X11
through `x11rb`, and Win32 through `windows-sys`
([ADR-0270](../../docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0311](../../docs/adr/0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md),
[ADR-0317](../../docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md)).
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
  A platform whose system holds the thread in a loop of its own, as Win32 while the person moves or
  resizes a window, turns the host from within its wait.
- Chooses its layer on Linux as it starts: Wayland's where `WAYLAND_DISPLAY` or `WAYLAND_SOCKET`
  names a compositor that opens the window, else X11's where `DISPLAY` names a server.
- Holds Wayland's layer, on Linux (`wayland`): its loop on `rustix`, one `poll` on the compositor's
  socket, an eventfd that wakes it, a timerfd on `CLOCK_BOOTTIME`, the host's clock, and the
  kernel's notice of a resume, a timerfd that the kernel cancels as the system resumes or its clock
  is set (`wait`, `clock`), which X11's layer shares; a resume told once in `Happened::resumed`,
  with the time suspended, measured from the host's clock against one that stops in a suspend
  ([ADR-0315](../../docs/adr/0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md));
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
  a frame asked when the server says that it lost the window's image; the pointer and its wheel,
  touch from XInput 2.2 and the keys through XInput2, with the server's times, and the server's
  own repeat; the keymap read through libxkbcommon-x11 and followed through XKB's notices, as the
  modifiers, these while the window holds the focus, each key translated in its own event's state;
  the focus, the window's own, or the server's above it, following the pointer or on an ancestor,
  while the pointer lies over the window, read from the server at each focus event, and lost while
  another client grabs the keyboard; the
  cursor's shapes from the person's theme, else the core cursor font; the scale from the X
  resources' `Xft.dpi`, read again as the window takes the focus; the loop turned only for events
  that tell the host something
  ([ADR-0313](../../docs/adr/0313-x11s-keyboard-through-xinput2-and-libxkbcommon-x11-its-cursor-themed-its-scale-from-xft-dpi.md)).
- Holds Windows' layer (`windows`): a Win32 window of its own class, aware of each display's scale
  (per-monitor awareness, its second version), opened at its logical size times that scale, which
  follows the display that holds it; its title, smallest size, filling its display without borders,
  brought to the front or flashed; its closing told to the host; the loop on Win32's messages,
  where a wake, the deadline and the signal each come as a message of the layer's own, so that
  Win32's modal loop of a move, a resize or a menu turns the host too, frames going on during a
  drag; frames on the vertical blank of the display that holds the window, waited for through the
  display driver's `D3DKMTWaitForVerticalBlankEvent` on a thread of its own, only while the host
  asks, a timer at the display's period where the wait fails or returns at once, as with the display
  off; the signal held while the window is minimized; the mouse's moves, buttons and wheel with
  their messages' times, its capture while a button is held, its leaving; the system's cursors;
  the host's clock, the performance counter, which counts sleep, against the unbiased interrupt
  time, a resume told at the first turn after it. Its keys, focus and touch come at M6d.7c2; the
  images' times, a covered window and the resume's own notice at M6d.7c3.
- Holds the keyboard's code of libxkbcommon, Linux's (`xkb`): the platform's keymap, as a text on
  Wayland or read from the X server, its state, the compose tables of the person's locale, each
  key turned into `slime-input`'s `KeyEvent`.

What waits for its milestone: Windows' keyboard and touch (M6d.7c2), its images' times, a
covered window and the resume's notice (M6d.7c3); macOS (M6d.7d), where `run` says that it has no
layer; a stylus, through `zwp_tablet_v2`,
with uwui's first stage; the clipboard and input methods (M6d.8).

## Entry points

`run`, `Handler`, `Window`, `Happened`, `Input` and `Opening`: the crate's rustdoc.

## Tests

`cargo test -p uwui-windowing` runs the core's tests on every platform, and on Linux the
keyboard's, on layouts that libxkbcommon compiles from its rules, and the translation of the
compositor's pointer, touch and keyboard events, without a compositor; X11's pointer, its wheel,
keys, focus, touch, scale, cursor's theme, its frames' signal and the display's period, without
a server; the time suspended that tells a resume, and the kernel's notice armed, read from
`/proc/self/fdinfo`, ending the wait and armed again. One test, ignored elsewhere, sets the clock
through `sudo` to make the kernel cancel the notice: CI's headless job alone runs it, on its
throwaway runner, with `UWUI_TEST_SETS_CLOCK=1`; a real suspend is checked by hand. Tests that
compose dead keys read the tests' own compose file, `tests/compose/Compose`, never the
developer's: each runs itself again in a process of its own whose `XCOMPOSEFILE` names it.

On Windows, the same command runs the core's tests, the clock's, the mouse's messages translated
and the capture they keep, the signal's judgement of a vertical blank and its grid, a timer's due
time, and the display driver's own wait for the primary display's vertical blank, which needs a
display: it fails in a session without one, a service's or one over SSH.

The window itself is tested through the host above it, by `uwui-desktop`'s `rest` tests in the
series of compositors, X11's under Xvfb, and by its `windows` test on the desktop of CI's Windows
runner. There too, `env -u WAYLAND_DISPLAY cargo test -p
uwui-windowing --test x11 -- --test-threads=1`, with `XCOMPOSEFILE` naming `tests/compose/Compose`
and `DESKTOP_STARTUP_ID` a start's id that holds a quote and a backslash, watches X11's window as
another client sees it. It needs an X server of its own that `DISPLAY` names, never the person's:
it sets the server's keymap, focus, pointer and X resources, and leaves the resources as it found
them. It checks:

- the fullscreen hint without a window manager and with one, the pings, the start's end;
- the signal held while hidden, a present told across a resize, the fallback from a Wayland
  compositor that does not answer;
- keys typed through `xdotool`, their repeat, a keymap changed, a dead key composed, Caps Lock
  locked, the modifiers told released as the focus leaves, a key's own modifiers kept through a
  long turn;
- the focus as other clients move it, through `PointerRoot`, the root window and a window
  manager's frame, as they grab the keyboard or the pointer, and as the host turns late, every key
  coming while the window is told focused;
- a click, the wheel, Shift or a status written on the root window elsewhere turning nothing, and
  a close asked, the frames' signal, an exposure and a ping each handled at rest;
- each cursor's shape as XFixes shows it, made again only when the X resources change;
- the scale and the smallest size that the X resources' `Xft.dpi` gives.

## Depends on

`slime-input`, the input's vocabulary; `uwui-time`, the times; `raw-window-handle`, the window's
pointers given to the host; `ctrlc`, the termination signals; `tracing`. On Linux,
`wayland-client` on libwayland's display, loaded at run time, `wayland-protocols`' xdg-shell,
xdg-decoration, the cursor's shapes, the fractional scale, the viewporter and activation;
`xkbcommon-dl`, libxkbcommon and libxkbcommon-x11 loaded at run time; `x11rb` on libxcb loaded
at run time, with XInput2, RandR, Present, XKB, the X resources and its loader of themed cursors,
which brings `xcursor`; and `rustix`, the loop's poll, eventfd and timerfd, the clocks, the
keymap's read and the machine's name. On Windows, `windows-sys`: Win32's window, messages and
waits, each display's scale, the mouse, DWM's period, the display driver's vertical blank and the
clock.

## Must not

- Depend on `uwui`, `uwui-render`, wgpu or its GPU's libraries, nor on an engine crate but
  `slime-input`, nor be depended on by a crate but `uwui-desktop`: `scripts/check.sh` checks all
  three.
- Show a type of a platform's library in its API.
- Run, poll or wake while nothing happens.
- Use `unsafe` but where it calls libxkbcommon and libxkbcommon-x11, and Win32, and where
  Windows' layer lends the host to its window's procedure and shares the window's state with it,
  each with its `// SAFETY:`; libxcb is reached through `x11rb`'s safe interface.
