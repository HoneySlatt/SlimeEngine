# Hunna Wood

A role-playing game made with SlimeEngine, through the engine's public API only, seen in the third
person, in a dark-fantasy world: the sample that every stage grows, since Stage 4a
([ADR-0067](../../docs/adr/0067-hunna-wood-replaces-nightwatch.md),
[ADR-0055](../../docs/adr/0055-games-in-the-workspace.md)).

## The game

Stage 4a makes one zone of the game's starting region, a forest path, which the hero explores by day
and by night ([`docs/ROADMAP.md`](../../docs/ROADMAP.md#stage-4a--cooked-assets--02)). The game reads it from
cooked data ([below](#the-zone)). The hero starts in the south, under an overcast sky, and follows
the path through the forest, over a stone bridge, past a campfire in a clearing, to a ruined chapel.
In the nave lies an old brass key: the hero takes it on walking up to it. The key opens the sealed
door in the chapel's north wall, which opens as the hero comes to it, onto a terrace that looks
over the valley towards the region's capital, far away. Without the key, the door does not move.
Resting by the campfire turns the day to night, and the night to day: by night, the moon lights the
forest dimly, lanterns light the path and cast their shadows, the fire flickers, its flames, its
embers and its light together, and the key glows in pulses, enough to be found by.

The hero moves relative to where the player looks, along which the camera stays behind it and a
little above, turns to face where it moves, and is stopped by trees, rocks, walls and the edge of
the zone, sliding along them. When one of them stands between the camera and the hero, the camera
comes nearer, in front of it.

There is no combat, dialogue, physics, animation or sound: the engine has none of them yet. The
hero is a still figure that glides on a flat ground, without jumping; no text tells the player
what happens, which the log says (`SLIME_LOG=info`).

## Playing

The game needs its zone cooked first, from the repository's root ([its assets](#its-assets)):

```sh
scripts/fetch-assets.sh zone/
cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
cargo run -p hunnawood                     # play
cargo run -p hunnawood -- --frames 120     # closes itself after 120 frames that draw the zone
cargo run -p hunnawood -- --assets <dir>   # reads the cooked data of another directory
cargo run -p hunnawood -- --night          # a new run starts at night, and R starts again at night
cargo run -p hunnawood -- --lose-device-at 300   # loses its GPU device after 300 frames of the zone
cargo run -p hunnawood -- --watch          # loads its zone, hero and shaders again as they change
cargo run -p hunnawood -- --new            # starts a new run rather than resume the save
cargo run -p hunnawood -- --save <file>    # saves to, and resumes from, another file
```

Without `--assets`, it reads `hunnawood-assets` beside the program if that directory holds a
manifest, as where CI ships it, and `target/cooked/hunnawood` otherwise.

Built with its default features, the game holds the engine's development channel
([ADR-0122](../../docs/adr/0122-the-development-channel.md)): started with a token, as the engine's
MCP server starts it ([`crates/slime-mcp`](../../crates/slime-mcp/README.md)), it listens on the
machine's own address for the tools and the agents that drive it, which see its view, named
`hero`, change its settings ([ADR-0127](../../docs/adr/0127-the-settings-of-a-view.md)) and read
its reports, `assets`, its asset cache's requests and memory, and `views`, its view's statistics
([ADR-0128](../../docs/adr/0128-asset-requests.md)); started
without one, as above, it does not. `cargo build -p hunnawood --no-default-features` builds it for shipping, without the
channel.

The window shows its first frame once the hero is loaded and the skies computed, while the zone
loads in the background: until it is drawn, game time is held, the hero stands at the start under
the sky, the camera turns around it, and the hero does not move. The log says when the zone is first
drawn, after how many frames, and the longest interval between two of them, the measure that
[ADR-0074](../../docs/adr/0074-uploading-textures-without-holding-up-frames.md) gives.

When the GPU device is lost (the driver crashed or was reset, as Windows resets a GPU that takes too
long to answer), the game goes on with a new one
([ADR-0075](../../docs/adr/0075-recovering-from-a-lost-gpu-device.md)): it makes its GPU objects
again and loads what its world draws again, behind the same view as at the start, while the world,
which holds no GPU object, keeps the run where it was, the hero where it stood, the key, the door
and the time of day as they were. The log says where the run was. The game goes on so wherever the
loss is found, by a frame, by a load or while the game makes its objects again, and gives up,
stopping with the error, when the device is lost again soon after each new one
([ADR-0103](../../docs/adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)). To try
it without a driver reset, `--lose-device-at` loses the device on purpose after a number of frames,
and so does the button of the dev overlay (F1).

With `--watch`, the game reads the manifest of its cooked data now and then
(`slime_assets::ManifestWatch`), its view once the zone is shown and nothing loads, and its rules at
each frame, beside `slime-cook watch` running on the zone's sources
([`CONTRIBUTING.md`](../../CONTRIBUTING.md#changing-assets-while-a-game-runs),
[ADR-0076](../../docs/adr/0076-hot-reload-of-cooked-data.md)): the meshes, materials and textures
that changed load again, the others are kept, and those drawn stay as they were until all of them
are loaded. When a scene changed, the zone's scene file, the land, the lantern or the hero, the game
makes a new game of the scenes as they are now, and compares its zone's layout with the layout of
the run's start, which it keeps ([`src/saves.rs`](src/saves.rs)), number by number, within a small
tolerance (`zone::SAME_WITHIN`), whatever the order of the colliders. When the layout changed, a
collider, the start, the key, the door, the campfire, the terrace or the bounds, the run starts
again in the new game, as R does. Otherwise the run is carried into it through a save: what the run
changed is saved and loaded into the new one, the hero where it stood, the key, the door and the
time of day as they were ([ADR-0125](../../docs/adr/0125-saves-per-scene.md)); what the run changed
of the layout itself, a collider through the world panel, does not start it again. What moved or
went changes at once, and what is new shows once its meshes are loaded, so that the zone changes in
parts rather than whole. Scenes that cannot be played leave the run as it is, while the meshes,
materials and textures that changed load all the same: the rules and the view follow the cooked data
apart ([ADR-0121](../../docs/adr/0121-a-game-as-a-simulation-and-a-presentation.md)). A reload that
fails keeps what is drawn; the log says why. The same option reads the renderer's shaders, in
`crates/slime-render/src`, when they change, and draws with them from the next frame; a shader that
does not compile leaves the frames as they were, and the log says where it fails
([ADR-0077](../../docs/adr/0077-hot-reload-of-shaders.md)).

The game resumes its save when it starts, and saves the run when it quits, however it quits; F5
saves the run, and F9 loads the save in place of the run, the hero at once where the save puts it
([ADR-0088](../../docs/adr/0088-saving-and-resuming-a-run.md)). `--new` starts a new run, whose
first save, on F5 or on quitting, writes over the save, as after R. The save is one file,
`hunnawood/save.toml` in the user's data directory: `$XDG_DATA_HOME`, else `~/.local/share`, on
Linux; `%APPDATA%` on Windows; `--save <file>` names another. It is text, which says what the run
changed of a new game by day (`slime_world::World::save`), and begins with what it says of itself:
the version of the game that wrote it, and the layout of the zone it was made on, its numbers.

A save is checked before it is resumed. One that is not resumed is set aside, so that the next save
does not write over it, and a new run starts. It is set aside under a name that says why, and never
in place of one set aside before, which keeps its own: `save.toml.later-version`, then
`save.toml.later-version.2`, and on. A save is not resumed when it cannot be read, not TOML, of a
later version of the format, or larger than the largest save that the engine reads
(`slime_serialize::MAX_SIZE`), which the game does not read further (`unreadable`); when an earlier
version of the engine wrote it, whose saves this one no longer reads, as those of the versions
before 0.5 (`earlier-version`); when a later version of the game wrote it (`later-version`); when it
was made on another layout of the zone, as hot reload starts the run again when the layout changes,
or says none (`other-layout`); when it holds what this version of the game does not know, a
component, a field or a value that a later version adds, or that an update removed without an
upgrade, which writing it back would lose (`unknown-content`); and when the run it holds cannot be
played, as when it removes the hero or the key (`unplayable`). One that can be neither read nor set
aside, locked by another program, is kept, and so is a directory in the save's place, which the game
neither reads nor moves; the run is not saved then: the log says so, and `--save` names another
file.

One copy of the game at a time holds the save: it locks a file beside it, `save.toml.lock`, while it
runs, which the system lets go however the game ends, and leaves on the disk. Another copy started
meanwhile with the same save says so, and starts a new run, which it does not save. A save is
written beside the file, in a file of the game's process, `save.toml.<process id>.new`, to the
disk, and renamed over it, so that the file is the save before or the new one, never part of one,
nor a mix of two; a game killed while it writes leaves that file beside the save. The log says what
was saved or loaded, and where the run stands. When Windows ends the session, the game is stopped
without quitting, and the run since the last F5 is lost (see the known debt in
[`docs/PLAN.md`](../../docs/PLAN.md#known-debt)).

[`fixtures/save.toml`](fixtures/save.toml) is a save made at the chapel's portal, at night, after
resting by the campfire, which every later version must resume: a test resumes it and walks on to
the terrace, and `scripts/check.sh` starts the game from a copy of it. It was made by the walk that
the game's tests script to the portal (`walk_to_the_portal_at_night`), when the engine's saves took
their layout of 0.5 ([ADR-0125](../../docs/adr/0125-saves-per-scene.md)). A change of the zone's
layout makes it a save of another layout, which the test then refuses: make it again at the same
place, with `cargo run -p hunnawood -- --new --save games/hunnawood/fixtures/save.toml`, resting by
the campfire and walking to the portal, then quitting without moving, which saves it, and say so in
the commit. [`fixtures/save-0.3.toml`](fixtures/save-0.3.toml) is the fixture of 0.3, made on the
zone before it was made of prefabs, whose identities name entities that are gone or elsewhere: a
test sets it aside as a save of an earlier version of the engine, and a new run starts.

| Key | Does |
|---|---|
| W, A, S, D (Z, Q, S, D on a French keyboard) | Move, relative to where the player looks |
| Left and right arrows | Turn where the player looks, and the camera around the hero, on real time: the game paused or slowed down too |
| E | Rest, by the campfire: the day turns to night, the night to day |
| R | Start again: the hero at the start, the key in its place, the door closed, at the time the game started at |
| F5 | Save the run |
| F9 | Load the save, in place of the run |
| F1 | Show or hide what each frame costs, in the engine's dev overlay, with the size the view draws at and its settings, which it changes (render scale, anti-aliasing, a mode of debugging that shows normals or shadows, and whether it draws the shadows, the sky and the bloom), whose slider sets the speed of game time, from a tenth to four times, and whose button loses the GPU device on purpose |
| F2 | Show or hide the world panel of the dev overlay: the world's entities by name, with a filter, and the components of the one selected, whose values it changes ([`slime-devui`](../../crates/slime-devui/README.md)) |
| F3 | Pause game time, or let it run again: the hero, the fire's flicker and the key's glow stop, and the camera still turns |
| F4 | While game time is paused, run one step of it, 1/60 s |
| Escape | Quit |

Keys are read by their position on the keyboard, so that the four movement keys sit in the same
place on every layout. While a text field of the dev overlay has the keyboard, or a menu of it is
open, the game ignores the keys, Escape included, which leaves the field or closes the menu, and
clicks on a panel are the overlay's; F1 and F2 still show and hide the panels. CI builds the game
for Windows with the examples: `hunnawood.exe` is in the `slime-examples-windows` artifact of each
run, with its cooked zone beside it.

## How it is made

The game is a library and its program
([ADR-0110](../../docs/adr/0110-how-the-editor-runs-a-games-code.md)): the library holds its
components, its rules and its saves, and the two parts that the engine's host runs
([ADR-0107](../../docs/adr/0107-the-execution-model.md),
[ADR-0121](../../docs/adr/0121-a-game-as-a-simulation-and-a-presentation.md)): its rules as a
simulation, `HunnaWood`, which changes the world on game time from the keys held, without a GPU or a
window, and what the player sees as a presentation, `Screen`, which reads the world and never writes
it. The program, `src/main.rs`, reads the command line, makes the run and runs both with
`slime_app::run`. A test runs the rules through a host of its own for hundreds of steps, keys
pressed by script, without a GPU.

The game keeps its zone, its hero and its run in the engine's world (`slime-world`), which
`slime-app`'s host owns and calls around each fixed step
([ADR-0079](../../docs/adr/0079-the-engines-own-world.md)). At the start, the game's scenes, read
from the cooked data through a library (`slime_world::SceneLibrary`), are spawned in it: the zone's
scene file, which places the land and six lanterns, and the hero's scene, each node an entity, with
the components of the game's that their nodes carry as typed properties ([below](#the-zone)):
`Collider`, `Marker`, `TimeOfDay`, `Flickers`. The world holds the run itself: the hero is where the
root of its scene stands, and the key taken, the door open and the time of day are components of the
game's, `Key` on the key, `Door` on the door and `Clock` on the zone's root; where the player looks
is a component of the hero's, `Look`, which the rules turn and the camera follows, so that both
parts read it from the world. At each step, the game hides what they say is not there
(`slime_world::Hidden`): the key once taken, the door once open, which also takes its colliders
away, and the lights of the other time of day. The run is data, which the dev overlay's world panel
(F2) changes as it changes any component, through commands that the host applies at the start of
the next frame, and which the game follows from the next step. The rules keep only the entities
they refer to and the time of day a new game starts at. Each of the game's components is reflected,
under a name such as `hunnawood::Collider`, as every component is
([ADR-0085](../../docs/adr/0085-reflection.md)), and `Game::spawn` registers them all in the world
before it spawns the zone.

Since the run is in the world, a save is the world's: what the run changed of the new game by day
that the game made and settled, and a run resumed is a new game by day to which the save is applied
([ADR-0125](../../docs/adr/0125-saves-per-scene.md)), as a run carried into scenes changed on disk
is. The game keeps the layout of that new game's zone, which its saves say. A new game by day,
whatever `--night` says, so that a save made at night is resumed at night. The fire flickers in the
frame that the view draws, not in the world, which holds its light as the zone gives it. A save
holds where the player looks; a run of a save made before the game kept it looks where the hero
faces. F9 makes a new world, in place of the host's, and a new `Game` with it; the view, which holds
no entity, draws it from the next frame, and the world panel finds the entity it showed again by its
identity.

| Module | What it holds |
|---|---|
| `main.rs` | The program: the command line, the world built before the window opens, a new run or the save resumed, and the rules and the screen run with `slime_app::run` |
| `lib.rs` | The library: its modules, and the two parts that a host runs |
| `simulation.rs` | The rules as a simulation (`slime_app::Simulation`): the hero walked at each fixed step as the keys held ask, where the player looks turned on real time, rest, restart, saving with F5 and when the game stops, loading with F9, Escape, and with `--watch` the run carried into the scenes that changed; a save resumed from its file, or set aside |
| `screen.rs` | What the player sees as a presentation (`slime_app::Presentation`): the view made when the window opens and again with a new GPU device, with its settings, which a tool reads and changes, the camera, game time held until the zone is drawn, the dev overlay and its keys, F3 and F4, and with `--watch` the cooked data and the shaders loaded again |
| `assets.rs` | Where the cooked data is, and the IDs of the zone's scene file and the hero, from their metadata files |
| `saves.rs` | The save: where its file is, held by one copy of the game at a time, what it holds and how it is read and written, and a run resumed from it once it is checked; a run carried into the scenes that changed on disk, or started again when the zone's layout changed |
| `rules.rs` | The rules, as functions over the world and without the GPU: the hero's movement, its heading, and its collisions, as a circle on the ground against the colliders that are not hidden and the zone's edge; the key, the door, the campfire and the time of day |
| `look.rs` | Where the player looks, a component of the hero's, which the movement keys move the hero relative to |
| `zone.rs` | The zone spawned in the world from its scene file, and the game's components that its nodes carry; where its colliders stand on the ground, boxes turned about the vertical and cylinders, the size of its ground, where the hero starts and the places of the key, the door and the campfire, read from the world; the tests of the cooked zone |
| `controls.rs` | What the keys held ask for, read from the engine's input (`slime_platform::Input`) |
| `camera.rs` | The camera behind the hero along where the player looks, coming nearer when a collider would stand between them |
| `view.rs` | What the player sees: the renderer, the asset cache (`slime_render::AssetCache`), which loads the hero's meshes before the first frame and the rest in the background, and a frame builder for each time of day (`slime_world_render::FrameBuilder`): by day, the overcast sky, the zone's sun behind its clouds; by night, a dark sky around the moon's disc, the zone's moon showing in the sky; the fire that flickers, its light in the frame and its materials, and the key that pulses at night, their materials found by name in the cooked data and changed at each frame (`Material::set_values`); each frame built from the world when the screen extracts, and drawn without it |
| `overlay.rs` | The dev overlay: what each frame costs, the settings of the view, and the speed of game time, with a button that loses the GPU device on purpose, and the world panel, whose edits the screen sends to the host as commands |

The rules run at the engine's fixed step, 1/60 s of game time, so that a game plays the same on
every machine and at every speed; the world draws the hero between its last two steps, by the share
of a step that the frame comes after the last one (`GameTime::alpha`), so that it moves smoothly at
any frame rate, and the camera turns on real time, at the frame's rate. A restart, made between two
steps, is drawn at once. The rules need no GPU, so that their tests run anywhere, in worlds of their
own. The screen holds game time until the zone is drawn, so that the hero does not walk where the
player cannot see: a hold of its own (`Context::hold`), apart from the pause that the player and the
tools set. A move is made in parts of at most half the hero's radius, each pushed out of
the colliders it overlaps, so that no step carries the hero through a thin collider or past a
corner.

The hero's meshes load before the first frame, so that its first shadows, which change the shape of
the frame, do not come in the middle of the zone's load
([ADR-0074](../../docs/adr/0074-uploading-textures-without-holding-up-frames.md)); meanwhile the zone
starts loading and both skies are computed. Each time of day has its frame builder, which keeps the
environment of its sky; the other time's is computed from a world that holds only that time's
lights, so that resting by the campfire computes none.

Of the workarounds that the first game, *Nightwatch*, found for what the engine did not give, the
game keeps one: its own collisions
([ADR-0057](../../docs/adr/0057-what-the-first-game-asks-of-the-next-stages.md)). The engine keeps
the keys held.

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
checks it against its SHA-256. A file already there with the right SHA-256 is not downloaded again.
Git ignores what it fetches, and keeps the list and the metadata files.

`assets/sources/` holds the files as they were published, about 190 MB: textures and models from
Poly Haven, leaves from ambientCG, and the human body of Blender Studio's *Human Base Meshes*, which
the zone's Blender script reads. `assets/zone/` holds what that script exports from them, about
115 MB, the land, the lantern and the hero, and the zone's scene file, `zone.scene`, written by hand
and kept in git, which the cooker cooks with them; the game needs only that part:

```sh
scripts/fetch-assets.sh zone/
cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
```

The cooker takes about half a minute in a release build, and several minutes in a debug one.

To add an asset: take it from a site that publishes it under CC0, add it to the list with its
files, attach the files to a new release of the mirror, and give each its mirror's address.

`scripts/credits.sh` writes, from the list, the notice that credits the assets' authors; CI puts it
beside the game's Windows program, as `hunnawood-CREDITS.txt`.

## The zone

A forest path, built in Blender by a script, [`zone/build_zone.py`](zone/build_zone.py), from the
sources and from a readable file of where it puts the zone's pieces,
[`zone/placements.toml`](zone/placements.toml).
The hero starts in the south, walks a path under oaks and conifers, among mossy rocks and ferns,
crosses a dry gully on a stone bridge, passes a campfire in a clearing, and reaches a ruined gothic
chapel; its north wall holds a sealed door, behind which a walled terrace looks over a cliff
towards the region's capital, painted on a card far away. A line of trees painted the same way
closes the other sides. Lanterns on posts line the path. The ground, the rocks and the stone,
photographed warmer, are graded toward the zone's palette, cold and grey. The script also makes the
hero: Blender Studio's body, its arms lowered, dressed in a tunic, trousers, boots and a hooded
cloak.

The zone is made of prefabs ([ADR-0089](../../docs/adr/0089-scenes-and-prefabs.md)). The script
exports the land, the zone but its lanterns, and the lantern on its post apart, each a glTF file. A
scene file written by hand, [`assets/zone/zone.scene`](assets/zone/zone.scene), places the land
under `land` and the lantern six times along the path: the third lights less, and the fifth, by
the chapel's portal, is broken, without its light and its glass, which the file removes. It also
shows the moon in the night's sky, which glTF cannot say. The zone's entities are known through it:
`zone/land/oak 12 wood`, `zone/lantern 3/light`. The script's modules:

| Module | What it makes |
|---|---|
| `build_zone.py` | Runs the others, exports the land, the lantern and the hero, and with `--preview DIR` renders views of the zone with EEVEE, the lanterns where the scene file places them |
| `common.py` | Materials, the photographed textures graded toward the zone's cold grey palette, meshes, the colliders, markers and lights that the game reads, their components written as typed properties, and the check that every node keeps the name it was given |
| `land.py` | The ground, the path, the gully and the cliff |
| `ruins.py` | The bridge, the chapel, its door's wall and the terrace |
| `props.py` | Poly Haven's models, made lighter, each model's meshes taken in the order of their names; the lantern on its post, its glass a node of its own; the campfire, the door and the key |
| `vegetation.py` | The oaks and conifers, their leaf cards, lit as masses of leaves, and the scatter of trees, rocks and ferns |
| `backdrop.py` | The pictures of the capital and of the forest beyond, rendered with Cycles, and their cards |
| `hero.py` | The hero |
| `preview.py` | The views of `--preview` |

Run it with Blender 5.2, from the repository's root, once the sources are fetched; it writes
`assets/zone/`, and takes a few minutes, most of them rendering the pictures, which it keeps in
`assets/build/` and renders again only when they are deleted:

```sh
blender --background --factory-startup --python games/hunnawood/zone/build_zone.py
```

The land and the lantern come out of the script the same from one run to the next: each model's
meshes are taken in the order of their names. The hero still differs a little from one run to the
next. The export is a source like the others
([ADR-0070](../../docs/adr/0070-the-sample-games-source-assets.md)): after a change, give the new
files an ID (`cargo run -p slime-cook -- scan games/hunnawood/assets/zone`), commit their metadata
files, attach the files to a new release of the mirror and list them in `assets/sources.tsv`, so
that everyone cooks the same files.

Besides meshes, materials and lights, the export holds what the game reads, as the game's
components: typed properties of the nodes, named `slime:` and the type's name, whose text is the
component's value as a scene file writes it
([`CONTRIBUTING.md`](../../CONTRIBUTING.md#writing-and-cooking-a-scene-file),
[ADR-0095](../../docs/adr/0095-cooked-scene-files.md)). The cooker keeps them typed, and the world
gives them to the nodes' entities:

- **Colliders** (`hunnawood::Collider`): `{ kind = "Box", height = 1.2 }`, or a `"Cylinder"`, the
  height in metres. A box's node is turned about the vertical only; its scale is half its size. A
  cylinder's scale is its radius. A collider's node is named after what it stops:
  `oak 12 collider`.
- **Markers** (`hunnawood::Marker`), each held by one node: `"Start"`, where the hero starts, facing
  along the node's −Z axis; `"Item"`, the key, whose mesh is the marker's child, hidden once taken;
  `"Door"`, the sealed door, whose leaves and collider are its children, hidden once the door opens;
  `"Campfire"`, where the hero rests; `"Terrace"`, behind the door; `"Bounds"`, whose scale is half
  the size of the ground the hero may walk.
- **Lights** (`hunnawood::TimeOfDay`): `"Day"` or `"Night"`: the sun by day; by night, the moon,
  which the scene file shows in the sky, a spot light in each lantern, which casts shadows, and the
  campfire's point light, which flickers around the intensity that `hunnawood::Flickers` gives it,
  `{ intensity = 120.0 }`, the light's own: the view sets the light's intensity at each frame from
  it, so that a scene file changes the fire's light through its `Flickers`. The lights of the other
  time of day are hidden; a light without a time of day shines at any time.
- **Materials** that the game changes, by their names: `flames` and `embers`, the campfire's,
  which flicker with its light, and `brass`, the key's, which pulses at night.

Every node has a name of its own, by which the scene file and the saves know its entity. Blender
numbers a name that another object holds, `oak 12.001`: the script fails then, so that no node
changes its name without a change of the script that the saves can be checked against.

To change the lantern, change `props.lantern` in the script and run it again: every lantern of the
zone follows, but what the scene file overrides, the third's intensity and the fifth's light and
glass. A change of how it looks reaches a game running with `--watch` beside `slime-cook watch`, and
the run goes on; a change of its collider is a change of the zone's layout, which starts the run
again. To move a lantern, change its `Transform` in the scene file, a change of layout too.

Tests read the cooked zone. In `src/zone.rs`: one marker of each kind, a start clear of every
collider, the key and the campfire within reach, the terrace not until the door opens; the zone's
layout the same with each of its numbers one unit in the last place away; the zone within the
budgets that the test sets, counted over the land and every lantern; the land spawned in a world
(`slime-world`), each node placed where its file puts it and known by its name, and six lanterns of
one prefab, the third dimmer and the fifth without its light and its glass; the lantern changed,
which every lantern follows but for what the scene file overrides; the colliders where 0.3 had them
([`fixtures/colliders-0.3.txt`](fixtures/colliders-0.3.txt)); no node named with Blender's numbers;
and the moon in the night's sky. In `src/rules.rs`: a walk along the path that rests by the
campfire, is saved at the chapel's portal and resumed in a new game, then takes the key, opens the
door and reaches the terrace, and starts again from the same state; the save committed as a fixture,
resumed, from which the walk goes on to the terrace, and the fixture of 0.3, a save of an earlier
version of the engine; a run changed (an oak removed, a lantern dimmed, the key hidden), saved and
resumed, which saves again to the same bytes and goes on the same for 300 steps; a walk to the door
without the key, which it stops; and the hero kept out of every collider. In `src/view.rs`: the
campfire's light, which flickers, and the materials that glow. In `src/simulation.rs`: the fixture
of 0.3, set aside, which leaves no run to resume. They need the cooked zone, so `cargo test` leaves
them out: `scripts/check.sh` fetches and cooks it, then runs them, and after the commands above,
`cargo test -p hunnawood -- --ignored` does. Two more are measurements: what reading, resolving and
spawning the zone cost, and, on a GPU, what building a frame of the zone's world costs on the CPU.

## Dependencies

The engine's crates only: `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`, and
`slime-devui` for the dev overlay and its world panel, engine tooling that turns `slime-gfx`'s
`wgpu` feature on for itself (the game uses no wgpu type); `slime-assets`, which reads the cooked
zone; `slime-color`, for the colors of its lights and skies; `slime-world`, its world, and
`slime-world-render`, which draws it; `slime-reflect`, which describes its components, as every
component is; `slime-serialize`, for the size of the largest save, up to which it reads its save,
and whose tests encode with it the components of the zones they make, as the cooker does; and
`tracing` for the log. Its default feature, `development`, turns on `slime-app`'s, the development
channel.

## Must not

- Use anything but the engine's public API: no `wgpu` feature of its own, no code of the examples.
- Be a dependency of an engine crate.
