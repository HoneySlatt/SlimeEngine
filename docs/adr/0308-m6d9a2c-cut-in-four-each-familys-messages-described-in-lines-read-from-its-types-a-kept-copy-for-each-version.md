# 0308. M6d.9a2c cut in four; each family's messages described in lines read from its types, a kept copy for each version

- Status: Accepted; supersedes in part
  [0295](0295-each-familys-messages-described-and-held-to-its-version-from-m6d9a2c.md) (the
  rule on a family's description, made precise)
- Date: 2026-11-18

## Context

M6d.9a2c, "Waiting" ([`stage-6d.md`](../stage-6d.md#m6d9a2c--waiting)), is the last part of
M6d.9a2 ([ADR-0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)).
Its card holds four pieces of work: each family's messages described and held to its version
([ADR-0295](0295-each-familys-messages-described-and-held-to-its-version-from-m6d9a2c.md));
the journal's `wait` and `end`; `interface/wait`; their tools, and the measures.

The facts, read in the repository on 2026-11-18, at `b44f5f31`:

- `uwui::agents::answer` answers a request at once, from the tree as it stands. A wait needs
  state kept across frames, and a reader of meaning lists no element removed: each wait keeps the
  elements that it matched.
- `slime-app`'s channel runs a frame while a request waits for one
  (`DevChannel::waiting`): a wait held the same way would keep frames running for up to 60 s.
- A connection holds 16 requests in flight; ADR-0269 notes that an agent waiting on 16 holds them
  all.
- The in-memory carrier of `uwui-mcp`'s tests answers each request at once, so none is in flight.
- No crate of the workspace names `schemars` in its manifest; it is in `Cargo.lock` through
  `rmcp`, which re-exports it.
- Every family's messages use internally or adjacently tagged enums of `serde`.
- `slime_channel::Reply` is shared by `play`, `world` and `editing`. The core's `cancel` and
  `subscribe` are read by hand from JSON, and its envelope is two private types.

A fresh agent attacked the lead's first proposal, a cut in three and a description as JSON Schema
with one file for each version. It found the third part larger than a session; held waits that
would run frames; a JSON Schema that fails at every reworded comment; a file for each version that
makes every addition a break; a feature that `cargo test` never turns on; and Windows' line
endings.

## Decision

The owner's decisions of 2026-11-18.

**M6d.9a2c is cut in four,** each part in a session of its own, in this order:

- **M6d.9a2c1 — Each family described.** The description of each family and of the core's own
  messages, and the test that holds each to its version. No version raised, no behaviour changed.
- **M6d.9a2c2 — The journal that waits.** `editing/journal`'s `wait` and `end`, `editing` 3;
  the request held in `slime-app`'s channel, as a request in flight; `slime-mcp`'s tool. It
  settles the debt of an agent that reads the journal again.
- **M6d.9a2c3 — Waits in uwui.** The waits in `uwui`: their state across frames and their
  counters, no change on the wire.
- **M6d.9a2c4 — The interface that waits.** `interface/wait`, `interface` 4, `slime-app`,
  `uwui-mcp`'s tool and its in-memory carrier, and the timing; it closes M6d.9a2.

Common to the waits of M6d.9a2c2 and M6d.9a2c4: a program at rest with a wait in flight runs no
frame and wakes no thread; a connection holds fewer waits than its places in flight, so that its
reads still pass; tests never wait the tools' bound of under 60 s.

**Each family's messages are described in lines read from its types.** The test derives each
message's JSON Schema with `schemars`, drops its prose, and writes one sorted line for each field
of each request, reply and event: its method or name, its path, its type, and whether it is
optional, as `request interface/elements params.cursor string optional`. The test fails on a
keyword of JSON Schema that it does not know. A family's file holds its lines; a reply shared by
several families goes whole into each one's file.

**A file shows the messages as they are, and a kept copy what each version held.** Each family
has:

- **its description,** the lines of its messages now; the test fails when the types give other
  lines, and a command writes it again;
- **a kept copy for each version,** written when the version is raised and never edited after.

The test fails, unless the family's version is raised, when a line of the current version's copy
is missing from the description, removed or changed, or when a request gains a field: a program of
before would ignore it silently. A field added to a reply or an event passes, as does a comment
reworded. Until majors and minors exist, raising the version is raising the major
([ADR-0278](0278-families-versioned-by-major-and-minor-tools-speaking-several-majors.md)).
`scripts/check.sh` refuses a change to a kept copy against the commit that it starts from.

**The core's own messages are described too:** the hello, the welcome and the refusal from their
types; `cancel` and `subscribe` given types of their own, read as before; the envelope written in
the test by hand, which the description says.

**`schemars` is a development dependency** of `uwui-agent` and `slime-channel`, at the version
that `rmcp` uses; the messages derive it under `cfg_attr(test, …)`. No program's build holds it.
The description's files have Unix line endings on every platform.

The rest of ADR-0295 and of ADR-0269 stands.

## Alternatives considered

On the cut:

- **Three parts, the interface's wait whole:** one session less, but uwui's waits, the wire,
  the host and the tool in one session, where an incremental walk of the tree took two closing
  reviews before (M6d.9a1b3b1).
- **Two parts, the description with the journal, then the interface:** each part over a session.
- **Whole:** four sessions' work for one closing review.

On the description:

- **JSON Schema as `schemars` writes it:** no walk of the schema, but some 660 lines for the
  interface alone, prose included: a reworded comment fails the test, and an update of `rmcp`
  may write every file again.
- **One file for each version, compared whole:** simple, but every addition a break, which
  ADR-0295 rejected.
- **A file written by hand, checked against example messages:** no dependency, but a field that
  writes nothing when empty escapes it.
- **`serde-reflection`:** it refuses tagged enums, which every family uses.
- **A tracer of our own over `serde`'s deserializer, or a derive macro:** no dependency, but
  clever code to keep, or a crate of procedural macros.
- **A feature `described` on both crates:** `cargo test --workspace` would never turn it on.

## Consequences

- Four sessions for M6d.9a2c; M6d.9a2's "Done when" is met at M6d.9a2c4.
- A change of a family's messages edits its description in the same change, and raises its
  version, with a new kept copy, where the test asks.
- A field added to a request raises the family's version, until minors exist.
- `schemars` moves with `rmcp`: an update of either one may change the lines and need the
  description written again, with no change to the messages.
- The milestone of `uwui dev`'s carrier adds the minor to the files and the test.

## Validation and reconsideration

Read: `uwui::agents::answer`, `DevChannel::waiting`, the in-memory carrier, the families' types and
the core's reading of `cancel` and `subscribe`; a probe of `schemars` on the interface's types,
which kept each method and each optional field. Assumed: that the lines of a schema stay readable
for every family. Proposing another decision is justified if keeping the kept copies costs more
than the changes they catch, or if `schemars` cannot describe a family's types.
