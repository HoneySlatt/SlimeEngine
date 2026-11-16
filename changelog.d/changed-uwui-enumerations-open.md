- uwui's public enumerations, and those of `slime-color` and `slime-input`, are `#[non_exhaustive]`
  but six closed by nature (`ButtonState`, `Close`, `InputMethod`, `Affinity`, `Edge`, `TokenOr`):
  a program that matches one gives a behaviour to the cases that it does not know (ADR-0282).
