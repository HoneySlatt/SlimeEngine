# 0043. Application errors during a run

- Status: Accepted; amends [0020](0020-rendering-in-the-main-loop.md)
- Date: 2026-10-14

## Context

[ADR-0020](0020-rendering-in-the-main-loop.md) lets only `App::init` return an error. An
application that hit an error it could not handle in another method, a save file it could not
write in `shutdown`, an asset that failed to load in `update`, a readback that failed in `render`,
logged it and requested an exit: `run` then returned `Ok(())`, and a game exited with success after
a failure. A launcher, a test harness or a service manager that looks at the exit code saw nothing
wrong.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 that an application
can fail a run from any method.

## Decision

- `Context::exit_with_error(error)` takes any error, or a message, as an `AppError`, and stops the
  run as `Context::request_exit` does: the current frame finishes and `shutdown` is called. `run`
  then returns the new `RunError::App`, with the error as its source, and logs it with its causes
  as it logs every failed run.
- `shutdown` can call it too, and so fail a run that stopped for another reason: a run that ended
  well but could not save is a failed run.
- Only the first error is returned; a later one is logged as an error.
- When the GPU device or the window's surface is lost as well, `run` returns the application's
  error. The loss is logged when it happens, while the application's error would be reported
  nowhere else.
- The `App` methods other than `init` still return nothing.

This amends ADR-0020: `run` returns `RunError::App` as well, and an application no longer requests
an exit to stop on an error.

## Alternatives considered

- **Every method returning a `Result`.** Every implementation of `update`, `render`, `on_event` and
  `fixed_update` would end with `Ok(())`, for a failure that most of them never have; ADR-0020's
  reason still holds.
- **A loss before the application's error.** The loss may be the cause of the application's error
  (a readback failing on a lost device), but it is in the log in either case; the application's
  error is not.
- **Returning every error.** `RunError` would hold a list, for a case that the log already
  covers.

## Consequences

- A game that fails anywhere exits with a failure, as the README's pattern shows
  (`if slime_app::run(..).is_err() { std::process::exit(1) }`).
- `RunError` gains a variant; it is `#[non_exhaustive]`, so callers already handle variants they do
  not name.
