# 0314. M6d.9a2c4 cut in two; an interface's wait gives the elements seen as `after`

- Status: Accepted; supersedes in part
  [0308](0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md)
  (M6d.9a2c4's part) and
  [0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
  (what an interface's wait sees between two calls); amends
  [0312](0312-waits-on-elements-kept-by-uwui-a-reader-for-waits-reads-what-a-condition-reads.md)
  (the elements seen, and a check while a change is not framed)
- Date: 2026-11-19

## Context

M6d.9a2c4, "The interface that waits" ([`stage-6d.md`](../stage-6d.md#m6d9a2c4a--the-library-that-waits-on-the-interface)),
puts uwui's waits on elements
([ADR-0312](0312-waits-on-elements-kept-by-uwui-a-reader-for-waits-reads-what-a-condition-reads.md))
on the wire as `interface/wait`, gives them an MCP tool, and holds them in the engine. ADR-0310
left the interface's `after` to it; ADR-0312, what `after` holds.
[ADR-0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
says that what changes while an agent does not wait is seen at its next call, from its handle; and
that the server keeps nothing for an agent.

The facts, read in the repository on 2026-11-19, at `e012fb30`:

- Each element has a number, given to no other element of its interface (`MeaningNode::number`).
- `Waits::wait` walks its target and may answer at once; a check is to run after the target's
  frame. slime-app does not say where a presentation frames its interface: the editor frames it
  in `extract`, slime-devui's overlay in its drawing, in `render`.
- A new element is styled at its target's next frame: read before it, an element that its style
  hides reads as drawn.
- `Waits::wait` holds a wait, and its reader, before it returns: a host that counts its waits
  afterwards and answers `BUSY` would leave one held that no request answers.
- `slime-app` holds interface requests only in a run with a window, which its own tests do not
  start; the engine's windowed tests run in `slime-mcp`, on Linux alone.
- The tests of `uwui-mcp` drop an answer that comes while they wait for another one.

A fresh agent attacked the lead's first design before it was built. It found a handle that grew
past the bound that it was refused at, tied to no condition, so that a button's handle met a
dialog's absence; a wait "for 0 seconds" that the tool could not answer; a check deferred while
the first answer was not; a `BUSY` that leaked a wait; tests that passed without their code; and a
milestone larger than one session.

## Decision

The owner's decisions of 2026-11-19.

**M6d.9a2c4 is cut in two**, each part in a session of its own, in this order:

- **M6d.9a2c4a — The library that waits on the interface.** `interface/wait`, `interface` 4 and
  its kept copy; uwui's side, `after` included; `uwui-mcp`'s tool, the bound of a tool that waits
  moved there, and the program in memory of its tests holding a request in flight. Done when an
  agent, through the tool, waits for a list to choose from to close on an interface in memory,
  seen once, on Windows too.
- **M6d.9a2c4b — The engine that waits on the interface.** `slime-app` holding the interface's
  waits; the editor's dialog closed, seen once, and the editor at rest with a wait in flight; the
  measures; the debts left. It closes M6d.9a2.

**An interface's wait gives `after`, the elements that it showed the agent.** A handle written as
a cursor is (`uwui_agent::page::Listing`), bound to its process, its interface and its condition
but its presence, which holds the numbers of the elements already given, with a sum of them:

- Without `after`, a wait reads the interface as it stands: present is met at once when an element
  matches; absent, when none does.
- Present, with `after`: met by an element that matches and is not among those seen. An element
  seen that stops matching, while the wait is held, is seen no longer: matching again meets it.
  An element that was not given, already there, which starts matching, a row selected by the
  click before the wait, meets it.
- Absent, with `after`: met once none matches, after one matched: one seen, one matching as the
  wait starts, or one that matches while it is held.
- The reply gives the elements that met it and were not seen, in the order of the tree, on one
  page, and `after`: the elements seen that still match, and those given. Those past the page are
  told by `more`, and given by the next wait, at once. An absence gives none, and an `after` that
  holds none, which differs from no `after`.
- A handle holds at most 1,000 elements: a wait whose elements would pass it is refused, to be
  narrowed. A handle of another condition, or whose sum does not hold, is refused; one of another
  interface is `STALE`, waited again without `after`.

So "wait for the dialog to close, seen once" is: wait for the dialog's presence, which gives it;
wait for its absence with that `after`, met when it closes, giving an empty `after`; waited again
with it, the absence is held until a dialog opens and closes.

The program keeps nothing between two calls. This changes ADR-0269's "seen at its next call" for
the interface: an element given that stopped and matched again between two calls is not new; one
that matched and went between two calls is missed, by a present wait and by an absent one with an
empty `after`; and an element is one instance, so that one made again, another kind under its key,
a layer opened again, a row of a list built again, is new.

The lead's decisions within ADR-0269, 0308, 0310 and 0312:

- **`interface/wait`** names its target, required, the main one 0; a role, any case, empty for
  any; a text that the name holds; the states that hold and those that do not, by `agents::State`'s
  names, any case; `absent`; `after`; and a page's `limit` and `bytes`. A role or a state that it
  does not know is refused (`BAD_PARAMS`), and so are `GenericContainer`, which never means
  something, a state both held and lacking, and a target that the interface does not have: each
  would match nothing, and its absence would be met at once; a target is the interface's when
  it is the main one, its host gave it a size, or a root stands on it. `Reply::Waited` holds the
  elements, `after` and `more`.
- **uwui answers the family's waits for every host:** `uwui::agents::wait` reads a request into
  a `Waits`, and `agents::met` makes the reply of a wait met. The handle counts in the page's bytes.
- **A wait or a check whose target holds a change not yet framed reads nothing,** and keeps its
  marks: the wait is then held, and the first check after the frame reads its target whole. A host
  may then check after each point where its presentation may frame the interface.
- **The host counts its waits before it asks a `Waits`,** so that a `BUSY` holds nothing.
- **`uwui-mcp` holds the bound of a tool that waits:** `waited` cancels the request through the
  core's cancel once the bound passed, never by dropping it, and `MOST_WAIT`, 50 s, which
  `editor_journal` uses too. The interface's tool, `wait_for_elements`, waits 1 to 50 s, 50 when
  left out: its arguments ask for no answer at once, which a held request cannot give before its
  next frame. A bound passed gives back the `after` that it was given.
- **The program in memory of `uwui-mcp`'s tests frames its targets after each request,** as a
  host frames them, holds the waits, answers those met, and takes a cancel; its tests keep an
  answer that comes while they wait for another.

## Alternatives considered

On `after`:

- **One bit, "one matched":** a handle of a few bytes, but a presence read as it stands: a wait for
  the next message is met at once while the last one shows.
- **The last element made, and the bit:** a short handle, bounded, but an element already there
  that starts matching between two calls, a button enabled again, a row selected, is missed: the
  likeliest change after an action.
- **A history kept by the program for each handle:** nothing missed between calls, but state kept
  for each agent, which ADR-0269 refuses.
- **A bound held by the program, answering "not met" with a handle:** a wait for 0 seconds, and
  what the wait saw kept past its bound, but a program at rest woken at each bound.

On the cut:

- **Whole:** a session less, but the wire, uwui, the library's tool and program in memory, the
  engine's host, its tests and three measures in one session, where a smaller part of M6d.9a2c
  needed two closing reviews.

## Consequences

- Two sessions for M6d.9a2c4; M6d.9a2's "Done when" is met at M6d.9a2c4b.
- `interface` rises to 4. `Waits::wait` takes the elements seen; `Met` gives the elements seen
  that still match beside those that met it.
- An agent that waits for a change after its own action waits for the state that it wants without
  `after`, or from the handle of what it saw before the action.
- A presence whose condition matches more than 1,000 elements is refused, `after` or not, so
  that its handle stays short: an agent narrows it, or lists them with `interface/elements`. An
  absence is held until they go.
- The debt of the host that lets its waits go stays open until M6d.9a2c4b, and so do the cancel
  untested and the journal's wait measured under load.

## Validation and reconsideration

Read: `uwui::agents`, `Waits`, `Ui::holds_change`, the element's number, the family's types and
description, `slime-app`'s channel, `uwui-mcp`'s carrier, tools and tests, and `editor_journal`.
Proposing another decision is justified if agents find the handle too long to pass back, or if
missing what came and went between two calls costs them more than a history would.
