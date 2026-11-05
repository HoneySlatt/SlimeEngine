# 0180. One canvas, and the framework's foundations

- Status: Accepted; amends [0178](0178-two-pilots-and-the-frameworks-name.md) (what leaves with the
  framework, and when its repository moves)
- Date: 2026-11-05

## Context

[ADR-0178](0178-two-pilots-and-the-frameworks-name.md) moves uwui, the engine's UI framework, to a
repository of its own at Stage 6d, once its drawing has left the renderer, "standing on base crates
that move with it". It counted the four crates that the framework's core depends on. A program
built on the framework alone needs more, as the code stands on 2026-11-05:

- `slime-ui` builds a display list without a window or a GPU.
- `slime-ui-render`'s `UiRenderer::draw` takes a whole `slime_render::Renderer`, to use its canvas.
  `Renderer::new` also builds what 3D drawing needs: its layouts and pipelines, materials, lights,
  shadows' resources, the BRDF table and the copies to the GPU. Some are made on demand, and the
  canvas draws alone in its pass; what the rest costs a program at its start and in memory is not
  measured.
- The canvas, `slime-render`'s `canvas.rs` and `canvas.wgsl`, about 1 100 lines, depends on
  `slime-gfx`, `slime-color` and a few helpers of the renderer's.
- A program also needs a window, its events, a loop that draws only when something changes and
  wakes from any thread, and, for agents, the development channel. `slime-app` holds that loop and
  its waker, over `slime-platform` and `slime-gfx`, but depends on `slime-world` whatever the
  program.

Moving crates to another repository does not make the framework stand alone.

## Decision

**One canvas.** The canvas leaves `slime-render` for the framework's drawing, which draws a display
list with a `Gpu` and a target, without a `Renderer`. The engine draws its own interfaces, the
editor's, the dev overlay and games', with that same canvas, over its views: no second
implementation.

**A shell without the world.** What runs an application, its window, its events, frames on demand,
the waker and the development channel, depends on neither the world, a game's host, the editor nor
games, as [ADR-0170](0170-applications-on-the-ui-framework.md)'s boundary says. Whether it is
`slime-app` without its world, its world made optional, or a crate beside it, is decided with the
shell's design at Stage 6d's start, without duplicating the loop.

**The foundations, an open decision.** Before the repository moves, Stage 6d decides where the
foundations that both the engine and the framework use live, the GPU layer, the platform and the
loop: they move with uwui, and the engine depends on its repository for them; uwui keeps a thin
layer of its own over wgpu and winit, at the cost of a second one; or another split. Not a
repository of the framework's that depends on the engine's.

**A check in CI.** From Stage 6d, CI builds a program on the framework and its shell alone, and
fails if it compiles `slime-render`, `slime-world` or the editor.

**Measured first, at Stage 6d:** what `Renderer::new` costs an application, at its start, in memory
and in video memory, against the canvas alone; an application's start to its first useful frame,
against the quality bar's 150 ms, the GPU's device and the system's fonts included; an application
at rest, under Wayland and under Xwayland; what an application compiles, its crates, its build time
and its size.

## Alternatives considered

- **The repository moved once the canvas leaves the renderer**, as ADR-0178 said: the window, the
  loop and the GPU layer would stay in the engine's repository, which the framework's would depend
  on.
- **A canvas of the framework's own beside the renderer's**: the framework free sooner, but two
  canvases to keep alike, which the engine's interfaces and applications would draw differently.

## Consequences

- The framework's repository moves after the foundations' decision, not with the canvas alone.
- The renderer composes the framework's canvas over its views, rather than owning it.
