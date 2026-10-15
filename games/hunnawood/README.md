# Hunna Wood

A role-playing game made with SlimeEngine, through the engine's public API only, seen in the third
person, in a dark-fantasy world: the sample that every stage grows, since Stage 4a
([ADR-0067](../../docs/adr/0067-hunna-wood-replaces-nightwatch.md),
[ADR-0055](../../docs/adr/0055-games-in-the-workspace.md)).

## The game

Stage 4a makes one zone of the game's starting region, a forest path, which the hero explores by day
and, from M4a.8, by night ([`docs/PLAN.md`](../../docs/PLAN.md#stage-4a--cooked-assets)). The game
reads it from cooked data ([below](#the-zone)). The hero starts in the south, under an overcast sky,
and follows the path through the forest, over a stone bridge, past a campfire in a clearing, to a
ruined chapel. In the nave lies an old key: the hero takes it on walking up to it. The key opens the
sealed door in the chapel's north wall, which opens as the hero comes to it, onto a terrace that
looks over the valley towards the region's capital. Without the key, the door does not move. By
the campfire, the hero can rest.

The hero moves relative to the camera, which stays behind it and a little above, turns to face
where it moves, and is stopped by trees, rocks, walls and the edge of the zone, sliding along them.
When one of them stands between the camera and the hero, the camera comes nearer, in front of it.

There is no combat, dialogue, physics, animation or sound: the engine has none of them yet. The
hero is a still figure that glides on a flat ground, without jumping; no text tells the player
what happens, which the log says (`SLIME_LOG=info`).

## Playing

The game needs its zone cooked first, from the repository's root ([its assets](#its-assets)):

```sh
scripts/fetch-assets.sh zone/
cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
cargo run -p hunnawood                     # play
cargo run -p hunnawood -- --frames 120     # closes itself after 120 frames
cargo run -p hunnawood -- --assets <dir>   # reads the cooked data of another directory
```

Without `--assets`, it reads `hunnawood` beside the program if that directory holds a manifest, as
where CI ships it, and `target/cooked/hunnawood` otherwise.

| Key | Does |
|---|---|
| W, A, S, D (Z, Q, S, D on a French keyboard) | Move, relative to the camera |
| Left and right arrows | Turn the camera around the hero |
| E | Rest, by the campfire |
| R | Start again: the hero at the start, the key in its place, the door closed |
| F1 | Show or hide what each frame costs, in the engine's dev overlay |
| Escape | Quit |

Keys are read by their position on the keyboard, so that the four movement keys sit in the same
place on every layout. CI builds the game for Windows with the examples: `hunnawood.exe` is in the
`slime-examples-windows` artifact of each run, with its cooked zone beside it.

## How it is made

| Module | What it holds |
|---|---|
| `main.rs` | The application: the window, the game's loop through `slime_app::App`, the command line |
| `assets.rs` | Where the cooked data is, and the IDs of the zone and the hero, from their metadata files |
| `rules.rs` | The rules, without the GPU: the hero's movement, its heading, and its collisions, as a circle on the ground against the zone's colliders and edge; the key, the door and the campfire |
| `zone.rs` | The zone, read from the cooked scene: its ground, its colliders, boxes turned about the vertical and cylinders, where the hero starts, and the places of the key, the door and the campfire; the tests of the cooked zone |
| `controls.rs` | Which of the game's keys are held |
| `camera.rs` | The camera that follows the hero, coming nearer when a collider would stand between them |
| `view.rs` | What the player sees: the renderer, the cooked zone and hero, the overcast sky and the zone's sun behind its clouds |
| `statistics.rs` | The dev overlay that shows what each frame costs |

The rules run at the engine's fixed step, 1/60 s, so that a game plays the same on every machine;
the hero is drawn between its last two steps, by the share of a step that the frame comes after the
last one (`Context::fixed_step_alpha`), so that it moves smoothly at any frame rate, and the camera
turns at the frame's rate. The rules need no GPU, so that their tests run anywhere. A move is made
in parts of at most half the hero's radius, each pushed out of the colliders it overlaps, so that no
step carries the hero through a thin collider or past a corner. The view leaves out what the zone's
nodes under the key's and the door's markers place once the key is taken and the door open
(`LoadedScene::instance_nodes`).

The game keeps the workarounds that the first game, *Nightwatch*, found for what the engine does not
give yet: it keeps the keys held, its own collisions, and the hero's previous position
([ADR-0057](../../docs/adr/0057-what-the-first-game-asks-of-the-next-stages.md)).

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
the zone's Blender script reads. `assets/zone/` holds what that script exports from them, about
120 MB, which the cooker cooks; the game needs only that part:

```sh
scripts/fetch-assets.sh zone/
cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
```

The cooker takes about half a minute in a release build, and several minutes in a debug one.

To add an asset: take it from a site that publishes it under CC0, add it to the list with its
files, attach the files to a new release of the mirror, and give each its mirror's address.

## The zone

A forest path, built in Blender by a script, [`zone/build_zone.py`](zone/build_zone.py), from the
sources and from a readable file of placements, [`zone/placements.toml`](zone/placements.toml).
The hero starts in the south, walks a path under oaks and conifers, among mossy rocks and ferns,
crosses a dry gully on a stone bridge, passes a campfire in a clearing, and reaches a ruined gothic
chapel; its north wall holds a sealed door, behind which a walled terrace looks over a cliff
towards the region's capital, painted on a card far away. A line of trees painted the same way
closes the other sides. Lanterns on posts line the path. The script also makes the hero: Blender
Studio's body, its arms lowered, dressed in a tunic, trousers, boots and a hooded cloak. Its
modules:

| Module | What it makes |
|---|---|
| `build_zone.py` | Runs the others, exports the zone and the hero, and with `--preview DIR` renders views of the zone with EEVEE |
| `common.py` | Materials, meshes, and the colliders, markers and lights that the game reads |
| `land.py` | The ground, the path, the gully and the cliff |
| `ruins.py` | The bridge, the chapel, its door's wall and the terrace |
| `props.py` | Poly Haven's models, made lighter, the lanterns and their posts, the campfire, the door and the key |
| `vegetation.py` | The oaks and conifers, their leaf cards, and the scatter of trees, rocks and ferns |
| `backdrop.py` | The pictures of the capital and of the forest beyond, rendered with Cycles, and their cards |
| `hero.py` | The hero |
| `preview.py` | The views of `--preview` |

Run it with Blender 5.2, from the repository's root, once the sources are fetched; it writes
`assets/zone/`, and takes a few minutes, most of them rendering the pictures, which it keeps in
`assets/build/` and renders again only when they are deleted:

```sh
blender --background --factory-startup --python games/hunnawood/zone/build_zone.py
```

Blender's output differs a little from one run to the next, so the export is a source like the
others ([ADR-0070](../../docs/adr/0070-the-sample-games-source-assets.md)): after a change, give
the new files an ID (`cargo run -p slime-cook -- scan games/hunnawood/assets/zone`), commit their
metadata files, attach the files to a new release of the mirror and list them in
`assets/sources.tsv`, so that everyone cooks the same files.

Besides meshes, materials and lights, the export holds what the game reads, as custom properties
of the scene's nodes, which the cooker passes on as their `extras`:

- **Colliders**: `collider` is `"box"` or `"cylinder"`, `height` is in metres. A box's node is
  turned about the vertical only; its scale is half its size. A cylinder's scale is its radius.
- **Markers**: `marker` names a place. `start`, where the hero starts, facing along the node's
  −Z axis; `item`, the key, whose mesh is the marker's child; `door`, the sealed door, whose leaves
  and collider are its children, which the game takes away once the door opens; `campfire`, where
  the hero rests; `terrace`, behind the door; `bounds`, whose scale is half the size of the ground
  the hero may walk.
- **Lights**: `time` is `"day"` or `"night"`: the sun by day; by night, the moon, a spot light in
  each lantern, which casts shadows, and the campfire's point light.

Tests read the cooked zone. In `src/zone.rs`: one marker of each kind, a start clear of every
collider, the key and the campfire within reach, the terrace not until the door opens, and the zone
within its budgets, which [`docs/PLAN.md`](../../docs/PLAN.md#m4a6--the-zones-sources) sets. In
`src/rules.rs`: a walk along the path that rests by the campfire, takes the key, opens the door and
reaches the terrace, then starts again from the same state; a walk to the door without the key,
which it stops. They need the cooked zone, so `cargo test` leaves them out: `scripts/check.sh`
fetches and cooks it, then runs them, and after the commands above,
`cargo test -p hunnawood -- --ignored` does.

## Dependencies

The engine's crates only: `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`, and
`slime-devui` for the dev overlay, engine tooling that turns `slime-gfx`'s `wgpu` feature on for
itself (the game uses no wgpu type); `slime-assets`, which reads the cooked zone; and `tracing` for
the log.

## Must not

- Use anything but the engine's public API: no `wgpu` feature of its own, no code of the examples.
- Be a dependency of an engine crate.
