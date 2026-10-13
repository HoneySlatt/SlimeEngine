# Assets of the examples

| File | What it is | Made with | License |
|---|---|---|---|
| `scene.glb` | The scene of the `scene` example and its image test: a tiled floor, two wooden crates, a sphere, a pillar, a brick wall and a ring, with seven textures drawn in code and embedded as PNG images, and three lights; 128 400 bytes | [`make_scene.py`](make_scene.py), run in Blender 5.2: `blender --background --python examples/assets/make_scene.py -- examples/assets/scene.glb` | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): public domain |

## What `scene.glb` holds

Seven primitives, one per object, with 1696 vertices and 2274 triangles in all. Each has normals,
one set of texture coordinates, and tangents (glTF's `TANGENT`), which Blender computes with
MikkTSpace. Six materials use glTF's metallic-roughness model, so that one small file exercises
each property the loader reads:

| Material | Drawn on | What it exercises |
|---|---|---|
| `tiles` | The floor | A base color texture, `tiles`. An occlusion texture, `tiles occlusion`: all of the ambient light reaches the tiles, 0.6 of it the grout. The only double-sided material: the floor is a single plane, which can be seen from below. A rough dielectric: roughness 0.8, metallic 0. |
| `planks` | Both crates | A base color texture, `planks`: wooden planks in an iron-grey frame. One packed texture, `planks ORM`, is both the occlusion texture and the metallic-roughness texture. Its red channel is the occlusion: 0.5 in the gaps between planks, 1 elsewhere. Green is the roughness: 0.7 on the wood, 0.4 on the frame. Blue is the metallic: 0 on the wood, 1 on the frame. Both factors are 1. |
| `bricks` | The wall | A base color texture, `bricks`. A tangent-space normal texture, `bricks normal`: the bricks stand above their mortar, with rounded edges. It follows glTF's convention: red points right, green up the image, blue out of the surface. The flat inside of a brick is (128, 128, 255). Roughness 0.8, metallic 0. |
| `red` | The sphere | A plain dielectric with roughness 0.35, which gives a sharper highlight than the other materials. |
| `grey` | The pillar | A plain metal: metallic 1, roughness 0.4. The script splits the pillar's 32-sided caps into triangles, because Blender computes tangents only for faces of three or four corners. |
| `teal` | The ring | An emissive texture, `ring glow`: eight teal segments on black. Its emissive factor is 1 and its emissive strength 127 324 (`KHR_materials_emissive_strength`), in candelas per square metre, glTF's unit of emission: four times what a white matte surface facing the sun reflects. Roughness 0.8, metallic 0. |

Every material but `tiles` is single-sided. The other objects are closed, so their back faces are
never seen. The script turns on Blender's backface culling for their materials, and the exporter
writes that as `doubleSided` false.

Three lights of `KHR_lights_punctual` light the scene, in its physical units
([ADR-0037](../../docs/adr/0037-lights-in-physical-units-and-pre-exposure.md)). None casts
shadows.

| Light | Kind and place | Strength | What it exercises |
|---|---|---|---|
| `sun` | Directional, travelling along (−0.5, −1, −0.6) in glTF's axes; slightly warm | 100 000 lux, direct sunlight | The sample's main light |
| `ring light` | Point, at the center of the ring, as a child of its node; teal; a range of 4 m | 37 700 candelas: the light that the ring's glowing segments give off, as a point light of the same flux | A light that a parent's transform places, and a range |
| `spot` | Spot, on the floor in front of the wall, aimed up at the side that the sun leaves in the shade; warm; full within 18° of its axis, dark beyond 30° | 80 000 candelas, a stage spotlight | A node's orientation, and the cone angles |

The script sets the lights in Blender's units, which the exporter converts in its standard
lighting mode: a sun's strength, in W/m², times 683 lm/W gives lux; a point or spot light's power,
in W, times 683 / 4π gives candelas. It makes Blender render with EEVEE before exporting: under
any other engine, the exporter reads a sun's strength from its node tree and writes 683 lux,
whatever the script set.

The textures are 128 × 128 pixels, drawn with numpy from fixed seeds, and share one sampler
(linear, mipmapped, repeating). The textures of colors (base color and emissive) hold sRGB values.
The textures of data (occlusion, metallic-roughness and normal) hold the values that shaders read;
Blender stores them unchanged, in its Non-Color color space. The file uses
`KHR_materials_emissive_strength` without requiring it, and requires `KHR_lights_punctual`, which
Blender's exporter always marks required; it uses no other extension.

To change the asset, edit `make_scene.py` and run the command above from the repository root;
do not edit the GLB file by hand. Two runs give files that differ only in the order of the
sphere's triangles, which Blender does not keep from one run to the next; the triangles
themselves, and their winding, are the same.

The glTF loader of the examples ([`examples/common/gltf.rs`](../common/gltf.rs),
[ADR-0029](../../docs/adr/0029-temporary-gltf-loading.md)) reads GLB files that hold their buffer
and PNG images in their binary chunk (what else it refuses, and what it ignores with a warning, is
listed in its module comment), so a file exported from Blender with its default settings works
when its textures are PNG; it is temporary, until the asset pipeline of Stage 4.
