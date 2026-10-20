# 0109. Stable references and undo

- Status: Accepted; amends [0079](0079-the-engines-own-world.md) and
  [0081](0081-stage-5a-the-world.md) (the editor's edits and undo name entities by their persistent
  identities, not their handles), [0092](0092-a-world-written-back-as-a-scene.md) and
  [0093](0093-saves-and-hot-reload-with-placements.md) (what the editor creates is named for good,
  and a created entity written into a scene is carried once) and
  [0101](0101-saves-and-identities-that-never-mislead.md) (what a start creates in code has a key)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 builds the editor on it, how the editor
names the entities it changes, so that undoing a deletion gives back the same entity (C1-03,
C4-01). [ADR-0098](0098-the-games-the-engine-aims-at.md) asks for identities of entities that hold
from one machine to another. The audit of 0.4 measured the figures below on the owner's RX
6900 XT at `v0.4.0`, on worlds of up to a million entities made for it; its report is kept outside
the repository, as the plan says.

An entity has two names. Its *handle*, `Entity`, an index and a generation, is valid in one world
during one run: loading a save or carrying a run into scenes that changed on disk gives new handles
([ADR-0089](0089-scenes-and-prefabs.md), [ADR-0093](0093-saves-and-hot-reload-with-placements.md)).
Its *persistent identity*, `PersistentId`, which files and saves use, is a path: `zone/door` for
what a scene places, a key for what a game creates in code at its start (`World::spawn_keyed`,
[ADR-0101](0101-saves-and-identities-that-never-mislead.md)), `created/<n>` for what a run creates
in code, numbered in the order of creation.

- **Undo is planned on handles.** The dev overlay's world panel returns each edit as an entity's
  handle, a component, a property path and a value, "the form that Stage 6's undo will record"
  ([ADR-0081](0081-stage-5a-the-world.md)); [ADR-0079](0079-the-engines-own-world.md) builds undo
  on handles, components and identities. `World::despawn` forgets an entity's identity; nothing
  creates an entity again under a given identity nested in a scene, nor puts a child at a given
  rank among its siblings (`World::set_parent` puts it last). Undoing a deletion would give an
  entity of another identity, which saves and files that named it would lose, and an undo stack
  kept across a hot reload would name other entities.
- **A created entity written into its level exists twice once carried.** An entity created in code
  under a scene's root keeps `created/<n>` while the world runs, and gets in the file an identity of
  its own, its name and four random letters or digits, `zone/torch-0rdx`, when the scene is written
  back ([ADR-0092](0092-a-world-written-back-as-a-scene.md)). Once the file is cooked again and the
  run carried into it, the world holds the file's torch and the run's: the audit's test confirmed
  it at 0.4, and again on 2026-10-20, after P1. It is the loop of an editor that writes a level
  while a game runs it.
- **What a start creates without a key is told by its rank.** A save tells a start that created
  entities without a key by a fingerprint of their numbers, their components' types and their
  names (ADR-0101): two such entities alike, without names, created in another order by a later
  version of a game, take each other's saved changes without a word, against
  [ADR-0100](0100-updates-of-shipped-games.md)'s saves that load in every later version.
- **An identity costs memory.** It is a vector of strings, held twice for an entity of a scene or
  of a key, in its node and in the world's map of identities: an entity of a scene weighs 593 to
  757 bytes in the world, and 2.5 to 3.6 KB with the resolved scene that the world keeps; finding
  an identity takes 37 to 196 ns from a thousand to a million entities. The audit estimates that a
  compact form, with the other fixes of the entities' size, takes a million entities from about
  2.5 GB to 0.4 GB.

The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**The editor names entities by their persistent identities, and undoes a deletion by creating
them again.**

- The editor's commands, the dev overlay's edits included, name entities by their persistent
  identities; a handle is a cache, found again after a load or a hot reload.
- Deleting records the subtree deleted: each entity's identity, its components as the values that
  reflection reads, the form a scene file holds, its rank among its siblings, and the references of
  other entities that named an entity of the subtree. Undoing creates the subtree again under the
  same identities at the same rank, and points those references to the entities created again.
- The world gains what this needs: creating an entity under a given identity, nested in a scene's,
  refused if that identity is taken; putting a child at a given rank among its siblings.
- A command is a value that `slime-serialize` writes: the editor keeps a *journal* of its commands,
  for undo and redo, for recovering an editor that crashed, for tests that replay an editing
  session, and for the agents of [ADR-0099](0099-ai-agents-drive-the-engine.md), whose acts undo as
  a person's do.
- Keeping a deleted subtree in memory, its components typed rather than reflected, is how a command
  may hold a large deletion, if a measurement asks for it; such a command still writes its subtree
  as values when the journal is written.

**What the editor creates is named for good.** The editor's command that creates an entity in a
level gives it at once the identity that the level's file will hold, `zone/torch-0rdx`, through the
creation under a given identity above: the world and the file agree from the start, and the
journal, references and hot reload name it one way. The world has no mode for editing: the command
chooses the identity. What a game's code creates while it runs keeps `created/<n>`, numbered in the
order of creation, so that the same code numbers them alike; loading a save numbers them again
after the world's own count (ADR-0101). A created entity that a scene written back
holds, under the ID that the world gave it, is carried once: carrying a run into the file cooked
again recognizes it by that ID, and applies the run's changes to the file's entity rather than
creating it again. ADR-0092 rejected giving a created entity the identity of its ID when first
written, because a save made before would no longer find it: a level being edited has no players'
saves.

**What a start creates in code has a key.** Every entity that a game creates in code while it makes
its start world holds a key (`World::spawn_keyed`) or is left out of saves (`Unsaved`); a save of a
world whose start created any other is refused, with an error that names them, so that a developer
meets it at the first save. What a run creates afterwards keeps `created/<n>`, and a save holds it
whole. The fingerprint of ADR-0101 stays only to read the saves of layout 3, which hold one.

**Identities across machines, and in memory.**

- Files, saves and the editor's journal keep identities as paths, readable, from Stage 6 on.
- Across machines, from Stage 10b: what a scene places and what has a key bear the same identity on
  every machine, which load the same files; what a run creates is numbered by the machine that
  holds the authority, the server, and each other machine keeps a table from the server's
  identities to its own handles; what a player's machine creates ahead of the server, the shot it
  predicts, has a provisional identity until the server confirms it. The servers of one world split
  between them number in spaces of their own, so that two never give one number.
- In memory, the world keeps identities in a compact form of its own, each part of a path held
  once and named by a number, when a measurement asks for it, and before Stage 10a's large worlds
  at the latest. The form is private: `PersistentId` stays what the API and files see.
- Identities of 128 bits in place of paths are weighed again with the foundations pass's study of
  open worlds, where an entity that moves from one cell to another, or follows the hero from one
  level to the next, may ask for them.

**When it is built.** At Stage 6, with the editor's command system and before any panel that
changes the world: creating under a given identity and at a rank, commands named by identities,
deletions undone, the journal, the editor's creations named for good, and the created entity
carried once. The key that a start's entities need, at Stage 6 too. The compact form in memory on
measurement, before Stage 10a; the rule across machines with Stage 10b's network.

## Alternatives considered

Undoing a deletion:

- **A copy of the world before each command**, a save then a load: nothing new to build, but each
  command costs the whole world, 3.7 ms for *Hunna Wood*'s zone and 0.8 to 1 s for 100 000
  entities, and every handle changes at each undo.
- **Disabling rather than destroying**, as Bevy's `Disabled` and Godot's nodes kept alive: undo in
  constant time, handles and references stay valid; but every query, the bridge to the renderer,
  saves and levels written back must leave disabled entities out, the memory stays taken, and
  carrying a run loses them.
- **Detaching the subtree and attaching it again, its components typed**: exact and fast, 0.12 to
  0.25 µs per entity, but not serializable, so no journal, no recovery after a crash and no replay;
  one more API of the world, and handles change when attached again all the same. It stays
  possible inside a command, as above.

What the editor creates:

- **The rule of ADR-0092 kept, the duplicate alone fixed**: an entity would change its identity in
  the middle of an editing session, which the journal and references would have to follow.
- **Every entity created under a scene named as in a file, while a game runs too**: one rule, but
  random letters would name the run's projectiles differently from one run, and one machine, to the
  next, which replays and the network cannot have, and saves would fill with them.

What a start creates:

- **The fingerprint of ADR-0101 kept**: the trap stays open, and silent: an update can swap two
  entities' saved changes in every player's save.
- **The fingerprint holding the values too**: the entities are told apart, but an update that
  changes one value of the start, a balance or a position, sets aside all that the save holds of
  them, and the player loses that progress.

Identities across machines and in memory:

- **Nothing decided before Stage 10**: the editor would write levels, saves and journals in a form
  that the network might ask to change, at the cost of migrating them.
- **Identities of 128 bits now**, as Unreal names the actors of its open worlds: compact, unique
  across servers, unchanged by a rename or a move; but unreadable in files and saves, the 3 scene
  files and 2 saves of today to migrate, and prefabs and their overrides to name again, for a need
  that the study of open worlds weighs better.

## Consequences

- The world panel's `Edit` names its entity by identity; the editor's undo, its inspector and its
  hierarchy hold identities, and look the handles up.
- A deleted entity comes back with its identity, its rank and the references to it; a save or a
  file that named it finds it again.
- Undo no longer needs the update in place that [ADR-0089](0089-scenes-and-prefabs.md) brings with
  Stage 6, its handles being a cache: that update stays for what holds handles across a hot
  reload, a game's code and Stage 8's scripts.
- A level edited and written back while a game runs it holds each entity once after hot reload.
- A game that creates entities in code at its start gives each a key or marks it `Unsaved`, or
  saving refuses; *Hunna Wood* and the examples are checked for it when it is built.
- What a command costs follows the entities it touches, measured when it is built; the audit
  estimates 10 to 15 ms to delete then restore a thousand entities.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6's command system names entities by identity
  and journals its commands; Stage 10b numbers what a run creates on the server.
