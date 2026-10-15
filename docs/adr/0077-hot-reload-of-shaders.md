# 0077. Hot reload of shaders

- Status: Accepted
- Date: 2026-10-15

## Context

[ADR-0072](0072-stage-4b-live-assets.md) decides how a game sees the renderer's shaders change
while it runs: the shaders stay WGSL files beside the renderer's Rust code, embedded in the
program; a game that hot-reloads reads them from `crates/slime-render/src` and gives the renderer
new sources when they change; the renderer makes all its pipelines again, or none, keeping the old
ones on an error that names its line and column; uniform layouts declare their minimum sizes, so
that a shader that does not fit its data is refused when its pipeline is made, not when it draws.
It leaves how to the milestone that builds it, M4b.7, and asks it to measure the pause on
Direct3D 12.

Where the code stood: the renderer embedded its four shaders with `include_str!` and made its ten
pipelines in `Renderer::new`, once. A uniform binding of `slime-gfx` declared no size, so wgpu
checked the size of the values a shader reads against the buffer only when a group was bound for a
draw, and panicked there.

## Decision

**Uniform bindings declare their size.** `BindingType::UniformBuffer` holds `size`, the bytes bound
there, at least what the shaders read, which `slime-gfx` gives wgpu as the binding's minimum size: a
pipeline whose shader reads more is refused when it is made (`GfxError::Pipeline`), and a group that
binds a smaller buffer panics when it is made, as a group that does not match its layout does
(ADR-0016). Every uniform layout of the engine declares the size of the buffer that its code writes.

**The renderer's shaders are sources it can be given.**

- `ShaderSources` holds the WGSL of the four shaders: the embedded ones by default
  (`ShaderSources::embedded`), or those of the files of a directory (`ShaderSources::read`);
  `ShaderSources::DIRECTORY` is `crates/slime-render/src` in the workspace that compiled the
  program.
- `Renderer::set_shaders(gpu, sources)` compiles the four shaders and makes the ten pipelines from
  them, then puts them in place of the old ones, all at once; any error leaves the old ones, which
  the renderer draws on with. A shader module is named after its file, so that an error says which
  file its line and column are in. The work is done on the calling thread, between two frames.
- `ShaderWatch` reads the four files every half second, as often as `ManifestWatch` reads a
  manifest, and gives their sources when they differ from the last ones it gave, the embedded ones
  at first; it is started again from the embedded ones when the game makes a new renderer, after a
  lost GPU device. A read that fails is told once.

**The game and the example.** With `--watch`, *Hunna Wood* and the `scene` example poll a
`ShaderWatch` beside their `ManifestWatch` and give the renderer the sources it gives; they log how
long the pipelines took, or why they were refused, with the error's causes, which hold the file,
the line and the column.

**Measured** by the renderer's test that makes the pipelines again from unchanged sources, on the
owner's RX 6900 XT (RADV): 7 ms in a release build when the driver's cache holds the shaders
already, as when one file changed, 76 ms when it does not (`RADV_DEBUG=nocache`); about twice as
long in a debug build, in wgpu's own work; 45 ms on lavapipe in a release build. On Direct3D 12,
CI's Windows job prints it on WARP, in a debug build, and M4b.7's outcome in `docs/PLAN.md` gives
that figure.

## Alternatives considered

- **Making the pipelines on another thread and swapping them when ready**: no frame would wait, but
  a load of its own for a pause that happens only while someone edits a shader.
- **Making again only the pipelines of the shader that changed**: less work, but four cases where
  one does, and a change of `shader.wgsl`, which six of the ten pipelines use, is the common one.
- **Checking the sizes by reading the shaders' declarations ourselves**: wgpu checks them already
  once it is given the sizes.
- **Shaders as cooked data, or the operating system's notifications**: rejected by ADR-0072.

## Consequences

- Code that creates a uniform binding with `slime-gfx` gives its size: a change to the API.
- The frame that gives the renderer new shaders is late by the time the driver takes, tens of
  milliseconds cold on Vulkan; Direct3D 12 compiles each pipeline's shaders again with DXC.
- The programs that CI ships run where `crates/slime-render/src` is not: with `--watch`, they say
  once that the shaders cannot be read, and draw with the embedded ones. Hot reload of shaders is
  checked on Windows by CI's tests alone.
- The shaders are the engine's own: a game has no shaders of its own yet to reload.
- `ShaderWatch` and `slime_assets::ManifestWatch` repeat the same dozen lines of polling, each
  over what it reads: no crate that both reach suits a type for both yet.
