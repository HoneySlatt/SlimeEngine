# 0266. uwui targets the owner's shell, its lock and login screens, and what Yuna draws

- Status: Accepted; amends [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (the owner's
  shell, its login and lock screens and what its compositor draws are uwui's targets again)
- Date: 2026-11-14

## Context

[ADR-0199](0199-uwuis-targets-tools-and-references-narrowed.md) narrowed uwui on 2026-11-06: neither
the owner's shell, nor its login and lock screens, nor what the compositor draws itself, so that
uwui's core would not be pulled towards the system. Its other decisions, M6d.1's comparisons, the
measures from outside, uwui's own counters, the matrix of platforms and the tools, are not touched
here.

Since then, the owner's compositor has a project of its own, Yuna, and its own interfaces have
grown: a picker of what to share, a bar while capturing, the bar of tabbed columns, an overview and
Alt and Tab. Without a toolkit, Yuna would lay out text and icons again by itself. uwui is built to
be hosted: the host lends its device, its queue, an encoder and a target
([ADR-0244](0244-the-engine-lends-its-device-through-a-guarded-door.md)). The owner's shell has a
name, Hollow: public, on Yuna alone, optional above it.

Yuna records its side in its own ADR-0063, *Vulkan alone, and uwui for what Yuna draws and for
Hollow* (github.com/HoneySlatt/Yuna, `docs/adr/0063-vulkan-alone-and-uwui-for-what-yuna-draws.md`,
2026-11-14): Yuna draws with Vulkan alone and hosts uwui on its own device.

## Decision

The owner's decision of 2026-11-14, made in a session of Yuna's design and confirmed for this
repository the same day: **the first decision of ADR-0199 is reversed.** uwui targets again:

- the owner's shell, Hollow, on Yuna alone;
- its login and lock screens;
- what the compositor draws itself: Yuna hosts uwui on its own Vulkan device for its picker of
  what to share, its bar while capturing, its bar of tabs, its overview and its Alt and Tab, under
  three rules of Yuna's:
  - a failure of uwui inside Yuna drops the interface and builds it again, never takes Yuna down;
  - uwui's time in each of Yuna's frames is counted and checked in CI;
  - over a game in full screen, uwui draws nothing unless the person opens an interface.

ADR-0199's other decisions stand.

## Alternatives considered

Yuna's ADR-0063 weighs them: Yuna's interfaces without uwui, a small toolkit written again inside
the compositor; in a process of their own, which the overview and Alt and Tab cannot be, since they
need every window's image live; Hollow on uwui with Yuna's interfaces decided later, two ways of
drawing meanwhile; another toolkit for Hollow, outside the owner's stack.

## Consequences

- uwui gains three hosts and their targets: a shell's surfaces through Wayland's layer shell, a
  lock screen's through the session lock, and a compositor that lends its own device, a Vulkan
  device of its own rather than one that uwui opens. Their needs are in
  [`uwui-needs.md`](../uwui-needs.md#what-the-owners-shell-and-compositor-ask), apart from the
  engine's; their cost is uwui's.
- uwui's roadmap, in its own repository, no longer excludes the owner's desktop, its shell, its
  login and lock screens nor its compositor.
- ADR-0199's concern holds as a rule for these hosts: what a shell or a compositor needs reaches
  uwui's core only as a general need of hosts, as the engine's do
  ([ADR-0190](0190-uwuis-requirements.md)); the rest stays in Hollow and in Yuna.
- `uwui-render` draws with a wgpu device today; a host that lends a Vulkan device of its own asks
  either that wgpu be made over that device or a path of drawing that takes it, a decision of
  uwui's when the first such host is built.
