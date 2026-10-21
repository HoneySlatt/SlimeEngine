# 0121. A game as a simulation and a presentation

- Status: Accepted; amends [0057](0057-what-the-first-game-asks-of-the-next-stages.md) (the keys
  and buttons held, kept by the engine at Stage 6), [0107](0107-the-execution-model.md) (the shape
  of a game's two parts and of its host, what the presentation reads, extraction before the
  window's image, the update while game time is paused, the pause apart from the speed, and a
  game's cooked data that changed applied by its own simulation until the editor's hot reload),
  [0110](0110-how-the-editor-runs-a-games-code.md) (a game's library holds its presentation too)
  and [0120](0120-a-frame-that-owns-what-it-draws.md) (the frame's lights name their entities)
- Date: 2026-10-21

## Context

[ADR-0107](0107-the-execution-model.md) decided the phases of a frame, game time apart from real
time, and a game split into a *simulation*, its rules, and a *presentation*, what the player sees,
run by a *host*; it left the shape of the two parts to the code, which would decide it with the
thirteen implementations of `App` of the time, eight in `slime-app`'s tests, four examples and
*Hunna Wood*, and the three examples of the crates' READMEs.
[ADR-0111](0111-the-frame-contract.md) made the presentation's phases two calls: extraction builds
the frame from the world, read only, and rendering draws it without the world.
[ADR-0109](0109-stable-references-and-undo.md) decided that a command names its entities by their
persistent identities, and [ADR-0110](0110-how-the-editor-runs-a-games-code.md) that a game is a
library and its program. [`docs/PLAN.md`](../PLAN.md) gathers them in M6a.5, which builds them.

Before it, every method of `App` received a `Context` that held the world, the GPU and the window:
*Hunna Wood* changed its world while it drew, the fire's flicker and the dev overlay's edits, kept
the keys held itself, held its camera's direction outside the world, made its fixed steps wait for
its view to show the zone, and put the fire's light back before each save, since the world held
each frame's flicker.

The owner chose on 2026-10-21 that the presentation reads the world alone, not its simulation.

## Decision

**Two traits, and a host that a test drives.** `slime-app` gives:

- `Simulation`: `start`, `fixed_step`, `update` and `stop`, each given the world to change and a
  `SimulationContext`, which holds the input, real time and game time, and the run's stop, and
  neither GPU nor window. A simulation asks to stop; its host decides what stopping means.
- `Presentation`: `start`, once the window and the GPU are open; `on_event`, for each event of the
  window; `extract`, given the world as `&World`; `render`, given the window's image and not the
  world; `gpu_lost` and `gpu_restored`; `stop`. Every method that reads the world receives it as
  `&World`, so that a presentation that writes the world does not compile. Its `Context` holds the
  window, the GPU, both times, game time's controls, the commands it sends, and the run's stop.
- `Host`: the world, the input, both times and the commands queued; `Host::frame` runs the first
  four phases of a frame. A test, and one day a dedicated server, drives it itself, pressing keys,
  pausing, stepping and sending commands, without a GPU.
- `run(simulation, presentation, config)`: the host of a game's program, which adds the window,
  the GPU and the presentation. A program with no rules passes `()` as its simulation, one that
  shows nothing `()` as its presentation; a headless run calls no method of the presentation.

**The presentation reads the world alone.** It reads neither its simulation's fields nor anything
but the world and its own state: what both parts need is a component of the world, which the
editor, the agents of [ADR-0099](0099-ai-agents-drive-the-engine.md), saves and, one day, the
network see as the rest. A simulation keeps in itself only what it can make again from the world,
as handles of entities, and its settings.

**A frame.** In the order of ADR-0107: the host applies the commands queued since the last frame,
in the order they came, each refusal logged as a warning and handed back to the presentation for
that frame; the simulation's fixed steps; its update, which runs at every frame, game time paused
or not, given a game time that did not advance; the world's transforms; the input's frame ends.
With a window, the presentation then extracts, before the window's image is acquired, at every
frame, and renders when an image is acquired. Extraction needs no image: it runs as a render thread
would have it run, and a window that cannot show a frame, minimized, still sees its presentation's
loads, commands and controls go on. The phases have profiling spans: `fixed_step`, `update`, which
holds the transforms, `extract`, `acquire`, `render`, `present`.

**Game time.** The host's game clock advances by the frame's real time times a speed, rounded to the
nanosecond, finite and above zero, 1 at the start; pausing stops it apart from the speed, which it
keeps. A step asked for while paused runs at the next frame, at most as many as a frame runs, the
rest at the next ones; asked for while game time runs, it does nothing, and running again drops
the steps not run. The fraction of a step accumulated, at which what moves is drawn, stays as it
was while paused, a step included: each step moves what is drawn by one step. `GameTime` gives
the steps run, the game time they covered, the game time that the frame added and that fraction;
`FrameTime` stays real time. The controls are the presentation's `Context`'s, and take effect at
the next frame.

**The input as data.** `slime_platform::Input` holds the keys and the mouse buttons held, and those
pressed and released since the last frame, auto-repeat left out. The host follows the window's
events, and ends the input's frame after the update. The presentation's `on_event` returns whether
it takes an event for itself, as the dev overlay takes the keys typed into a field: the input does
not see a key or a button that it puts down, and always sees one let go, so that none stays held.
The cursor and the wheel stay the presentation's events.

**Commands.** `slime_world::Command::SetProperty` names an entity by its persistent identity, a
component, a property path and a value; `World::apply` applies a command, and refuses one that
names no entity alive with `WorldError::NotFound`. The dev overlay's world panel returns commands,
which a presentation sends to its host, and keeps the entity selected by its identity too, finding
it again when a load or a hot reload replaces the world. The overlay builds its panels when the
presentation extracts, reading the world, and draws them when it renders; panels built and not
drawn hand their changes of egui's textures to the next drawing.

**A game's cooked data that changed.** ADR-0107 has the host apply it at phase 1. Until the
editor's hot reload, which the host will apply as it applies commands, a game applies its own: only
its rules know how to carry its run into scenes that changed, and they do it in their update, at
phase 3, where the simulation changes the world; its presentation loads again what it draws on its
own. The two follow the cooked data apart: scenes that cannot be played leave the run as it is,
and the meshes, materials and textures that changed load all the same. The owner chose so on
2026-10-21.

**The frame's lights name their entities.** `slime_world_render::Frame::lights` pairs each light
with its entity, so that a presentation changes in the frame what it shows without the world:
*Hunna Wood*'s fire flickers there.

**Hunna Wood.** The game is a library and its program, as ADR-0110 decides; the library holds its
presentation too, so that the program reads the command line, makes the run and runs it, and
stays thin. `HunnaWood`, the simulation, walks the hero at each fixed step where the keys held ask,
relative to where the player looks, a component of the hero's, `hunnawood::Look`, which the arrow
keys turn in the update, on real time, so that the player looks around a game paused or slowed
down; it reads the keys pressed in its update, saves at its stop, and with `--watch` carries the
run into the scenes that changed. `Screen`, the presentation, places the camera behind the hero
along that look, draws, flickers the fire and makes the key glow on game time, so that they stop
with it, holds game time paused until the zone is drawn, and loads again with `--watch` what it
draws. Its overlay pauses and runs game time on F3, runs one step on F4, and sets the speed with a
slider. The world holds the fire's light as the zone gives it, and saves no longer put it back.

## Alternatives considered

- **The presentation reads its simulation too**, which the owner rejected: simpler to write, the
  look kept in the simulation and one watch of the cooked data; but state of the game outside the
  world, which the editor, the agents and saves would not see, and a presentation tied to its
  simulation's type.
- **One trait for both parts, or `App` kept with a context that changes per phase**: one object to
  hand over, but nothing to keep the presentation from the simulation's state, and a host that
  could not run the simulation alone.
- **Extraction after the image is acquired**: what each frame draws extracted only when drawn, but
  a minimized window would hold the presentation's loads, commands and controls, and a render
  thread would split them again.
- **No update while game time is paused**: a paused world that nothing changes, but the keys, the
  look and the dev overlay's work would stop with the game, which a pause in the editor must not
  do; a simulation that changes nothing on a zero delta keeps its world still.
- **A speed of zero as the pause**: one value fewer, but running again would forget the speed.
- **The fire's flicker in the simulation's update**: no change of `slime-world-render`, but the
  world written at each frame, a visual effect saved and put back before each save, and a server
  flickering lights.
- **The look in the presentation**, sent to the simulation as a command at each frame: the camera
  wholly the presentation's, but a command per frame for what is the player's input, the
  simulation a frame late, and two copies of one value.
- **A method of the simulation at phase 1**, in which a game applies its cooked data that changed:
  ADR-0107's phase kept to the letter, but a method for one user, before the editor's hot reload
  sets what the host applies there; and the presentation would still load what it draws apart,
  unless the world told it which cooked data the rules took.
- **Commands and the input in `slime-app`**: one crate fewer to change, but a command is a change
  of the world that the editor will journal, and the keys held are the platform's input; both are
  read where `slime-app` is not.

## Consequences

- The thirteen implementations of `App`, and the three examples of the READMEs, are simulations,
  presentations or both. `Context::world`, `Context::world_mut` and `Context::fixed_step_alpha` are
  gone: the world is given to each method that may have it, and the fraction is
  `GameTime::alpha`.
- A test runs a game's rules through a host for hundreds of steps without a GPU, *Hunna Wood*'s
  among them, with scripted keys.
- *Hunna Wood*'s saves hold where the player looks, a component that saves of 0.4 and P1 lack: a
  run resumed from one looks where its hero faces, as before. M6a.7 makes the save fixture again.
- The world holds no flicker: the fire's light changes in the frame only.
- A presentation that wants the cursor's position or the wheel follows the events itself.
- M6a.6's development channel drives the host's controls and sends it commands, as the dev
  overlay does.
