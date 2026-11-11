# 0235. Content from outside the tree: the element is uwui's, the player the application's

- Status: Accepted; amends [0171](0171-effects-of-the-interface.md) (over a plane that uwui cannot
  read, glass falls back to a material without a live blur, or to the system's glass on Apple's
  platforms) and [0190](0190-uwuis-requirements.md) (uwui shows frames decoded by the GPU without a
  copy, and decodes none; its optional "video and HDR" is this element)
- Date: 2026-11-11

## Context

[ADR-0190](0190-uwuis-requirements.md) asks for "video decoded by the GPU (VA-API, Direct3D 11,
MediaCodec)" and lists "video and HDR" among uwui's optional parts. Two of the owner's applications
show content that uwui does not draw:

- Aerith, a media client: mpv in its own process on Linux, its buffers placed in a subsurface by a
  small Wayland compositor of Aerith's, and on Windows a child window under a DirectComposition
  interface; Media3 in a SurfaceView on Android; AVFoundation on Apple's platforms.
- Magnolia, a DAW: plugin windows drawn by other processes, their input and focus forwarded and
  given back ([Magnolia's needs](https://github.com/HoneySlatt/Magnolia/blob/main/docs/uwui-needs.md)).

Without a line drawn, the family keeps two video stacks. What was read on 2026-11-11:

- Android's `TextureView` turns HDR video into SDR; a `SurfaceView` keeps it, and the interface
  cannot read its pixels ([HDR playback](https://developer.android.com/media/grow/hdr-playback)).
  Child layers move atomically only within one `ASurfaceTransaction`; Kodi drops a frame at each
  move of its video view.
- A plane that the system composes leaves the application at rest while a video plays; composing
  4K video into uwui's own frame costs a full write at each of the video's frames.
- Compose Multiplatform places native views under its canvas, above it behind an experimental flag,
  with their accessibility, and changed that API across three releases.
- Apple gives custom elements its own glass (`NSGlassEffectView`, `UIGlassEffect`), which Core
  Animation composes over layers that the application cannot read.

## Decision

The owner's decision of 2026-11-11.

- **uwui holds an element for content from outside the tree:** a surface of another process, a
  layer of the platform (a `SurfaceView`, an `AVPlayerLayer`, a DirectComposition visual or a child
  window), or frames of the GPU imported (dmabuf, shared handles, `IOSurface`). It carries the
  content's colour description ([ADR-0234](0234-uwuis-colour-model-before-its-api-is-stable.md));
  its place under or above the interface; input and focus forwarded and given back, shortcuts
  kept while it holds the focus; a node of accessibility; its size negotiated, a morph scaling the
  plane rather than resizing its producer.
- **A plane by default.** The system composes it, and uwui draws nothing while only the video
  moves. Where uwui can read the frames and an effect or a motion needs them, it composes them
  itself for those frames, then hands them back as a plane.
- **In step where the platform allows it.** The plane moves with the interface's frame where the
  platform commits both together; elsewhere a still image of the plane stands in while it moves.
- **Glass over what uwui cannot read** falls back to a material without a live blur, a tint or a
  blurred still; on macOS and iOS, the system's glass between the plane and uwui's layer is one step
  of that fallback, Apple's look at that place alone.
- **Live planes have a budget**; the planes beyond it show their poster.
- **The host carries the element's requests:** the content's frame rate, the screen kept on, the
  headroom wanted.
- **The player is the application's:** demuxing, decoding, sound, synchronisation, subtitles,
  passthrough, and the small compositor that serves a child process. If Aerith and Magnolia both
  write one, it becomes a crate shared outside uwui's core, decided then.
- **When.** Before the API is declared stable, its foundations alone: a transparent surface over a
  plane, foreign layers in the host's API, colour descriptions. The whole element after the first
  stage. Its API stays experimental, behind a feature and outside semantic versioning, until Aerith
  or Magnolia ships on it.

## Alternatives considered

- **Everything in the applications**, uwui leaving a hole: a smaller uwui, but placement, colour and
  glass over the video written again by each application.
- **The video composed into uwui's frame always**: one path and every effect, but a full 4K write
  at each of the video's frames, HDR lost on Android, and the system's tone mapping replaced by
  uwui's own.
- **A simple player in uwui**, short silent clips played by it: useful to Slatty and Opium, but no
  two applications ask for it yet. Kept as an idea.
- **The element's API stable with the rest**: one promise, but frozen after a trial whose code is
  thrown away.

## Consequences

- ADR-0190's "video decoded by the GPU (VA-API, Direct3D 11, MediaCodec)" reads "frames decoded by
  the GPU shown without a copy".
- The host's API grows layers of the platform beside its surface, on each host.
- Imported frames rest on wgpu's interop, which changes with its majors: each path is tested in CI
  on its platform.
- Picture in picture, where the plane outlives its element, and protected content, which has no
  still image, are decided with the whole element.
