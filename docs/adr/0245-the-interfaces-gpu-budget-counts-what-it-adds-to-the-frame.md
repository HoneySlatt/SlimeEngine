# 0245. The interface's GPU budget counts what it adds to the frame

- Status: Accepted; amends [0137](0137-the-canvas.md) (what the interface's budget counts, and how
  it scales to the tiers), [0114](0114-hardware-tiers-and-the-gpus-limits.md) (the tiers' ratio
  scales the interface's passes' work, not the boundaries between them) and
  [0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md) (its glass's
  figures counted the game's view under the menu)
- Date: 2026-11-12

## Context

[ADR-0137](0137-the-canvas.md) bounds the interface at 0.5 ms of GPU on each tier's machine: on the
RX 6900 XT, 500 µs at 3840 × 2160, 250 µs at 1920 × 1080 and 50 µs at 1280 × 800, scaled by
[ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)'s ratios. `render.canvas` measures it as the
canvas's pass on the GPU, after its items are written. The closing review of M6d.4a asked whether
the budget counts the passes alone or the frame's span, its copies and the gaps between its passes
included.

The prototype of [ADR-0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md)
measured both on the RX 6900 XT, and its figures, read again on 2026-11-12, give:

- Each boundary between two passes leaves the GPU waiting some 3.7 µs, 3.5 to 4.2 µs, at every
  resolution and number of items: wgpu writes its timestamps once earlier work is done, and RADV
  waits for every pixel's work at a barrier after drawing; wgpu has one queue, which nothing else
  fills meanwhile. The wait is a cost, not an artefact of the measure.
- Four glass panels take some 20 passes: about 70 µs of boundaries at every size, more than the
  handheld tier's 50 µs, which the ratio derives from throughput, a tenth of the RX 6900 XT's.
- ADR-0243's figures for the glass, 546 to 586 µs from the first copy to the last pass at
  3840 × 2160, counted the pass that draws what lies behind the menu, the game's view and the
  menu's dimming box, 146 to 166 µs. Without it, the glass takes 382 to 420 µs.
- Engines and platforms time a whole frame or command buffer, Unity's `gpuFrameTime`, Metal's
  command buffers, Android's `GPU_DURATION`; none times one part of a frame from its uploads. On GL
  ES and WebGL2, wgpu gives no timestamps.

## Decision

**The budget counts what the interface adds to the frame**: the GPU time of a frame with the
interface, less the same frame without it, over a view that the renderer draws, the two runs
interleaved frame by frame, the GPU's clocks held. Where the device gives no timestamps, as on GL
ES, the same difference is taken on the wall time of a loop that the GPU bounds.

**Each milestone that touches the canvas also gives the interface's own span**, from the start of
its first pass to the end of its last, with its passes' summed time and each step's beside it, so
that the time's place shows.

**The tiers' ratios scale the passes' work alone.** A tier's budget is its ratio times the
RX 6900 XT's figure for the passes' work, plus the boundaries counted at their cost on that tier's
own machine: the number of passes a frame takes is an exact counter in CI, and the cost of a
boundary is measured on each tier's reference machine, the owner's Steam Deck for the handheld
tier.

## Alternatives considered

- **The interface's own span, scaled by the ratio as today**: one figure, simple to take; but the
  boundaries' fixed wait divided as if it shrank with the GPU's throughput, so that any effect of
  several passes looks over budget on the handheld tier.
- **The passes alone**, as `render.canvas` measured them: the history comparable; but some 3.7 µs
  for each boundary, which the game loses, left unseen, and an effect of many passes made to look
  cheaper than it is.
- **The span from the frame's first copy of the items**: once the interface sits in the renderer's
  frame, its uploads run before the frame's commands, where no timestamp of the frame reaches them,
  and a span from the first copy would take in the 3D views.

## Consequences

- The quality bar's line for the interface's GPU time names what it counts; its figures so far,
  the passes alone, stay met until the next milestone that measures the interface on the
  RX 6900 XT measures them this way.
- `render.canvas` and `ui.draw.menu` gain the frame with and without the interface, over a view,
  and the interface's own span; the number of passes joins the counters checked in CI. M6d.4b2
  adds them; M6d.4b1 keeps today's measure, so that its figures before and after the move
  compare.
- The cost of a boundary on the Steam Deck is measured once a benchmark runs there; until then,
  the handheld tier's budget for an interface of several passes is not judged.
- The game's frame budget times its passes too, "without the gaps between its submissions"
  (`benches/src/render.rs`): one definition for both, the frame's whole span, is weighed when the
  game's budgets are next measured; until then, the interface's is the only one of this kind.
- ADR-0243's glass keeps 0.5 ms at 3840 × 2160 on what the interface adds, some 380 to 420 µs; at
  1280 × 800 its boundaries alone pass 50 µs, judged once a boundary's cost on the Deck is known.
