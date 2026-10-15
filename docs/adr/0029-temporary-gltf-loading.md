# 0029. Temporary glTF loading, in the examples

- Status: Superseded by [0069](0069-the-asset-pipeline.md) (the loader was deleted at Stage 4a: the
  cooker imports glTF files, and the renderer loads what it cooked); amended before by
  [0035](0035-physically-based-materials.md) (its lists of what the loader reads, refuses and warns
  about were replaced)
- Date: 2026-10-13

## Context

The exit criterion of Stage 3a is a glTF scene rendered under a moving camera
(`docs/ROADMAP.md`). The roadmap plans "temporary direct glTF loading" for it, and the asset
pipeline of Stage 4, which imports glTF among other formats, cooks and loads assets, deletes it.
Until then, something must read a glTF file and create the renderer's meshes, materials and
textures from it, and a sample scene is needed, with a license that lets the repository carry it.
When the stage was planned, the owner validated the `gltf` crate for the reading, and a scene made
in Blender for the asset.

## Decision

- **Where.** The loader is part of the examples (`examples/common/gltf.rs`), not of an engine
  crate: the `scene` example and its image test are its only users, and Stage 4 deletes it
  without touching the engine. It builds `MeshData`, materials and textures through
  `slime-render`'s and `slime-gfx`'s public API, and flattens the file's node hierarchy into a
  list of instances with their world transforms, since the renderer draws flat lists (ADR-0027).
- **What it reads.** GLB files that hold their buffer and images in their binary chunk (data URIs
  are refused), with the `gltf` crate without its `import` feature, which would bring the `image`
  and `base64` crates (its other default features, `names` and `utils`, stay on); PNG images are
  decoded with `png`, already used by the examples. Triangle primitives with positions, with or
  without normals (computed from the faces when missing), texture coordinates (zero when missing)
  and indices; materials' base color factor and base color texture read with the file's sampler
  settings; the scene's node hierarchy. It stops with a clear error on what it cannot draw right: an
  external buffer or image file, an image that is not PNG, an extension the file requires, a texture
  read through another set of coordinates than the first, a primitive that is not a triangle list.
  It warns about what it ignores and draws without: animations, skins, cameras, extensions the file
  uses without requiring them.
- **The asset.** `examples/assets/scene.glb` is built by `examples/assets/make_scene.py`, run in
  Blender 5.2, which draws its textures in code and exports the file; the script is the asset's
  source and provenance. The file is released under CC0 1.0, embedded in the `scene` example
  with `include_bytes!`, so that the example is one file on every platform, and `--scene <file>`
  loads another GLB file.
- **Removal.** Stage 4 replaces the loader with the importer of the asset pipeline and deletes
  it, as the roadmap says; the asset stays as a sample.

## Alternatives considered

- **A parser of our own**, on `serde_json`. More code for something temporary; the `gltf` crate
  reads the format correctly, and its last release, in 2024, follows a specification that has not
  changed since.
- **The `gltf` crate with its `import` feature.** It reads external files and data URIs and
  decodes images with the `image` crate: two dependencies the examples do not need, for files the
  sample does not use.
- **The loader in `slime-render`.** A temporary thing inside an engine crate, to delete from it
  later, and a dependency of the engine on `gltf` until then.
- **A sample model of the Khronos Group.** Their licenses require attribution or are not open;
  a scene made for the project is public domain, and its script says how it was made.

## Consequences

- `gltf` and what it pulls in (`gltf-json`, `serde`, `serde_json`) are dependencies of the
  examples only; no engine crate depends on them.
- The loader covers what the sample needs and what a plain Blender export produces; a file with
  external images, JPEG textures or a required extension is refused with a message that says why.
- Stage 4 deletes `examples/common/gltf.rs` and moves the `gltf` dependency to the importer.
