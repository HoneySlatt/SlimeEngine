# 0046. Constructors for the structs a game fills in

- Status: Accepted
- Date: 2026-10-14

## Context

A game describes each frame to the renderer with a `Scene` and its `Instance`s, structs with
public fields that it writes out in full: neither has a `Default`, since a scene needs a camera and
its instances, and an instance a mesh and a material, which have no default. Every field added to
either breaks every game, and both will grow: the next one is whether an instance casts shadows,
and the renderer's later work will add more.

Structs with a `Default`, such as `MaterialData`, the lights and `Camera`, do not have the problem:
games write the fields they set and fill the rest with `..Default::default()`, which a new field
does not break.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14, with the per-instance
shadow opt-out, that `Instance` and `Scene` get constructors so that later fields stop breaking
games.

## Decision

- `Instance` and `Scene` are `#[non_exhaustive]`: outside `slime-render`, they cannot be written
  as struct literals, so a field added later breaks no caller.
- `Instance::new(mesh, material, transform)` and `Scene::new(camera, instances)` take the fields
  without a default. The others start at a neutral value, which each field's documentation states:
  a new scene has no lights, no environment, an exposure of 1 and no bloom.
- The fields stay public: a game sets the others by assigning them once the struct is made
  (`scene.exposure = 0.5`), and reads them.
- The same rule applies to the public structs that follow: a struct that games fill in has either
  a `Default`, or, when some of its fields have none, `#[non_exhaustive]` and a constructor taking
  those. Structs that the engine fills in and games read, such as `RenderStats`, are
  `#[non_exhaustive]` without a constructor.

## Alternatives considered

- **Builder methods (`Scene::new(camera, &instances).lights(&lights).exposure(0.5)`).** A second
  way to set each field next to the public field, or private fields with a getter each; one method
  per field to write and document, for a gain only in expressions: a builder can take a temporary
  array (`.lights(&[sun])`) where an assignment needs it in a variable.
- **A `Default` with placeholder references.** A default mesh and material would have to exist
  before any renderer, and a forgotten field would draw a placeholder instead of failing to
  compile.
- **Keeping struct literals.** Every new field breaks every game, which is what this decision
  removes. Nothing is promised stable before 1.0, but breaking each game at each field costs
  without need.

## Consequences

- A game writes `let mut scene = Scene::new(camera, &instances);` then sets the fields it needs;
  lights and instances given as arrays live in variables (`let lights = [sun]; scene.lights =
  &lights;`).
- Adding a field to `Instance` or `Scene` breaks no game, provided it has a neutral value.
