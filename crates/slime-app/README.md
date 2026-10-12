# slime-app

The application lifecycle: starting an engine application, running its main loop, and shutting it
down.

## Status

Skeleton: the crate exists so that the workspace, its policies and its checks are in place, but
it contains no code yet. Its first code arrives in milestone M1.1 (see `docs/PLAN.md`).

## Role

- Starts an application, runs its frames, and shuts it down, with or without a window. Running
  without a window (headless) is a normal configuration, used by automated tests and later by
  dedicated servers.
- Sets up the engine's diagnostics (logging and panic reporting) and owns frame timing.

## Dependencies

None yet. When windows arrive (milestone M1.4), it depends on `slime-platform` for them.

## Must not

- Use the windowing library directly: windows and input go through `slime-platform`.
- Touch the windowing system when it runs headless.
