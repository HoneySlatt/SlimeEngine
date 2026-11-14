# 0259. The interface's family in `uwui-agent`, answered by `uwui`; an element's own style by a request of its own; M6d.9a1b3 cut in two

- Status: Accepted; amends
  [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (M6d.9a1b3 cut in two; why the families' versions rise) and
  [0143](0143-what-agents-see-of-the-interface.md) (each element named with its target; an
  element's own style read by agents)
- Date: 2026-11-14

## Context

[ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
gave M6d.9a1b3 two parts of work:
- the cursor of a long list;
- the interface's family moved into `uwui` behind its feature `agents`
  ([ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)), with the
  places of the code that wrote an element's own style
  ([ADR-0231](0231-an-elements-own-style-written-in-code.md)).

It left the owner two questions: where the family's messages live, and how the places are given.

The facts, read in the repository on 2026-11-14:

- The family's four requests are variants of `slime-channel`'s `Request`: `interface/elements`,
  `interface/element_at`, `interface/act` and `interface/capture_element`. `slime-app` answers
  them from `Ui`, and crops a capture from its frame's image. `ElementSummary` and
  `InterfaceAction` are `slime-channel`'s types.
- `OwnStyle::places` gives the place of each call that wrote an element's own style; nothing reads
  it. Each declaration keeps its field, written as a style sheet writes it
  (`layout.padding.start`), its state, the token that it names, if any, and its place.
- A place is the compiler's: relative to the workspace for its members, absolute for other crates,
  so it may hold a person's home folder.
- `slime-channel` sends every refusal as `REFUSED`, and its tool reads every fault as a refusal:
  a family's own codes do not cross the channel.
- The shipping step of `scripts/check.sh` pipes `cargo tree` into `grep`: it passes when `cargo`
  fails, and when the binary that it searches is missing. Nothing shows that its detectors find
  anything.

A fresh agent attacked the lead's design before it went to the owner. It found:
- the milestone too large for one session;
- the codes of faults lost at both ends;
- an element's target missing from what agents read, against ADR-0256;
- the elements' cursor resumed by a path that some keys cannot read back;
- cursors that a restarted program would read as its own;
- an item larger than the tool's line that would stop a list for good;
- the shipping check's faults above.

## Decision

The owner's decisions of 2026-11-14.

**M6d.9a1b3 is cut in two,** each part in a session of its own:

- M6d.9a1b3a, the interface's family moved:
  - its messages in `uwui-agent`;
  - its answers in `uwui` behind `agents`;
  - `interface/style`;
  - the build for shipping checked free of the feature.
- M6d.9a1b3b, long lists:
  - the cursor of entities, elements and the editor's journal, and the log's pages bounded in bytes;
  - a family's codes of faults carried across the channel;
  - the MCP server's tools paged.

  Its questions are put to the owner at its start, the attack's findings on the cursor among them.

The family moves first: the elements' cursor is then written once, in `uwui`.

**The family's messages live in `uwui-agent`,** in a module `interface`:
- the family's name, version and events;
- its requests and replies;
- `ElementSummary` and `InterfaceAction`.

They are plain `serde` types, so that a tool speaks the family without building `uwui`. The module
stands apart from the core, so that M6d.10 can make it a crate of its own without changing the wire.

`uwui` answers them in a module `agents`, compiled by its feature `agents`. With the feature,
`uwui` depends on `uwui-agent`, the owner's leave of 2026-11-14; without it, on neither `uwui-agent`
nor `serde`. The module serves nothing by itself: a host gives it each request of the family, with
the `Ui` that answers it.

`slime-channel` carries the family as it carries `editing`, and re-exports its types, so that
`slime-mcp` gains no dependency. `slime-app`'s feature `development` turns on `uwui/agents`.

**An element's own style is read by a request of its own,** `interface/style`, given an element's
path. Its answer groups the declarations by place, in the order of the calls. Each place has its
file, line and column. Each declaration has:
- its field, as a style sheet writes it: `layout.padding.start`;
- its state, none for the base;
- the token that it names, if any, so that an agent changes the theme rather than the code.

It gives no values: a second writer of declarations would have no other reader. Lists of elements
keep their size.

**A place is given as the compiler gave it,** as ADR-0231 keeps it:
- a member of the workspace by its path from the workspace's root;
- another crate by its absolute path, which may hold a home folder.

The agent reads files on the machine that runs the program, and learns nothing that the program
could not show it. M6d.9a3b's rights by family can refuse `interface/style` to an agent. The tests
of the repository write relative places alone.

**The lead's decisions within them:**

- **Each element names its target.** `ElementSummary` gives the target of its root
  ([ADR-0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md):
  every reader names the target that it reads).
- `interface/element_at` reads the target that it names, or the active target when it names none.
- `interface/capture_element` is answered by `uwui` with the element's target and the pixels that
  it covers. The host crops its image of that target
  ([ADR-0249](0249-the-hosts-api-its-items-and-their-table.md)).
- **`interface` rises to version 2** at M6d.9a1b3a, its messages changed, and to 3 with the
  cursor. `world` and `editing` rise to 2 with the cursor.
  - ADR-0250 gave the wrong reason for raising them. A program of before reads no `cursor` and
    gives no `next`, as no family refuses an unknown field. A tool of after would therefore read
    one page and take it for the whole list: a silent cut, not an endless first page.
  - ADR-0254 decided the raise.
- **Refusals stay `REFUSED` until M6d.9a1b3b,** which carries every code both ways.
- **The shipping step of `scripts/check.sh`, which the owner allowed this milestone to change:**
  - it reads each tree into a variable first, so that a `cargo` that fails fails the step;
  - it also refuses `uwui-agent` among the game's crates, `uwui`'s feature `agents`, and the
    family's methods in the binary;
  - it reads every target's dependencies (`--target all`);
  - it fails when the binary that it searches is missing.
- **The check is shown to fail at every run.** Each detector also runs on the game built with its
  default features, built already by an earlier step. There it must find what it looks for, or
  the check fails: a feature renamed, an output of `cargo tree` changed or a string moved cannot
  leave a check that always passes. Once, by hand, a manifest that gives the shipping build
  `uwui/agents` makes the check fail; the milestone's result records it.

## Alternatives considered

The cut:

- **In two, the cursor first:** the journal's debt paid sooner, but the elements' cursor written in
  `slime-app`, then again in `uwui`.
- **One milestone,** as the card had it: one review and one raise of versions, but the longest
  session of the stage, whose last two parts each needed two closing reviews.

Where the messages live:

- **In `uwui` behind `agents`, messages and answers together:** one home, but every tool builds
  `uwui`'s text and layout to speak the family, which ADR-0246 made `uwui-agent` apart to avoid.
- **A crate of their own:** clean now, but one more crate for five requests, its name M6d.10's.

The places:

- **In each element's summary:** no request more, but every page of elements larger, and an
  agent's context with it.
- **`interface/style` with values as text:** more complete, but a second writer of declarations,
  kept in step with the style sheets' reader, for no other reader.
- **Absolute places hidden,** a place outside the workspace named as such: more discreet if an
  agent is ever remote, but an agent could not open a style written in a dependency.

The targets:

- **The host answering on the target that it shows,** no target in the messages: shorter, but a
  default that ADR-0256 rejects, and a host of two windows could not tell which image to crop.

## Consequences

- `slime-channel` loses `ElementSummary`, `InterfaceAction` and its four requests of the family,
  which `uwui-agent` holds; a tool of before cannot speak the family to a program of after.
- `uwui-agent` holds a family beside the core. uwui's later families go there too until M6d.10
  names their crates.
- A new edge in the crates' graph: `uwui` to `uwui-agent`, behind `agents`
  ([`ARCHITECTURE.md`](../ARCHITECTURE.md), [`dependencies.md`](../dependencies.md)).
- The interface's tests of `slime-app`'s channel move into `uwui`'s module. A small test of the
  family's carrying stays in `channel.rs`. M6d.5c3b changes the frames of both.
- `OwnStyle::places` has its first reader.
- The game built for shipping is checked free of `agents`, and the check of its development
  channel no longer passes when `cargo` fails.
