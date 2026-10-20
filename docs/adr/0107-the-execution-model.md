# 0107. The execution model

- Status: Accepted; amends [0009](0009-frame-timing.md) (game time apart from real time),
  [0010](0010-main-loop-ownership.md) and [0020](0020-rendering-in-the-main-loop.md) (a game as a
  simulation and a presentation, run by a host; what each method of a frame may change),
  [0072](0072-stage-4b-live-assets.md), [0073](0073-loading-scenes-in-the-background.md) and
  [0074](0074-uploading-textures-without-holding-up-frames.md) (the engine's task pool, and one
  owner of the copies to the GPU), [0079](0079-the-engines-own-world.md) (the phases of a frame,
  decided before Stage 7's systems) and
  [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (the time of a game, brought
  from Stage 7 to Stage 6); amended by [0118](0118-the-task-pool-and-the-copies-to-the-gpu.md) (the
  loads' tasks two at a time, and a budget of copies that lasts a frame or a frame's time)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 builds the editor on it, where the
engine's work runs and when: the execution model. The audit of 0.4 found the following, and
measured it on the owner's RX 6900 XT at `v0.4.0` (its figures below are its own unless a
document of the repository is named; its report is kept outside the repository, as the plan says):

- **Threads started four ways, none with a priority or a budget** (C1-06, C7-05). `slime-render`
  has a private pool of up to four threads that load scenes, with one queue in the order of arrival
  ([ADR-0072](0072-stage-4b-live-assets.md), [ADR-0073](0073-loading-scenes-in-the-background.md));
  each load starts a thread of its own that waits for its tasks; computing a sky's environment
  starts one thread per logical core, sixteen on that machine, from the thread that asks, the main
  thread when `slime-world-render`'s `FrameBuilder` makes the sky again; the cooker starts its own.
  The loading tasks create the GPU objects and submit their copies as they read them
  ([ADR-0074](0074-uploading-textures-without-holding-up-frames.md)). A load slows every frame
  while it lasts (B4-F01): the render CPU went from 0.5 to 2.9 ms with four threads loading, and to
  16.6 ms at the 99th percentile, a frame missed at 60 Hz, with textures of 4096² texels; on
  *Hunna Wood*'s zone, from 0.54 to 0.77 ms at the median and 2.2 ms at the 90th percentile
  ([`measurements.md`](../measurements.md), P4's outcome in the plan). In the audit's test, one
  thread that submits the copies cost the frames 0.85 to 0.91 ms where four cost 2.9, and sent
  65 % more bytes per second: the cause is memory bandwidth, the locks of wgpu and the driver, and
  the GPU's one queue, not too few threads. At *Hunna Wood*'s start, its two skies, computed on
  every core while the pool loads, took about 33 ms each where each takes 22 to 25 ms with nothing
  else running, and the hero arrived after 149 ms where it arrived after 101 ms with the skies not
  competing for the cores. The editor adds background work of its own (thumbnails, cooking,
  shaders compiled, previews), and Stage 10a's streaming makes a load the case of every frame;
  [ADR-0098](0098-the-games-the-engine-aims-at.md) asks that a hidden load never drop a frame.
- **Every method of a frame may change everything** (C1-01, C5-01). A frame runs its fixed steps,
  `App::update`, the transforms, then `App::render`, where the game draws; every method receives a
  `Context` that holds the world, the GPU and the window. *Hunna Wood* changes its world while it
  draws: the fire's flicker, the dev overlay's edits, and its whole world when its cooked data
  changes. No render thread can draw one frame while the next is built, the editor's edits have no
  moment of their own, and nothing tells the state a game simulates from what it shows.
- **One time** ([ADR-0009](0009-frame-timing.md)). The engine has the frame's time, limited after a
  pause, and fixed steps taken from it; nothing pauses a game, slows it down or runs one step of
  it. Play in the editor pauses and steps a game; ADR-0098's painted action role-playing game slows
  its combat while its UI and its camera run at real speed. ADR-0097 left the time of a game to
  Stage 7, undecided.
- **A game is one block** (C1-01, C5-01). `App` binds a game's rules to the process, the window and
  the GPU; `Context::request_exit` ends the run, so that a game run by the editor as it is would
  close the editor on Escape. *Hunna Wood*'s rules are already a function of its world,
  `Game::step`, but its fixed step waits until its view shows the zone, and its camera and the keys
  it holds live outside the world. The editor runs a game in a child process first (ADR-0097),
  which it pauses, steps and inspects; ADR-0098 asks for a simulation that holds the authority apart
  from what is shown, so that a server can one day run it alone; tests check a game's rules without
  a GPU; [ADR-0099](0099-ai-agents-drive-the-engine.md)'s agents play a game with scripted input.
  *Hunna Wood*'s rules cost 43 µs per frame.

The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists: the background work, which the first two parts below set out;
the phases of a frame; game time; and the split of a game.

## Decision

**One task pool, the engine's own, with two priorities.** A crate at the bottom of the workspace,
on the standard library alone, holds the engine's task pool, which replaces `slime-render`'s
private one. Its threads are the machine's logical cores but one, at least one: a core stays for
the main thread. A task has one of two priorities:

- *frame*: work that the frame being made waits for, such as a sky's environment;
- *background*: work that no frame waits for, loads, cooking, the editor's thumbnails.

A free thread takes a frame task before any background task. Background tasks never hold every
thread of the pool when it has more than one, so that a frame task always finds a thread. A task
still never waits for another, which could be queued behind it, until Stage 7 gives tasks
dependencies. Compute work runs on the pool: no crate starts threads of its own, the editor
included. A crate starts a thread of its own only for work that waits rather than computes, as
`slime-app`'s thread that logs a termination signal, or the renderer's that submits the copies to
the GPU, and its rustdoc says why. The figures, how many threads background tasks may hold, are
set by measurement when the pool is built, and its rustdoc holds them.

**One owner of the copies to the GPU, within a budget of bytes per frame.** Loading tasks read,
check and decode the cooked data, create the GPU objects, and write the texels and vertices into
staging buffers that are reused from one load to the next. One thread of the renderer's alone
submits the copies from those buffers to the GPU, at most a budget of bytes per frame, which each
frame renews; what a frame's budget leaves waits for the next frames. A load still ends when all
its copies are done, and a game still takes a scene whole
([ADR-0072](0072-stage-4b-live-assets.md)). A load no longer starts a thread of its own to wait
for its tasks: the last of its tasks to end completes it, which the game sees when it polls the
load. The budget is set by measurement on the RX 6900 XT,
against ADR-0072's bar, and the renderer's rustdoc holds it. The sky's environment runs as frame
tasks on the pool; that it is computed on the CPU at all stays known debt.

**The phases of a frame.** A frame runs in this order, and each phase says what may change the
world:

| Phase | What runs | The world |
|---|---|---|
| 1. Start | The host applies what arrived: background work done, loads taken, and the commands queued since the last frame (the editor's, the dev overlay's, hot reload's); it gathers the input | changed by the host |
| 2. Fixed steps | The simulation's fixed steps, on game time | changed by the simulation |
| 3. Update | The simulation's update, once per frame | changed by the simulation |
| 4. Transforms | The world's global transforms computed | changed by the engine alone |
| 5. Extraction | The presentation reads the world and makes what it draws | read only |
| 6. Render and present | The frame drawn and presented | read only, or not at all |

A change asked outside phases 1 to 3, by the editor, the dev overlay, a load or the presentation,
is a command, queued and applied at phase 1 of the next frame: the form of a command is the
editor's command system, which P6 decides with stable references. The types hold these rules:
what runs at phases 5 and 6 receives the world as `&World` or not at all, so that a game that
writes the world while it draws does not compile. Until P6's frame contract decides a frame that
the renderer owns, phases 5 and 6 are one call of the presentation, which reads the world and never
writes it. The code has the phases that have users, and no phase without one: Stage 7 inserts its
systems in the order that this text sets, which [ADR-0079](0079-the-engines-own-world.md) left to
Stage 7: at phase 1, the input mapped to actions; at phase 2, the game's logic, then physics and
what physics writes back; at phase 3, the logic that runs once per frame, then animation; at
phase 4, the transforms; at phase 5, audio and the presentation, which read the world.

**Game time apart from real time.** The engine keeps two times:

- *real time*, the time between frames, limited after a pause as ADR-0009 decided, for the UI, the
  camera, the editor, the dev overlay and, later, the network;
- *game time*, the simulation's, which advances at a *speed*: 1 by default, less to slow it down,
  0 to pause it. While game time is paused, a step command runs exactly one fixed step.

A fixed step is always the same duration of game time, 1/60 s by default: the speed changes how
many steps run per second of real time, never their duration, so that a simulation gives the same
results at every speed, as replays, physics and the network need. At a quarter of the speed, a
step runs about every fourth frame at 60 Hz, and the presentation interpolates between the last two
steps, as it does already. The limits of ADR-0009, the longest frame and the most steps per frame,
apply to game time as to real time, and time stays in whole nanoseconds. Each phase is given both
times and reads the one it lives on: the simulation game time, the presentation and the UI real
time. Game time per object, an enemy slowed while the hero is not, is left to Stage 7's animation
and physics, if a game asks for it.

**A game is a simulation and a presentation, run by a host.**

- The *simulation* is a game's rules. It receives the world, game time and the input as data: the
  keys and buttons held, and later the actions they map to. It has no GPU, no window, and does not
  end the process: it asks to stop, and its host decides what stopping means.
- The *presentation* is what the player sees and hears: the camera, the drawing, the dev overlay,
  later sound. It keeps state of its own, the camera's, and reads the world at phase 5 without
  writing it.
- The *host* owns the loop, the world, both times and the decision to stop. The game's program is
  one host; the child process that the editor starts, a test, and one day a dedicated server are
  others, which run the same simulation, the last two without a presentation. Asked to stop, the
  game's program ends; the editor stops playing it.
- The input goes through the host: from the keyboard and the mouse, from a replay's file, from a
  test's or an agent's script, one day from the network. The simulation cannot tell them apart.

`App` gives way to these two parts, whose shape the code decides with its thirteen implementations
today, eight in `slime-app`'s tests, four examples and *Hunna Wood*, and the three examples of
the crates' READMEs. *Hunna Wood*'s fixed step no longer waits for its view: its program keeps
game time paused until the zone is drawn. Its fire's flicker leaves `render`, for the simulation's
update or for what the presentation changes in the frame it extracts, not in the world; its
overlay's edits are commands; its cooked data that changes is applied at phase 1. Its camera goes
to the presentation, the keys held to the input.

**When it is built.** At Stage 6, after frustum culling and instancing, its first milestone, and
before play in the editor, which builds on it: the pool and the owner of the copies, about one
milestone, which ends B4-F01; the phases, game time and the split of `App`, done together in about
one more. Stage 7's job system grows from the pool, with dependencies between tasks, loops split
across threads, and work stealing if a measurement asks for it, without changing what the editor
calls.

## Alternatives considered

The background work:

- **Waiting for Stage 7's job system**: nothing to do now, but the editor would add its own threads
  meanwhile, B4-F01 would stay, and Stage 7 would take everything apart again.
- **Stage 7's job system now**, with dependencies, work stealing and loops split across threads:
  one refactoring, but at least two milestones before the editor for a need that nothing measured,
  where *Hunna Wood*'s rules cost 43 µs per frame and the measured cost is threads that compete,
  not too few; an API designed before its users, against the rule of a second concrete use.
- **`rayon`**: widely used, and with a license compatible with MPL-2.0, but a new dependency, a
  global pool without priorities nor a place for loads, which
  [ADR-0073](0073-loading-scenes-in-the-background.md) rejected.
- **The copies submitted by the main thread** within a budget, ADR-0072's fallback: it adds the
  submissions to the frame's own CPU, where a thread of their own takes them off it.

The phases of a frame:

- **The frame as it is**, every method free to change the world: no render thread could draw one
  frame while the next is built, the editor's edits would come at any time, and each new game
  would learn to write the world while it draws.
- **A scheduler now**, the phases as objects in the code where the engine and the game register
  systems: Stage 7's work, brought forward before physics, animation or audio exist to use it;
  its shape depends on Stage 7's job system, which it would design too early.

Game time:

- **Nothing in the engine**, each game slowing and pausing itself: the editor could not pause a
  game that it does not control, and Stage 7's animation, particles and audio would not know which
  time to follow.
- **Shorter steps to slow a game down**: the simulation would give other results at other speeds.
- **Game time per object now**: nothing uses it before Stage 7, and it adds, as a factor per
  object, onto the two times.

The split of a game:

- **`App` as one block**, the child process of the editor keeping it from a game's crash: the
  editor could not pause the simulation without freezing what is shown, no server could run
  without a GPU, a game's rules could not be tested without one, and each new game would mix both
  parts, dearer to split later.
- **A model of the network now**, a simulation that is copied, rolled back and simulated again for
  the prediction of a shooter: it depends on the world's shape, which P6 decides apart, and on the
  network of Stage 10b; nothing uses it before.

## Consequences

- `App` and its thirteen implementations change once, for the phases and the split together. Code
  that writes the world in `render` moves to the simulation's update, or becomes a command.
- The editor's background work, and every crate's, runs on the engine's pool: no crate adds a
  thread that computes.
- A load costs the frames at most the work of submitting a budget of bytes, and takes longer when a
  frame's budget is smaller than what it brings: the budget trades one against the other, and is
  measured.
- A game that slows its time down sees its steps run less often, never shorter; one that wants
  shorter steps sets its fixed step.
- A dedicated server, a test of a game's rules and a replay run the simulation without a
  presentation; the editor pauses and steps it in its child process.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds the execution model before play in
  the editor; Stage 7's job system grows from the engine's pool.
