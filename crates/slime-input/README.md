# slime-input

SlimeEngine's vocabulary of input: the keys of a keyboard, by their position and by what they mean
with the current layout, the mouse's buttons, the modifier keys and the wheel
(`docs/adr/0142-input-and-focus.md`). It holds types alone, without the windowing library, so that
what reads input without a window, the interface's framework (`slime-ui`), reads the same types as
the window's platform layer (`slime-platform`), which translates winit's events into them.

## Role

- `KeyEvent` is a key going down or up: its position, `KeyCode`, what it means, `Key`, which names
  a `NamedKey` or holds a character, the text it produces, and whether it repeats.
- `Modifiers`, `MouseButton`, `ButtonState` and `ScrollDelta`: the modifier keys held, a mouse
  button, whether a key or a button went down or up, and how far the wheel scrolled.
- `for_each_key_code!` and `for_each_named_key!` list the names of the key codes and the named
  keys, once: the enums are declared from them, and `slime-platform` translates winit's values by
  them.

## Depends on

Nothing but the standard library.

## Must not

- Depend on the windowing library or on any other engine crate: it is the vocabulary that the
  others share.
