# 0069. The asset pipeline: identity, cooking and what a game loads

- Status: Accepted; amended by [0080](0080-reflection-and-serialization.md) (`slime-assets` depends
  on `slime-reflect`; metadata files read through the engine's serialization),
  [0102](0102-a-damaged-file-gives-an-error.md) (who reads KTX2 files, and the cooked files reused)
  and [0106](0106-the-measured-quick-wins.md) (images whose sides are not multiples of 4)
- Date: 2026-10-15

## Context

Until Stage 4, a game makes its meshes in code, and the `scene` example loads one GLB file at
startup through a temporary loader in example code
([ADR-0029](0029-temporary-gltf-loading.md)), which a game cannot use. The renderer computes the
tangents that a file lacks per triangle ([ADR-0035](0035-physically-based-materials.md)) and the
mipmaps of a texture on the CPU when it is created
([ADR-0032](0032-mipmaps-computed-at-texture-creation.md)), and keeps textures uncompressed, four
bytes per texel.

Stage 4a ([ADR-0068](0068-stage-4-in-two-halves.md)) gives the engine an asset pipeline: source
files (glTF, PNG, JPEG) become data that a game loads, and moving a source file breaks no
reference. *Hunna Wood*'s first zone needs about forty textures and the models of a forest
([ADR-0067](0067-hunna-wood-replaces-nightwatch.md)). Uncompressed, with their mipmaps, one set of
three textures 2048 texels wide (colour, normals, occlusion-roughness-metallic) takes 64 MiB of GPU
memory, and forty such sets 2.5 GiB.

## Decision

**Identity.**

- Every source file that the pipeline imports has a *metadata file* beside it, `<file>.meta`,
  committed with it, which holds its *asset ID*: a random 128-bit UUID (version 4), given by the
  cooker the first time it meets the file. A file moved or renamed with its metadata file keeps its
  ID.
- The assets inside a file, a glTF's meshes, materials, scenes and embedded images, are its
  *sub-assets*. Each takes an ID derived from the file's ID, its kind and its name, with BLAKE3's
  key derivation; one without a name, or whose name repeats, takes its index instead, with a
  warning. Renaming an object in Blender changes its sub-asset's ID.
- Cooked data refers to other assets by ID, never by path. The cooker refuses two metadata files
  with one ID, as a file copied with its metadata has, and reports a metadata file without its
  source; the checks refuse a source file without metadata.
- The metadata format is a one-key subset of TOML, `id = "<uuid>"`, read and written by the
  engine's own code, so that it can grow into TOML with import settings once the engine has a
  serialization format (Stage 5).

**Crates.**

- `slime-assets`: asset IDs, the cooked formats and the manifest, which games read. It depends on
  no engine crate and on no GPU, so it runs headless.
- `slime-cook`: a library and its program, `slime-cook`, in `crates/` like the other engine crates,
  since the editor will call the library (Stage 6). It reads metadata files and gives IDs, imports
  glTF and images, computes tangents with MikkTSpace and mipmaps, compresses textures, writes cooked
  files and the manifest. It depends on `slime-assets`, never on the GPU. A game never links it: a
  shipped program holds no importer.
- `slime-render` depends on `slime-assets` and turns cooked meshes, textures and materials into its
  own GPU objects: each system loads its own kinds of asset, as audio will at Stage 7.

**Cooked data.**

- *Textures*: KTX2 files holding every mipmap level, computed by the cooker: colour averaged in
  linear light, normals renormalized. They are compressed for the GPU: colour in BC7 with sRGB,
  other data such as occlusion-roughness-metallic in BC7 without it, normal maps in BC5, two
  channels from which the shader rebuilds the third. A texture whose sides are not multiples of
  four stays uncompressed. An image is cooked once per use that materials make of it, colour,
  normals or data, which sets how it is compressed and read.
- *Meshes*, one per glTF primitive: positions, normals, texture coordinates and tangents, computed
  with MikkTSpace when the file has none, 32-bit indices, bounds and the ID of the material; only
  the vertices its indices name.
- *Materials*: their values, and their five textures by ID with their samplers.
- *Scenes*: their nodes in order, each after its parent: name, parent, transform, mesh, light, and
  the glTF `extras` the node carries, flat keys to booleans, numbers or strings, which the engine
  passes on without reading them. A game reads its colliders and markers from them.
- Binary formats written by the engine's own code: a magic number and a version, then
  little-endian arrays. A program refuses a version it does not know, and names the command that
  cooks again.
- *The manifest* of a game lists each cooked asset's ID, kind, file, size and the IDs it needs to
  load; source paths appear in it only for messages.

**Cooking.**

- The cooker keeps its output in a cache named by content: each cooked file is named by a BLAKE3
  hash of all that made it, the kind, the version of the cooked format and of the importer, the
  settings, the asset's ID and the contents of every file read. A source that has not changed is
  not cooked again, and a source moved with its metadata costs nothing. The cache lives under
  `target/`; cooked data is never committed.
- `slime-cook` cooks before a game runs, run by the developer, by the checks and by CI. A game in
  development reads the manifest and the cooked files from the directory it is given, a shipped
  game from a directory beside its program. A game whose cooked data is missing or of another
  version says which command cooks it. How a running game sees what was cooked again is Stage 4b's.
- A GPU that cannot sample BC textures is refused with an error that names it: every desktop GPU on
  Linux and Windows can, and so do lavapipe and WARP by what their drivers report, which the tests
  of the images' milestone confirm.

**Dependencies**, approved by the owner on 2026-10-15: `gltf` moves from the examples to
`slime-cook`, with its `extras` feature; `zune-jpeg` decodes JPEG; `bevy_mikktspace` computes
tangents; `block_compression` encodes BC7 and BC5; `ktx2` reads and writes KTX2; `blake3` hashes
and derives IDs; `uuid` makes the files' IDs. `docs/dependencies.md` lists each one, with its use
and its licence, from the milestone that adds it.

## Alternatives considered

- **Identity in one registry file** mapping paths to IDs: one file that every change of every branch
  touches, and a move that only a tool can record. **Paths as identity, with a command that moves a
  file**: a move made outside the command breaks references, the opposite of what the roadmap asks.
  Unity and Godot keep metadata files beside the sources, as here.
- **Metadata in TOML through `serde`**: two dependencies and derived code before Stage 5 decides
  the engine's serialization.
- **Uncompressed textures**: four times the GPU memory, 2.5 GiB instead of 640 MiB for forty sets
  of 2048-texel textures. **`intel_tex_2`**, bindings to Intel's encoders written in ISPC: native
  code that the build links, where `block_compression` ports the same encoders to Rust. **Decoding
  BC textures on the CPU for GPUs that cannot sample them**:
  twice the code for GPUs that the engine does not aim at.
- **One loading crate above `slime-assets` and `slime-render`**: a crate that every new kind of
  asset changes. **`slime-assets` depending on `slime-render`**: the headless part would need the
  GPU.
- **`bincode`** for the cooked formats: unmaintained (RUSTSEC-2025-0141). **`postcard` or `rkyv`**:
  `serde` or derived code before Stage 5.
- **Cooking inside the game in development**: every development build of every game would link the
  importers and the encoders, and cook at startup. **Committed cooked data**: binary files changing
  with every source, and different on each platform.
- **SHA-256 for the cache's names**: slower than BLAKE3, without the key derivation that the IDs
  use.

## Consequences

- At Stage 4a's fifth milestone, the temporary loader is deleted and ADR-0029 superseded; the
  renderer keeps its per-triangle tangents and its mipmaps for meshes and textures made in code.
- A source committed without its metadata, or metadata copied with a file, is caught by the checks.
- A glTF file names its buffers and images by path, relative to it: it moves with its folder.
- The cooked bytes differ between Linux and Windows, where the mathematical functions that
  MikkTSpace calls come from different libraries: tests compare decoded values across platforms,
  not bytes.
- Every program that draws cooked data needs the cooker to run first: the checks and CI cook before
  they test and run, which adds to their time, and a shipped game carries its cooked directory.
- Compression costs cooking time, which the images' milestone measures, and a little detail: the
  reference images are rendered again.
