# 0047. Reversed depth with an infinite far plane

- Status: Accepted; amends [0026](0026-resource-binding-and-depth.md),
  [0027](0027-the-render-crate.md), [0033](0033-gpu-layer-for-multi-pass-rendering.md) and
  [0038](0038-shadow-mapping.md); amended by [0126](0126-views-apart-from-the-renderer.md) (an
  orthographic camera's far plane)
- Date: 2026-10-14

## Context

Depth ran from 0 at the camera's near plane to 1 at its far plane, in 32-bit floats
([ADR-0026](0026-resource-binding-and-depth.md), [ADR-0027](0027-the-render-crate.md)). A
perspective projection gives depth `far (d − near) / (d (far − near))` at distance d: it crowds
every distance beyond a few times `near` into the last fraction below 1, where floats are coarsest.
With a near plane at 0.1 m and a far plane at 10 km, two surfaces 5 km away must be about 15 m
apart for their depths to differ, so that nearer ones fight (z-fighting), and the far plane hides
whatever lies beyond it. The games the engine aims at, open worlds seen up to the horizon, need
neither limit; the examples' orbit camera even fitted its planes to each scene to keep enough
precision.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 on reversed depth
with an infinite far plane, the convention of Unreal and Frostbite, before the API is used by
games, and asked that the shadow map's bias be checked on RADV and WARP.

## Decision

- **Reversed depth everywhere.** Depth runs from 1, the nearest, to 0, the farthest. `slime-gfx`
  clears depth targets to 0; `DepthCompare` has `Greater`, `GreaterEqual` and `Always` (`Less` and
  `LessEqual` are gone, since they would never pass against a cleared target); a negative
  `DepthBias` pushes a surface away. There is one convention, for the camera and the shadow map
  alike.
- **An infinite far plane.** `Camera::far` is removed: `Camera::projection` is glam's
  `perspective_infinite_reverse` (WebGPU's clip space, +Y up), whose depth is `near / d`. A float
  keeps the same relative precision from 1 down to 10⁻³⁸, so depths resolve distances to about one
  part in 8 million of the distance, at any distance, whatever `near` is: 0.6 mm at 5 km. `near`
  only clips what is nearer.
- **The sky.** The sky pass draws its triangle at depth 0, at infinity, and keeps it where the
  depth target still holds 0 (`GreaterEqual`). The inverse of the projection is finite: a point
  of depth 0 unprojects to a direction (w = 0), which the shader uses as it is.
- **The shadow map.** Its orthographic projection runs from 1 at the point nearest to the light to
  0 at the farthest; the shadow pass tests with `Greater`, the comparison sampler is
  `GreaterEqual` (lit where nothing nearer to the light is stored), and the bias is
  `DepthBias { constant: -32, slope_scale: -4.0 }`. The constant part was 16 in
  [ADR-0038](0038-shadow-mapping.md): its unit is the last bit of the depth, which is finer near 0,
  where reversing puts the surfaces farthest from the light, the ground under what casts shadows.
  With a floor at that end of the map, the smallest constant without acne went from 4 to 8 on
  lavapipe, the strictest driver measured; 32 keeps the margin of four that 16 had. The test of
  acne now places the floor there.

This amends ADR-0026 (depth targets cleared to the farthest depth, now 0), ADR-0027 (the depth
convention and the camera's far plane), ADR-0033 (the depth comparisons and the bias's sign) and
ADR-0038 (the shadow map's depth, comparison and bias).

## Alternatives considered

- **Keeping depth from 0 to 1 with a finite far plane.** Precision falls with the square of the
  distance, and games must choose a far plane that clips their horizon.
- **Reversed depth with a finite far plane.** It keeps the same precision but still clips at
  `far`, a value games must choose and that nothing needs.
- **A shadow map that keeps depth from 0 to 1.** An orthographic depth is linear, so reversing it
  gains nothing, and the ground would keep the coarser unit of the bias. But `slime-gfx` would need
  a second clear value and the comparisons both ways, two conventions where the larger constant
  costs nothing visible: 32 units near the light are 2 × 10⁻⁶ of the map's depth range.
- **A 24-bit depth format.** Fixed-point depths do not gain from reversing, and their precision
  depends on the driver ([ADR-0026](0026-resource-binding-and-depth.md)).

## Consequences

- Distant surfaces no longer fight, nothing is clipped by distance, and the camera has one plane
  to choose. The examples' orbit camera keeps a near plane of a thousandth of the scene's radius.
- Code that wrote depths, depth tests or biases must reverse them; a `Camera` literal that set
  `far` no longer compiles. The images do not change beyond rounding: the image tests pass against
  the same references.
- The shadow map's bias depends on how its depth range is fitted: a change of the fit must check
  it again on lavapipe, RADV and WARP.
