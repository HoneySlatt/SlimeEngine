- uwui's pointer events, `Input::PointerMoved`, `PointerLeft` and `PointerButton`, name their
  pointer: `slime_input::Pointer`, the mouse, a finger or a stylus with its pressure and tilt. A
  target follows one pointer at a time, the first finger or stylus down, else the mouse; a second
  finger does nothing yet
  (ADR-0285).
