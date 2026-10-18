# slime-assets

The assets as games load them: their identity, the cooked data that the cooker makes, and the
manifest that lists it ([ADR-0069](../../docs/adr/0069-the-asset-pipeline.md)).

## Status

Started in M4a.2. It holds the asset ID; the cooked formats, textures in KTX2 files
([ADR-0071](../../docs/adr/0071-cooked-textures.md)), meshes, materials and scenes in formats of
the engine's own, and scene files in the binary encoding of documents
([ADR-0095](../../docs/adr/0095-cooked-scene-files.md)); the manifest of a directory of cooked
data; and the reading of that directory by ID, which `slime-render` does in the background, from
threads of its own.

## Role

- Names every asset, a texture, a mesh, a material, a scene or a scene file, by an ID that stays
  the same when its source file is moved or renamed: cooked data refers to assets by ID, never by
  path.
- Holds the cooked formats, written by the cooker and read by games: each format is read and
  written here, in one place.
- Reads a directory of cooked data for a game, on the thread that calls it.

## Entry points

- `AssetId`: a 128-bit number, written as a UUID; `AssetId::from_u128` names an asset in a game's
  code, and its `Display` and `FromStr` write and read its text form. It is reflected, as `Extra`
  is (`slime-reflect`), so that components hold them.
- `CookedAssets::open(directory)` reads the manifest of a directory of cooked data;
  `CookedAssets::texture(id)` reads a texture, which `slime_render::Renderer::create_texture`
  uploads.
- `CookedAssets::mesh`, `material` and `scene` read the other kinds; a glTF file's default scene
  is read by the file's own ID. A `CookedScene`'s nodes carry the glTF `extras` they were given, as
  `Extra` values, and their typed components, the `slime:<type name>` ones, as the bytes of a
  document that a world reads through its registry; `world_transforms` places them.
  `CookedAssets::scene_file` reads a scene file's bytes, which a world reads too
  (`slime_world::SceneLibrary`).
- `ManifestWatch` tells a game that hot-reloads when the manifest of its cooked data changed, and
  gives the cooked data again ([ADR-0076](../../docs/adr/0076-hot-reload-of-cooked-data.md)).
- `CookedTexture` and its KTX2 file (`to_ktx2`, `from_ktx2`); `CookedMesh`, `CookedMaterial`,
  `CookedScene` and `Manifest`, and their binary files (`to_bytes`, `from_bytes`). Their rustdoc
  gives the formats.

## Dependencies

`slime-reflect`, which it describes its asset IDs and the custom values of nodes to, and no other
engine crate. `ktx2` to read and write KTX2 files, `thiserror` for its error types.

## Must not

- Depend on the GPU, a window or another engine crate than `slime-reflect`: it runs headless, and
  the renderer depends on it to load its own kinds of asset.
- Link an importer or an encoder: those belong to `slime-cook`, which a shipped game does not
  link.
