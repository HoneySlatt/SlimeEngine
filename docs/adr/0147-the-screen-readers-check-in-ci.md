# 0147. The screen readers' check in CI

- Status: Accepted; amends [0146](0146-screen-readers.md) (a check of CI reads and uses an example
  through AT-SPI, which Orca reads)
- Date: 2026-10-26

## Context

[ADR-0146](0146-screen-readers.md) gave screen readers the examples' interfaces through AccessKit's
adapter, and left the path from the adapter to a screen reader to the owner's checks by hand: CI ran
no screen reader, and `slime-app`'s tests stopped at AccessKit's own tree, before the system's
service. Building M6b.6b, a client of AT-SPI, the service through which Orca reads on Linux, was run
by hand, on a session bus of its own with the AT-SPI service and a headless Weston: it read the
`interface` example's tree and used it. The owner decided on 2026-10-26 to make it a check of CI.

## Decision

**`scripts/check.sh` reads and uses the `interface` example through AT-SPI**, in the windowed part,
in the headless Weston where the example runs. `scripts/screen-reader-check.sh` starts a session bus
of D-Bus of its own, which activates no service, so that the desktop's portals stay out; on it, the
AT-SPI bus and its registry, which a desktop's session starts; turns AT-SPI on, as a screen reader
does; and runs the example. `scripts/screen-reader-check.py`, a client of AT-SPI in Python through
PyGObject, finds the example's tree and checks it: the window's name, the bar and its buttons, which
take an action, the field of text's text, caret and words, the field of numbers' value; it gives
the field a text, which the example reads as committed, moves the caret, and presses the button
that opens the dialog, which appears.

**The tools come from the Nix shell**: at-spi2-core, its bus and registry; D-Bus, for
`dbus-run-session`; GLib's `gdbus`, which turns AT-SPI on; Python with PyGObject; and the types of
AT-SPI, GLib and D-Bus that PyGObject reads (`GI_TYPELIB_PATH`).

## Alternatives considered

- **A client of AT-SPI in Rust**, on the `atspi` crate that AccessKit's adapter already builds: the
  checks in one language, but a new direct dependency of the tests, where PyGObject drives the
  library that Orca itself uses, libatspi.
- **The owner's checks by hand alone**: no new tool, but the path from the adapter to the system's
  service unchecked between two of the owner's checks.

## Consequences

- The Nix shell grows by at-spi2-core, D-Bus, GLib and Python with PyGObject; contributors without
  Nix run the check only where they install them.
- A change that breaks what AT-SPI reads of an interface fails CI on Linux. Windows' UI Automation,
  and Orca and Narrator themselves, stay the owner's checks by hand.
- The check adds the example's start and some seconds of reading to the windowed part of CI.
