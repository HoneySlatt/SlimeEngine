# 0313. X11's keyboard through XInput2 and libxkbcommon-x11, its cursor themed, its scale from Xft.dpi

- Status: Accepted
- Date: 2026-11-19

## Context

M6d.7b2, "X11's keyboard, cursor, scale and touch"
([`stage-6d.md`](../stage-6d.md#m6d7b2--x11s-keyboard-cursor-scale-and-touch)), is the second part of
M6d.7b ([ADR-0311](0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md)). After
M6d.7b1, uwui's desktop host on X11 takes no key, shows the root window's cursor, draws at a scale
of 1 and takes no touch. Its card asks its dependencies of the owner at its start.

The facts, read on 2026-11-19:

- The layer already reads the pointer through XInput 2.1's events of the master devices. XInput2
  gives the keys too, with the server's times; it marks a key held's repeats (`KeyRepeat`) and
  sends no release between them, which core events do unless a client asks XKB for detectable
  repeats. XInput 2.2 gives touches, each with its own number.
- The keyboard's code of libxkbcommon, `crates/uwui-windowing/src/xkb.rs`, is Linux's: it
  translates Linux's codes, which X11's keycodes are plus 8. libxkbcommon-x11 reads the server's
  keymap and state as the server holds them, through XKB on libxcb's connection; its library ships
  with libxkbcommon's, and `xkbcommon-dl` 0.4.2 loads it at run time with its `x11` feature. XKB's
  notices of a new keymap and of the modifiers need `x11rb`'s `xkb` feature, its protocol's code;
  winit already enables it, so it adds no crate.
- `x11rb`'s `cursor` feature loads the person's Xcursor theme at the size that the environment or
  the X resources name, with `xcursor` 0.3.11 (MIT, no dependency), already in the lockfile through
  winit's Wayland cursor, and `x11rb`'s `render` and `resource_manager` features. Without a theme,
  its loader returns no cursor; the server's core cursor font still has the four glyphs that uwui's
  shapes need.
- X11 has no scale of its own. The X resources' `Xft.dpi`, in the root window's
  `RESOURCE_MANAGER`, is what Xft, Qt and winit read, and what `xrdb` loads from a person's
  `~/.Xresources` at a session's start; 96 dots per inch is a scale of 1.
- Without a window manager, as under Xvfb, the server's focus follows the pointer (`PointerRoot`):
  XInput2 then tells the window that has the pointer that it has the focus through its `Enter`
  event's `focus`, and sends it no `FocusIn` (read under Xvfb 21.1.24).
- libxkbcommon reads a person's compose files, `XCOMPOSEFILE`, then `$XDG_CONFIG_HOME/XCompose` and
  `~/.XCompose`, before the locale's table; `xkbcommon-dl` 0.4.2 binds no function that compiles a
  table from a file or a buffer: the keyboard's tests of dead keys failed for a developer whose own
  sequence redefines `^` then `e`.

## Decision

The owner chose the dependencies on 2026-11-19, one decision at a time: `xkbcommon-dl`'s `x11`
feature with `x11rb`'s `xkb`; then `x11rb`'s `cursor`, which brings `xcursor` 0.3.11, `render` and
`resource_manager`. The owner chose too, the same day, after the closing review, when the scale is
read again.

**The keyboard.** The keys come through XInput2's `KeyPress` and `KeyRelease` of the master
keyboards, with the server's times, translated by `xkb.rs`, shared with Wayland's layer, each in its
own event's state: the modifiers and the layout, depressed, latched and locked, that XInput2 gives
with the key, from before it; the keyboard then returns to the state that the server last told. The
keymap is the server's core keyboard's, read through libxkbcommon-x11 at the start and again at
XKB's `NewKeyboardNotify` and `MapNotify`. The modifiers told the host come from each key's own
state, before the key, and from XKB's `StateNotify`, of the modifiers' and the layout's parts alone,
not the pointer's buttons, after it, and only while the window holds the focus: as it takes it, the layer asks for the notices and reads XKB's
state once, and tells the modifiers if they changed meanwhile, as Wayland's compositors do; as it
loses it, the notices stop and the modifiers are told released, on Wayland too, whose protocol asks
a client to take every key as lifted. The repeat is the server's: XInput2's `KeyRepeat` marks it,
and the layer keeps no timer of its own.

**The focus.** The window holds it when the server's focus is the window itself, or lies above it,
following the pointer (`PointerRoot`) or on an ancestor, such as the root window, and the pointer
lies over it. At each of the core protocol's `FocusIn` and `FocusOut`, which the server sends where
XInput2 sends none, as for a move to an ancestor, the layer reads where the server's focus now is
(`GetInputFocus`, then the window's ancestors through `QueryTree`), rather than reading it from the
events' details, which mislead between an event out and an event in. Another client's grab of the
keyboard, a `FocusOut` of the mode `Grab`, takes the focus until its `FocusIn` of the mode `Ungrab`,
or until a key reaches the window, where the server tells the grab's end to no one, though the
server's focus stays: a menu, a screen locker, a window manager's Alt and Tab. A key that reaches
the window gives it the focus until the next event of the focus, since the server sends keys only
to the window that holds them, whatever an event read late said. XInput2's `Enter` and
`Leave` follow the pointer, those of another client's grab of the pointer aside. `WM_HINTS` asks the
window manager for the focus, ICCCM's passive model.

**The loop turns only for what tells the host something.** Events that tell it nothing, as a
keyboard that the server's core keyboard now follows, or a window manager's ping, are read and
answered, the answers sent before each wait, and the loop waits again without a turn of the host.

**The cursor.** Each shape is looked up in the person's theme by its CSS name, then by X11's older
one (`text`, then `xterm`), through `x11rb`'s loader, which reads `XCURSOR_THEME` and
`XCURSOR_SIZE`, then the X resources' `Xcursor.theme` and `Xcursor.size`, then a size from
`Xft.dpi`. Where no theme holds it, or the theme named is `core`, the core cursor font's glyph
shows, from the layer's own table: `x11rb` 0.13.2's gives the font's glyphs at half their numbers. Each shape is made once,
when first shown, and set on the window; the arrow is set as the window opens, in place of the
root window's cursor. A change of the X resources makes them again.

**The scale.** `Xft.dpi` over 96, read from the root window's `RESOURCE_MANAGER` alone, held
within 0.5 and 8; 1 where it gives no positive number. The property is read at the start, and again
each time the window takes the keyboard's focus, the owner's choice: a change applies as the
person comes back to the application, which follows the change of the setting. The window opens at
its logical size times the scale, its minimum size the same; a scale that changes keeps its pixels,
its logical size following.

**Touch.** XInput 2.2's `TouchBegin`, `TouchUpdate` and `TouchEnd`, asked where the server gives 2.2;
each finger a pointer of its own, named anew at each touch, through the core's `fingers`, which
Wayland's layer now uses too. Where the server gives 2.1 alone, no touch.

**The keyboard's tests compose from a file of their own,** `crates/uwui-windowing/tests/compose/Compose`,
which `XCOMPOSEFILE` names. A unit test that composes runs itself again in a process of its own
with the variable set, and checks that it passed there; the series' X11 test is run with it set.

## Alternatives considered

- **The keys through core events, detectable repeats asked of XKB:** a second path of events beside
  XInput2's, whose times and devices the pointer already reads; a per-client flag where XInput2's
  mark is in each event.
- **The modifiers told from each XInput2 key's own state alone:** no notice when they change without
  a key of the window, as Caps Lock locked by another device, and a key's state is from before it,
  so that the last key would leave a modifier told held; `StateNotify` follows.
- **The focus read from the events' details, XInput2's alone:** the closing review found it told
  lost as the server's focus moved through `PointerRoot`, and never told for a move to an ancestor,
  which XInput2 does not send.
- **A keymap compiled again from the rules' names that the server announces, or the core keymap
  alone:** both wrong where the person changed the keymap with `xmodmap` or `xkbcomp`, and the
  second without XKB's layouts and levels. Put to the owner, who chose libxkbcommon-x11.
- **The core cursor font alone, or a reader of Xcursor's files of the layer's own:** the first
  black and white, of no theme nor size; the second what `xcursor` already does. Put to the owner,
  who chose `x11rb`'s `cursor`.
- **The X resources followed as they change:** X11 tells a change of the root window's properties
  only to a client that asks for every one, and a status bar that writes the time on the root
  each second would wake the loop each second, at rest. Put to the owner.
- **The X resources read at the start alone:** no wake either, but a change of the scale needs the
  application started again. Put to the owner.
- **The scale from XSETTINGS' `Xft/DPI`:** a settings daemon's selection protocol, given only
  where a desktop runs one; `Xft.dpi` is the older source, and the one that more programs read.
- **The scale from RandR's physical sizes:** displays report sizes that are often wrong, and X11
  gives no way to tell the person's choice.
- **The X resources read from the person's files too (`~/.Xresources`), as `x11rb`'s
  `new_from_default`:** a session's tools load them into the property; reading them again would
  differ from what the session's other programs use, and make tests read the developer's files.
- **The compose tables compiled from a buffer through a binding of the layer's own:** a dependency
  that loads libraries, put to no one, for tests alone. **Setting `XCOMPOSEFILE` inside the test
  process:** unsound while other threads read the environment from C.

## Consequences

- `docs/debt.md`'s items "On X11, uwui's desktop host takes no key, shows the server's cursor and
  draws at a scale of 1" and "uwui's desktop host's keyboard tests read the developer's own compose
  files" are settled.
- `crates/uwui-windowing` gains `x11/keyboard.rs`, `x11/cursor.rs`, `x11/touch.rs` and the core's
  `fingers.rs`; [`dependencies.md`](../dependencies.md) records the features and `xcursor`.
- Each event of the focus costs a round trip to the server, and one more for each ancestor walked
  through, two under a window manager's frame; on gaining the focus, XKB's state and the X
  resources are read too. Negligible on the machine's own server; a round trip each over a
  network, not measured.
- At rest, the layer's thread wakes for no input elsewhere on the screen: a click, the wheel or a
  modifier over another window. It wakes, without a turn of the host, when the server's core
  keyboard follows another device, as XTest's or a second keyboard's, and reads the keymap again.
- The series' X11 column types into the example's field, and reads the cursor and the scale from
  the layer's own test ([`compositors.md`](../compositors.md)).
- Debt left ([`debt.md`](../debt.md)): touch runs nowhere in CI, Xvfb having no touch device; a
  scale per display is not followed, X11 giving one for the whole screen; a layout switched, and
  XKB's `NewKeyboardNotify`, reach no test against a server; where the server's focus follows the
  pointer, another client's grab of the pointer through XInput2 that ends over another window leaves
  the window told focused until the pointer crosses its border again, the owner's choice of
  2026-11-20: the server tells it nothing, and no key reaches it meanwhile.
