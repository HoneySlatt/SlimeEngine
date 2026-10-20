# 0083. Transforms and the hierarchy

- Status: Accepted; amended by [0087](0087-the-worlds-documents.md) (keys and names that hold `/`)
  and [0106](0106-the-measured-quick-wins.md) (the children read through an iterator, linked to
  their siblings)
- Date: 2026-10-16

## Context

[ADR-0079](0079-the-engines-own-world.md) decides that the world keeps the hierarchy; that
translations are `f64`; that an entity's local transform is a translation, a rotation, a uniform
scale that its children inherit and a stretch that they do not; that its transform in the world is
computed again only for the subtrees that changed, and is current whenever it is read; that change
detection covers transforms only; and how an imported node's scale splits. It leaves how to the
milestone that builds them, M5a.2, which also spawns a cooked scene as entities and gives them
persistent identities.

What M5a.1 left ([ADR-0082](0082-the-worlds-storage-and-handles.md)): components are stored by type,
and `World::get_mut` and `World::query_mut` hand out `&mut` to any component, after which the world
learns nothing. A cooked scene's nodes hold a name, a parent, an `f32` matrix, meshes, a light and
`extras`; each node comes after its parent. *Hunna Wood*'s zone has 543 nodes, all named, 25 of them
scaled unevenly, all without children.

## Decision

**Transforms are components.** `Transform` holds the translation (`DVec3`), the rotation (`Quat`),
the uniform scale (`f32`) and the stretch (`Vec3`), so that reflection, the dev overlay's panel and
saves reach it as they reach any component. Where an entity is in the world is a value,
`GlobalTransform`, a translation, a rotation and a scale that `World::global_transform` gives: the
parent's composed with the child's, translations in `f64`, rotations and scales in `f32`. Its
`to_affine(stretch)` is the matrix that places the entity's meshes. An entity without a `Transform`
has no transform in the world, and counts as none for its children.

**The hierarchy is the world's.** Each slot keeps its parent and its children in the order they
became children. `World::set_parent` refuses an entity that is not alive and a parent that is the
child or one of its descendants (`WorldError`); the child keeps its `Transform`, now relative to its
new parent. Destroying an entity destroys its subtree.

**Change detection.** Every way of changing a `Transform` marks the entity's subtree stale: attaching
or detaching one, reaching it through `get_mut`, or naming `&mut Transform` in a `query_mut`, which
marks each entity it visits, and reparenting. `World::update_transforms` computes each stale
subtree once, from its top down. `World::global_transform` is current without it: a transform that is
not stale is read from where `update_transforms` left it; a stale one is computed from the nearest
ancestor that is current. A stale node's descendants are stale too, which marking keeps true.

**Cooked scenes.** `World::spawn_scene(key, &scene)` spawns a root entity, then an entity per node,
each the child of its parent's, the scene's root nodes children of the root, in the scene's order.
Each holds a `Transform`, its `Name` if it has one, the `Meshes` and the `Light` its node places, and
its `Extras`, the cooked scene's own values. A node's matrix splits by ADR-0079's rule: a scale
uniform within a relative 10⁻⁵ becomes the uniform scale, otherwise the stretch; a node with
children scaled unevenly or mirrored is logged with a warning, and its descendants are placed from
where the file puts them in the world. A shear, which a node turned under a node scaled unevenly
gets, is dropped: the node's rotation keeps the direction of its first axis that is not flattened,
and makes the others square to it. The world splits the matrix itself rather than through glam's
`to_scale_rotation_translation`, which glam's assertions refuse for a matrix that flattens an axis,
and which gives a sheared matrix a rotation that is not a unit quaternion.

**Persistent identities.** The root's identity is the key; each node's is the key and the node's
name, or its index, `#12`, with a warning, when the name is missing, repeated, or starts with `#`
as indices do, so that no two entities of a scene share an identity. The world keeps a map from
identity to entity (`World::find`, `World::persistent_id`); spawning a scene whose identities an
entity has already, as one spawned twice under a key, is refused before anything is spawned.

**Measured** by the crate's ignored tests, in a release build on the owner's machine (Ryzen 7
5800X), with 1 000 roots of 99 children each: computing the 100 000 transforms in the world took
3.4 ms; computing again the 1 000 whose root moved, 32 µs; reading 99 000 current transforms in the
world one by one, 3.1 ms. Each slot now keeps its place in the hierarchy, its identity and its
transform in the world, 112 bytes, written when an entity spawns, so that ADR-0082's measurement
changes: spawning its 100 000 entities takes 10.5 ms (5.3 ms without the hierarchy), mostly writing
that memory for the first time; destroying them, 1.6 ms (0.85 ms); reading their positions one by
one, 1.4 ms (0.98 ms); its queries cost what they did. *Hunna Wood*'s zone spawns in a world
placed where its file puts it, each of its 543 nodes known by its name.

## Alternatives considered

- **Transforms outside the components**, behind an API of their own that alone changes them:
  nothing could change one unseen, but reflection, the panel and saves would each treat them apart.
- **Marking every component reached mutably**, a tick per slot: change detection for every type,
  which ADR-0079 leaves until a user needs it, at a cost on every access.
- **Transforms in the world computed only when the world is updated**, read from where the update
  left them: one step per read, but a transform changed in a frame would be read a frame late, as in
  Bevy.
- **No stored transform in the world**, each read composing every ancestor: nothing to keep current,
  but each read costs as many steps as the entity is deep.
- **`GlobalTransform` as a component**: a game could write it by mistake, and the world would have
  two places to keep consistent.
- **An identity from the path of names in the hierarchy**, which a change of parent breaks, or from
  the node's index, which an export that reorders the nodes breaks.

## Consequences

- A `query_mut` that names `&mut Transform` marks every entity it visits, whether its closure changes
  the transform or not: a game that only reads transforms names `&Transform`.
- Reading a transform in the world before `update_transforms` costs a step per stale ancestor; after
  it, a lookup. M5a.3 has `slime-app` call `update_transforms` around each fixed step. Until it
  runs, the world's list of the subtrees that changed grows with each change.
- `update_transforms` does not report which entities it computed again, and `GlobalTransform` holds
  no stretch: M5a.3, which draws what moved between two fixed steps, decides how it learns what
  moved, and whether a stretch changed during a step is drawn between its two values.
- A mirrored node with children is flattened, its children placed apart; a negative uniform scale
  could pass the mirror on to them, which no file has needed yet.
- A key or a node's name may hold a `/`, with which `PersistentId`'s display joins its parts: the
  parts stay apart in the world, and M5a.8, which writes identities into documents, decides how
  they are written.
- `slime-world` depends on `slime-assets` for cooked scenes, on glam for its vectors, on `thiserror`
  for `WorldError` and on `tracing` for its warnings.
- `Meshes`, `Light` and `Extras` hold what a cooked scene's nodes hold; M5a.3 decides how what the
  world draws is described, and M5a.4 what *Hunna Wood* reads from the extras.
- A node renamed in Blender changes its identity, and the saves of M5a.9 lose what they knew of it.
