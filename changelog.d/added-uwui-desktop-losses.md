- uwui's desktop host draws again after its GPU device or its window's surface is lost: it replaces
  a lost device at once and makes a surface given up again a second later, the window holding its
  frames meanwhile; the fourth of either within five minutes ends the run with `EXIT_HOST_FAILED`.
  A surface whose configuration is refused counts as lost rather than panicking.
  `Window::lose_device` and `Window::lose_surface` lose them on purpose, for the tests and the
  tools; the `application` example takes them on its input with `--commands` (ADR-0305).
