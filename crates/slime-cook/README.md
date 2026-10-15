# slime-cook

The cooker: it turns *source assets*, the files that people make or download, into the *cooked
data* that games load, before they run ([ADR-0069](../../docs/adr/0069-the-asset-pipeline.md)). A
library, and its program, `slime-cook`.

## Status

Started in M4a.2. It gives every source file an asset ID that survives a move, kept in a metadata
file beside it, checks the metadata files of a directory, and derives the IDs of the assets inside
a file from the file's. Importing images and glTF files, cooking them and the manifest come with
the next milestones of Stage 4a.

## Role

- Knows which files are sources: glTF scenes (`.gltf`, `.glb`) and PNG and JPEG images.
- Gives each source file a random ID the first time it meets it, in a *metadata file* beside it,
  `<file>.meta`, committed with it: a file moved or renamed with its metadata file keeps its ID.
- Reports a metadata file left without its source, and two that hold one ID, as a file copied with
  its metadata has; the checks refuse a source file without metadata.
- Derives the IDs of a file's *sub-assets*, its meshes, materials, scenes and embedded images, from
  the file's ID, their kind and their name.

## Entry points

The program:

```sh
cargo run -p slime-cook -- scan examples/assets    # gives an ID to each source file without one
cargo run -p slime-cook -- check examples/assets   # writes nothing; fails on what is wrong
```

The library: `scan` (the module and its function) finds the source files of a directory and their
IDs, and what is wrong with their metadata; `metadata` reads and writes metadata files; `ids`
makes new IDs and derives those of sub-assets. Their rustdoc gives the formats and the rules.

## Dependencies

[`slime-assets`](../slime-assets/README.md), for the asset ID. `uuid` for random IDs, `blake3` to
derive the IDs of sub-assets, `thiserror` for its error types.

## Must not

- Be linked by a game: a shipped program holds no importer.
- Depend on the GPU or a window: it runs headless, in the checks and in CI.
