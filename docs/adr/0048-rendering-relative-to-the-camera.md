# 0048. Rendering relative to the camera

- Status: Accepted; amends [0027](0027-the-render-crate.md)
- Date: 2026-10-14

## Context

The scene shader worked in world space: each vertex was moved there by its instance's transform,
then projected by the camera's view and projection, and the shading took differences of world
positions (towards the camera, towards a point light), all in 32-bit floats. An `f32` holds 24 bits
of a number: 10 km from the origin, its steps are 1 mm, and the difference of two positions near
each other, there, carries that error. Drawn from 30 cm away into 64 × 64 pixels, a scene 10 km
from the origin differed from the same scene at the origin in 1 236 of the output's channels,
some lit at the origin and black far from it. Open worlds, which the engine aims at, put the
camera kilometres from the origin.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 to render relative
to the camera now, and to decide at Stage 5 whether the API's positions stay `f32`.

## Decision

- The renderer draws in world space moved so that the camera's eye is at the origin, with world
  space's axes: the frame's space. Before anything reaches the GPU, the CPU subtracts the eye in
  `f64` from each instance's transform (`translate(−eye) × transform`), from each point and spot
  light's position, and from the boxes that the shadow map is fitted to. Two `f32` positions near
  each other then differ by what their own values hold, however far from the origin they are.
- The camera's view in that space is its rotation alone, from the direction to its target
  computed in `f64`; the inverse of the view and projection gives the sky's directions directly.
  The shader no longer receives the camera's position: the camera is at the origin.
- The shadow map's light view is centred on the camera too, so its depths are computed from
  numbers of the scene's size, not of its distance to the origin.
- The API is unchanged: positions are world positions in `f32`, as `Camera::eye`,
  `Instance::transform` and the lights give them. An `f32` holds them in steps of 1 mm at 10 km,
  6 cm at 1 000 km; the renderer keeps what the scene holds, and Stage 5, which brings the world
  model, decides whether positions need more.

This amends [ADR-0027](0027-the-render-crate.md): the shader's space is no longer world space.

## Alternatives considered

- **Moving the origin of the world (origin rebasing).** The game shifts every position when the
  camera strays too far, as some engines do; every system that holds positions must follow, and
  the renderer still loses precision between two shifts.
- **`f64` positions in the shaders.** Not available on every GPU wgpu supports, and slower where
  it is.
- **Subtracting the eye on the GPU.** The subtraction of two large `f32`s is where the precision is
  lost: it must happen before the numbers become `f32`s near each other, on the CPU in `f64`.
- **Waiting for Stage 5.** The renderer's part does not depend on how the world stores positions,
  and costs one subtraction per instance and light; games written before would draw wrong far from
  the origin.

## Consequences

- A scene 10 km from the origin is drawn exactly as the same scene at the origin, when its
  positions are the same numbers there, as `tests/precision.rs` checks bit for bit.
- Each frame does a few `f64` operations per instance and light, negligible beside the rest of
  the frame's CPU work.
- The shader's positions are no longer world positions; a shader effect that needs world
  coordinates (a texture projected in world space, fog by altitude) must add the eye back, which
  the frame's values would then have to carry.
