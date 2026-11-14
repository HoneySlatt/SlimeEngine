# 0263. The cursor of elements anchored on its next sibling; what a list may skip; the slots read

- Status: Accepted; amends
  [0260](0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)
  (what the cursor of elements holds and what a list of elements may skip; what
  `World::entities_after` gives)
- Date: 2026-11-14

## Context

[ADR-0260](0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)
said that the cursor of elements holds the next element to visit and its ancestors' ids, that only
the siblings of an element removed between pages may be skipped or listed twice, and that
`World::entities_after` gives the entities alive after an entity's slot.

Building and reviewing M6d.9a1b3b1 showed, on 2026-11-14:

- A cursor that resumed each level by the place that it kept there listed elements twice when
  siblings were added before it, and never ended in a list that grows at its top at each page.
- A cursor that resumed at the place of an element removed skipped as many siblings as were
  removed before it: six boxes removed above the cursor, the page's own among them, skipped the
  six that followed.
- Siblings that change their order between pages, or an element moved to another parent, make any
  cursor that holds no snapshot of the list skip or double some of them. Kubernetes avoids it with
  a snapshot that each page reads.
- A page whose work is bounded by the slots that it reads must read the empty slots too, and know
  where it stopped among them: a cursor of the last entity alive would read a long run of empty
  slots again at each page.

## Decision

The owner's decision of 2026-11-14.

**The cursor of elements holds**, from the root down, each element by its id with the place of its
next child, and the id of its next sibling at the time. Its last element is the last that the page
gave, or the one that the page held back, which the next page gives first.

**A page resumes** each element still under its parent after its place among its siblings now, so
that siblings added or removed before it shift nothing. An element gone, or moved to another
parent, resumes at its next sibling of then, where that sibling stands now; if that sibling went
too, at the place where the element was.

**An element present from the first page to the last is listed once**, whatever is added or
removed beside it, but in three cases, where its siblings may be skipped or listed twice, with all
that they hold:
- siblings that change their order between pages;
- an element moved to another parent;
- the element where a page ended removed with the sibling that followed it.

**`World::entities_after` takes a slot's number,** none for the first, and gives each slot after
it, by its number, with the entity alive in it, if any: the empty slots too, so that a page counts
them and its cursor passes them.

## Alternatives considered

- **The place alone at each level,** as first built: a shorter cursor, but a list listed twice or
  without end when rows are added at its top.
- **No anchor, the place of an element removed:** some 5 bytes a level fewer, but a list skips as
  many rows as were removed above it, as when old rows leave a log's top.
- **A snapshot of the list for each cursor:** exact pages, rejected by ADR-0250 for the memory it
  holds and the lifetime that it needs.
- **`entities_after` given an entity, giving the entities alive:** ADR-0260's words, but a page cut
  by time in a long run of empty slots would read it again at each page, and never move on.

## Consequences

- The cursor of elements grows with the depth of the tree, five numbers a level: past a depth that
  `uwui_agent::page::MARGIN` gives, a page for a tool whose line is short may pass that line
  ([`debt.md`](../debt.md)).
- The family's rustdoc (`uwui_agent::interface::Request::Elements`) states the three cases.
