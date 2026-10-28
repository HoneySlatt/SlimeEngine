# 0129. Drawing for the editor

- Status: Accepted; amends [0041](0041-render-graph.md) (two passes of a client's drawings in a
  view's graph), [0111](0111-the-frame-contract.md) (the unlit kind of material, the second) and
  [0115](0115-what-the-gpu-layer-gives-the-editor.md) (lines drawn by the renderer, not by the GPU
  layer; the first point after the sky; the scene's depth resolved and a part of a texture written
  wait for their first user; the milestone in two); amended by
  [0159](0159-picking-and-the-selection.md) (what a client draws gains an outline, for which the
  scene's depth is resolved)
- Date: 2026-10-22

## Context

[ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md) decided what the GPU layer, `slime-gfx`,
and the renderer give the editor before its viewport: blending, lines, an integer format of target,
reading a target back without waiting, writing a part of a texture, a pass that resolves the
scene's depth into one sample; an unlit kind of material, for handles and icons; picking by the GPU
and by the CPU; two points of each view's graph where a client, an editor or a tool, inserts its
draws, in the scene after the opaque instances and after tone mapping; a view's image sampled by
another pipeline, which the editor's interface shows. It left the shape of each piece to the
milestone that builds them, M6a.10 in [`docs/PLAN.md`](../PLAN.md).

Before it, a pipeline replaced what its target held, without blending; the renderer had one kind of
material, physically based ([ADR-0035](0035-physically-based-materials.md)), whose alpha mode
`BLEND` it refused; its passes were a closed list in a private graph
([ADR-0041](0041-render-graph.md)), and nothing but the scene was drawn into a view.

The owner took five decisions on 2026-10-22, one at a time:

- **The milestone in two**: M6a.10a draws for the editor, lines, blending, the unlit material, the
  two points and a view's image in a panel; M6a.10b picks, by the GPU and by the CPU, through the
  MCP server too; rather than one milestone, of one long review, or three, whose first, the GPU
  layer alone, would have nothing to check by hand.
- **What a client draws, as data**: for each view and each point, lines and instances, which the
  renderer draws with pipelines of its own; rather than passes of the client's own, which would
  hand it the graph's formats, samples and scale of light that ADR-0115 keeps private, or both,
  two ways of doing one thing.
- **Lines a width in pixels wide**: each segment a band that faces the camera, drawn by the
  renderer; rather than the GPU's lines, one pixel wide whatever the GPU, which the editor's handles
  of two or three pixels would have to replace.
- **An unlit material's color is the color shown**, whatever the exposure; rather than a light in
  the scene's units, which the exposure scales as it scales an emission, or either, by a setting
  that nothing uses yet.
- **What has no user waits for it**: the scene's depth resolved into one sample, which an outline
  or handles partly hidden need, and a part of a texture written, which an atlas of icons or the
  editor's interface need, come with them at Stage 6b; rather than built now, with tests alone,
  their shapes guessed.

## Decision

**Blending in the GPU layer.** A pipeline's fragment stage says how it mixes its colors with what
its target holds, `slime_gfx::Blend`: none, replacing it, or `Blend::Alpha`, its color laid over by
its alpha, straight, the GPU mixing the linear values of an sRGB target. The GPU layer gains no
primitive of lines.

**An unlit kind of material, the second.** `Renderer::create_unlit_material` makes one from
`UnlitData`: a color, times an optional sRGB texture, an alpha mode and a sidedness. Its color is
written into the scene's target unexposed, so that the tone curve, Khronos PBR Neutral, shows it as
it is, but for the 0.04 that the curve takes from a color between 0.08 and 0.76; a mode of debugging
shows its normals. It casts no shadows. Its parameters, which reflection describes, are
`UnlitValues`, the color and the alpha mode. `AlphaMode` gains `Blend`, glTF's `BLEND`: the unlit
kind draws it with pipelines of its own, after the scene's other surfaces and after the sky, which
would otherwise cover it where nothing lies behind it, laid over them by its alpha and without
writing its depth, in no particular order among themselves; the physically based kind refuses it
until Stage 9a sorts transparent surfaces by their depth. A kind says whether it
casts shadows and whether it blends (`MaterialKind`), and the renderer makes only the pipelines it
needs.

**What a client draws into a view.** `ViewDraw::drawings` gives a view `Drawings`, at the two points
of its frame:

- `in_scene`, a `DrawList` of lines and instances, of any kind of material, drawn in the pass after
  the sky pass, into the scene's targets, with their depth and samples: hidden by the surfaces in
  front of them, through the bloom and the tone curve as the scene is. ADR-0115 put it in the scene
  pass, after the opaque instances: after the sky, what writes no depth, as a line or a blended
  icon, is not covered by the sky drawn where no surface is. That pass draws the client's opaque
  and masked instances, then every blended surface, the scene's and the client's, then the
  client's lines. The instances are culled as the scene's are, their values after the scene's in
  the view's buffer, and cast no shadow.
- `over_image`, lines alone, drawn in a pass after the tone-mapping pass, over the view's
  rectangle of its output, over everything, in the output's colors. Instances there need a depth of
  their own, so that a handle's parts hide one another: they come with the editor's handles.

Each pass runs only in the frames that draw something there, its shape changing the graph, whose
transient targets are kept. The view's statistics give their times (`after_sky_gpu_time`,
`over_image_gpu_time`), and count the client's instances with the scene's.

**Lines as bands.** A `Line` is a segment between two points of the world, a color with its alpha,
and a width in the output's pixels. The renderer packs a view's segments relative to the camera
(ADR-0048) into a buffer of its own and draws each as two triangles, built in `lines.wgsl` from the
vertex's index alone: the segment, cut where it passes behind the near plane, is projected, then
widened across the screen by half its width and a pixel, its ends squared, and each pixel covered
in part fades by the share it covers, whatever the view's anti-aliasing. Each pixel measures that
share against the segment's ends on the screen, which its vertices hand on unchanged: values
interpolated across the band would be bent by perspective, and, on lavapipe, lost where the GPU
clips a band that leaves the screen. In the scene, through
targets of the view's render scale, its width is counted in the output's pixels all the same; it is
drawn where nothing is nearer, without writing its depth, so that a line that lies on a surface may
be hidden by it in places, which a client avoids by lifting it off. A segment that is not finite,
not wide, or whose color is out of 0 to 1, is not drawn; past what the device's storage buffers
hold, the last are left out, with a warning, as instances are.

**A view's image in a panel.** A view drawn into an output of its own is sampled by any pipeline of
the GPU layer, which `slime_gfx::BindingResource::RenderTarget` binds already: a test shows it.
`Mesh::bounds` gives a mesh's box in its own space, around which an editor draws what is selected.

The `world` example draws a grid on the ground in the scene, and an icon above each light, facing
the camera, through an unlit blended material; Tab selects an entity that places meshes, whose box
both views draw over their image.

## Alternatives considered

Those of the five decisions are above. Besides:

- **The first point in the scene pass, after the opaque instances**, as ADR-0115 wrote it: no pass
  more, but the sky pass, after it, would cover the lines and the blended instances drawn where no
  surface is.
- **A pull of the lines' depth towards the camera**, so that a line lying on a surface shows whole:
  at a grazing angle, a surface's depth changes across a pixel by more than any pull that keeps a
  line from showing through what lies close in front of it.
- **Picking's pieces in M6a.10a**, an integer target and a target read back without waiting: they
  come with picking, their user, in M6a.10b.

## Consequences

- An editor or a tool draws its grid, its icons and the box of what is selected without code of
  its own on the GPU; outlines and handles come at Stage 6b, with the pieces they need.
- A stylized game draws unlit surfaces in its scene, blended ones without sorting until Stage 9a.
- `FragmentState` gains `blend`, `ViewDraw` gains `drawings`, `ShaderSources` gains `unlit.wgsl` and
  `lines.wgsl`, and the renderer makes more pipelines: those of the unlit kind and of the lines.
