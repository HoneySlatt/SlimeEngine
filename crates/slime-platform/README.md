# slime-platform

The engine's link to the operating system's windowing and input services.

## Status

Skeleton: the crate exists so that the workspace, its policies and its checks are in place, but
it contains no code yet. The window arrives in milestone M1.4 and keyboard and mouse input in
M1.5 (see `docs/PLAN.md`).

## Role

- Owns every interaction with the windowing system: creating windows, receiving their events,
  and translating keyboard and mouse input into engine-owned types.
- It is the only crate allowed to use the windowing library, and that library's types never
  appear in its public API (`docs/adr/0004-workspace-layout-and-crate-naming.md`).

## Dependencies

None.

## Must not

- Depend on `slime-app` or on any crate above it: dependencies point from `slime-app` to
  `slime-platform`, never the reverse.
- Expose the windowing library's types in its public API.
