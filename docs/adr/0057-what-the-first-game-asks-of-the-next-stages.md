# 0057. What the first game asks of the next stages

- Status: Accepted
- Date: 2026-10-15

## Context

Stage 3c made a first game, *Nightwatch* (`games/nightwatch`), with the engine's public API only,
so that what a game finds hard shapes the stages that follow
([ADR-0052](0052-amend-roadmap-before-0-1.md)). As the game was built, each thing it needed and the
engine did not give was recorded as a finding of the stage, with what the game did instead. There
were eleven. Two of them, spot lights that cast no shadows and jagged edges, became Stage 3d
([ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)). The owner decided on the
others on 2026-10-15, at the end of the stage. This record keeps them: the plan of a stage is cut
from `docs/PLAN.md` once the stage is done.

## Decision

Each finding goes to the stage below, which the roadmap lists.

**Done in Stage 3c: simple shapes.** The engine had none, so the game, the examples and the
renderer's tests each made their own: two cubes, three spheres and five squares. `slime-render`
makes them now, as `MeshData::cube`, `MeshData::sphere` and `MeshData::square`, one metre across,
before Stage 3d, whose tests need them.

**Stage 3d: the cost of the scene pass on Direct3D 12.** In the Windows virtual machine, on an RX
6900 XT, the scene pass of the game's first view took 2.9 ms on Direct3D 12 and 1.2 ms on Vulkan;
the other passes cost about the same on both. The cause is not measured: the code of shaders
compiled by FXC ([ADR-0036](0036-direct3d-12-shader-compiler.md)), which may be slower than DXC's,
or the GPU's clock under a light load. Stage 3d, which makes that pass heavier with multisample
anti-aliasing, measures it on Direct3D 12 with FXC and with DXC, and on Vulkan; if FXC is the
cause, ADR-0036 is reopened with the figures.

**Stage 4.**

- **Loading a model.** The temporary glTF loader is example code (ADR-0029), so the game draws
  shapes made in code. Stage 4's exit asks already that the game load its scenes from cooked
  assets.
- **Tangents.** `MeshData::tangents` is required, even for a mesh whose materials have no normal
  texture. The engine computes the tangents of a mesh that has none, with MikkTSpace, as glTF asks;
  Stage 4's importer must compute them anyway.
- **A material whose values change in place.** A material cannot change: the game makes one per
  state of what it shows (a switch red, brighter within reach, then green) and picks one at each
  frame, which serves a few states but not a value that changes smoothly, such as a lamp that
  pulses. Hot reload needs the same thing: a material whose source changes must change in the game
  that runs.

**Stage 5.**

- **The world keeps what is drawn.** A game keeps its own objects and rebuilds the list of
  instances at every frame; the world model keeps what is drawn, and a game changes only what
  moves.
- **The world keeps where things were.** To draw what moves between its last two fixed steps
  (`Context::fixed_step_alpha`), a game keeps the previous transform of each moving thing; the world
  keeps the transforms of what moves at both steps, and draws it between them.

**Stage 6, or else 7: the keys and buttons held.** The engine reports a key going down and up, not
the keys held, so a game keeps them, which the engine makes reliable: it releases the keys and
buttons still down when the window loses the focus. The engine keeps them at Stage 6 if the editor
moves its viewport's camera with them, together with the question of sharing the keyboard between
the editor's UI and the game; otherwise at Stage 7, with the input action mapping.

**Stage 7: queries on the world.** The game's camera and its lights' sight test a line against the
game's own blocks. Queries, a ray or a segment against what the world holds, come with the physics,
which the roadmap lists already: what stops a line of sight is a collision shape, which the physics
knows, not what is drawn.

**Stage 9b: values set per instance.** A tint or an emission that differs between instances of one
material comes with the material graph.

## Alternatives considered

- **Simple shapes at Stage 4**, with the assets: everything about meshes in one place, but the
  copies stay and Stage 3d adds to them. **No shapes in the engine**: every game writes the same
  code, where other engines give them.
- **The keys held now**: small, but the sharing of the keyboard with a UI that has the focus would
  be decided before the editor that raises it. **At Stage 7 only**: the editor may need them first.
- **Materials that change, and values per instance, now**: no game needs them yet, and they would
  come before Stage 3d. **Both at Stage 9b**: hot reload needs materials that change at Stage 4.
- **Queries at Stage 5**, against the boxes around what is drawn: sooner, but those boxes follow the
  drawing, not the collisions, and the physics would do the work again.
- **Measuring Direct3D 12 now**: one more session in the virtual machine for a gap that hinders
  nothing yet. **Not before the performance work of Stage 11**: every game on Windows would pay
  meanwhile if FXC is the cause.

## Consequences

- The roadmap's Stages 3d, 4, 5, 6, 7 and 9b gain these items.
- Until those stages come, *Nightwatch* keeps its workarounds: the keys it holds, its own lines of
  sight, its previous positions, a material per state.
- The game, the examples and the renderer's tests share the engine's shapes; a test that needs
  another size scales them with its instance's transform.
