# 0273. M6d.7a2 cut in three; Wayland's keyboard, cursor, scale and activation in the host

- Status: Accepted; amends
  [0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)
  (M6d.7a2 cut in three)
- Date: 2026-11-15

## Context

M6d.7a2's card ([`stage-6d.md`](../stage-6d.md#m6d7a2--wayland-whole-and-the-host-measured))
completes uwui's desktop host on Wayland and measures it. It asks for twelve things: the keyboard
through libxkbcommon, the cursor's shapes, the fractional scale, activation; the presentations'
times, frames drawn just in time; a blank window, a tool that measures it against the example in
one run, input to present at 240 Hz on the GPU; the crash report and the session's marker; a lost
device and a lost surface made again, which the owner added on 2026-11-15. Several of them are each
as large as a part of M6d.7a1: the keyboard alone needs libxkbcommon's calls, its compose tables
and the keys' repeat, which Wayland leaves to its clients.

The facts, read on 2026-11-15:

- `xkbcommon-dl` 0.4.2 loads `libxkbcommon.so.0` and its compose functions at run time; it pulls
  `xkeysym`, `dlib`, `log` and `once_cell`, all in the lockfile already through winit.
- `wayland-protocols`' feature `staging`, which holds `wp_cursor_shape_v1`,
  `wp_fractional_scale_v1` and `xdg_activation_v1`, is built already for winit: the host turning it
  on adds no crate.
- The series' compositors with input, sway, KWin and Mutter, give `wp_cursor_shape_v1` and the
  fractional scale; Weston gives integer scales alone, and no input
  ([`compositors.md`](../compositors.md)).
- `scripts/compositor-input.py` moves the pointer and types keys; it cannot click.
- The host's measure of its own cost must come last: what it measures is the host as M6d.7a2 leaves
  it, its device made again and its crash report included.

## Decision

**M6d.7a2 is cut in three,** the owner's choice of 2026-11-15, each part in a session of its own:

- **M6d.7a2a — Wayland's input and window.** The keyboard through libxkbcommon, its compose and its
  repeat; the cursor's shapes; the fractional scale; activation; a click in
  `scripts/compositor-input.py`; a text typed into the example's field, checked in the series.
- **M6d.7a2b — The host made robust.** A lost device and a lost surface made again within
  [ADR-0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)'s
  budget; the crash report and the session's marker
  ([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
- **M6d.7a2c — Frames and the host measured.** The presentations' times read and given as
  predicted; frames drawn just in time
  ([ADR-0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)); the
  blank window; the tool that measures both in one run; input to present in a headless sway at
  240 Hz, on the GPU with the owner's go; the applications' budgets' first figures in uwui's
  repository.

M6d.7a2's "Done when" is met at M6d.7a2c.

**The keyboard, M6d.7a2a's,** the lead's choices within ADR-0270:

- The keymap is the compositor's, read from `wl_keyboard`'s file; libxkbcommon's state follows
  its modifiers and gives each key its keysym. The host's code that calls libxkbcommon is Linux's,
  outside Wayland's layer, so that X11's layer takes it at M6d.7b.
- Each key gives `slime-input`'s `KeyEvent` as its contract says, the contract that the engine's
  winit layer meets: the physical key from Linux's code, the logical one from the keysym, Control
  never applied to a character, the text of named keys the same on every platform.
- Dead keys and sequences compose through libxkbcommon's compose tables, of the locale that
  `LC_ALL`, `LC_CTYPE` or `LANG` names, `C` without one. Where no table loads, keys type without
  composing, and the host says so once.
- Wayland leaves a key's repeat to its client: the host repeats the last key pressed that the
  keymap says repeats, at the compositor's `repeat_info`, from a deadline of its loop, and stops at
  its release, at the focus's loss and at a new keymap. A repeat's time is its deadline on the
  host's clock, `EventSource::Received`.
- The focus given and taken is `Input::Focus`, the modifiers `Input::Modifiers` at each change.

**The cursor** is set by `wp_cursor_shape_v1`, at each pointer's entry and at each shape that the
interface asks. A compositor without it keeps its own cursor: loading a theme's images needs
`wayland-cursor`, put to the owner if a compositor of the field lacks the protocol.

**The scale:** `wp_fractional_scale_v1` with `wp_viewporter` where both are given, the image
`round(logical × scale)` pixels and the viewport the window's logical size; else
`wl_surface`'s preferred integer scale; else 1. The pointer's positions and the wheel's pixels are
multiplied by the scale, as uwui counts in physical pixels.

**Activation:** the start's token, `XDG_ACTIVATION_TOKEN`, activates the window as it opens. The
application's `Window::request_attention` asks a token without a person's input and activates
with it, which compositors show as a window asking for attention; `Window::bring_to_front` asks
one with the last input's serial, which a compositor may honour by raising the window. Where the
compositor gives no `xdg_activation_v1`, both are told once and do nothing.

## Alternatives considered

- **M6d.7a2 whole, across several sessions:** no ADR of a cut, but a milestone that no review
  reads at once, and a session that stops halfway.
- **Cut in two,** the input, the window and the robustness together, then the frames and the
  measure: one session fewer, its first part as large as M6d.7a1 whole.
- **The keys' repeat left to uwui:** every host but Wayland's gets it from its platform; the
  platform's rate and delay are the host's to read.
- **The cursor through theme images (`wayland-cursor`) everywhere:** a dependency and a theme's
  files to read, where every compositor of the series with input gives the shapes.
- **The fractional scale without the viewporter, by an integer buffer scale rounded up:** sharp
  but drawn larger than the compositor shows it, and slower.

## Consequences

- Stage 6d grows by two sessions.
- The host's keyboard, cursor, scale and activation are tested in the series' compositors with
  input; Weston checks rest alone.
- A compositor without `wp_cursor_shape_v1` shows its own cursor over the window
  ([`debt.md`](../debt.md)).
- X11's layer reuses the host's code of libxkbcommon at M6d.7b.
