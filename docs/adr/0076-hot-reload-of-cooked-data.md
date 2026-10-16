# 0076. Hot reload of cooked data

- Status: Accepted; amended by [0084](0084-what-the-world-draws.md) (the asset cache reloads what
  the world draws)
- Date: 2026-10-15

## Context

[ADR-0072](0072-stage-4b-live-assets.md) decides how a game sees its assets change while it runs:
`slime-cook watch` cooks again what changes, checking the sources every half second with the
standard library alone; temporary files carry the process ID, and the manifest is written last,
only when its content changes; a game that hot-reloads, with `--watch`, checks its manifest, loads
the scene again in the background, reusing the GPU objects that did not change, and swaps the
scenes between two frames; a reload that fails keeps the old scene; *Hunna Wood* reads its zone's
rules again, and starts the run again only when the game's layout changed. It leaves how to the
milestone that builds it, M4b.6, and asks it to measure whether the way the cooker names a glTF
file's outputs defeats the reuse.

Where the code stood:

- `slime_cook::cook` names each cooked file by a BLAKE3 hash of all that made it: a texture of an
  image file after the cooker's version, the role, the image's ID and bytes; what a glTF file gives
  after the cooker's versions, the whole file, its buffers and the IDs of the images it names, then
  the asset's ID. What is in the output under its name is not cooked again. It wrote every file
  under `<name>.part`, then renamed it, and wrote the manifest at the end of every cook.
- `slime_assets::CookedAssets::open` reads a manifest; the renderer loads a scene from it on
  threads of its own ([ADR-0073](0073-loading-scenes-in-the-background.md)).

Measured on *Hunna Wood*'s zone, whose 56 textures are cooked from image files beside its glTF
file: moving one node in that file, then cooking, renames the 74 meshes, 24 materials and the scene
that the file gives, 5.2 MiB, and keeps the 56 textures, 151 MiB.

## Decision

**The cooker.**

- `slime_cook::watch(sources, output, report)` cooks, then looks every half second
  (`WATCH_INTERVAL`) at the size and the time of change of every file inside the directory of
  sources: sources, metadata files and the files they read, such as a glTF file's buffers. Files
  that changed, appeared or went make it cook again, once they have stayed as they are for one look,
  so that an export that writes several files is cooked whole. It gives each cook's result to
  `report`, which says whether to go on; a cook that fails does not end the watch, nor a directory
  inside the sources that goes while it is read. It gives the sources their IDs before it looks, so
  that the metadata files a cook writes are not taken for a change. `slime-cook watch <sources>
  <output>` runs it until it is stopped.
- Temporary files are named `<name>.<process ID>.part`: two cooks of the same sources at once write
  the same bytes under the same final names, and neither renames the other's temporary file. A test
  runs two cooker processes at once: without the process ID, between a fifth and a half of its runs
  failed, depending on the machine.
- The manifest is written only when its bytes change, so that a cook that changed nothing reloads
  nothing.
- **The names of a glTF file's outputs stay as they are.** The measurement shows that a change of
  the zone's glTF file keeps its textures, 97 % of its bytes: the reuse holds where it counts. The
  meshes that the change did not touch are uploaded again, 5 MiB, in a load that goes on while
  frames are drawn.

**The cooked data.** `slime_assets::ManifestWatch` reads a directory's manifest again every half
second, as often as the cooker looks at its sources, and gives the cooked data again when the
manifest differs from the last one it gave; a manifest that cannot be read is told once. A game
starts it from the data it loaded, and starts it again from that data when it has to load it again
anyway, after a lost GPU device.

**The renderer.** `Renderer::reload_scene_in_background(gpu, assets, id, previous)` loads a scene
as `load_scene_in_background` does, but keeps the meshes and textures of `previous`, a loaded
scene, whose cooked file the new manifest still names: a loaded scene remembers, for each of its
meshes and textures, its ID and the name of the file it was read from. Equal names mean equal data,
since a name says what made the file. The materials are made again, from the textures kept or
uploaded, with the values that the cooked data gives: they are small, and a game that changes their
values finds them again by name in the new scene. `previous` stays whole, so that the game draws it
until the new scene is there.

**The game and the example.**

- With `--watch`, *Hunna Wood* and the `scene` example poll a `ManifestWatch` once their scene is
  shown, and while no reload is in progress: a reload replaced half-way would wait for its running
  tasks. When the manifest changed, they start the reload of their scenes, *Hunna Wood* of both its
  zone and its hero, and take the new scene at the frame where its load ends. A reload that fails,
  or a zone whose rules cannot be read, logs why and keeps what is drawn.
- *Hunna Wood* reads its zone's rules and lights again from the reloaded zone, and takes them with
  the cooked data they came from, which a new view loads from after a lost GPU device. When the
  layout differs, everything that `Zone` holds but the nodes that are the key and the door's
  leaves, the run starts again, as the restart key does; otherwise it goes on, and draws by the new
  nodes. The night sky is computed again only when its moon moved, and the `scene` example's sky
  when its sun moved: the frame that shows the scene pays for it.
- No crate gains a dependency.

## Alternatives considered

- **Naming each output of a glTF file after its own bytes**: a change of one mesh would upload that
  mesh alone, but the measurement finds 5 MiB to save, against a change of every cooked file's name,
  and so a cook from nothing, for every project that has cooked data.
- **Keeping the materials of the scene reloaded**: nothing to make again, but a material is a bind
  group of its textures: one whose texture changed must be made again anyway, and a game that keeps
  a material to change its values would hold the old one after some reloads and not after others.
- **Comparing the manifest's time of change rather than its bytes**: one call less, but a manifest
  written in the same instant as the last read would be missed; reading 26 KB twice a second costs
  nothing that shows.
- **A file that the cooker writes at the end of each cook, for games to watch**: a second signal to
  keep in step with the manifest, which already changes when, and only when, what it lists does.
- **Reloading the zone only, not the hero**: less to write, but a texture of the hero changed on
  disk would not change in the game, against what `--watch` promises.
- **Each game reading its manifest itself**: the game and the example did at first, and differed in
  when they read it and in what they did with a failure.

## Consequences

- A texture changed on disk changes in the running game a second or two after it is saved, plus
  the cook: the cooker cooks it within one to two looks, the game sees the new manifest within half
  a second after the cook, and the load uploads that texture alone.
- The files that no source makes any more stay in the output: each change of an image leaves its
  old texture there, each export of a glTF file its old meshes, materials and scenes, and a cook
  stopped half-way, as Ctrl+C stops a watch, its temporary files. Deleting the directory of cooked
  data cleans it; the next cook makes it again from nothing.
- Two cooks at once of a source that has no metadata file yet give it two IDs, one each: the
  metadata file that the second writes replaces the first's. `slime-cook scan` before both, or a
  single watch, avoids it.
- A reload holds, until it ends, the GPU objects of both scenes but those they share; the frames go
  on during it, as during a first load (ADR-0074), but for the frame that computes a sky again.
- Tests check the cooker's watch, two cooks at once, the manifest left alone, `ManifestWatch`, the
  renderer's reload and the game's rules read again; the game's and the example's `--watch` are
  checked by hand, on Linux. On Windows, the programs that CI ships hold no cooker and no source
  assets (ADR-0072): hot reload is checked there by CI's tests alone.
