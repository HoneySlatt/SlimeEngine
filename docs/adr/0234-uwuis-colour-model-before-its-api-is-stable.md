# 0234. uwui's colour model before its API is stable, HDR content after

- Status: Accepted; amends [0137](0137-the-canvas.md) (the interface blends in sRGB's encoding
  on every target, a target of floats included) and [0190](0190-uwuis-requirements.md) (HDR and
  wide gamuts are not "from Stage 6d": the colour model comes in uwui's first stage, HDR images and
  videos after it)
- Date: 2026-11-11

## Context

[ADR-0190](0190-uwuis-requirements.md) asks for "HDR and wide gamuts for images and videos from
Stage 6d"; it was written before [ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md) left the
trials and the ports to uwui's own first stage. uwui's roadmap puts "video decoded by the GPU, HDR"
after that stage, once the API is declared stable. The two disagree, and HDR changes the colour
model: the colour types, the themes' format, the formats of the images that groups are drawn into,
the blur of glass. Aerith, a media client, needs HDR video under an SDR interface.

What was read on 2026-11-11:

- wgpu 30.0.1 configures a surface's colour space (`SurfaceColorSpace`: sRGB, Display P3, their
  extended forms, scRGB, BT.2100 PQ and HLG); `Surface::display_hdr_info` is filled on Direct3D 12
  and Vulkan on Windows, Metal and the web, not on Linux, Android, iOS nor tvOS. On the web, PQ and
  HLG are not offered.
- [ADR-0137](0137-the-canvas.md) blends in sRGB's encoding into an sRGB target, as design tools and
  browsers do, but linearly into a target of floats. A window moved to `Rgba16Float` for HDR would
  change every theme's look: 50 % of black over white gives 128/255 in sRGB's encoding, 188/255 in
  light.
- The headroom above SDR white changes while an application runs: macOS notifies its changes,
  Android declares a ratio for each buffer, Windows asks applications to follow the display's SDR
  white level
  ([Microsoft](https://learn.microsoft.com/en-us/windows/win32/direct3darticles/high-dynamic-range)).
- Wayland's colour management describes an image by its primaries, transfer function and
  luminances, its reference white among them, and its mastering display
  ([color-management-v1](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/color-management/color-management-v1.xml)).
  On Wayland, the driver's swapchain holds the colour description of the surface that it presents
  (Mesa 26.2); a second one on the same surface is a protocol error.
- Floats in 16 bits double the memory and bandwidth of 8 bits: a full 4K write at 240 Hz is about
  8 GB/s in RGBA8 and 16 GB/s in RGBA16F, computed, not measured. Apple, Android and Windows turn
  extended range on per layer, for the content that needs it.

## Decision

The owner's decision of 2026-11-11. In uwui's first stage, before its API is declared stable:

- **One way to blend.** The interface blends in sRGB's encoding on every target, 8 bits or floats,
  extended values continuing the encoding, so that a theme looks the same on SDR and HDR outputs and
  on every tier. Transitions' interpolation is checked against it in the same milestone.
- **1.0 is SDR white.** Values above it are brighter than SDR white. The display's headroom is read
  live and applied once, at the end of the frame, so that a change of headroom redraws nothing that
  is kept.
- **A colour description on every surface, image and plane**, extensible on Wayland's model:
  primaries, transfer, luminances with the reference white, the mastering display. Per-frame
  metadata (HDR10+, Dolby Vision) stays with the planes that carry it.
- **The output's description from the host**: from wgpu where it gives one, from the host's own
  API elsewhere. On Wayland, uwui's own surface gets its description through wgpu's colour space,
  never a second object of the protocol.
- **Floats while content asks.** A window composes in 16-bit floats only while it shows content
  beyond the output's SDR range or gamut, on every tier; 8 bits otherwise. Images kept for groups
  keep values above 1.0 unclamped. On Windows, a wide gamut goes through scRGB.
- **Wide gamuts now.** The themes' colours may lie outside sRGB; images tagged Display P3 or with
  an ICC profile show right, since images are in the first stage. The contrast check of CI is
  defined for such colours.
- **Glass over HDR content**: by default, what lies behind is brought down to SDR white before the
  blur, so that glass keeps its look beside highlights.

After the first stage, on the same model: HDR images, gain maps kept as their base and their map
and applied at the live headroom, AVIF and JPEG XL in HDR; HDR video planes
([ADR-0235](0235-content-from-outside-the-tree.md)).

## Alternatives considered

- **All of HDR after the first stage**, as the roadmap had it: nothing more now, but an API
  declared stable on 8-bit sRGB, then broken or patched when HDR video and images come.
- **All of HDR in the first stage**, images and videos included: everything proven before the API
  is stable, but a much longer stage for content that none of its four programs shows.
- **Blending in light on floats**, as ADR-0137 had it: closer to physics, but the same theme looking
  otherwise once a window holds HDR content, and opacities unlike the designer's tool.
- **Floats by tier**, always on the high tier: simpler, but twice the bandwidth and memory at rest,
  against the lead's budget.
- **Glass lit above SDR white over HDR content**: brighter, but an interface surface in HDR wherever
  glass covers a video, and a look that changes with the display.

## Consequences

- The first stage gains the colour model, about one to two milestones, estimated; its place among
  the milestones is cut at SlimeEngine's M6d.10.
- The canvas's path for floats changes its blending; its tests on `Rgba16Float` change with it.
- The host's API carries the output's colour description and its headroom, and their changes.
- The GLES and WebGL2 fallbacks stay sRGB: the boxes and browsers that use them get no wide gamut.
- Apple's colour spaces wait for wgpu's fixes, unreleased on 2026-11-11
  ([ADR-0233](0233-uwuis-platforms-apples-chips-ios-and-the-tvs.md)).
