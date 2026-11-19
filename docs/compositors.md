# The series of headless compositors

The editor and *Hunna Wood* run in five compositors without a screen, each the reference of a
family: Weston, sway (wlroots), KWin (KDE), Mutter (GNOME), and X11 through Xvfb
([ADR-0195](adr/0195-a-series-of-headless-compositors.md),
[ADR-0240](adr/0240-the-series-input-its-table-and-its-jobs.md)). This file holds what each gives and how the
engine adapts there. [`scripts/compositors.sh`](../scripts/compositors.sh) observes each row of the
table below and fails where an observation differs from its cell: a change of behaviour then changes
the table, or the code. CI's `compositors` workflow runs it for each compositor. Its figures, the
frames' pacing, are in [`measurements.md`](measurements.md).

```sh
nix develop .#compositors -c scripts/compositors.sh weston|sway|kwin|mutter|x11
```

Each run writes its report, the compositor's protocols as `wayland-info` lists them, and its logs to
`target/compositors/<compositor>/`. KWin and Mutter draw with Mesa on the CPU, as on CI's runners,
which have no GPU; on a machine that has one, Mutter still opens it to allocate its buffers. CI's
runs set the table.

## What each gives, and what the engine does there

A cell that starts with "—" is not observed: what it says comes from elsewhere.

| | Weston | sway | KWin | Mutter | X11 (Xvfb) |
|---|---|---|---|---|---|
| Version | — 16.0 | — 1.12 | — 6.7.5 | — 50.4 | — 21.1.24 |
| Input | — none | — wlroots' virtual pointer and keyboard | — libei | — libei | — XTest |
| Backend | wayland | wayland | wayland | wayland | x11 |
| Output's refresh | 240 Hz | 240 Hz | 60 Hz | 240 Hz | — none |
| Presentation times | CLOCK_MONOTONIC_RAW | CLOCK_MONOTONIC | CLOCK_MONOTONIC | CLOCK_MONOTONIC | — none |
| Presentation feedback asked | no | no | no | no | — none |
| Decorations | the program's, no protocol | the compositor's | the compositor's | the program's, no protocol | — none, no window manager |
| Fractional scale's protocol | absent | used | used | used | — none |
| Window's scale | 2.0 | 1.5 | 1.5 | 1.5 | — X11 gives no scale for an output |
| Cursor | not run, the pointer never entered | by shape | by shape | by shape | — X11's own cursors |
| Clipboard | none | opened | opened | none | opened |
| Clipboard's protocol | none | ext | ext | none | — X11's selections |
| Clipboard round trip | not run, no input | round trip | round trip | nothing pasted | round trip |
| Game minimized | at rest | draws its frames | at rest | at rest | draws its frames |
| Agents' sessions and rest | pass | pass | pass | pass | pass |
| uwui's host at rest | pass | pass | pass | pass | pass |
| uwui's host's bar | none, no protocol | the compositor's | the compositor's | none, no protocol | — no window manager under Xvfb |
| uwui's host's period told | 4.167 ms | none, measured from the signals | 16.667 ms | 4.167 ms | none, measured from the signals |
| uwui's host's keys | not run, no input | typed | typed | typed | not run, X11's layer takes no key yet |
| uwui's host's cursor | not run, the pointer never entered | by shape | by shape | by shape | — the server's: X11's layer sets no shape before M6d.7b2 |
| uwui's host's scale | 2.0 | 1.5 | 1.5 | 1.5 | — X11's layer reads no scale before M6d.7b2 |
| uwui's host's losses | drawn again | drawn again | drawn again | drawn again | drawn again |

The rows:

- **Version**: nixpkgs' at the revision that `flake.lock` pins.
- **Input**: how the series gives a person's input ([`scripts/compositor-input.py`](../scripts/compositor-input.py)).
- **Backend**: the windowing system that winit chose, as the engine's log says.
- **Output's refresh** and **Presentation times**: the output's current rate, and the clock of
  `wp_presentation`, as `wayland-info` lists them; the series asks Weston, sway and Mutter for
  240 Hz.
- **Presentation feedback asked**: whether the program, its Vulkan driver included, asked for
  `wp_presentation`'s feedback, in the editor's log written with `WAYLAND_DEBUG=client`.
- **Decorations**: the compositor's when it accepts `zxdg_toplevel_decoration_v1`'s server side;
  otherwise winit draws them ([`debt.md`](debt.md): no title in them).
- **Fractional scale's protocol** and **Window's scale**: whether the editor bound
  `wp_fractional_scale_manager_v1`, and the scale that the `window` example reports on an output at 1.5,
  at 2 in Weston, which takes integers alone.
- **Cursor**: how the editor set its cursor as the pointer entered its window, by
  `wp_cursor_shape_v1`'s shape or by an image of a theme.
- **Clipboard**, **Clipboard's protocol**, **Clipboard round trip**: whether the engine opened the
  system's clipboard, through which protocol of the clipboard's managers, and whether a text copied
  with Ctrl+A and Ctrl+C in a field of the editor, then pasted with Ctrl+V, came back, the keys
  typed through the compositor.
- **Game minimized**: *Hunna Wood* minimized, its game time running, watched a few seconds later
  (slime-mcp's `drive`): at rest, waking now and then, or drawing its frames, as where the
  compositor keeps its window shown.
- **Agents' sessions and rest**: slime-mcp's `edit_window` and `drive`, which find the editor and
  the game at rest ([ADR-0185](adr/0185-rest-checked-in-ci.md)).
- **uwui's host at rest** and **uwui's host's bar**: `uwui-desktop`'s `application` example, on
  uwui's own layers, `uwui-windowing`'s Wayland's and, under Xvfb, X11's
  ([ADR-0270](adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
  [ADR-0311](adr/0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md)):
  an image presented and at rest after its start; where the compositor takes input, the pointer
  put on its button *Add*, whose hover draws frames, and at rest again; then ended by SIGTERM
  through its closing (its test `rest`); and whether the compositor took
  `zxdg_toplevel_decoration_v1`'s server side, in its log written with `WAYLAND_DEBUG=client`.
  Xvfb has no window manager, which would draw the bar on X11.
- **uwui's host's period told**: the display's period that `wp_presentation` tells with the same
  example's first image shown, in its log; where it tells none, the host measures the period from
  the signals' gaps. On X11, Present tells the images shown where the driver presents through it,
  which lavapipe does not, and RandR the period, which Xvfb does not: the trace of the same run
  tells whether any showed
  ([ADR-0309](adr/0309-input-to-shown-measured-beside-input-to-present-in-motion-an-input-reaches-the-next-image.md)).
- **uwui's host's keys**, **uwui's host's cursor** and **uwui's host's scale**: the same example,
  where the compositor takes input, its field clicked, `s` and `a` typed through the compositor,
  and the row that *Add* makes named `sa`; its cursor set by `wp_cursor_shape_v1`'s shape, or left
  to the compositor; the scale that its window drew at on an output at 1.5, at 2 in Weston, its
  image's width over its logical width. On X11, the layer takes no key and sets no cursor yet,
  and reads no scale
  ([ADR-0273](adr/0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md)).
- **uwui's host's losses**: the same example, its GPU device lost on purpose, then its surface
  given up, each followed by a new swap chain's image presented and the example at rest; then the
  fourth device lost ending it with status 1, the application stopped
  ([ADR-0305](adr/0305-uwuis-desktop-host-makes-its-device-and-surface-again-a-programs-images-made-again-from-their-source.md)).

## What differs

- **Decorations.** Weston and Mutter draw none: winit's are drawn, without a title; uwui's host
  has no bar there until M6d.7e.
- **Fractional scale.** Weston gives integer scales alone; sway, KWin and Mutter give fractional
  ones, which the engine reads.
- **Clipboard.** Weston and Mutter give no protocol of the clipboard's managers: the engine opens
  no clipboard there, and its fields copy and paste nothing ([`debt.md`](debt.md)). sway and KWin
  give `ext_data_control_v1`, X11 its selections. M6d.8 designs the desktop host's clipboard
  across the series.
- **A minimized window.** sway minimizes nothing, and Xvfb, without a window manager, iconifies
  nothing: the window stays shown, and the game rightly draws its frames. Mutter configures a
  minimized window again about 3 s later, which wakes the game once, then leaves it at rest.
- **Rates and clocks.** KWin's virtual output refreshes at 60 Hz: neither an option nor a custom
  mode of its settings changes it. Weston's presentation clock is `CLOCK_MONOTONIC_RAW`, the others'
  `CLOCK_MONOTONIC`. Xvfb gives no refresh rate: the engine's frames there follow its own schedule.
  Its Present still notifies each refresh asked, on a timer of the server's own at 60 Hz, in
  microseconds of `CLOCK_MONOTONIC`: uwui's host's frames follow them (read on 2026-11-19,
  Xvfb 21.1.24). It refuses, about once, a connection made just as another closes: the X11
  layer's own tests wait a tenth of a second before each window.
- **Presentation feedback.** No program of the engine asks for it today: uwui's desktop host reads it
  from M6d.7a2 ([ADR-0270](adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
- **Focus on X11.** Without a window manager, Xvfb gives no window the keyboard's focus: the series
  gives it to the window under the pointer, as a manager whose focus follows the pointer would.

## What the series does not check

- Variable refresh rates: no headless compositor emulates one; they are checked by hand on the
  owner's displays (ADR-0195).
- Weston's input: Weston takes none, so its cursor and its clipboard's round trip do not run.
- On X11, decorations, a window manager's minimizing and its cursors: Xvfb runs no window manager.
- KWin at 240 Hz, which its virtual output does not give.
- Windows and macOS, checked by hand in their virtual machines.
