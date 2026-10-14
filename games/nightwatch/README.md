# Nightwatch

A small stealth game made with SlimeEngine, through the engine's public API only: the first game
of the engine, made in Stage 3c, and the sample that every later stage grows
([ADR-0052](../../docs/adr/0052-amend-roadmap-before-0-1.md),
[ADR-0055](../../docs/adr/0055-games-in-the-workspace.md)).

## The game

At night, in a courtyard closed by walls, the player, a small glowing slime, crosses from the
entrance at the west end to the gate in the east wall. Inner walls split the courtyard into three
parts, joined by an opening at the south end of the first and the north end of the second, and
crates stand where one can hide. A guard walks a round in each part, a flashlight in hand, slower
than the player, stopping at each corner of its round to look round; a floodlight on the north
wall lights the middle part, and another, on the east wall, the way to the gate.

A light that sees the player ends the game: inside its cone and its reach, with no wall or crate
in between. So does a guard that comes too near, whichever way it looks. The slime turns red under
a red light, and a new game starts a moment later. The switch on the north wall of the east part
turns the second floodlight off: its button glows red, brighter within reach, and green once used.
Reaching the gate wins: the courtyard lights up under a green light, and a new game starts.

The engine's spot lights cast no shadows, so a flashlight seems to light what stands behind a crate:
the rules, which decide who sees the player, know better than the picture. Stage 3d gives spot
lights their shadows
([ADR-0056](../../docs/adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)).

There is no text, sound or physics: the engine has none of them yet. The game moves the player
and stops it at walls and crates on its own, on a flat ground, without jumping.

## Playing

```sh
cargo run -p nightwatch                     # play
cargo run -p nightwatch -- --frames 120     # closes itself after 120 frames
```

| Key | Does |
|---|---|
| W, A, S, D (Z, Q, S, D on a French keyboard) | Move, relative to the camera |
| Left and right arrows | Turn the camera around the player |
| E | Use a switch within reach |
| R | Start a new game |
| F1 | Show or hide what each frame costs, in the engine's dev overlay |
| Escape | Quit |

Keys are read by their position on the keyboard, so that the four movement keys sit in the same
place on every layout. CI builds the game for Windows with the examples: `nightwatch.exe` is in
the `slime-examples-windows` artifact of each run.

## How it is made

| Module | What it holds |
|---|---|
| `main.rs` | The application: the window, the game's loop through `slime_app::App`, the command line |
| `rules.rs` | The rules, without the GPU: the player's movement and its collisions with the blocks, as a circle against boxes on the ground; the guards' rounds; what sees the player; the switch; winning, losing and a new game |
| `sight.rs` | What a light sees: a cone on the ground, its reach, and the line of sight that blocks cut |
| `courtyard.rs` | The courtyard: its blocks, the guards' rounds, the floodlights, the switch, its entrance and its exit |
| `controls.rs` | Which of the game's keys are held |
| `camera.rs` | The camera that follows the player, looking down more steeply, or coming nearer, when a wall would stand between them |
| `view.rs` | What the player sees: the renderer, the engine's cube and sphere, the materials, the moon and the night sky, the guards' flashlights and the floodlights as spot lights, and how a game ends, in light |
| `statistics.rs` | The dev overlay that shows what each frame costs |

The rules run at the engine's fixed step, 1/60 s, so that a game plays the same on every machine;
what moves is drawn between its last two steps, by the share of a step that the frame comes after
the last one (`Context::fixed_step_alpha`), so that it moves smoothly at any frame rate, and the
camera turns at the frame's rate. The rules need no GPU, so that their tests run anywhere: among
them, scripted games that cross the courtyard to the gate, and that walk straight into a light.

What the game needed and the engine did not give was recorded as the findings of Stage 3c; the
stage each one goes to is in
[ADR-0057](../../docs/adr/0057-what-the-first-game-asks-of-the-next-stages.md).

## Dependencies

The engine's crates only: `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`, and
`slime-devui` for the dev overlay, engine tooling that turns `slime-gfx`'s `wgpu` feature on for
itself (the game uses no wgpu type); and `tracing` for the log.

## Must not

- Use anything but the engine's public API: no `wgpu` feature of its own, no code of the examples.
- Be a dependency of an engine crate.
