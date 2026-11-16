# 0285. Every pointer named in the host's events; the tree following each with touch

- Status: Accepted
- Date: 2026-11-16

## Context

uwui is to take, beyond the mouse, several fingers, a stylus with its pressure, tilt and eraser,
the touchpad's gestures and scrolling with inertia ([ADR-0181](0181-uwuis-scope.md),
[ADR-0190](0190-uwuis-requirements.md)); touch is uwui's first stage's
([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)).

On 2026-11-16, the host's events know one pointer without an identity: `Input::PointerMoved`,
`PointerLeft` and `PointerButton` ([ADR-0249](0249-the-hosts-api-its-items-and-their-table.md),
[ADR-0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md)). The tree
keeps one hover, one press and one capture for each target. Track B writes the platforms' layers
of uwui's desktop host now, Wayland's, then X11's, Win32's and AppKit's
([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)),
each translating its platform's pointer into these events; the engine's bridge translates winit's.

## Decision

The owner's decision of 2026-11-16.

**Now, the shape.** Each pointer event of the host's API carries the pointer's identity and its
device: the mouse, a finger, a stylus with its pressure and tilt. The mouse is pointer 0. Each
platform's layer fills them from the start, Track B's next milestone of M6d.7a2 first, and the
engine's bridge fills the mouse's. The tree follows one pointer meanwhile, the primary one: the
mouse, or the first finger down.

**With touch, the behaviour.** In uwui's first stage, before its library of components: hover,
press and capture kept for each pointer, the touchpad's and the fingers' gestures recognised by
uwui, and scrolling with inertia. M6d.10 places the milestone.

## Alternatives considered

- **The whole model now:** right on every platform at once, but a large work that holds Track B
  back.
- **Touch read as the mouse,** one finger, several later: simple, but wrong on phones, and the
  model changed again when several fingers come.

## Consequences

- Each platform's translation of its pointer is written once, against the final shape.
- Until uwui's first stage, a second finger on a screen is not followed.
- The library of components is written on a tree that follows each pointer.

## Validation and reconsideration

Read: the host's pointer events in `crates/uwui/src/host.rs`; ADR-0181 and ADR-0190's input.
Assumed: that pressure and tilt cover what the platforms' styluses give that uwui needs.

Proposing another decision is justified if a platform's pointer carries something that the shape
cannot hold without breaking it.
