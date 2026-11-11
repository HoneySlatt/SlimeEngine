# 0236. One trial of content from outside, for Aerith and Magnolia

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (a fourth bounded trial before the API is
  declared stable, and a third milestone of Android's on a television box) and
  [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) (the order of the trials)
- Date: 2026-11-11

## Context

Before uwui's API is declared stable, three bounded trials run, each of two milestones, thrown away,
each with its report: Android, the web, then iOS
([ADR-0181](0181-uwuis-scope.md), [ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)).
Magnolia, a later client of uwui, asks for a small trial of a surface drawn by a child process on
Wayland, its input and focus forwarded, and for its screen of reference to be measured. Aerith, a
later client too, asks for a trial of video in an element at the display's rate, an HDR plane under
an SDR interface, and a screen driven by a remote, at the end of the first stage.

A challenger's review found Aerith's trial too wide for two milestones and too late: the drawing of
groups and glass would be built before uwui knows where planes show through them. Of the
compositors that uwui tests on, KWin and Mutter manage colour; sway, under which the screens of
reference are measured, does not.

## Decision

The owner's decision of 2026-11-11.

- **One trial of content from outside the tree**, shared by Aerith and Magnolia, two milestones,
  thrown away, with its report. It runs right after uwui's first material of glass.
  - On Linux: the surface of a child process, mpv for Aerith and a plugin's window for Magnolia,
    with input and focus forwarded and given back; its colour description; glass falling back over
    it; a transition that moves it.
  - An HDR plane under an SDR interface, on a compositor that manages colour (KWin or Mutter), glass
    over it brought down to SDR white
    ([ADR-0234](0234-uwuis-colour-model-before-its-api-is-stable.md)).
  - On the owner's Pixel: Media3 in a `SurfaceView` under uwui's surface, its rectangle following
    the interface.
  - Aerith's player screen measured as the trial's screen, against mpv alone, Stremio, Jellyfin's
    desktop client and Kodi. Variable refresh is checked by hand.
- **The television box in uwui's own Android trial**, as its third milestone
  ([ADR-0233](0233-uwuis-platforms-apples-chips-ios-and-the-tvs.md)).
- **The order:** Android with its milestone on the box; the web; the trial of content from outside,
  after the first glass; iOS, on a Mac; then the API declared stable, the element of
  [ADR-0235](0235-content-from-outside-the-tree.md) staying experimental.
- **The fifth screen of reference stays Opium's** song playing, in Glass: a screen of reference
  measures the owner's own program, and Aerith has none in the first stage.

## Alternatives considered

- **Aerith's trial as it asked**, at the end of the first stage, the box and VRR within: everything
  seen together, but likely to overflow, and too late to correct the glass.
- **Two trials**, Magnolia's and Aerith's apart: each narrower, but one mechanism tried twice.
- **No trial for Aerith**, its needs filed alone: nothing more, but an API declared stable without
  having met a video.
- **Aerith's player as the fifth screen of reference**: a harder screen, moving at each frame, but
  measured on a program that does not yet exist.

## Consequences

- uwui's first stage holds four trials; their milestones are cut at SlimeEngine's M6d.10, which
  weighs Aerith's list of needs with the others.
- The trial needs mpv and a plugin's window run as child processes in CI's headless compositors, and
  KWin or Mutter for its HDR part.
- Aerith and Magnolia each write the small compositor that their trial uses, or share it.
- Magnolia's list of needs and Aerith's are updated in their own repositories, by their sessions.
