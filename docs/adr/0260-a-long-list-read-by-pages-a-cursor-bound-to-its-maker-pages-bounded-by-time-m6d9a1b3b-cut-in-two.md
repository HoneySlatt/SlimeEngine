# 0260. A long list read by pages: a cursor bound to its maker, pages bounded by time and bytes, long texts cut; M6d.9a1b3b cut in two

- Status: Accepted; amends
  [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (the cursor: what it is bound to, its fault, each list's place, a page's bounds) and
  [0143](0143-what-agents-see-of-the-interface.md) (a list of elements by pages, its texts cut);
  amended by [0263](0263-the-cursor-of-elements-anchored-on-its-next-sibling-what-a-list-may-skip-the-slots-read.md) (what the cursor of elements holds and what a list of elements may skip;
  what `World::entities_after` gives) and [0265](0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md) (the journal's number, its bounds and its
  floor)
- Date: 2026-11-14

## Context

[ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
decided the cursor of a long list: an opaque text, `cursor` in a request and `next` in an answer,
a page bounded by a count and by bytes, each list resumed where it cheaply can, a stale cursor
faulting −32602.
[ADR-0259](0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md)
moved the interface's family into `uwui` first, so that the elements' cursor is written once, and
listed what an attack on the cursor had found.

The facts, read in the repository on 2026-11-14:

- `interface/elements` and `world/entities` give at most 1,000 items and count the rest
  (`more`). The elements' answer builds `Ui::meaning()` whole, every node's path formatted; the
  entities' walks `World::entities()` from slot 0.
- `play/logs` gives the lines after a number, at most 1,000 of any length. `editing/journal`
  reads the journal's file whole into one answer, and fails past the tool's line.
- An answer larger than the tool's line becomes `TOO_LARGE` whole, cursor included: one item
  larger than the line would stop a list for good.
- The tool's line limit is known to the core's session alone; the host answers without it.
- The game (`*world = resumed`) and the editor opening a level (`*world = fresh`) replace the
  world: its slots start again from 0. A new `Ui` starts its ids again.
- A floating panel raised, or a group moved below a modal root, reorders `Ui`'s roots without
  removing them. Each element gets a number at its insertion that is never given again.
- The editor's journal holds the level whole on one line: its first entry escapes the level's
  text. The `Editor` is replaced when a level is opened.
- `slime-channel` sends every refusal as `REFUSED`; `slime-app` drops uwui's codes too.
- The first round of the tools' requests runs whole each frame; seven agents listing a thousand
  entities each already pass the 0.5 ms of a frame's start alone
  ([`measurements.md`](../measurements.md#m6d9a1b1-requests-in-flight)).
- `std`'s `RandomState` draws its keys from the system on Linux, Windows and Android, but not on
  `wasm32-unknown-unknown`. `slime-channel` draws its tokens with `getrandom` already.

The field, read on 2026-11-14 (sources saved by the lead):

- MCP `2026-07-28`, the latest revision: an opaque `cursor` and `nextCursor`; the server alone
  chooses a page's size, and clients must not assume one; an invalid cursor should fault −32602;
  no lifetime is stated. Only MCP's own lists page, not a tool's call.
- Google's AIP-158: a page may be short, or empty, and still give a token; only an empty token ends
  the list; a token given with other parameters should be refused; a total is optional and may be
  estimated.
- Kubernetes: a `continue` token expires after some 5 minutes, answered 410 Gone with a reason of
  its own; the pages come from one snapshot; the count of the rest is left out under a selector.
- AT-SPI's `GetMatchesFrom` has no cursor: a client resumes from the last object that it read.
- Claude Code moves an MCP tool's output past 25,000 tokens, or 50,000 characters, into a file,
  and gives the agent its path: an agent reads a smaller page better than a larger one.

A fresh agent attacked the lead's first design before it went to the owner. It found the worlds
and roots above, the journal's single line, the generation's home, the first round's cost, and
the milestone too large for one session.

## Decision

The owner's decisions of 2026-11-14.

**M6d.9a1b3b is cut in two**, each part in a session of its own:

- M6d.9a1b3b1, lists: a family's codes of faults carried both ways; the tool's line limit given to
  the host; the cursor; entities, with `slime-world`; elements; long texts cut; the log's pages
  bounded in bytes; `world` 2 and `interface` 3; the MCP server's lists paged.
- M6d.9a1b3b2, the editor's journal: its generation, its pages read by seek, `editing` 2,
  `editor_journal` paged.

**`slime-world` is on M6d.9a1b3b1's card,** for two things:

- `World::entities_after`, the entities alive after an entity's slot, whether that entity lives or
  not: a page costs what it reads, not the slots before it.
- A number for each `World`, from a counter of the process: a world replaced gives its cursors up.

**A cursor is bound to the process and to what made it.**

- The carrier draws a number for the process once, with `getrandom`, and gives it with each
  request: `slime-channel` for the engine, a web carrier from its host's randomness later.
- The maker adds its own: the `World`'s number, the `Ui`'s (a counter of the process), the
  journal's generation.
- A cursor also holds its list and its filter, normalized as the match reads it.

**A cursor that no longer holds faults with a code of its own,** `STALE`, −32005, a code of the
core's: the process, the world, the `Ui` or the journal is another. A tool then starts the list
again, on that code alone. A cursor that does not read, or of another list or filter, faults with
`BAD_PARAMS`, −32602, as a bug's would. uwui's MCP library (M6d.9a2) gives `STALE` to MCP's
clients as −32602, which MCP's specification asks.

**An element gone between pages does not stop the list.** The cursor holds the next element to
visit and its ancestors' ids. A page resumes at the first of them still alive, at the place of the
child removed. The roots are walked in the order of their numbers, which raising a panel does not
change.

**A page's work on the program's thread is bounded by time.** Each page gets its share of the
frame's start, shared among the round's pages, with a floor of visits so that a list always moves
on. The clock is read every few dozen visits. Items visited count, matched or not, dead slots
included: a page may come back empty with `next`.

**Long texts are cut and marked.** Each text of an item is cut, on a character's boundary, to a
limit drawn from the page and counted in bytes of JSON: an element's path, name, value,
description and placeholder, an entity's identity and name, a log's line, cut as it is kept. The
item lists its fields cut (`cut: ["value"]`). No item is larger than a page, so no list stops on
one.

**`more` is removed.** `next` alone says that the list goes on: counting the rest walks it whole.

**A request may ask a smaller page:** `limit`, in items, and `bytes`, in bytes of JSON, each
capped by the program. The MCP server's lists ask about 32 KiB by default, write `next` first, and
let the agent ask less.

**The lead's decisions within them:**

- A page holds at most 1,000 items, and at most the smallest of 1 MiB, the tool's line and the
  request's `bytes`, in JSON. Each item is counted as it is written; the envelope takes its
  margin.
- The session gives the tool's limits in its welcome to the carrier, which keeps them with the
  connection and gives them with each request.
- Each list's place: entities after the last slot visited; elements as above; the log after its
  last line's number, as now.
- `Reply::Failed` of `slime-channel` carries its code both ways; `slime-app` passes uwui's codes on.
- `interface` rises to 3 and `world` to 2 at M6d.9a1b3b1, `editing` to 2 at M6d.9a1b3b2, as
  ADR-0254 and ADR-0259 decided. `play` stays at 1: its log gains `partial`, which a tool of after
  reads as false from a program of before, rightly, as such a program gives every line kept; a
  tool of before asks again after the last line as it does now. ADR-0254 rejected raising `play`
  for no reader.
- The guarantee stated for each list: an item present from the first page to the last is listed
  once; one added or removed between pages may or may not be, and among elements, the siblings
  of one removed may be skipped or doubled. A list's rows named by number keep
  their ids while their content shifts, and may be skipped or doubled: written as debt.
- `editing/history`, `world/entity` and `play/report` stay whole: written as debt.

## Alternatives considered

The cut:

- **One milestone**, as the card had it: one review and one raise of versions, but the stage's
  longest session, the journal a problem of its own.

`slime-world`:

- **The world's number alone**, slots skipped in `slime-app`: each page costs every slot before
  it, past a page's share of the frame from 100,000 entities.
- **Nothing in `slime-world`**, `slime-app` counting the replacements that it sees: blind to those
  that the game and the editor make themselves.

What a cursor is bound to:

- **`RandomState` drawn in `uwui-agent`, with the maker's number**: nothing to give, but not random
  on the web.
- **The process alone**: a world or a `Ui` replaced in the same process gives one list sewn from
  two, with no fault.

A stale cursor:

- **−32602 for every cursor refused**, as ADR-0250 and MCP: a tool cannot tell a list to restart
  from a bug, and a rule of one restart hides the bugs.

An element gone:

- **A fault, the tool starting again**, ADR-0250's rule: a list that never ends in an interface
  that changes at each page, as one whose notifications come and go.

A page's work:

- **A count of visits measured on the reference machine**: simple and the same each run, but up
  to eight times a frame's start with seven agents, and slower on a modest machine. ADR-0250
  rejected a count for its rounds as blind to a request's cost.

Long texts:

- **Cut, but paths and identities whole**: an element can always be acted on, but a huge path
  stops its list for good.
- **`TOO_LARGE` as now**: the fault replaces the page, its `next` included.

`more`:

- **Kept**: each page walks the rest. **A total when no filter is given**: cheap, but a second
  field often missing.

A page's size:

- **The program's alone**, as MCP: the MCP server gets 1 MiB and must cut inside a page, with no
  cursor for the rest.

## Consequences

- `interface` 3 and `world` 2: a tool of before cannot list on a program of after, nor the
  reverse; their other requests are refused with the family.
- The core gains the fault `STALE`; its version stays, as only the families' new versions send a
  cursor.
- `slime-world` gains `World::entities_after` and a world's number; `uwui` a `Ui`'s number and
  the walk of the tree from an element.
- The carrier draws a number per process and gives it, with the tool's limits, to each request's
  answer: `uwui::agents::answer` takes them.
- A list of elements is in the order of the roots' numbers, not their order of drawing.
- An agent reads a text cut where it is huge, and acts on no element whose path is cut.
- An agent does not learn how long a list is before reading it.
- The figure of a page of a thousand elements, and of the frames with seven agents listing, are
  measured again at M6d.9a1b3b1.
