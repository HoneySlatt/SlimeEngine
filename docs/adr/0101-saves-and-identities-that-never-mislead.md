# 0101. Saves and identities that never mislead

- Status: Accepted; amends [0080](0080-reflection-and-serialization.md) (aliases that collide),
  [0085](0085-reflection.md) (ranges compared in a field's own type, and what the derive refuses),
  [0087](0087-the-worlds-documents.md) (the identities of what a game creates in code, saves of
  layout 3, references compared by the identity they name, and what loading warns of),
  [0088](0088-saving-and-resuming-a-run.md) (what *Hunna Wood*'s saves say, and how the game checks,
  sets aside and writes them), [0089](0089-scenes-and-prefabs.md) (`Unsaved` in a level written
  back, and the numbers of what a run spawned), [0092](0092-a-world-written-back-as-a-scene.md)
  (`Unsaved` in a level written back) and [0093](0093-saves-and-hot-reload-with-placements.md) (a
  change of level, and the numbers of what a run spawned); amended by
  [0109](0109-stable-references-and-undo.md) (what a start creates in code has a key)
- Date: 2026-10-19

## Context

The audit of 0.4 ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) found
that saves, scene files and levels written back could lose or change what they hold, or name
another entity than the one they meant, without a word;
[ADR-0096](0096-the-files-the-engine-reads.md) ranks such defects. The audit pass's first
milestone, P1, fixes them; [`docs/PLAN.md`](../PLAN.md) describes each finding by its number and
lists what P1 delivers, as the owner decided with ADR-0097:

- **G01.** An entity that code creates was named `created/<n>`, its rank of creation, and a save
  named what it changed by those numbers: an update of a game that created one more entity at its
  start, or created them in another order, made a save move or destroy another entity, or be
  refused whole.
- **G02.** Loading the save of a run that changed level, a scene spawned under the key of another,
  destroyed what of the old level survived in the run, and after two changes refused the save.
- **G03, G11, G35, G36.** *Hunna Wood* resumed a save without checking it, set every save aside
  when one of the zone's numbers moved by a unit in the last place, wrote back without what it did
  not know a save that a later version wrote, replaced a save set aside with the next, and let two
  copies of the game write one save.
- **G08, G09.** A range was checked in `f64`, so that `0.7` in an `f32` field was refused by a range
  that starts at 0.7; an alias equal to another field's name read a value back as the other field.
- **G10.** `World::write_scene` left out what a scene file marks `Unsaved`.
- **G24.** A saved reference was compared by the bits of its handle in each world.
- **G39.** A section of a save misspelled, or of the wrong kind, was ignored.

This ADR records how P1 delivered it. The plan names the ADRs that P1 amends; what they say that
the code no longer does is listed under each decision.

## Decision

**What a game creates in code at its start is named by a key.** `World::spawn_keyed(key)` creates
an entity whose persistent identity is `key`, one part, which shares its namespace with the keys of
scenes: a key that an entity alive has, a scene's root or another entity created under it, is
refused (`WorldError::IdentityInUse`), and an empty key, one that holds `/`, or `created`, is not a
key (`WorldError::InvalidKey`).
A run that creates an entity under a key saves it under `created` by its key, and loading creates
it again under it. `created/<n>` stays for what a game creates without a key, the run's
projectiles and effects.

**Saves are of layout 3, and number what the run created against the world they are loaded
into.** A save says what its start created outside scenes, in a section `start`: the count that
the start reached, `created`, and `content`, a fingerprint of the start's numbered entities, the
FNV-1a hash over 64 bits of their numbers, the names of their components' types and their `Name`s.
Loading gives what the run created, each number from the start's count up, the numbers that follow
the world's own count, whatever the start creates now. What the save holds of the start's numbered
entities is applied when the world's start has the same fingerprint, and set aside otherwise, each
difference reported with the reason. The fingerprint holds no values: a start that creates entities
alike in their types, without names, in another order, is not told apart, and the save's changes go
to the entity of each number; a key names an entity whatever the start creates. What loading sets
aside names its entity as the save does, by the save's numbers. The saves of 0.4, of layout 2,
which do not say what their start created, keep their numbers, as before; 0.4 does not read a save
of layout 3. A save that gives one key both to a scene that it spawns and to an entity that it
creates is refused, as an identity given twice (`DocumentError::IdentityInUse`).

ADR-0087 says that a created entity keeps its number and that loading keeps the larger of the two
counts; [ADR-0089](0089-scenes-and-prefabs.md) and ADR-0093 say that a placement spawned from code
takes its number from a count that saves keep, under which loading spawns it again, and that a
number that the world gives already refuses the save. This holds for the saves of layout 2 only.

**A saved reference is compared by the identity that it names.** `World::save` compares a
reference of the start's entity with the run's by the entity of the run that has the identity that
the start's reference names, whatever the order the two worlds were made in.

**A change of level keeps what survived it.** When loading spawns a scene under a key under which
the world spawned another, as when a run changed level, it destroys only that scene's entities, and
takes out of them first what is under them without being of that scene, but what a save leaves out
(`Unsaved`): the save removes it, or places it where the run left it, as a hero that the start
placed in the level and that walked out of it. ADR-0093 said that the scene was destroyed "with
what is under it". `World::spawn_scene` refuses a key under which the world spawned another scene
of which an entity lives (`WorldError::KeyInUse`): that entity's identity is under the key, as the
new scene's would be, and the next save could not be loaded. A level that keeps something of the
one before it goes under a key of its own. How an entity could live on under the key of a scene
that another replaces, as a companion that follows the hero from one level to the next, is a
question of the foundations pass's study of open worlds ([`docs/PLAN.md`](../PLAN.md), P6).

**Aliases that collide are refused.** An alias of a field or of a variant that is the name, or an
alias, of another field or variant beside it makes the derive fail to compile, `Registry::register`
panic on a type described so by hand, and `Registry::register_dynamic` return an error. A name that
another field or variant took before needs a new version of the type and an upgrade: ADR-0080 says
that renames are aliases, which holds for a name that nothing else takes.

**A range is checked in its field's own type, by one function.** `Range::holds` compares a 32-bit
float with the bounds rounded to the nearest 32-bit float, a 64-bit float with the bounds, and an
integer exactly, beyond 2⁵³ too; the registry, documents' readers and writers and the world panel
use it. `Problem::OutOfRange` carries the number as a `Number`, in its own type. The derive refuses,
besides what [ADR-0085](0085-reflection.md) lists, an integer bound that a 64-bit float does not
hold exactly: written as a number, at once, and given as a constant, by an assertion that the
compiler evaluates.

**Loading checks a save's sections, and warns of what it does not know.** A key that a save does
not hold, at the root or in a section, is a warning in `Loaded::warnings`, as a field that its type
does not have is; a section, or an entry of one, that is not a table refuses the save. A component
of a type, or whose value names a variant, that the program does not know is left out whole with a
warning, as a later version of the game writes them, where ADR-0087 set it aside as a value that
its type refuses: a game tells a save that it does not fully know from one whose entities an
update removed.

**A level written back writes `Unsaved`.** `Unsaved`, which leaves an entity out of saves, is
written as any other component for the root, the scene's entities and its placements'. Only the
entities created outside scenes that hold it, or that are under an entity other than the root that
holds it, are left out, with their descendants, and reported: what a run creates and does not save.
A reference becomes `""` only for an entity left out. An entity that a key names is left out, and
reported, as a file cannot name it. ADR-0089 and ADR-0092 said that the entities marked `Unsaved`
were left out with their descendants, the component too.

***Hunna Wood* checks its saves, and never writes over one.** What ADR-0088 decided for the game's
saves changes so:

- A save says, in `about`, the version of the game that wrote it, `version`, and the layout of the
  zone that it was made on by its numbers, `zone`, where it said a fingerprint of them, `layout`:
  793 numbers for the zone, about 4 kB of a save's text.
- Layouts are compared number by number, the same within a ten-thousandth of a metre or of a
  radian, the colliders whatever their order, and so is hot reload's. The saves of 0.4 say the
  fingerprint, and are compared by it, as 0.4 compared them.
- A save is read up to the size of the largest document (`slime_serialize::MAX_SIZE`), then
  checked, in order: it reads, as TOML of a format and a layout that this version knows; no later
  version wrote it; it says that it was made on the zone's layout, which a file that is not a save
  does not say; loading warned of nothing; and the game is found again in the world loaded, its zone, markers and
  hero by their persistent identities, as loading may have destroyed or spawned them again. One
  that fails is set aside, and a new run starts. A save of which loading set differences aside, as
  when an update removed what the run changed, is resumed without them, as ADR-0093 decided.
- A save is set aside under a name that says why, `unreadable`, `later-version`, `other-layout`,
  `unknown-content` or `unplayable`, after the save's, and never in place of one set aside before,
  which keeps its name: `save.toml.unreadable`, then `save.toml.unreadable.2`, and on.
- What is not a file in the save's place, a directory, is neither read nor moved, and the run is
  not saved.
- One copy of the game at a time holds a save, through the lock of a file beside it,
  `save.toml.lock`, taken with the standard library (`File::try_lock`), which the system lets go
  when the program ends: another copy started with the same save plays a new run that it does not
  save. Each copy writes through a file of its own, `save.toml.<process id>.new`.

## Alternatives considered

- **For what a game creates at its start: numbers per type rather than one count**, which an
  entity of another type created first does not shift, but which two entities of one type created
  in the other order still swap. **Every save's changes to the start's numbered entities set aside
  whenever the start created one more**: no wrong entity, but every update that adds one would
  lose them. **A key required for every entity created in code**: a game would name its every
  projectile.
- **A fingerprint that holds the start's values too**, which tells apart entities alike in their
  types: an update that changes one default value of a start's entity would set aside all that
  saves hold of the start's numbered entities, and its values' references would have to be written
  by identity. The owner decided on 2026-10-19 to keep the fingerprint without values, and to weigh
  it again with the foundations pass's stable references.
- **What survived a change of level kept under the old key**: two entities would claim identities
  under one key; the question goes to the foundations pass.
- **Ranges compared in `f64` within an epsilon**: one more bound chosen by hand, which a number at
  its own bound may still miss.
- **Aliases that collide resolved by an order of priority**: a value would still read back as
  another field's, without a word.
- **`Unsaved` still left out of a level written back**: the effects that a level makes again at
  each run would go at the first save of the level.
- **A coarser rounding for the layout's fingerprint**: its boundaries move, and other numbers lie
  on them. **A fingerprint of the scenes' contents**: a lamp changed in Blender would set every save
  aside.
- **A save that a later version wrote resumed, with a copy of it set aside**: the run played would
  be applied in part, which ADR-0096 refuses, and its next save would not be the later version's
  run.
- **Saves set aside under a date**: the date's formatting, without a dependency, is more code than a
  counter, which tells the order too.
- **The lock on the save's file itself**: each save renames a new file over it, which the lock
  would not follow. **A file that says which process holds the save**: it outlives a crash, and
  the next copy must guess whether it is stale; the system's lock goes with the process. **A
  dependency for the lock**: the standard library has it since Rust 1.89.
- **Unknown types and variants still set aside, the game reading the reasons**: a game would parse
  the engine's messages.

## Consequences

- A save of layout 3 is read by this version and later ones, not by 0.4; the saves of 0.4 load as
  they did, *Hunna Wood*'s fixture among them.
- A game names by keys what it creates at its start and that saves must find; what it creates
  without one relies on the fingerprint and its limit, which the rustdoc of `World::spawn` and
  `World::load` states.
- A change of level under one key waits until nothing of the level before lives, until the
  foundations pass decides how an entity lives on across scenes.
- A player who goes back to an earlier version of *Hunna Wood* finds the save of the later one
  set aside under a name of its own, and plays a new run, which the later version then resumes:
  the save set aside comes back only when the player renames it. 0.4 itself sets the saves of later
  versions aside as of another layout, under its fixed name, in place of one that it set aside so
  before, as G35 found.
- `save.toml.lock`, and the file of a game killed while it wrote, stay beside the save.
- The fingerprint that the saves of 0.4 say is compared as 0.4 compared it, its fragility included,
  until *Hunna Wood*'s fixture is made again, which [ADR-0100](0100-updates-of-shipped-games.md)
  allows before the game's first release.
