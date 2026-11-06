# 0194. Stage 6d: uwui leaving the engine

- Status: Accepted; amends [0170](0170-applications-on-the-ui-framework.md) (Stage 6d's exit and
  its release: no release of its own, later releases moved back by one),
  [0171](0171-effects-of-the-interface.md) (the effects' foundations built in uwui's first stage),
  [0178](0178-two-pilots-and-the-frameworks-name.md) (the pilots ported in uwui's first stage),
  [0181](0181-uwuis-scope.md) (the order of Stage 6d's milestones; the trials and the ports after
  the move, in uwui's repository) and [0190](0190-uwuis-requirements.md) (what Stage 6d builds;
  its decisions taken in milestone M6d.1); amended by
  [0198](0198-screens-of-reference-and-the-rivals-measured.md) (M6d.0 cut in three; the rivals
  measured), [0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) (M6d.1 cut in
  four) and [0203](0203-pilots-replaced-pluto-and-a-daw.md) (the programs of uwui's first stage)
- Date: 2026-11-06

## Context

Stage 6d was planned on 2026-11-06, once Stage 6c paused
([ADR-0190](0190-uwuis-requirements.md)). [ADR-0181](0181-uwuis-scope.md) had cut its first
milestones from the code of 2026-11-05: uwui's documents read with `serde`, the canvas moved into
`uwui-render`, the host's API and a desktop host, the move to a repository of its own, then touch,
fonts and the two trials. The stage then held the drawing, the motion, the components and the four
programs ported.

Three facts weighed on the plan:

- After the move, nearly all of the stage's work lands in uwui's repository. Each project keeps
  its memory in its own repository: a plan of that work kept here would be read by sessions
  working elsewhere.
- The release map gave Stage 6d 0.8 and Stage 6c 0.7, though Stage 6c now ends after uwui's 1.0
  and [ADR-0190](0190-uwuis-requirements.md) keeps 0.6 as the engine's last release meanwhile.
- [ADR-0190](0190-uwuis-requirements.md) left to the stage's start how an application's state
  changes its interface, the format of layouts and Rust's code reloaded. The host's API depends on
  them: the interface's executor and its waker, a reload that the host survives, the time of each
  frame that motion reads, and an application started in one call.

A reviewing agent read the plan against the code and found, among other things:

- The canvas reads each item by its index in the fragment shader. Handing the item on as values of
  the vertices, as ADR-0181's single path of instances would, cost 101.2 µs against 76.3 µs at
  1280 × 800 ([`measurements.md`](../measurements.md)). WebGL2's limits in wgpu allow no storage
  buffer and 15 values between the shader's stages; an item holds 9 vectors today.
- uwui's styles cascade as reflected values (`theme.rs`, `tree.rs`): leaving `slime-reflect` asks
  for typed styles first.
- `slime-color` derives `slime_reflect::Reflect`, and the engine's GPU layer, renderer and world
  depend on it. Moved into uwui, Rust's orphan rule would put that implementation in
  `slime-reflect`, making the engine's lowest crate depend on uwui.

## Decision

The owner's decisions of 2026-11-06.

**Stage 6d is uwui leaving the engine.** Its milestones, detailed in [`PLAN.md`](../PLAN.md):

- M6d.0, the figures before: the screens of reference chosen and frozen, the four programs
  measured with their present frameworks, the owner's very modest laptop named and measured, and
  [ADR-0180](0180-one-canvas-and-the-frameworks-foundations.md)'s figures measured first.
- M6d.1, how an application is written: a design milestone, which keeps no code.
- M6d.2, typed styles; M6d.3, documents with `serde`; M6d.4, the canvas in uwui on wgpu; M6d.5,
  the host's API.
- M6d.6, a series of headless compositors ([ADR-0195](0195-a-series-of-headless-compositors.md)).
- M6d.7 and M6d.8, the desktop host; M6d.9, agents reach an application.
- M6d.10, uwui designed as a project; M6d.11, the move; M6d.12, the stage's audit.

**How an application is written is decided first,** at M6d.1, before the decoupling: the host's
API is written once, knowing the executor, the reload and the time of a frame. Its prototypes are
thrown away, their figures kept; its ADRs move with uwui.

**The rest is uwui's first stage,** in its repository: touch and the platforms' behaviours, fonts
given as bytes, the Android and web trials, the authoring API built, drawing, images, motion, text
and input, the library of components, and the four programs ported, with Stage 6d's former exit as
its exit. uwui's later stages follow, to its 1.0. Their order is designed at M6d.10.

**No release of its own.** Stage 6d is released with 0.7, as Stages 3d and 4b were with the next
release. Later releases move back by one: Stage 7 becomes 0.8, Stage 11 0.14, then 1.0.

**Settled at a milestone's start:** the cooked form of uwui's documents and `slime-color`'s home
(M6d.3); the canvas's layout, by a prototype counted against WebGL2's limits, and wgpu's `gles`
feature (M6d.4); how the engine follows uwui after the move, and which repository holds the
interface's budgets (M6d.10).

**After the move,** the editor and the game no longer test each change of uwui. What stands for
them in uwui, the interface's benchmarks and their counters, a menu driven by a gamepad, is
decided at M6d.10.

## Alternatives considered

- **Stage 6d kept as written,** its milestones after the move followed in this repository's plan:
  the roadmap unchanged, but uwui's memory outside uwui's repository, and an engine's release that
  depends on another repository's work alone.
- **uwui kept here to the stage's end:** a change of both sides one commit, the editor testing
  uwui at each push; but ADR-0181's order undone, the editor ported at each change of the API while
  it changes most, and uwui, the owner's main project, without its repository, its method or its
  CI on macOS for months.
- **The decoupling first,** as ADR-0181 ordered it: uwui's repository sooner, but the host's API
  written before the executor, the reload and the time of a frame are known, then written again.

## Consequences

- The roadmap's Stage 6d is reshaped, and its former content moves to uwui's roadmap.
- The release map moves back by one from Stage 7.
- The editor and *Hunna Wood* keep their checks and budgets at each push of this repository, on
  the revision of uwui that the engine takes.
- M6c.Q2f still waits for the rule of the frames' rate, which M6d.7's ADR decides.
