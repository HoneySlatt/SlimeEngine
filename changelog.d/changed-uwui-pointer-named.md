- uwui's pointer events, `Input::PointerMoved`, `PointerLeft` and `PointerButton`, name their
  pointer: `slime_input::Pointer`, the mouse, a finger or a stylus with its pressure and tilt. A
  target follows the mouse and one finger or stylus at a time; a second finger does nothing yet
  (ADR-0285).
