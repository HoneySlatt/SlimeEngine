# uwui-time

uwui's times: the host's clock, when each event came and each frame shows, and each root's clock
([ADR-0227](../../docs/adr/0227-the-time-that-motion-reads.md)). `uwui` gives them again as
`uwui::time`, the path that programs use. They stand in a crate of their own, below `uwui`, so that
the platform layer of uwui's desktop host, which knows nothing of uwui's interface, gives its
events' times in them
([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)).
Its name is provisional, until M6d.10a names uwui's crates; it leaves with uwui at M6d.11, under
MIT OR Apache-2.0.

## Role

- `Instant`, a time of the host's clock, and `RootTime`, of a root's clock, in whole nanoseconds,
  which cannot be mixed; a gap between two is a `Duration`.
- `EventTime` and its `EventSource`: when an input event came, and whether the platform stamped it,
  the host received it, or a test gave it.
- `FrameShown` and its `FrameSource`: when a target's frame shows, and where that time comes from.
- `RootClock`: a root's clock at a frame, its time and its speed.

## Entry points

The crate's rustdoc; programs read it through `uwui::time`.

## Depends on

Nothing but the standard library.

## Must not

- Depend on another crate: `uwui`, the desktop host's platform layer and every host read it.
