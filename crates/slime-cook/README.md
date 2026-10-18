# slime-cook

The cooker: it turns *source assets*, the files that people make or download, into the *cooked
data* that games load, before they run ([ADR-0069](../../docs/adr/0069-the-asset-pipeline.md)). A
library, and its program, `slime-cook`.

## Status

Started in M4a.2. It gives every source file an asset ID that survives a move, kept in a metadata
file beside it, and checks the metadata files of a directory. It imports PNG and JPEG images and
cooks them into textures, their levels of detail computed and compressed in BC7 or BC5
([ADR-0071](../../docs/adr/0071-cooked-textures.md)), into a directory of cooked data with its
manifest, cooking again only what changed. It imports glTF files, `.gltf` with their buffers and
images beside them or GLB, into meshes, with MikkTSpace tangents when they have none, materials and
scenes, and cooks the textures their materials read in the roles they read them; their nodes'
`slime:<type name>` properties become typed components. It cooks scene files into the engine's
binary encoding of documents, after it checks them
([ADR-0095](../../docs/adr/0095-cooked-scene-files.md)). It cooks a directory again each time one
of its files changes, for games that load their data again while they run
([ADR-0076](../../docs/adr/0076-hot-reload-of-cooked-data.md)).

## Role

- Knows which files are sources: glTF scenes (`.gltf`, `.glb`), PNG and JPEG images, and scene
  files (`.scene`).
- Gives each source file a random ID the first time it meets it, in a *metadata file* beside it,
  `<file>.meta`, committed with it: a file moved or renamed with its metadata file keeps its ID.
- Reports a metadata file left without its source, and two that hold one ID, as a file copied with
  its metadata has; the checks refuse a source file without metadata, and a cook refuses two
  files with one ID.
- Derives the IDs of a file's *sub-assets*, its meshes, materials, scenes and embedded images, from
  the file's ID, their kind and their name.
- Cooks a texture for its *role*, colors, data or normals, which sets how its levels are averaged
  and how it is compressed: each image in the roles that materials read it in, or as colors under
  its own ID if none reads it.
- Imports glTF files: each primitive a mesh of only the vertices it names, flat normals and
  MikkTSpace tangents where the file has none; each material with its textures by ID; each scene's
  nodes with their transforms, meshes, lights and `extras`, the `slime:<type name>` ones as typed
  components, checked as written, at the versions that `slime:$versions` gives. It refuses what it
  cannot read right with a message that says what and where, and warns about what it leaves out.
- Cooks scene files: reads their layout without the types of their values, through `slime-world`;
  refuses, by file and line, an asset of their table that the sources do not make, a prefab that is
  not a scene, a parent missing, entities that would be each other's ancestors and scene files that
  place each other; warns of a path that a table gives where the source is not; writes their
  binary encoding, which needs the assets of their table.
- Names each cooked file by the hash of all that made it, so that what is already cooked is not
  cooked again, and the same sources cook into the same bytes on one platform. Writes its files
  under temporary names of the process's own, then renames them, so that two cooks at once leave
  the cooked data whole, and the manifest only when what it lists changed.

## Entry points

The program:

```sh
cargo run -p slime-cook -- scan examples/assets              # gives an ID to each source without one
cargo run -p slime-cook -- check examples/assets             # writes nothing; fails on what is wrong
cargo run -p slime-cook -- cook <sources> target/cooked      # cooks the sources, with a manifest
cargo run -p slime-cook -- watch <sources> target/cooked     # cooks them again at each change
```

Cooking is slow in a debug build: `cargo run --release -p slime-cook` cooks a texture of 2048 ×
2048 texels in about a second.

The library: `scan` (the module and its function) finds the source files of a directory and their
IDs, and what is wrong with their metadata; `metadata` reads and writes metadata files, documents
of a `Metadata` value, through `slime-serialize`; `ids` makes new IDs and derives those of
sub-assets and textures; `image` decodes images; `texture::cook_texture` makes a cooked texture of
an image for a role; `gltf_import::GltfFile` opens and imports a glTF file; `cook` (the module and
its function) cooks a directory, and `watch` cooks it again at each change.
Their rustdoc gives the formats and the rules.

## Dependencies

[`slime-assets`](../slime-assets/README.md), for the asset ID and the cooked formats;
[`slime-color`](../slime-color/README.md), for the sRGB transfer function of the mipmaps;
[`slime-serialize`](../slime-serialize/README.md), which reads and writes metadata files and
encodes scene files, and [`slime-reflect`](../slime-reflect/README.md), which describes what
metadata files hold; [`slime-world`](../slime-world/README.md), which reads the layout of the scene
files that it checks. `uuid` for random
IDs, `blake3` to derive the IDs of sub-assets and name cooked files, `png` and `zune-jpeg` to decode
images, `block_compression` to compress textures, `gltf` to read glTF files, `bevy_mikktspace` to
compute tangents, `glam` for their vectors, `thiserror` for its error types.

## Must not

- Be linked by a game: a shipped program holds no importer.
- Depend on the GPU or a window: it runs headless, in the checks and in CI.
