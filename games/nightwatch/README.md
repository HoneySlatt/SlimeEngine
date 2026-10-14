# Nightwatch

A small stealth game made with SlimeEngine, through the engine's public API only: the first game
of the engine, made in Stage 3c, and the sample that every later stage grows
([ADR-0052](../../docs/adr/0052-amend-roadmap-before-0-1.md),
[ADR-0055](../../docs/adr/0055-games-in-the-workspace.md)).

## The game

At night, in a courtyard closed by walls, the player, a small glowing slime, crosses from the
entrance at the west end to the gate in the east wall. Inner walls split the courtyard into three
parts, and crates stand where one can hide. The moon casts the shadows the player hides in.

Planned for Stage 3c, and not there yet: guards who patrol with flashlights, fixed floodlights, a
switch that turns one off, being seen, which starts the game again, and reaching the gate, which
wins (`docs/PLAN.md`, Stage 3c).

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
| Escape | Quit |

Keys are read by their position on the keyboard, so that the four movement keys sit in the same
place on every layout. CI builds the game for Windows with the examples: `nightwatch.exe` is in
the `slime-examples-windows` artifact of each run.

## How it is made

| Module | What it holds |
|---|---|
| `main.rs` | The application: the window, the game's loop through `slime_app::App`, the command line |
| `rules.rs` | The rules, without the GPU: the player's movement, and its collisions with the blocks, as a circle against boxes on the ground |
| `courtyard.rs` | The courtyard: its blocks, its entrance and its exit |
| `controls.rs` | Which of the game's keys are held |
| `camera.rs` | The camera that follows the player |
| `view.rs` | What the player sees: the renderer, the meshes, the materials, the moon and the night sky |
| `meshes.rs` | The meshes, built in code: a cube and a sphere |

The rules run at the engine's fixed step, 1/60 s, so that a game plays the same on every machine;
the camera turns at the frame's rate. The rules need no GPU, so that their tests, and the scripted
games of the next milestones, run anywhere.

What the game needed and the engine did not give is recorded, as it is met, in the findings of
Stage 3c in [`docs/PLAN.md`](../../docs/PLAN.md#findings).

## Dependencies

The engine's crates only: `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`; and
`tracing` for the log.

## Must not

- Use anything but the engine's public API: no `wgpu` feature, no code of the examples.
- Be a dependency of an engine crate.
