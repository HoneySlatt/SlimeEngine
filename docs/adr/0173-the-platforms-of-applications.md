# 0173. The platforms of applications

- Status: Accepted; amends [0015](0015-platforms-and-ci.md) (CI builds and tests the framework and
  the shell of applications on macOS from Stage 6d, without supporting it),
  [0098](0098-the-games-the-engine-aims-at.md) (macOS supported once a Mac can check it; wgpu stays
  for macOS after Stage 9c), [0100](0100-updates-of-shipped-games.md) (Stage 9b's packages and
  updates serve applications too) and [0170](0170-applications-on-the-ui-framework.md) (the
  platforms that Stage 6d aims at);
  amended by [0181](0181-uwuis-scope.md) (Android phones and the web aimed at; Android without
  condition)
- Date: 2026-11-04

## Context

[ADR-0170](0170-applications-on-the-ui-framework.md) made applications the third client of the
engine's UI framework. The owner wants it as portable as Electron makes a program, without
Electron's cost: Electron ships a whole browser and its JavaScript with each program.

What the framework stands on reaches further than the engine's two platforms, Linux and Windows
([ADR-0015](0015-platforms-and-ci.md)): winit opens windows on macOS, Android and the web; wgpu
draws through Metal, Vulkan, Direct3D 12 and WebGPU; parley and fontique find the system's fonts
on macOS; AccessKit has adapters for macOS and Android; the dialogs and the clipboard that
`slime-platform` uses run on macOS. The framework draws itself, so an interface looks the same
everywhere, as Electron's do. What the roadmap held:

- macOS supported at Stage 11, with the question at Stage 9c of whether wgpu stays, macOS among
  its uses, once the engine has native backends, Vulkan and Direct3D 12, of which macOS has neither
  ([ADR-0098](0098-the-games-the-engine-aims-at.md));
- builds packaged, and shipped games updated, at Stage 9b, for games, on Linux and Windows
  ([ADR-0100](0100-updates-of-shipped-games.md));
- no mobile platform.

The owner has no Mac and plans none soon. Nobody can then check by hand that a program works on
macOS, as the owner checks Windows in a virtual machine, and macOS runs in a virtual machine
lawfully only on Apple's hardware. GitHub's machines build and test on macOS, free for a public
repository.

Electron does not aim at the web: its programs are web pages, which a browser also runs. A
framework that draws in a canvas, as Flutter's web target does, is a lesser citizen of the web:
screen readers need a hidden tree of HTML, which AccessKit does not build; selecting, copying,
typing through an input method, finding in the page and the browser's translation all go through
HTML; the first visit downloads megabytes of WebAssembly and fonts, the system's fonts being out of
reach; threads need a server configured for them, and there is no file system. The launcher and the
music player of ADR-0170 run games installed on the machine and play its files.

## Decision

The owner's decisions of 2026-11-04.

**macOS kept within reach.** From Stage 6d, CI builds and tests the framework and the shell of
applications on macOS, the tests that need a GPU as far as GitHub's machines allow, so that a change
that breaks macOS shows at once. The documentation says it plainly: they build and pass their tests
on macOS, which is not supported.

**macOS supported once a Mac can check it.** The day the owner has a Mac, a short step brings what
support needs: checks by hand of windowed behaviour, and macOS's package, signed and notarized by
Apple, which needs an Apple developer account. It may come before Stage 11.

**wgpu stays for macOS.** Stage 9c's native backends do not drop macOS: wgpu, through Metal, keeps
drawing there. Whether it also stays as a fallback or for debugging is still Stage 9c's question.

**Packages and updates for applications too, at Stage 9b.** The packages and updates that Stage 9b
builds for games, on Linux and Windows, are designed for applications as well: one tool, made once.
Until then, each application packages itself, as the launcher and the music player already do.

**Neither the web nor mobile platforms for now.** The web is not aimed at: an ADR decides it the
day a project needs it. Android waits for a measurement: after Stage 6d, a prototype of the music
player's screens drawn by the framework on a phone is measured against the same screens in Jetpack
Compose, Android's own toolkit, in smoothness, start-up and power; a stage brings applications to
Android only if the framework matches or beats it.

## Alternatives considered

- **macOS supported at Stage 6d**, or in a stage of portability right after it: nobody could check
  it by hand, and support that nobody checks is a promise the project cannot keep.
- **macOS left to Stage 11 without a check in CI**: nothing to maintain until then, but a year of
  changes that nobody builds on macOS, each a fix to find later.
- **Packages and updates brought forward to a stage after 6d**: applications packaged by the
  engine sooner, but without a Mac only for Linux and Windows, which the two applications already
  package themselves; the game would wait a stage more for that.
- **A check that the framework builds for the web**: it would prepare a platform that no plan
  holds, which [AGENTS.md](../../AGENTS.md#scope) forbids.

## Consequences

- Stage 6d adds a job on macOS to CI, within the owner's limit on a job's time, and its failures to
  fix.
- macOS's support waits for a Mac, not for Stage 11.
- Stage 9b designs its packages and updates for programs that are not games.
- The roadmap records the web and Android as decisions to take later, the second after a
  measurement.
