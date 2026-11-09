# 0211. The interface's executor, and other threads through channels

- Status: Accepted; amends [0190](0190-uwuis-requirements.md) (the interface's executor: its
  timers waited for by the host), [0205](0205-signals-change-uwuis-interface.md) (how other
  threads reach signals decided) and [0210](0210-signals-named-for-a-reload.md) (another thread's
  handle across a reload answered)
- Date: 2026-11-07

## Context

[ADR-0190](0190-uwuis-requirements.md) gives uwui two executors. The first is uwui's own, for the
interface's tasks: bound to the components that start them, cancelled with them, and run on time
simulated in tests. The second is a neutral adapter for heavy work, with an official one for
tokio. [ADR-0205](0205-signals-change-uwuis-interface.md) chose signals and left open how other
threads reach them. [ADR-0210](0210-signals-named-for-a-reload.md) asked how another thread's
handle survives a reload, which makes new signals.

The programs of uwui's first stage ask the same thing. Opium, Pluto's client, Slatty and the
engine change state from other threads in bursts: a library synced, search results, downloads,
streams, an agent's request. These changes must be gathered into the next frame, and the
interface's thread must never wait. Slatty's core runs on tokio and its models are pure: it asks
for signals fed by a state outside uwui, in batches, by parts and by key. Magnolia and Slatty ask
that uwui's own threads stay few. The engine asks that a host give the core its threads
([ADR-0209](0209-platforms-the-editor-and-agents.md)). Nothing may run, draw or wake at rest.

[uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#4-the-executor-and-its-waker)
read the field:

- SwiftUI and Floem bind a task to its view's life; GPUI and Rune bind it to a value that the code
  keeps.
- GPUI alone runs tests on simulated time, and its timers wake through a thread of their own on
  Linux.
- Slint and Makepad carry closures from other threads to the interface's.

The prototype went on from M6d.1c1's, on the bench's branch. The owner decided on 2026-11-07, one
question at a time.

## Decision

**The interface's tasks.** `spawn(async move { … })` starts a task on the interface's thread. Its
future need not be `Send`. The scope that runs the call owns it: a view's, a row's or a dynamic
view's. When that scope is disposed, the task is cancelled and its future dropped, with what it
holds.

- The executor runs within the frame, after the frame's input. A woken task is polled once a frame
  at most, after the bindings that the view's change asked for. A task woken again in its own frame
  waits for the next one and asks for it.
- A task takes at most a budget of values from channels in one poll, then waits for the next
  frame: a sender faster than the interface slows the frames, never stops them.
- The executor outlives a reload. A waker of a task that a reload cancelled wakes no other task.
  An application closed drops the tasks that still wait.

**Time.** A task waits with `sleep(duration)`. Its timers follow the frame's clock: the one that a
test moves, and later the clock of each root. The host waits for the next deadline itself, as
winit's `WaitUntil` does: the first timer joins `Ui::next_change`, the time of a tooltip or a
notification, which hosts already wait for. uwui starts no thread for its timers. A timer dropped
with its task forgets its deadline, so the host does not wake for it.

**The waker.** A task woken outside a frame asks the host's waker for one frame, once until that
frame ends. A task woken during a frame asks nothing of it. The tasks woken go to the executor
through the standard library's channel, which takes no lock; a read may spin briefly behind a send
caught midway.

**Other threads hold a channel's sender, never a signal.** uwui gives a channel:

- its `Sender` is `Send` and `Clone`; `send` fails with `Closed` once the receiver is gone, and
  waits only behind another sender waking the receiver;
- its `Receiver` reads on the interface's thread through `&self`, never waiting, and `recv` ends
  with `None` once every sender is gone;
- a task of the view receives, and sets the signals. One task receives at a time: a second one
  would take the first's waker.

A burst sent between two frames is received in one poll of that task, in the next frame. The
bindings that it changed run once, after it.

Batches, parts and keys are the application's messages: a batch is one message, a part is a set of
one signal, a key names the signal that changes. The core never holds a signal: it holds a
channel's sender, uwui's or tokio's, which tokio says works with any executor. Heavy work
runs where the application chooses, tokio, the engine's task pool or a thread, and its result comes
back through a channel.

**A channel that must outlive a reload is made outside the view.** The application makes it
before its view, as a program's data loaded at its start, and gives the view an `Rc` of the
receiver. A reload disposes the view and its task. The new view's task reads the same receiver,
and what was sent meanwhile waits for it. The sender never changes. A value that a task took and
held across another `.await` is lost with the task, as any cancelled work is. A channel made within
a view closes with it, and its senders' sends fail.

## Alternatives considered

On who waits for a timer's deadline:

- **A thread of uwui's for timers**, as GPUI's on Linux: no change to the host, but one more thread
  per application, against Magnolia's and Slatty's needs and the threads that the engine's host
  gives. Tests would have to bypass it to simulate time.
- **Frames asked for until the deadline:** nothing new, but a frame at each refresh of the display
  for as long as a timer waits.

On how other threads reach signals:

- **Handles to signals that any thread holds**, `signal.remote()`, whose sets wait for the next
  frame: the shortest code for one value, and a burst merged by itself. But the core holds the
  interface's signals, each batch by key needs a handle of its own shape, and the handle must
  survive a reload by the signals' names. That makes two ways to do one thing, since heavy work's
  result comes back through a task anyway.
- **Closures posted to the interface's thread,** as Slint's `invoke_from_event_loop` and Makepad's
  `UiRunner`: the most general, but a closure that holds a signal names one that a reload
  disposes, the core holds a handle to the interface, and each change allocates.

On where the receiver lives across a reload:

- **A new subscription at each mount,** as tokio's `watch` and `broadcast`: nothing of uwui's, but
  what comes between the old receiver and the new one is lost. The view must start again from a
  snapshot, and the core must serve several receivers.
- **The receiver kept by its name,** as a kept signal but moved in memory, not written through
  `serde`: the channel made within the view, but a second kind of kept value. And the limits of a
  scope's path would give it, without a word, to another scope.

## Consequences

- The host's API ([M6d.5](../stage-6d.md#m6d5--the-hosts-api)) needs nothing new for timers:
  `slime-app`'s windowed loop already waits for `Ui::next_change`. A run of `slime-app` without a
  window waits on its waker without a timeout: it would run no timer.
- An author learns `spawn`, `sleep`, `channel` and `.await`, and one rule: a channel that must
  outlive a reload is made outside the view. The other threads' code does not change.
- The budget of values a poll is the prototype's, and tokio's own does not apply outside tokio:
  uwui's first stage sets it, and whether channels are bounded. Unbounded, what other threads send
  while the host runs no frame, a window hidden for instance, waits in memory.
- A task that a task spawns belongs to the root's scope in the prototype; uwui's first stage gives
  it its parent's scope.
- A list fed by its changes by key, at their cost and not the list's, stays uwui's first stage's
  ([ADR-0205](0205-signals-change-uwuis-interface.md)). The channel only carries those changes.
- The figures are in
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md):
  a change from another thread to the frame that shows it, a burst and a batch, and a frame at rest
  while a thousand tasks wait.
- The prototype is thrown away once M6d.1 ends. uwui's first stage builds the executor and the
  channel in its core, and the adapter for tokio apart.
