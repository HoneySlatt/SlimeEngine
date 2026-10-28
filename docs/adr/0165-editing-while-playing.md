# 0165. Editing while playing

- Status: Accepted
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided (its decision D5) that while the game that Play runs
plays, each step that the editor makes is sent to it as the same commands, over the development
channel, and that a command that the game refuses, as when its run changed or destroyed what it
names, is shown, not hidden; and that the MCP server drives two programs, the editor and the game
that it plays, as the token handed over of [ADR-0110](0110-how-the-editor-runs-a-games-code.md)
implies. Milestone M6c.4 was cut in three at its start, on 2026-10-28, as ADR-0155 allows; this ADR
records the second part, M6c.4b.

What the code held before this milestone:

- Play starts the game on a private copy of the level as the editor holds it, and drives it from a
  thread of its own through a `Client` of the development channel: pause, step, speed and stop
  ([ADR-0164](0164-play.md)). The steps made while it runs did not reach it.
- The world's commands have a text, which the editor's history and journal keep and steps of the
  channel carry (`Command::write`, `Command::read`, [ADR-0134](0134-the-editors-core.md)); reading
  one that places a prefab resolves the prefab through a library of scenes.
- A running game answers a value set (`Request::SetProperty`), which it applies through its rules
  (`Simulation::apply`); nothing else of the world's commands reached a game that is not an editor.
- Undoing a deletion restores what it deleted as it was, a value that the history holds and that
  has no text ([ADR-0134](0134-the-editors-core.md)).
- The MCP server drove one program, the one that `start_game` started.

## Decision

**A request of the channel applies the texts of commands.** `Request::Apply` carries texts of the
world's commands, which the game reads as its world is when each comes, through the library of
scenes that the game gives for a placement (`Simulation::scenes`, of development builds), and
applies through its rules, in their order. It fails at the first command that cannot be read or
that the game refuses, saying which and why and how many after it were not applied; those before it
stay applied. The protocol is at version 11. *Hunna Wood* and the `world` example give their
library; a game that gives none refuses the placements. An editor answers it too, through its
level's library, each command a step of its own, as a value set is.

**The editor sends what its steps applied.** While Play runs, the editor's core records the texts
of the commands that each step made, undone or done again applied, in their order: a step's own, an
undo's inverses, a redo's again. Each request that the editor answers, whether a person's from the
window or an agent's, ends by sending the game what was recorded since, one `Request::Apply` for
each step, from Play's thread, so that a step that the game refuses holds up no other; the steps
made while the game starts are sent once it runs, the game having read the level as Play wrote it.
A gesture's step reaches the game when the gesture ends, not at each frame of the drag, and Play
does not start during a gesture, whose cancel would not reach the game. A refusal is told, as a
notification and in the editor's log, and the level stays as the editor holds it: the game and the
level differ from then on, as they differ already where the game's rules moved what the level
holds. Another level does not open while the game plays the one open, which the steps made in the
other would not reach.

**An undone deletion does not reach the game.** Its restoration has no text; the editor tells the
user that the game plays on without what it restored, and a later step that names it is refused by
the game, which is told too.

**Agents drive both programs.** The editor answers where the game that it plays listens, and its
token (`Edit::Played`), once the game runs. The MCP server's `drive_played_game` connects to it
with them: the game's tools then reach the game that the editor plays, and the editor's tools the
editor still, until `drive_played_game` points them back at the editor, or the game no longer
answers, which points them back by itself. `start_game` and `stop_game` start and stop the program
that the server runs, the editor, whose end ends the game that it plays; `play.stop` stops the
game alone. The server never shows the token to the agent. A step reaches the game a moment after
the editor answers it, from Play's thread. As an agent may pause the game, step it or change its
speed itself, Play's thread asks the game how its time stands at each of its looks, every 200 ms,
and the toolbar follows what it answers.

## Alternatives considered

- **The steps sent as the editor's own requests, `Edit::Step`**: a game is not an editor, and has
  no history to make a step of; a request that applies commands answers any game that keeps the
  channel.
- **A step applied whole or not at all in the game**, its commands undone at the first refused:
  the game would need the inverses of what its rules applied, which `Simulation::apply` does not
  give; a step that the game refuses leaves the game and the level apart in either case, and the
  user is told.
- **An undone deletion sent as the commands that create again what it deleted**: the commands of a
  copy, which [ADR-0162](0162-creating-copying-and-arranging.md) writes, but under the identities
  that the game no longer has, and without the run's own changes to it; it waits for a need.
- **Each frame of a gesture sent**: a drag moving the game's entity as it moves the level's, but a
  request a frame, and the game applying what the gesture undoes and applies again at each; the
  end of the gesture suffices to see it.
- **Two MCP servers, one per program**, or **a parameter of every game tool naming the program**:
  an agent would hold two sessions, or give a program at every call; one switch keeps the tools as
  they are.
- **The game's address and token given to the agent**, which would connect a second server: a
  token shown is a token kept in the agent's context; the server keeps it.

## Consequences

- An agent edits a level while the game plays it, and reads, steps and changes the game itself, in
  one session.
- A step that names what the game's run destroyed, or that the run changed, is refused or applied
  over the run's change; the user is told of a refusal, not of an override.
- A step that the game refuses part of stays applied in the game up to the command refused.
- Play's thread asks the game how its time stands five times a second, a request that the game
  answers within a frame.
- What an undone deletion restored is missing from the game until Play starts it again.
- Any tool with a game's token applies the world's commands to it, as it set values already.
