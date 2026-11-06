# 0198. The screens of reference and the rivals measured

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (the media application written anew on
  uwui, not ported) and [0194](0194-stage-6d-uwui-leaving-the-engine.md) (M6d.0 cut in three; the
  rivals measured with the owner's programs); amended by [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (M6d.1's
  references and M6d.0b's measures widened)
- Date: 2026-11-06

## Context

[ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md) starts Stage 6d with M6d.0, the figures
before: screens of reference chosen before any work and never tuned for, and the four programs of
uwui's pilots measured with their present frameworks. Those programs are the owner's own. Their
figures make a floor: uwui's first stage ends when they run on uwui at least as well
([uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#4-stages)). They do
not make a bar: uwui aims at the best of its field, and the owner's programs are not that, though
Slatty is well optimised.

The best applications of the market measure differently:

- The open ones run on test data, the same as the owner's programs: Strawberry, Elisa and GNOME
  Music read local files; Nautilus and gThumb show a grid of images; Zed opens a file.
- The closed ones, Spotify, Cider, Steam and Heroic, need an account; a test account shows an
  empty library. The owner allowed their own accounts for these measures.
- None can carry a probe in its code: their frames, start and memory must be read from outside.
- Apple Music's own application exists only on Windows and macOS. The owner set it aside for its
  poor performance; Cider, a client of Apple Music, stands for it.

Magnolia, the media application among the four programs
([ADR-0181](0181-uwuis-scope.md)), was written quickly as a prototype: its figures protect
nothing.

## Decision

The owner's decisions of 2026-11-06.

**Four screens of reference,** written and frozen in
[uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#screens-of-reference):

- gpui-kit's "Hello, World!": the "Basic Example" of its README, copied word for word at a
  revision pinned;
- Slatty's library at 10,000 games, on the fake profile of `slatty-measure`;
- Opium's table of songs, 8,500 local songs, a song playing;
- Zed scrolling a file of 100,000 lines.

**Two figures for each.** The owner's programs give the floor, the port's check. The rivals give
the bar to beat, the best of them criterion by criterion:

- the "Hello, World!": GPUI with gpui-kit, Flutter, Compose for the desktop, Electron, Tauri,
  Qt Quick, Slint, Iced and egui, each written as its own documentation shows;
- Slatty's library: Steam and Heroic on the owner's accounts; Nautilus and gThumb on the same
  10,000 covers;
- Opium's table: Spotify and Cider on the owner's accounts; Strawberry, Elisa and GNOME Music on
  the same 8,500 songs;
- Zed's file: Zed is its own reference; uwui has no editor of code before its 1.0.

**Magnolia is not ported.** uwui's first stage writes a media application anew on uwui. It has no
figures before; its bar comes from the market when it is designed.

**M6d.0 is cut in three:**

- M6d.0a: the screens written; [ADR-0180](0180-one-canvas-and-the-frameworks-foundations.md)'s
  figures; Slatty and Opium measured.
- M6d.0b: a method that measures any application from outside, its frames, start, memory, rest
  and power; then the ten "Hello, World!".
- M6d.0c: the applications of the market.

## Alternatives considered

- **The owner's programs alone,** as ADR-0194 planned: less work, but a floor taken for a bar,
  and the market met only at uwui's 1.0.
- **The market alone:** no check of the port, and no reference for Slatty's screen, whose rivals
  run only on accounts with other libraries.
- **Apple Music's own application:** the reference of the player's look, but measured only in a
  virtual machine, and set aside by the owner for its performance.
- **A screen of Magnolia's,** or Magnolia measured without one: a figure that protects nothing.
- **M6d.0 kept whole, or cut in two:** fewer milestones, but one review over far more work.

## Consequences

- The closed rivals' figures depend on the owner's libraries, whose sizes are written beside
  them, and on versions that change: each figure names its version and its date.
- No figure of a closed rival is reproducible from the repository alone.
- The tools to measure from outside are designed at M6d.0b, before any rival is measured.
- The owner's checks by hand stay as ADR-0194 set them: the very modest laptop and its measures,
  the music player's Android prototype in Compose on the Pixel.
