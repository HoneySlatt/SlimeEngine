# Hunna Wood

A role-playing game made with SlimeEngine, through the engine's public API only, seen in the third
person, in a dark-fantasy world: the sample that every stage grows, since Stage 4a
([ADR-0067](../../docs/adr/0067-hunna-wood-replaces-nightwatch.md),
[ADR-0055](../../docs/adr/0055-games-in-the-workspace.md)).

## The game

Stage 4a makes one zone of the game's starting region, a forest path, which the hero explores by day
and by night ([`docs/PLAN.md`](../../docs/PLAN.md#stage-4a--cooked-assets)). Until that zone is made
from its sources and cooked, the game is played on a test ground built in code: a flat square of
ground 40 m wide, under an overcast sky, where the hero walks among the pieces of a ruined wall, a
fallen block, a colonnade of four pillars and the trunks of a few trees, all shapes made in code.

The hero is a stand-in for now: a dark body, a head, and a face on the side it looks to. It moves
relative to the camera, which stays behind it and a little above, turns to face where it moves,
and is stopped by the ruins, the pillars, the trunks and the edge of the ground, sliding along
them. When one of them stands between the camera and the hero, the camera comes nearer, in front of
it.

There is no combat, dialogue, physics, animation or sound: the engine has none of them yet. The
game moves the hero on its own, on a flat ground, without jumping.

## Playing

```sh
cargo run -p hunnawood                     # play
cargo run -p hunnawood -- --frames 120     # closes itself after 120 frames
```

| Key | Does |
|---|---|
| W, A, S, D (Z, Q, S, D on a French keyboard) | Move, relative to the camera |
| Left and right arrows | Turn the camera around the hero |
| R | Start again |
| F1 | Show or hide what each frame costs, in the engine's dev overlay |
| Escape | Quit |

Keys are read by their position on the keyboard, so that the four movement keys sit in the same
place on every layout. CI builds the game for Windows with the examples: `hunnawood.exe` is in the
`slime-examples-windows` artifact of each run.

## How it is made

| Module | What it holds |
|---|---|
| `main.rs` | The application: the window, the game's loop through `slime_app::App`, the command line |
| `rules.rs` | The rules, without the GPU: the hero's movement, its heading, and its collisions, as a circle on the ground against the zone's colliders and edge |
| `zone.rs` | The zone: its ground, its colliders, boxes turned about the vertical and cylinders, and where the hero starts; the test ground |
| `controls.rs` | Which of the game's keys are held |
| `camera.rs` | The camera that follows the hero, coming nearer when a collider would stand between them |
| `view.rs` | What the player sees: the renderer, the engine's cube and sphere and a cylinder made here, the materials, the overcast sky and the sun behind its clouds, the hero's stand-in |
| `statistics.rs` | The dev overlay that shows what each frame costs |

The rules run at the engine's fixed step, 1/60 s, so that a game plays the same on every machine;
the hero is drawn between its last two steps, by the share of a step that the frame comes after the
last one (`Context::fixed_step_alpha`), so that it moves smoothly at any frame rate, and the camera
turns at the frame's rate. The rules need no GPU, so that their tests run anywhere. A move is made
in parts of at most half the hero's radius, each pushed out of the colliders it overlaps, so that no
step carries the hero through a thin collider or past a corner.

The game keeps the workarounds that *Nightwatch* found for what the engine does not give yet: it
keeps the keys held, its own collisions, and the hero's previous position
([ADR-0057](../../docs/adr/0057-what-the-first-game-asks-of-the-next-stages.md)). The engine's
simple shapes include no cylinder, so `view.rs` makes one for the test ground's pillars and trunks
and the hero's stand-in, until the zone's models replace them.

## Its assets

The zone is made from free assets, all under CC0 1.0, and from what a Blender script adds to them
([ADR-0070](../../docs/adr/0070-the-sample-games-source-assets.md)). They are not in git:
[`assets/sources.tsv`](assets/sources.tsv) lists each of them, where it was published, its authors
and its licence, and for each of its files its size, its SHA-256, the address it was published at
and the address of its copy on the game's mirror, the immutable releases of
[HoneySlatt/SlimeEngine-assets](https://github.com/HoneySlatt/SlimeEngine-assets). Fetch them
from the repository's root:

```sh
scripts/fetch-assets.sh            # every file, into games/hunnawood/assets/
```

The script checks the whole list first, and refuses an asset under another licence than CC0; then
it downloads each file from the mirror, or from where it was published if the mirror fails, and
checks it against its SHA-256. A file already there is not downloaded again. Git ignores what it
fetches, and keeps the list and the metadata files.

`assets/sources/` holds the files as they were published, about 190 MB: textures and models from
Poly Haven, leaves from ambientCG, and the human body of Blender Studio's *Human Base Meshes*, which
the zone's Blender script reads. The zone itself is not made yet: until it is, the game is played
on the test ground.

To add an asset: take it from a site that publishes it under CC0, add it to the list with its
files, attach the files to a new release of the mirror, and give each its mirror's address.

## Dependencies

The engine's crates only: `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`, and
`slime-devui` for the dev overlay, engine tooling that turns `slime-gfx`'s `wgpu` feature on for
itself (the game uses no wgpu type); and `tracing` for the log.

## Must not

- Use anything but the engine's public API: no `wgpu` feature of its own, no code of the examples.
- Be a dependency of an engine crate.
