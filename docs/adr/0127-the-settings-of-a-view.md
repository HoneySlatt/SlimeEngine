# 0127. The settings of a view

- Status: Accepted; amends [0034](0034-hdr-rendering-and-tone-mapping.md) (the tone-mapping pass
  reads the scene's light bilinearly), [0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)
  (a view may draw without anti-aliasing), [0111](0111-the-frame-contract.md) (how a view's
  settings are built), [0122](0122-the-development-channel.md) (version 4 of the protocol: a view's
  settings read and changed) and [0126](0126-views-apart-from-the-renderer.md) (a view's targets
  made again once its size holds still); amended by [0128](0128-asset-requests.md)
  (`ReflectedSettings` becomes `Reflected`, which reports use too),
  [0158](0158-the-viewport.md) (a view says when its new size has held still) and
  [0159](0159-picking-and-the-selection.md) (the modes of debugging of the base color and of the
  light alone)
- Date: 2026-10-22

## Context

[ADR-0111](0111-the-frame-contract.md) decided that each view sets the scale it renders at, its
anti-aliasing, a mode of debugging that shows normals or shadows, and which passes it runs, and that
a view's targets are made again once its size holds still.
[ADR-0126](0126-views-apart-from-the-renderer.md) built the views, and left these to M6a.8b, the
second half of M6a.8.

After M6a.8a, a `View` drew every pass at four samples per pixel, at the size of the rectangle it
draws into. When that size changed, the view dropped its frame and waited for the GPU before it made
the targets of the new size, so that the GPU never held both (G18): a window's edge dragged, which
changes the size at every frame, waited for the GPU at every frame, and built the view's frame again
with new timers at every frame, so that it reported no tone-mapping or bloom time. The tone-mapping
pass read the texel under each pixel, its targets being the rectangle's size. Multisample
anti-aliasing at four samples was the renderer's one way to draw
([ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md),
[ADR-0060](0060-resolving-the-samples-weighed-by-their-brightness.md)): four samples are what every
GPU at the engine's floor offers for half-precision targets. Temporal anti-aliasing belongs to Stage
9a.

The development channel ([ADR-0122](0122-the-development-channel.md)) lets an agent capture a view's
part of the image (ADR-0126); it could not change how a view draws, which a debugging session of an
agent needs, as a shadow or a normal that looks wrong. `slime-app`, whose presentations hold the
views, does not depend on `slime-render`.

The owner took seven decisions on 2026-10-22, one at a time:

- **Q1, where the settings live**: in the `View`, which keeps them from frame to frame, rather than
  in each `ViewDraw`, or at the view's creation only.
- **Q2, which anti-aliasing**: none, or multisample at four samples, the two that every GPU at the
  engine's floor offers; not two or eight samples, which depend on the adapter, nor FXAA, which
  blurs detail and which Stage 9a's temporal anti-aliasing would replace.
- **Q3, the render scale**: from 0.25 to 2, the image read bilinearly by the tone-mapping pass,
  rather than a pass of a sharper filter, or a scale below 1 only.
- **Q4, a size held still**: 200 ms without a change, a constant, rather than a number of frames,
  or a setting of the view.
- **Q5, the modes of debugging**: the normals and the shadows, which ADR-0111 names, rather than a
  wider list, or wireframe, which needs lines that M6a.10 brings.
- **Q6, the passes a view leaves out**: three switches, the shadows, the sky and the bloom, rather
  than the sun's and the spot lights' shadows apart, or the shadows and the bloom only.
- **Q7, an agent and a view's settings**: read and changed through the MCP server, by reflection,
  rather than read only, or nothing yet.

## Decision

**A view holds its settings.** `ViewSettings` is a value of the view: `View::set_settings` changes
it, from the next frame, `View::settings` reads it; `Renderer::create_view` makes a view with the
default settings, a scale of 1, four samples, no debugging and every pass. Its fields are public and
its type is not exhaustive, made from its default (ADR-0046). It is described by reflection: the
dev overlay and the development channel read and change it through its description, within its
ranges, without knowing its type.

**A view draws without anti-aliasing, or at four samples.** `AntiAliasing::Off` draws the scene and
the sky at one sample per pixel into the target that the later passes read, without the resolve
pass; `AntiAliasing::Msaa4` draws them at four, resolved as ADR-0060 decided, the default. The
renderer makes the scene's three pipelines of each kind and the sky's for both numbers of samples:
fourteen pipelines where it made ten. Stage 9a's temporal anti-aliasing will be a third value.

**A view renders at a scale of its rectangle.** Its targets are its rectangle's size times its
render scale, from 0.25 to 2, each side rounded, at least 1 and at most the GPU's largest side. The
camera's image keeps the rectangle's aspect. The tone-mapping pass reads the scene's light where the
centre of each pixel of the rectangle falls in the targets, bilinearly through the bloom's sampler:
at a scale of 1, the centre of the texel under it; below 1, between texels, blurred; at 2, the
average of the four texels of each pixel. The bloom reads its first level at the same place.

**A view's targets follow its size once it holds still.** The size that a view's rectangle and
render scale ask of its targets must stay the same for `RESIZE_DELAY`, 200 ms, before the view
makes them at that size; until then it draws into the targets it has, stretched over its rectangle
by the same bilinear reading, its frame, and so its timers, kept. A view without targets, at its
first frame or after a frame that could not make them, makes them at once. When it makes them, it
frees the old ones first, waiting for the GPU, as ADR-0126 decided: a window's edge dragged makes
them once, when it is let go, and waits once. A new anti-aliasing, or a pass turned on or off,
builds the frame again at once, at the size its targets have.

**A mode of debugging shows normals or shadows as they are.** `DebugMode::Normals` shows the normal
each surface is shaded with, its normal texture applied, in world space, each axis from −1 to 1 as
a channel from 0 to 1. `DebugMode::Shadows` shows the share of the shadowing lights' light that
reaches each surface, the product of the sun's, through its cascades, and of each spot light's
where its cone reaches, tinted by the finest cascade that holds the surface, red, green, blue, then
yellow from the nearest. The scene shader branches on a value of the frame's uniform, the same for
every pixel: no pipeline more. A mode of debugging draws neither the sky nor the bloom, and the
tone-mapping pass writes its colors without the exposure or the tone curve.

**A view leaves out the shadows, the sky or the bloom.** `ViewSettings::shadows` turns off the
sun's cascades and the spot lights' tiles for the view, whose lights then light every surface they
reach; the first view of a call that draws shadows draws the spot lights' tiles for every view.
`ViewSettings::sky` leaves the sky pass out, black where no surface is; `ViewSettings::bloom`, the
bloom's passes. The scene pass and the tone mapping always run, and the resolve follows the
anti-aliasing.

**A tool reads and changes a view's settings.** `slime-app` describes settings by reflection,
`ReflectedSettings`, their value with the registry of their type: `Presentation::view_settings`
gives those of a view that it drew, and `Presentation::set_view_settings` takes them back changed,
both refusing by default. Version 4 of the development channel's protocol lists each view of the
game's state with its settings as a document in the engine's notation (`ViewSummary`), and sets a
part of them, `Request::SetViewSettings`, checked against their type and the ranges of their fields,
from the next frame. The MCP server's `game_state` gives them, and its `set_view_settings` changes
them. `slime-devui`'s `edit_reflected` shows any reflected value with the world panel's widgets: the
`world` example's views panel, on F3, and *Hunna Wood*'s overlay, on F1, edit their views' settings
with it.

## Alternatives considered

Where the settings live:

- **In each `ViewDraw`**: a setting that changes often is given with the camera, but the caller
  keeps and gives them again at each frame, and a forgotten field changes the targets silently.
- **At the view's creation only**: the simplest, but a game's options menu or the overlay would make
  views again to change one.

Which anti-aliasing:

- **Two, four or eight samples**: more choice, but two and eight depend on the adapter and the
  format, which the floor does not promise; each needs a check of the GPU and a test, and eight
  samples of half precision take twice the memory of four.
- **FXAA beside multisampling**: cheap, and it smooths the edges inside textures and those that
  masked materials cut, but it blurs detail and guesses edges from the image (ADR-0056), and
  temporal anti-aliasing replaces it for most uses.

The render scale:

- **A pass of a sharper filter**, Catmull-Rom or a port of FSR 1: sharper at low scales, but a pass
  and a target more at the output's size, halos around very bright light from the filter's negative
  lobes, and Stage 9a's temporal upscaling would replace it.
- **Below 1 only**: simpler to describe, but no supersampled captures and thumbnails.

A size held still:

- **A number of frames**: deterministic, but its time follows the frame rate: 40 ms at 240 frames a
  second, which a pause of the cursor in the middle of a drag passes, half a second at 20.
- **A setting of the view**: more flexible, but nothing asks for it.

The modes of debugging:

- **A wider list**, base color, roughness and metalness, light alone, cascades alone: useful to the
  editor, but work that nothing asks for before Stage 6b, each with an image test.
- **Wireframe**: much asked of editors, but drawing polygons as lines is an optional feature of
  wgpu, not at the floor; lines come with M6a.10.

The passes:

- **The sun's and the spot lights' shadows apart**: finer, but the spot lights' tiles are drawn once
  for every view, so that a view that leaves them out saves little.
- **The shadows and the bloom only**: fewer cases, but a material's preview on a plain background
  would need a black environment in the scene, which would light the other views differently.

An agent and a view's settings:

- **Read only**: less work, but an agent could not look at the normals or the shadows.
- **Nothing yet**: the overlay shows them by hand only, until the editor or an agent asks.

Each view's settings in `slime-app`:

- **The renderer's type in `slime-app`**: direct, but `slime-app` would depend on `slime-render`,
  which a headless game does not need, and the channel would know one renderer's settings.

## Consequences

- A game holds its views' settings in them, or, to keep them through a lost device, in its own
  state, giving them to each view made again; the `world` example and *Hunna Wood* give them at each
  frame.
- A window's edge dragged keeps the view's targets and timers, its image stretched, and makes them
  once when it is let go: the known debt of a view that waits for the GPU at each change of size,
  and loses its times then, is retired; a view whose passes change at every frame still loses them.
- A view without anti-aliasing takes under a third of the memory of one with it; a render scale
  multiplies a view's memory by about its square.
- The renderer makes fourteen pipelines; its shaders read a mode of debugging and a scale of the
  scene's light.
- The protocol of the development channel is at version 4: tools and games of version 3 refuse each
  other. `slime-app` depends on `slime-reflect` in every build, as the world already did.
