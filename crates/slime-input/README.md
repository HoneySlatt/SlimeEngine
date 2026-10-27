# slime-input

SlimeEngine's vocabulary of input: the keys of a keyboard, by their position and by what they mean
with the current layout, the mouse's buttons, the modifier keys and the wheel
(`docs/adr/0142-input-and-focus.md`), what an input method does and the pointer's shapes
(`docs/adr/0145-text-edited.md`). It holds types alone, without the windowing library, so that
what reads input without a window, the interface's framework (`slime-ui`), reads the same types as
the window's platform layer (`slime-platform`), which translates winit's events into them.

## Role

- `KeyEvent` is a key going down or up: its position, `KeyCode`, what it means, `Key`, which names
  a `NamedKey` or holds a character, the text it produces, and whether it repeats.
- `Modifiers`, `MouseButton`, `ButtonState` and `ScrollDelta`: the modifier keys held, a mouse
  button, whether a key or a button went down or up, and how far the wheel scrolled.
- `Ime`: what an input method does while the user composes text through it, the text being
  composed, then the text committed. `CursorShape`: the pointer's shapes, which the interface asks
  for and the window shows.
- `for_each_key_code!` and `for_each_named_key!` list the names of the key codes and the named
  keys, once: the enums are declared from them, and `slime-platform` translates winit's values by
  them. `KeyCode::name` and `KeyCode::from_name`, and `NamedKey`'s, give each as text and back, as
  the tools of the development channel name keys.

## Depends on

Nothing but the standard library.

## Must not

- Depend on the windowing library or on any other engine crate: it is the vocabulary that the
  others share.
