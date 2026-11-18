# 0310. The journal's wait answered once the simulation ran, through a read of its own; its tool gives `end`

- Status: Accepted; supersedes in part
  [0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
  (what the tools that wait give back)
- Date: 2026-11-18

## Context

M6d.9a2c2 ([`stage-6d.md`](../stage-6d.md#m6d9a2c2--the-journal-that-waits)) makes
`editing/journal` wait for the editor's next steps, as
[ADR-0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
decided, under the rules that
[ADR-0308](0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md)
set for every wait: a program at rest with a wait in flight runs no frame and wakes no thread; a
connection holds fewer waits than its places in flight; tests never wait the tools' bound.
[ADR-0267](0267-a-page-of-the-journal-apart-from-the-one-handler-a-cut-journals-start-refused-a-floor-of-4-kib.md)
lets a read that changes nothing join the page of the journal outside the editor's one handler,
by an ADR: this is one.

The facts, read in the repository on 2026-11-18, at `2f4f7d1a`:

- `Simulation::edit` answers a request at once, and takes the world to change it.
- `slime-app`'s channel answers requests at a frame's start. What it keeps for a later frame,
  captures, steps, pickings and edits behind a gesture, counts in `DevChannel::waiting`, which
  makes the run's loop run frames.
- The journal grows only on the editor's thread: in a frame, by a tool's step at its start,
  before the simulation's phases, and within them (`Host::frame`), by a command that the
  presentation sent, applied as they start, and by a person's step or a level opened, in the
  update; after the last frame, by the commands that `Host::stop` applies, when the waits still
  held are answered that the game has stopped. A level opened makes a new journal, whose number
  makes the old cursors stale.
- The listener wakes the game when a request is cancelled or a connection closes.
- ADR-0269 says that the tools that wait give `after`, a handle that the next call passes back,
  and that the journal's is its cursor. `read_logs`, in the same server, gives `end:` for the
  cursor at its log's end.

A fresh agent attacked the lead's first design before it was built. It found a contract that let
a request asked again change the level at each frame; a timeout that would leave the request held;
tests that could not fail; a test session that dropped answers that came out of order; the file
read once for each held wait; and the name `end` against ADR-0269.

## Decision

The owner's decision of 2026-11-18, on what the tools that wait give back:

**The tools that wait take their handle back as an argument, and give it back as a line of
their result.** The journal's tool, `editor_journal`, gives `end`, a cursor at the journal's end,
as `read_logs` names its own and as the program's reply names it: one name in the server for a
cursor at a list's end. A wait that no entry answers gives its cursor back, as `cursor:`. The
interface's tool, at M6d.9a2c4, gives `after`: its handle is no list's cursor. The rest of
ADR-0269 on requests that wait stands: each waits at most a bound under 60 s, and the server keeps
nothing for an agent.

The lead's decisions within ADR-0269 and ADR-0308:

- **A read of its own answers a request that waits:** `Simulation::wait`, which takes `&self`
  and gives no reply while what it waits for has not come. `edit` keeps answering at once, so
  that a request asked again can change nothing.
- **The channel holds the waits apart,** not counted among what makes frames run, and asks them
  again in each frame that runs, once the simulation's phases ran, before the frame is drawn; the
  waits that ask the same page are asked once, within the requests' budget parted among them. A
  cancel, or a connection that closes, wakes the run, whose frame answers it so.
- **A connection holds at most 8 waits,** half its 16 places, counted across every family, those
  cancelled but not yet answered left out: one more that would wait is answered `BUSY`.
- **The journal finds a wait at its end without reading its file:** its number, its torn mark and
  the cursor's place say it, so that a wait asked again costs a cursor's parse.
- **`editor_journal` takes `wait`, in seconds, at most 50,** under the 60 s after which Codex
  stops a call; more is cut to 50. Its bound cancels the request through the library's own
  cancel, never by dropping it, so that the editor frees the request's place. The pages after the
  first are gathered without waiting, by the call's own clock.

## Alternatives considered

What the journal's tool gives back:

- **`after`, as ADR-0269 said:** no ADR needed, but two names for one thing in one server.
- **`after` everywhere, `read_logs` renamed:** one name for every wait's handle, but `read_logs`'s
  output changes, and the name differs from the program's field.

The contract:

- **`edit` answering `None` while it waits:** one method, but a simulation that answered `None` to
  a step would make that step again at each frame.

Where the waits are asked again:

- **At a frame's start:** a step made in a frame's update would be seen only at the next frame,
  which a run at rest never runs.
- **At a frame's end, after it is drawn:** what M6d.9a2c4's waits on the interface may need, but
  the answer then waits for the drawing, a long one on a CPU driver.

The tool's bound:

- **A timer that drops the request's future:** simple, but the request would stay held in the
  editor until the connection closes, and eight timeouts would fill its places.

## Consequences

- `slime_app::Simulation` gains `wait`; a game leaves it to its default refusal.
- `editing` rises to 3; `Reply::Journal` gains `end`, which the families `play` and `world`
  describe too, without a version raised.
- A frame in which the journal grew while waits are held spends up to another requests' budget,
  past one page's floor for each page asked, after the simulation's phases.
- A request that waits holds a place for up to 50 s; with edits held behind a person's gesture,
  a connection's places may still all be taken. A call of the tool lasts up to 50 s of waiting
  and the time to gather the pages after it.
- M6d.9a2c4's interface waits share the channel's holding and its count of 8, and choose their
  own point of the frame.

## Validation and reconsideration

Tested: a held wait runs no frame in a run on demand, and the editor at rest with one in flight
wakes none of its threads in 10 s; its connection's reads pass, a ninth wait of it is `BUSY`, and
another connection's wait is held; a step answers every wait in its frame; a level opened while a
wait is held answers it stale; a journal whose write failed refuses a wait; a tool's bound passed,
or its agent's cancel, cancels the request in the program, on Windows too. Proposing another
decision is justified if holding waits costs a frame's budget with many agents, or if an MCP
client's limit on a call's time falls under 50 s.
