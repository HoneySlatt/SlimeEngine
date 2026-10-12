# Assets of the examples

| File | What it is | Made with | License |
|---|---|---|---|
| `scene.glb` | The scene of the `scene` example and its image test: a tiled floor, two wooden crates, a sphere, a pillar, a brick wall and a ring, with three textures drawn in code and embedded as PNG images | [`make_scene.py`](make_scene.py), run in Blender 5.2: `blender --background --python examples/assets/make_scene.py -- examples/assets/scene.glb` | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): public domain |

The glTF loader of the examples (`examples/common/gltf.rs`, [ADR-0029](../../docs/adr/0029-temporary-gltf-loading.md))
reads GLB files whose buffers are embedded and whose images are PNG, so a file exported from
Blender with its default settings works; it is temporary, until the asset pipeline of Stage 4.
