# slime-assets

The assets as games load them: their identity, the cooked data that the cooker makes, and the
manifest that lists it ([ADR-0069](../../docs/adr/0069-the-asset-pipeline.md)).

## Status

Started in M4a.2. It holds the asset ID, cooked textures in KTX2 files
([ADR-0071](../../docs/adr/0071-cooked-textures.md)), the manifest of a directory of cooked data,
and the reading of that directory by ID. Cooked meshes, materials and scenes come with the importer
of glTF (M4a.4); loading in the background, with Stage 4b.

## Role

- Names every asset, a texture, a mesh, a material or a scene, by an ID that stays the same when
  its source file is moved or renamed: cooked data refers to assets by ID, never by path.
- Holds the cooked formats, written by the cooker and read by games: each format is read and
  written here, in one place.
- Reads a directory of cooked data for a game, on the calling thread.

## Entry points

- `AssetId`: a 128-bit number, written as a UUID; `AssetId::from_u128` names an asset in a game's
  code, and its `Display` and `FromStr` write and read its text form.
- `CookedAssets::open(directory)` reads the manifest of a directory of cooked data;
  `CookedAssets::texture(id)` reads a texture, which `slime_render::Renderer::create_texture`
  uploads.
- `TextureData`, a cooked texture, and its KTX2 file (`to_ktx2`, `from_ktx2`); `Manifest`, the list
  of what was cooked, and its binary file (`to_bytes`, `from_bytes`). Their rustdoc gives the
  formats.

## Dependencies

No engine crate. `ktx2` to read and write KTX2 files, `thiserror` for its error types.

## Must not

- Depend on the GPU, a window or another engine crate: it runs headless, and the renderer
  depends on it to load its own kinds of asset.
- Link an importer or an encoder: those belong to `slime-cook`, which a shipped game does not
  link.
