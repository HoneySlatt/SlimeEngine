# 0267. A page of the journal apart from the editor's one handler; a cut journal's start refused; a page's floor of 4 KiB

- Status: Accepted; amends [0156](0156-the-editors-window-and-its-operations.md) (the one
  handler answers every request of the window and of the development channel) and
  [0265](0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md)
  (a journal's file cut apart from the editor; a page's floor)
- Date: 2026-11-14

## Context

[ADR-0156](0156-the-editors-window-and-its-operations.md) made one handler of the editor's core,
`Session::handle`, answer what the window asks and what the development channel asks, the
channel's `Edit` requests converted into the window's own `Request`s. After each request, the
handler publishes the core's state to the window: it copies every step's label, and raises the
window's revision, which asks for a frame.

[ADR-0265](0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md)
reads the journal by pages, on the editor's thread, each page bounded by the share of the frame's
start that the channel gives its request, a clock that `Simulation::edit` receives. It made a file
shorter than what the journal wrote, cut apart from the editor, `STALE`, and a page's floor one
character.

Building M6d.9a1b3b2 and its two closing reviews showed, on 2026-11-14:

- The window's requests wait in a queue of the shared value, which holds no clock: a page cannot
  pass through it with the time that bounds it.
- A page changes nothing: publishing after it copies the labels and asks a frame for nothing, as
  many times as an agent reads pages.
- A cut file stays cut: a tool that lists the journal again on `STALE`, as ADR-0260 asks, gets
  `STALE` again from the journal's start, and loops.
- A page bounded by time through `uwui_agent::page::Pager` takes its floor of visits whatever its
  time. Counting each character as a visit made a page cost some 3.5 ns a byte; counting blocks of
  64 bytes, some 0.35 ns
  ([`measurements.md`](../measurements.md#m6d9a1b3b2-the-editors-journal)), with a floor of 64
  blocks, 4 KiB, some 2 µs.

## Decision

The owner's decisions of 2026-11-14.

**A page of the journal is answered apart from the one handler.** `Simulation::edit` reads it from
the journal that the editor keeps, within its request's page and clock, and publishes nothing to
the window. Every other request, of the window or of the channel, still goes through the one
handler.

**A journal's file cut apart from the editor refuses its start.** A cursor given while the file is
shorter than what the journal wrote is `STALE`, as ADR-0265 decided; the journal's start is
`REFUSED`, its message saying that the file was cut, so that a tool does not loop.

**A page's floor is 4 KiB of text,** 64 visits of 64 bytes each, its first read: it reads no further
piece once its time has passed. It still gives one character at least whatever its bytes.

## Alternatives considered

- **The page through the handler, which would no longer publish after a read:** one path, but it
  changes what the handler does for every read, the history and the operations among them, and
  the clock would have to cross a handler that the window calls without one.
- **`STALE` from the journal's start too, a tool restarting once:** ADR-0260 rejected a rule of one
  restart, which hides bugs.
- **A floor of one character, with a clock of the journal's own:** a second way of bounding a
  page's time, beside `Pager`'s.

## Consequences

- An agent's pages of the journal ask the window for no frame, nor copy the history.
- A read that changes nothing and needs its request's clock may join this exception, by an ADR.
- A journal cut apart from the editor is read by pages again only once the editor makes it anew,
  as it opens a level.
- Eight connections whose pages start a frame's first round read 32 KiB at most whatever their
  time.
