# 0060. Resolving the samples weighed by their brightness

- Status: Accepted; amends [0059](0059-multisampling-in-the-gpu-layer.md) (shaders read the
  samples of a multisampled target)
- Date: 2026-10-15

## Context

Stage 3d draws the scene and the sky at four samples per pixel
([ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)), and each pixel then gets one
value from its samples, the *resolve*. The GPU's own resolve
([ADR-0059](0059-multisampling-in-the-gpu-layer.md)) averages them, in the light the scene pass
drew: high dynamic range, before tone mapping. Tone mapping compresses bright light towards white,
so a pixel half on a lamp, of light 20, and half on the night, of light 0.05, averages to 10, which
shows almost as white as the lamp: the edge keeps the steps it had without anti-aliasing. The
owner looked at *Nightwatch* both ways, on 2026-10-15: the lamps' edges and those of crates under a
floodlight kept their steps with the plain average.

Engines that resolve light of a high dynamic range weigh each sample down by its brightness, so
that each counts about as much as it shows once tone mapped: with a weight of 1 / (1 + its
brightest channel), the pixel above shows about 1, between the lamp and the night.

## Decision

- **A resolve pass of its own.** After the sky pass, a pass draws one triangle over the scene's
  target of one sample per pixel and gives each pixel the average of its samples, each weighed by
  1 / (1 + its brightest channel, exposed): `resolve.wgsl`. The sky pass no longer resolves.
- **Its cost reported.** `RenderStats::resolve_gpu_time` gives the pass's GPU time; the dev overlays
  of the `scene` example and of *Nightwatch* show it.
- **Shaders read the samples.** `slime-gfx` gains `BindingType::MultisampledTexture`
  (`texture_multisampled_2d<f32>`), and the targets of `Gpu::create_multisampled_render_target`
  can be bound at one, through `BindingResource::RenderTarget`: a shader reads them sample by
  sample with `textureLoad`. They still cannot be read back, nor sampled through a sampler. This
  amends ADR-0059, whose targets nothing read.

## Alternatives considered

- **The plain average of the GPU's resolve** (ADR-0059): no pass of its own, but the edges of bright
  surfaces keep their steps, and a game at night is made of them.
- **Tone mapping each sample, then averaging**: exact for what the display shows, but the bloom
  reads the resolved light before tone mapping, which would then no longer be light.
- **A weight from luminance** rather than from the brightest channel: a saturated red lamp, whose
  luminance is low, would keep its steps.

## Consequences

- The resolve pass costs a little more than the GPU's own resolve: on an RX 6900 XT (RADV), with
  *Nightwatch* in a window of 1901 × 2082 pixels, 57 µs, where the sky pass resolving cost 35 µs
  more than without.
- A bright sample counts for less: the light of an edge is lower than its plain average, and the
  bloom, which reads it, receives a little less of it there.
- The number of samples a shader reads is the target's own (`textureNumSamples`), so the shader
  needs no change if `MULTISAMPLE_COUNT` does.
