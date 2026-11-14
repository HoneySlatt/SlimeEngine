# 0265. The editor's journal read by pages: its number drawn by the journal, pages of whole entries in bytes, read on the editor's thread, bytes not UTF-8 replaced and marked

- Status: Accepted; amends
  [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (the journal's place: a byte, not an entry) and
  [0260](0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)
  (the journal's number, its bounds and its floor)
- Date: 2026-11-14

## Context

[ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
placed a page of the journal "by each entry's place in its file with a generation".
[ADR-0260](0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)
bound every cursor to its process and its maker, gave each list `limit` and `bytes`, bounded a
page's work by a share of the frame's start past a floor of visits, and left the journal to
M6d.9a1b3b2, whose card kept its generation in the editor's session.

The facts, read in the repository on 2026-11-14:

- `editing/journal` takes no parameter. Its answer holds the whole file, read with
  `fs::read_to_string` on the editor's thread, and fails past the tool's line.
- The journal's first entry, `# level`, holds the level's whole text on one line: an entry can be
  larger than any page.
- The `Editor` is replaced at each level opened, and `Session::open` takes `&self`. Only
  `Journal::create` and `Journal::resume` write a journal's file anew, both at a level's opening.
- `Journal::add` writes an entry with one `write_all`, which the system may split.
- `Simulation::edit` receives neither the request's page nor its time. An error that it returns
  reaches the tool as `REFUSED`, whatever its cause.
- `Session::handle` publishes the editor's state to its window after every request, reads
  included: it copies every step's label and asks the window for a frame.
- `page::cut` measures a whole text's JSON before it walks it; the answer is measured again as it
  is sent. For a page of 1 MiB, both run on the editor's thread.
- A tool's line may be shorter than `page::MARGIN`, which gives a page of 0 bytes, and a request
  may ask `bytes: 0`.
- `<[u8]>::utf8_chunks` reports a character that a read split at its end as an invalid run, as it
  reports a byte that is not UTF-8 (`core::str::lossy`, Rust 1.99). On Windows, `std` opens a
  file sharing its reading, writing and deletion, so that a second handle reads the journal that
  the editor writes (`std::sys::fs::windows`).

A fresh agent attacked the lead's first design before it went to the owner. It moved the number
from the session into the journal, bounded pages by the bytes that the journal wrote, made the
floor one character, kept ADR-0260's end of a list, and found the page of 0 bytes, the character
split at a read's end, and the costs of measuring twice and publishing.

## Decision

The owner's decisions of 2026-11-14.

**The journal draws its own number.** `Journal::create` and `Journal::resume` draw it from a
counter of the process, as a `World` and a `Ui` draw theirs. The cursor holds it as its maker's
number, beside the process's number that the carrier gives, and one place: the byte of the file
where the next page starts. A level opened, or the same level opened again after another, makes
a new journal and its cursors stale. Opening the level that is open, or failing to open another,
keeps the journal and its cursors.

**The journal counts the bytes that it wrote in whole entries,** raised once each `add` succeeded.
A page reads no further, so that it never shows a torn entry nor bytes added from outside.

**At the journal's end, no `next`,** as for every list (ADR-0260). An agent that follows the
editor's steps waits for an event of them, with its first reader.

**A page is bounded by `bytes` alone,** in bytes of JSON, its quotes included, within the
program's page: no `limit`, as the journal has no items. A page always gives one character at
least, or one run of bytes that are not UTF-8, whatever its bytes and its time, so that a list
always moves on.

**A page is read on the editor's thread,** by seek, never the whole file, in one pass that counts
each character's JSON and stops on its bytes or once its share of the frame's start has passed.
`Simulation::edit` receives the request's page and its clock. Reading off that thread would need a
`Simulation::edit` that answers later: written as debt.

**Bytes that are not UTF-8 are read lossily and marked.** Each invalid run becomes one U+FFFD, and
the page says `lossy`.

**The lead's decisions within them:**

- The cursor is ADR-0260's, `Listing { list: "journal", filter: "" }`.
- A page ends just after its last line's end, `\n`, when it holds one and does not reach the
  journal's end; else at a character's boundary. The next page's byte is counted from the runs
  that `utf8_chunks` gives, never by looking for U+FFFD in the text, which the journal may hold.
- An invalid run at a read's end, before the journal's end, is read again with the next bytes: it
  may be a character that the read split. A read takes 4 bytes at least, enough for any
  character.
- A cursor that does not read, of another list, or whose byte falls inside a character or past the
  bytes written, faults `BAD_PARAMS`: it was not given by this journal. A file shorter than the
  bytes written was cut from outside: `STALE`, its message naming the journal. A file replaced from
  outside by one as long or longer is read as the journal: the family's rustdoc says so.
- A page is answered without publishing the editor's state to its window, which it does not
  change. Its faults keep their codes: `Simulation::edit` returns them as `Reply::Failed`.
- `editing` rises to 2: `editing/journal` takes `cursor` and `bytes`, both optional, in `params`,
  which a request gives, `{}` for the start; its answer gives `text`, `next` while the journal goes
  on, and `lossy` when it replaced bytes.
- `editor_journal` takes `cursor` and `bytes`, gathers pages up to the smaller of the agent's
  `bytes` and the MCP server's page, writes `next` first, and never joins a whole journal.
- `uwui_agent::page` gives the JSON size of one character, which `cut` reads too: one count of
  JSON's escapes.

## Alternatives considered

The journal's number:

- **A counter in the editor's session,** raised at each level opened, as the milestone's card had
  it: simple, but each place that writes a journal anew must remember to raise it, and one forgotten
  sews two journals into one list with no fault.
- **The file's identity,** its size, time or inode: not the same on Windows and the web, and read
  at each page.

The journal's end:

- **`next` always, with `end`:** an agent resumes at the end later, but every reader of pages
  learns an exception, and must still read the whole journal once to reach its end.
- **The end's cursor given by `editing/history`:** an agent skips the level's entry, but no reader
  needs it yet.

A page's bounds:

- **`bytes` and `limit` in lines:** one line may be gigabytes, so `bytes` stays needed; two bounds
  to test for little use.

The thread:

- **Off the editor's thread,** answered later: frames untouched on a slow disk, but a wider change
  of `slime-app`'s `Simulation`, which another milestone changes at the same time.
- **A floor of one piece of 64 KiB:** eight connections in the frame's first round would read
  eight pieces whatever their time.

Bytes that are not UTF-8:

- **Refused:** the list stops for good at the first such byte.
- **Escaped as `\xNN`:** no longer the file's text, and a journal that holds that text reads the
  same.

## Consequences

- `editing` 2: a tool of before cannot read a program of after, nor the reverse; their other
  families are served.
- `Simulation::edit` takes the request's page and its clock: every simulation that edits changes.
- The editor's thread reads at most a page and three bytes for each page, within its share of the
  frame's start past one character; on a slow or remote disk, one page may still take a frame:
  written as debt.
- A journal of any size reads by pages; opening a level still reads its journal whole, written
  with the journal's growth across recoveries ([`debt.md`](../debt.md)).
- An agent that follows the editor's steps reads the journal again from its start, until an event
  of the editor's steps.
