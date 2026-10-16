# 0088. Saving and resuming a run

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (a save begins with what the game
  says of it)
- Date: 2026-10-17

## Context

[ADR-0081](0081-stage-5a-the-world.md) decides how *Hunna Wood* saves: F5 saves the run and F9
loads the save; the game saves when the player quits and resumes the save when it starts; one file
in the user's data directory, `$XDG_DATA_HOME`, else `~/.local/share`, on Linux, `%APPDATA%` on
Windows, another with `--save <file>`, and `--new` for a new run; a save made on another layout of
the zone starts a new run, as hot reload does; the checks never touch the user's save; a save
committed as a fixture, which every later version must load; the log says what was saved or loaded.
[ADR-0087](0087-the-worlds-documents.md) decides what a world's save is: what differs between the
world and the world that the game makes at the start of a run.

That leaves, for the milestone that builds it, M5a.9:

- **How a save says the layout it was made on.** A save holds only differences from a world made
  again from the scenes as they are now: what a run was made on is the same in both worlds, and no
  difference keeps it.
- **What the start world is**, when `--night` starts a new game at night.
- **What the game writes at each frame**: the view makes the campfire's light flicker by writing its
  intensity into the world at each frame, which every save would hold.
- **How a file is written, and what becomes of one that cannot be read.** [ADR-0087](0087-the-worlds-documents.md)
  leaves a save cut at the boundary of a section loadable in part, to the game that writes files.

## Decision

The owner took the first decision on 2026-10-17.

**A save begins with what the game says of it.** `World::save` takes `about`, keys and texts that
the engine writes first, under `about`, and does not read; `World::about` reads them from a save's
text without a world and without loading the save, so that a game tells whether it can load a save,
and later lists its saves by what they say: the time played, where the run stands, the version that
made it. *Hunna Wood* writes `layout`, the fingerprint of its zone's layout, the FNV-1a hash of the
layout's numbers, the same on every platform and with every version of Rust; a save whose layout is
not the zone's, or that says none, is not resumed, and a new run starts.

**The start world is a new game by day.** Saves differ from a new game by day, whatever `--night`
says, and a run resumed is a new game by day to which the save is applied, so that a save made at
night is resumed at night, with or without the option; `--night` decides only how a new run starts,
and R.

**What the game makes again at each frame is put back before it saves.** The campfire's light goes
back to the intensity it flickers around, which the zone gives it, so that a save of a run that has
not changed is the same save, whatever the frame.

**A save is written whole or not at all.** The game writes it to a file beside the save's, then
renames that file over it: the save's file holds the save before or the new one, never part of one.
A save that cannot be read is set aside, as `save.toml.unreadable`, in place of the one set aside
before, so that the next save does not write over it, and a new run starts. A save of another
layout is left where it is, and the next save writes over it.

**F9 replaces the world.** A run loaded is a new world, given to the engine in place of its own,
with a new `Game` that refers to its entities; the view holds no entity and draws it from the next
frame, the hero at once where the save puts it, since a new world keeps no transform from before a
step, and the camera behind the hero.

**The checks never touch the user's save.** `scripts/check.sh` runs the game with `XDG_DATA_HOME` in
its private directory, starts its runs with `--new`, and starts one from a copy of the fixture with
`--save`, which must log that it resumes the run. The fixture, `games/hunnawood/fixtures/save.toml`,
is a save made with F5 at the chapel's portal, at night: a test resumes it and walks on to the
terrace. A change of the zone's layout makes it a save of another layout, which the test refuses:
it is made again then, as the game's README says.

## Alternatives rejected

- **The layout kept in the world**, a component that only the run's world holds, so that every save
  writes it: no engine change, but a device that every game would make again, and a save that must
  be loaded whole to be read.
- **A new run whenever the zone's scene changed**, as the fingerprints of ADR-0087 say: nothing to
  add, but a decorative rock moved or a light changed in Blender would end the players' runs at the
  next update.
- **The start world made as `--night` asks**: a save made at night would be resumed by day without
  the option.
- **The flicker left in saves**: harmless when loaded, the next frame writing it again, but every
  save of an unchanged run would differ, and a save would hold what nothing reads.
- **A save written in place**: a crash or a full disk in the middle would leave part of a save,
  which ADR-0087 loads in part.

## Consequences

- Every game can say in its saves what they were made on, and read it without loading them; reading
  it parses the save whole, as loading does.
- A save that the game cannot resume is never lost silently: it is set aside, or kept until the next
  save, and the log says why.
- The fixture is made again when the zone's layout changes, deliberately, in the commit that changes
  it.
- When Windows ends the session, the game is stopped without quitting, and the run since the last F5
  is lost: [ADR-0081](0081-stage-5a-the-world.md) leaves a fix, which needs a new dependency, to a
  later decision.
