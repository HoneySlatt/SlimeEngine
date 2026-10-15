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
scenes, and cooks the textures their materials read in the roles they read them.

## Role

- Knows which files are sources: glTF scenes (`.gltf`, `.glb`) and PNG and JPEG images.
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
  nodes with their transforms, meshes, lights and `extras`. It refuses what it cannot read right
  with a message that says what and where, and warns about what it leaves out.
- Names each cooked file by the hash of all that made it, so that what is already cooked is not
  cooked again, and the same sources cook into the same bytes on one platform.

## Entry points

The program:

```sh
cargo run -p slime-cook -- scan examples/assets              # gives an ID to each source without one
cargo run -p slime-cook -- check examples/assets             # writes nothing; fails on what is wrong
cargo run -p slime-cook -- cook <sources> target/cooked      # cooks the sources, with a manifest
```

Cooking is slow in a debug build: `cargo run --release -p slime-cook` cooks a texture of 2048 ×
2048 texels in about a second.

The library: `scan` (the module and its function) finds the source files of a directory and their
IDs, and what is wrong with their metadata; `metadata` reads and writes metadata files; `ids` makes
new IDs and derives those of sub-assets and textures; `image` decodes images;
`texture::cook_texture` makes a cooked texture of an image for a role; `gltf_import::GltfFile`
opens and imports a glTF file; `cook` (the module and its function) cooks a directory.
Their rustdoc gives the formats and the rules.

## Dependencies

[`slime-assets`](../slime-assets/README.md), for the asset ID and the cooked formats. `uuid` for
random IDs, `blake3` to derive the IDs of sub-assets and name cooked files, `png` and `zune-jpeg`
to decode images, `block_compression` to compress textures, `gltf` to read glTF files,
`bevy_mikktspace` to compute tangents, `glam` for their vectors, `thiserror` for its error types.

## Must not

- Be linked by a game: a shipped program holds no importer.
- Depend on the GPU or a window: it runs headless, in the checks and in CI.
