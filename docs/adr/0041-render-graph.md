# 0041. The render graph

- Status: Accepted; amends [0027](0027-the-render-crate.md),
  [0033](0033-gpu-layer-for-multi-pass-rendering.md),
  [0034](0034-hdr-rendering-and-tone-mapping.md), [0038](0038-shadow-mapping.md),
  [0039](0039-image-based-lighting-computed-on-the-cpu.md) and [0040](0040-bloom.md)
- Date: 2026-10-14

## Context

A frame of the renderer has 3 to 23 passes (17 at 1920 × 1080 with shadows and the bloom): the
shadow pass, when a light casts shadows; the scene; the sky; the bloom's 2N − 1 passes, when its
strength is above 0 and the output has a chain; tone mapping. Each was recorded by hand, in an
order written once in `Renderer::render`, into its own command encoder, submission and timer
([ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md), which left submission to this
milestone). The bloom's passes cost 0.51 to 0.63 ms of CPU per frame on the owner's machine,
mostly their submissions and timers ([ADR-0040](0040-bloom.md)). The roadmap asks for a minimal
render graph, extracted from the passes that exist: passes declare the targets they read and
write, the graph owns the targets that live for one frame and orders the passes; no memory
aliasing, no asynchronous compute, no graph for the engine's users (`docs/PLAN.md`, M3b.8).

Known: `slime-render` uses no wgpu ([ADR-0027](0027-the-render-crate.md)). wgpu reports a pass's
misuse when its encoder is finished, and refuses a mapped buffer in a submission. On the owner's
machine (Ryzen 7 5800X, RX 6900 XT, RADV of Mesa 26.2.4, Linux 7.2.9), each submission and its
timer cost the CPU 0.03 to 0.04 ms. That GPU has no resizable BAR: the memory the CPU can map is a
window of 256 MiB, which the desktop's programs (a browser, Steam) fill to 200 to 245 MiB. With the
window filled to 241 MiB by a test program, one submission of a whole bloomed frame at
1920 × 1080 (17 passes) cost a mean 2.4 ms and 8 ms in one frame of ten, spent in the kernel's
submission call, which waited for memory; submissions of 2 to 6 passes kept their usual cost, and
submissions of 8 stalled. Filled to 251 MiB, every way of submitting stalled (5 to 9 ms per frame),
four passes per submission least.

To decide: where the graph lives; what passes and targets declare; how passes are ordered and what
is refused; how a frame is submitted and timed; what the statistics report; how the renderer's
frame is declared. Three designs were prototyped (the least code, the most rigorous tests, the best
performance). The owner decided on 2026-10-14 that a frame is recorded four passes to a submission
rather than in one, that `RenderStats` keeps one GPU time per kind of pass, and that the bloom's
levels are freed while the bloom is off. The rest was common to the designs or measured.

## Decision

- **Place.** A private module of `slime-render`, `graph.rs`, used by the renderer alone.
  `slime-gfx` gains one primitive: `Gpu::submit(label, |commands| ...)` records passes, through
  `Commands::render` and `Commands::render_depth`, into one command encoder, then submits them at
  once. `Gpu::render` and `Gpu::render_depth` keep their signatures and are a submission of one
  pass. A target counts as drawn into (for `SurfaceFrame::present`) once its pass is submitted; a
  misuse inside a pass panics when the commands of its submission are finished, with wgpu's message
  naming the pass.
- **Timers.** `Gpu::create_timer(label, passes)` gives a timer for up to `passes` passes per
  submission. The passes of one submission that name it are numbered from 0 in the order they are
  recorded, the next ones untimed; they share one of its four slots: one range of queries, one
  resolve, one copy, one readback. `GpuTimer::latest(pass)` gives pass `pass`'s time in the latest
  submission whose times came back: a timer's times all come from one submission.
- **Targets.** A *transient* target is created by the graph at the output's size divided by
  2^shift, rounded to the nearest, halves up (ADR-0040's rule, now `graph::scaled_side`), of a
  format or depth; its content lives for one frame, and the graph of the next frame shape reuses it
  when its label, format and size match. An *imported* target is created elsewhere and given to
  each recording: the output, which may be a window's image, and the shadow map. It holds what it
  held before the frame, so a pass may sample or keep it without an earlier writer. Target ids are
  typed, one type for render targets and one for depth targets, and made by the graph only. The
  environment, the BRDF table and the buffers are not targets: no pass writes them.
- **Passes.** A pass declares what it draws into, as one of `slime-gfx`'s two kinds of pass (a
  render target, with a depth target of its size or none, both cleared to opaque black and the
  farthest depth or both kept; or a depth target alone, cleared), and the targets its bind groups
  sample, at most two. The renderer keeps the kind of each pass (`Pass`: shadow, scene, sky, bloom
  down or up of a level, tone mapping) and records it, in one `match`, when the graph reaches it.
  The bind groups that sample transient targets are built from the declarations, so that what a
  pass declares and what it binds cannot differ.
- **Order.** The passes that draw into one target run in the order they were added, the first
  clearing it and the others keeping it; a pass that samples a target runs after every pass that
  draws into it. Kahn's algorithm, taking the earliest added pass among those ready: the order
  depends on the declarations alone, and is the order of addition whenever that order is valid.
- **Refusals.** As `GraphError` values naming the passes and targets: two transient targets with
  one label, since the graph of the next shape finds the targets it reuses by label; a pass that
  samples a transient target no pass draws into, or is the first to draw into one and keeps it; a
  pass that clears a target an earlier pass drew into; a pass that samples what it draws into; a
  cycle; a transient target no pass uses. The renderer panics on one: its frame comes from its own
  code and no user data, so a refusal is a bug in it
  ([ADR-0008](0008-errors-logging-and-assertions.md)).
- **When the graph is built.** Once per frame *shape*: the output's size, whether the shadow pass
  runs, whether the bloom's passes do. The renderer compares the shape every frame; on a change it
  declares the frame again, the graph orders it, creates the transient targets and the timers, and
  the renderer builds the bind groups. Each frame only walks the ordered passes.
- **Recording.** The passes in order, four to a submission named after the output, each
  submission timed by a timer of its own: a frame takes 1 submission without the bloom and, with
  it, 4 or 5 from 1280 × 720 to 3840 × 2160 (1 to 6 at other sizes), instead of 3 to 23.
- **Statistics.** `RenderStats` keeps its fields and their meaning: the time of each kind of pass,
  the bloom's passes summed. After a change of shape each can be `None` for a few frames, until its
  submission's new timer gives its first times; times of passes in different submissions can come
  from different frames, usually consecutive ones.
- **The renderer's frame.** shadow → scene → sky → bloom down 0 … N − 1 → bloom up N − 2 … 0 → tone
  mapping, each pass reading what the one before drew. The shadow pass and the bloom's are left out
  when they do not run: without the bloom there is no bloom target, and the tone-mapping pass binds
  the scene's light in place of the blurred copy, which its shader does not read at a strength of
  0. The scene pass declares the shadow map even without a shadow pass: it binds it.
- **The dev overlay** keeps its own pass and submission, after the renderer's.

This amends:

- **ADR-0027:** a frame's passes are declared to the render graph, which orders them; they are
  recorded four to a submission; the bloom's levels exist only while the bloom runs.
- **ADR-0033:** several passes share a submission (`Gpu::submit`); a misuse panics when the
  commands of its submission are finished; a timer times the passes of a submission
  (`create_timer(label, passes)`, `latest(pass)`). "Each pass is still its own submission" no
  longer holds.
- **ADR-0034, ADR-0038, ADR-0039:** the scene, tone-mapping, shadow and sky passes share
  submissions and are timed by their submissions' timers, not each in its own submission with its
  own timer; the high-dynamic-range and depth targets are the graph's, created again when the
  frame's shape changes and its size with it.
- **ADR-0040:** the bloom's passes are recorded four to a submission with the others, rather than
  one submission each as that ADR left them until this milestone, timed by their submissions'
  timers, not by one timer each; its levels are freed while the bloom is off.

## Alternatives considered

- **One submission per frame**, the plan's first wording: 0.23 ms instead of 0.38 at 1920 × 1080
  with the bloom while the mappable window has room, but with it filled to 241 MiB, a state the
  owner's desktop reaches with a browser and Steam open, a mean 2.4 ms, worse than one submission
  per pass. **Six or eight passes per submission**: six stayed stable at 241 MiB, but next to the
  stall that eight met, which a heavier scene pass would bring nearer. **One per pass**: 0.81 ms.
  GPU vendors advise fewer than about ten submissions per frame, each with enough work to keep the
  GPU busy; a frame of the sample holds about 0.1 ms of GPU work in all.
- **One timer per pass inside a shared submission**: on RADV it cost as much as the submissions it
  saved (0.75 ms for one submission timed by 17 timers, 0.22 ms with one timer).
- **The graph in `slime-gfx`**, which owns encoders, submissions and targets: public API of the GPU
  layer for one user, with notions of a frame that ADR-0027 keeps out of it. **A public graph in
  `slime-render`**: no second user, and the plan leaves a graph for games out.
- **Versioned reads** (each read names the version of a target that its last writer produced, as
  Frostbite's frame graph does): the order becomes the order of addition, a cycle cannot be
  declared, and ordering passes by what they read and write would be vacuous.
- **At most one pass clearing a target and one keeping it**: it refuses a second pass drawing over
  a target (debug lines over the output, say) until a rule says how to order it; the chain in the
  order of addition is that rule.
- **A boxed closure or a trait object per pass**: two passes count their draws into one
  `RenderStats`, which their closures would borrow at once; an allocation per pass per frame; five
  types for five arms of a `match`.
- **Bind groups built apart from the declarations**: a prototype declared a wrong read that no test
  saw; built from the declarations, the two cannot differ.
- **Declaring and ordering the frame every frame**: 7 µs and a few allocations per frame, for passes
  that change only with the shape.
- **A list of every pass's time in `RenderStats`**: 17 entries whose names become a promise; a GPU
  capture tool shows them already (decided by the owner).
- **Keeping the bloom's levels while the bloom is off**: 4.7 MiB at 1280 × 720, 10.5 MiB at
  1920 × 1080 and about 42 MiB at 3840 × 2160 held for nothing, and an exception in the graph for
  targets that no pass uses (decided by the owner).
- **The overlay inside the renderer's submissions**: `App::render` would hand out a recording
  object to pass to both, an application API change, for about 0.035 ms per frame.
- **`Gpu::render` removed, every caller on `Gpu::submit`**: about 45 call sites in tests, the
  overlay and READMEs grow, for no change of behaviour.

## Consequences

- On the owner's RX 6900 XT, `Renderer::render` takes 0.33 ms of CPU instead of 0.71 at
  1280 × 720 with the bloom, 0.38 instead of 0.81 at 1920 × 1080, and 0.11 instead of 0.22 without
  the bloom, whether the mappable window is as the desktop leaves it (201 MiB) or filled to
  241 MiB. GPU times do not change. On lavapipe, a submission waits for the previous one to finish:
  `render` still holds the GPU work of the frame's earlier submissions, as it did with more of them.
- Every pixel is unchanged: the readbacks of every test of the workspace and 300 frames covering
  shadows, environments, bloom strengths and output sizes are identical, byte for byte, on lavapipe
  and RADV (`docs/PLAN.md`, M3b.8).
- Four passes per submission is a number measured on one machine, against a stall in the kernel
  whose cause is not established; Direct3D 12 and other GPUs are not measured.
- After a change of shape, the GPU times can be `None` for a few frames, and that frame builds the
  graph again: on the owner's RX 6900 XT at 1920 × 1080, 0.26 ms more when the bloom starts,
  0.12 ms when it stops, 0.28 ms for a resize, which cost 0.10 ms before (the targets were created
  again then too): a resized frame with the bloom, 0.67 ms, still costs less than an ordinary one
  did before, 0.81 ms.
- A renderer whose frame changes shape at every call, drawing into outputs of two sizes in turn
  for example, reports no tone-mapping or bloom time: their submission's times come back during the
  next call, read by then from new timers. One renderer per output size avoids it (`docs/PLAN.md`,
  known debt).
- The graph cannot see inside bind groups: the scene pass's group holding the shadow map, built
  with the renderer, is ordered by its declaration alone.
- A misuse inside a pass panics at the end of its submission, with wgpu's message naming the pass.
- Adding a pass to the frame is a `Pass` variant, a declaration and an arm of a `match`; the graph
  orders it and refuses a frame it cannot order.
