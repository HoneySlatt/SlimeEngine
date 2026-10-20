# slime-tasks

The engine's task pool: the threads on which the engine computes, with a priority for the work that
a frame waits for ([ADR-0107](../../docs/adr/0107-the-execution-model.md)).

## Status

Partial. One pool of threads, started by the first task, as many as the machine's logical cores
but one, at least one; tasks of two priorities, *frame* and *background*, a free thread taking a
frame task first, background tasks never holding every thread; items computed in parallel, the
calling thread taking its share. No dependencies between tasks, no loops split for the caller, no
work stealing: Stage 7's job system grows them from this pool when a measurement asks for them.

## Role

- Runs the engine's compute work: the loads of cooked data, the sky's environment, the cooker's
  encoding. No crate of the engine starts threads that compute; one that only waits, as the thread
  that submits the copies to the GPU, says why in its rustdoc (ADR-0107).
- Keeps a thread free for the work that the frame being made waits for, whatever the background
  holds.

## Entry points

```rust
use slime_tasks::Priority;

// A load's task, which no frame waits for.
slime_tasks::run(Priority::Background, || {
    // Read and decode a cooked file...
});
// The items of a computation that this frame waits for, in order, the calling thread taking its
// share: here, sixteen squares.
let squares = slime_tasks::map(Priority::Frame, 16, |index| index * index);
assert_eq!(squares[15], 225);
```

- `run(priority, task)` queues a task, which a free thread runs once.
- `map(priority, count, item)` computes `item(i)` for each `i` on the pool and on the calling
  thread, and gives them in order; it finishes when called from a task too.
- `threads()` says how many threads the pool has.
- `Priority`: `Frame` or `Background`.

The documentation of each item holds its contracts.

## Dependencies

None: the standard library alone, at the bottom of the workspace, so that every crate can use it.

## Must not

- Depend on any crate, of the engine's or not: every crate that computes depends on it.
- Let a task wait for another task, which could be queued behind it; `map` takes its items itself
  while it waits.
