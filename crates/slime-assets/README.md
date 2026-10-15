# slime-assets

The assets as games load them: their identity, and, as Stage 4a builds them, the cooked data that
the cooker makes and the manifest that lists it
([ADR-0069](../../docs/adr/0069-the-asset-pipeline.md)).

## Status

Started in M4a.2. It holds the asset ID, `AssetId`, and its text form, a UUID. The cooked formats
and the manifest come with the next milestones of Stage 4a.

## Role

- Names every asset, a texture, a mesh, a material or a scene, by an ID that stays the same when
  its source file is moved or renamed: cooked data refers to assets by ID, never by path.
- Holds what games read of the asset pipeline. The cooker, [`slime-cook`](../slime-cook/README.md),
  makes what this crate reads, and depends on it.

## Entry points

`AssetId`: a 128-bit number, written as a UUID; `AssetId::from_u128` names an asset in a game's
code, and its `Display` and `FromStr` write and read its text form. Its rustdoc says how IDs are
given.

## Dependencies

No engine crate. `thiserror` for its error type.

## Must not

- Depend on the GPU, a window or another engine crate: it runs headless, and the renderer
  depends on it to load its own kinds of asset.
- Link an importer or an encoder: those belong to `slime-cook`, which a shipped game does not
  link.
