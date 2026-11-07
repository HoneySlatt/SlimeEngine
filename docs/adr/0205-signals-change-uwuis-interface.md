# 0205. Signals change uwui's interface, and live values its drawing

- Status: Accepted; amends [0190](0190-uwuis-requirements.md) (how state changes the interface
  decided; live values in uwui's tree); amended by
  [0206](0206-slatty-ported-again-the-daw-later.md) (checked against Slatty's list once written);
  checked against Slatty's list on 2026-11-07: each need that it marks as general covered
- Date: 2026-11-07

## Context

[ADR-0190](0190-uwuis-requirements.md) left to Stage 6d how an application's state changes uwui's
interface: signals, views that say they changed as in GPUI, or messages as in Elm, beside today's
rebuild by keys. [ADR-0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) set how they
are compared. M6d.1b wrote each screen of reference in each model, then prototyped and measured the
four on one bench, without a window. The figures, and what they say, are in
[uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#three-models-beside-the-control-on-m6d1bs-bench-2026-11-07).
In short:

- The four do the same work in the tree's phases. They differ in the application's code that
  runs, and in what they hold.
- Signals cost least where a list scrolls or one part changes alone; they hold more, and cost
  more to open.
- Views and messages describe a whole view again for a change of one part; messages run `view`
  whole at each frame of a motion.
- A changed type keeps its state across a reload in the control, views and messages; signals
  replayed in the order that they are made lose it.
- Signals and views cancel a task with its component; messages, a subscription; the control,
  nothing.

The choice waited for what three programs of uwui's first stage ask of it
([ADR-0203](0203-pilots-replaced-pluto-and-a-daw.md)). Each wrote its list after an interview
with the owner:

- Opium, in its repository's `docs/ui-framework-needs.md`;
- the native client of Pluto, the owner's harness of agents, in its private repository;
- the DAW, Magnolia, in its repository's `docs/uwui-needs.md`. The DAW took the name after the
  media application of [ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md) was set
  aside.

The needs that the lists mark as general and that weigh on a state model:

- state changed from other threads and tasks in bursts, gathered into the next frame, the
  interface's thread never waiting (Opium, Pluto);
- many streams that change state which does not show, at the cost of the change, nothing built for
  what is hidden (Pluto);
- text that streams in, at the cost of the part appended (Pluto);
- a list or table of 100,000 rows changed in place by key, at the cost of the change, the item
  looked at and the selection kept (Opium); a list reordered by key, the selection kept (Pluto);
- values that change at each frame, bound to what is drawn alone (Magnolia, Opium);
- such values gathered between two frames by the thread that writes them: the highest since the
  last frame, or a short history (Magnolia);
- timers that wake only while their text shows, at the granularity that it shows (Pluto);
- several windows, each paced by its own output (Magnolia).

No model covered the frame's values as Magnolia and Opium ask them: written by another thread
without a lock, read as the element is drawn, without the application's code, styles, layout or an
allocation, and gathered between two frames, so that a meter loses no peak.

## Decision

The owner's decisions of 2026-11-07.

**Signals.** An application's state changes uwui's interface through signals, as in the
prototype: a view's function runs once; a closure given where a value would be, a text, a class,
a selection, changes that part of its element alone when a signal that it read changes; a list
makes the rows that come in scopes of their own and disposes those that leave; a scope disposed
cancels what it started.

**Live values, a primitive of uwui's tree,** below the state model: a cell shared with any thread,
written without a lock, which keeps its latest value, the highest since it was last read, or a
short ring of its history, and which an element reads as it is drawn. A change of it draws again
that element alone, and asks for a frame, without running the application's code, styling or
laying out anything, or allocating.

What the prototype left open, which later milestones answer:

- how a signal's state migrates across a reload of its type, which the order in which signals are
  made does not keep: M6d.1c, with the reload;
- how other threads reach signals, their changes gathered into the next frame: M6d.1c, with the
  executor;
- a list fed by its changes by key, at their cost and not the list's; a timer at a granularity,
  read only by what is mounted; the two panics' messages: uwui's first stage.

## Alternatives considered

- **Views that say they changed:** the fewest concepts, and every reload keeps its state, but a
  change renders its whole view again, and Pluto's streams and long lists ask for many small views.
- **Messages:** the most natural for commands and undo, as Magnolia's project is written, but
  `view` runs whole at each message, under streams, hidden updates and motion.
- **The control kept:** nothing new to learn, but every revision moved by hand, ids kept, and no
  task cancelled with its component.
- **Live values in the state model,** as signals that other threads write: one way to change the
  interface, but bound to signals, and a binding still run on the interface's thread at each
  value. **Left to applications,** on the canvas: nothing in uwui, but each program writing it
  again, and the canvas drawn again at each frame.

## Consequences

- uwui's first stage builds its own signals; whether a library serves is put to the owner then,
  as a dependency, from what the prototype written by hand
  ([ADR-0204](0204-signals-written-by-hand-for-the-prototypes.md)) and Floem show.
- Signals' memory and the allocations of a screen that opens are figures for uwui's benchmarks to
  watch.
- M6d.1c answers the reload and the executor on signals; M6d.1d the format of layouts, which their
  bindings read.
- The editor and *Hunna Wood* keep today's builders until they move onto uwui's components, at
  Stage 6c's resumption ([ADR-0190](0190-uwuis-requirements.md)); the places in their code that
  change are counted in uwui's measurements.
- The bench's prototypes are thrown away once M6d.1b ends; their figures stay in uwui's repository.
