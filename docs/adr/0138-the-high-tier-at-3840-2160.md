# 0138. The high tier at 3840 × 2160

- Status: Accepted; amends [0114](0114-hardware-tiers-and-the-gpus-limits.md) (the high tier's
  resolution) and [0117](0117-frustum-culling-and-instancing.md) (the benchmarks' resolutions)
- Date: 2026-10-23

## Context

[ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md) set three hardware tiers, each with a
reference machine and budgets: handheld, at 1280 × 800; modest, at 1920 × 1080; and high, the class
of an RX 6900 XT, at 2560 × 1440, at 60 frames per second for a realistic game and 144 for a
competitive shooter. It left 3840 × 2160 to an *enthusiast* tier, of the class of an RTX 4090, with
ray tracing, added with Stage 9c. The benchmarks measure at the tiers' resolutions, and
[`measurements.md`](../measurements.md) compares each figure with its tier's budget, scaled to the
RX 6900 XT.

When the interface's canvas set its budget per tier ([ADR-0137](0137-the-canvas.md)), the owner
asked, on 2026-10-23, that the high tier be 3840 × 2160 rather than 2560 × 1440.

## Decision

**The high tier draws at 3840 × 2160, natively, everything included.** Its budgets keep their
frame rates and their times: a realistic game at 60 frames per second, 16.7 ms, and a competitive
shooter at 144, 6.9 ms, at 3840 × 2160, the GPU taking about 85 % of a frame, as ADR-0114 sets. Its
reference machine is unchanged, the class of an RX 6900 XT, an RTX 4070 Ti or an RX 7800 XT, and so
is its ratio to the RX 6900 XT, one. A game's scene, its post-processing and its interface are drawn
at the screen's resolution, with no upscaling.

The benchmarks measure at 1280 × 800, 1920 × 1080 and 3840 × 2160. The figures before this decision,
at 2560 × 1440, stay in `measurements.md` as they were measured.

The enthusiast tier stays at Stage 9c, at 3840 × 2160 too: ray tracing, and a GPU of about twice the
high tier's, set it apart.

## Alternatives considered

- **A screen of 3840 × 2160, its scene drawn at 2560 × 1440**, then enlarged to the screen, as games
  of the high end do, the interface drawn at the screen's resolution: the renderer's budget
  unchanged, a view's render scale already drawing below its output's size, and upscalers that
  rebuild the detail at Stage 9c; but a scene less sharp than the screen until then.
- **The enthusiast tier now**, a fourth tier at 3840 × 2160, the high tier staying at 2560 × 1440:
  the whole scale at once, which ADR-0114 rejected, the tier having no technique of its own before
  ray tracing.
- **2560 × 1440, as ADR-0114 set it**: a budget easier to hold on the reference machine, but a
  resolution below the screens that the high end plays on.

## Consequences

- At the high tier, each frame draws 2.25 times the pixels of 2560 × 1440 in the same time: what
  costs per pixel, the scene's shading, its post-processing and the interface, has 2.25 times less
  time per pixel on the reference machine. A realistic game at 60 frames per second at 3840 × 2160
  on the class of an RX 6900 XT, without upscaling, is an ambitious target, which techniques of
  Stage 9a are measured against, and which a game may meet by lowering its quality at that tier.
- The interface's canvas has 500 µs on the RX 6900 XT at 3840 × 2160 (ADR-0137).
- `slime-benches` measures the high tier at 3840 × 2160; `benches/README.md` and the figures from
  now on say so.
