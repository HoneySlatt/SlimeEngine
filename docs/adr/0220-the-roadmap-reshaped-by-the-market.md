# 0220. The roadmap reshaped by the market

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (stages split and their
  contents), [0190](0190-uwuis-requirements.md) and [0194](0194-stage-6d-uwui-leaving-the-engine.md)
  (a stage of measures before Stage 6c resumes; Stage 6d's milestones widened) and
  [0207](0207-what-slimeengine-is-to-become.md) (CI's runners on the Deck and a desktop GPU before
  Stage 7)
- Date: 2026-11-07

## Context

[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) to
[ADR-0219](0219-test-games-in-place-of-hunna-wood.md) measure the engine and uwui against the
market, rebuild light and geometry, reshape the open world, join the agents' systems and replace
*Hunna Wood* by test games. A review of the code and of the roadmap against them found:

- Every new bar is judged against rivals measured first, with tools that do not exist: a game
  measured from outside, a bench for the light sensor, a tool of blind tests, the rivals' editors
  and compiling, scenes rebuilt in Unreal.
- Stage 7 grows by the criteria of physics, sound and AI, menus' foundations and tester agents;
  Stage 9a by the architecture of light, virtual geometry, learned inference, the reference path
  tracer first, stylised tools and realism; Stage 9b by faces, animation in steps, the menus'
  timeline and the first game sold; Stage 10a by destruction, generation, the data's foundations and
  the test worlds.
- Stage 6d's next milestones would lock limits that the decisions remove: a theme that cannot change
  a component's structure, colours drawn in silence, no primary modifier for Cmd, a host's API that
  fits neither UIKit nor AppKit, a canvas prototype without backdrops, groups or transforms, two
  systems of agents.
- Changing a game's code and running it again takes about 3.1 s in a debug build, against the budget
  of 2 s: 1.9 s to build, as each engine change is compiled once for the game's features and once
  for the editor's, and 1.15 s for Play to give control.

## Decision

The owner's decisions of 2026-11-07.

**Stage 6e — the market measured,** after uwui's stages and before Stage 6c resumes, as M6d.0 was for
uwui: *Hunna Wood* moved out and `games/open-world` in its place; a tool that measures a game on PC
from outside, on the owner's machine and the Steam Deck; the light sensor's bench; a tool of blind
tests; the rivals' editors on the same level and their compiling; the first figures of each. No
release of its own.

**Before Stage 7:** the scenes rebuilt in Unreal for physics, sound and animation; CI's runners on
the Steam Deck and on a desktop GPU; the ADR of the shader's three layers.

**Stages split when they are planned:**

- Stage 7 in three: execution, determinism and replays, the profiler and the runners; physics,
  riding and animation's core; sound, input, games' interfaces and accessibility.
- Stage 9a in three: the foundations, compute, the raised floor, shader layers, the open graph, the
  sky on the GPU, the reference path tracer and the measure of closeness to it, virtual geometry,
  learned inference and the backends decided by measure; dynamic light per tier, many lights, the
  denoiser, temporal anti-aliasing and the upscaler, the mode at 120 frames a second; looks and
  subjects, stylised tools, skin, eyes, hair, transparency, fog, decals, the day-night cycle, HDR
  output, the blind tests and the test of signature.
- Stage 9b with the chain of production first, the material graph, packages, updates and Steam, so
  that the first game sold does not wait; then characters, faces and AI; then story, sequencer,
  menus' timeline and effects.
- Stage 10a in two: streaming and the data's foundations; then destruction, generation in play,
  terrain, vegetation, water, weather and crowds.

**Exit criteria** take the figures of ADR-0215 to ADR-0217: each stage that touches rendering ends
with its blind tests and its test of signature; Stage 7 with its criteria measured against Unreal's
scenes and a clean run of the tester agents; Stage 10a with the test worlds crossed and destruction
and generation that replay and save the same; Stage 10b with the network's criteria and both in
sync online; 0.14 with the licence and the contributors' agreement in force and the agent bench run.

**Compiling, now:** one set of features for the whole workspace, so that an engine change is
compiled once; dependencies optimised in debug builds; a faster linker chosen by measure and put to
the owner as a dependency; a profile for shipping with optimisation across crates; the times of
compiling measured at each milestone.

**Stage 6d's next milestones widened:**

- M6d.1d: what a theme is, values, rules and the components' structure; an application started in
  one call names its look, blank included.
- M6d.2: styles typed per component, with parts that a theme extends, and no colour hidden in the
  defaults.
- M6d.3: a primary modifier, Ctrl or Cmd, in the format of shortcuts, so that users' shortcuts
  migrate once; a menu's shortcut shown per platform.
- M6d.4: the canvas's prototype holds a blur of what lies behind, a group drawn apart, a transform
  per item and a path; its ADR records the canvas's passes.
- M6d.5: the host's API checked, item by item, against winit, Android, the web, UIKit and AppKit: a
  surface per root, input methods both ways, insets, the system's preferences, an application's
  events, a model of menus for macOS's menu bar.
- M6d.7: a blank window among the programs measured; the host's local crash reports.
- M6d.9 in two, as [ADR-0218](0218-one-core-for-agents-in-uwui.md) decides.
- M6d.10: every decision of ADR-0213 to ADR-0219 weighed for uwui's first stage, the services of
  files, settings, network and long work among them, with ADR-0211 amended for the web; who owns
  commands, undo and their journal.
- M6d.11: the editor's look moved into the official theme Pro.

**Stage 6c, when it resumes:** each step carries its author and time; M6c.Q4 counts hours of use,
catches crashes that are not panics, keeps a journal that a torn write does not stop, and a test in
CI kills the editor at random and finds no work lost.

**Before the first agent bench:** a command that makes a new game and writes its `AGENTS.md`, and
cooking that works outside this repository. **At Stage 7:** games get the registry of operations
that the editor has, and input actions that tester agents can drive.

## Alternatives considered

- **The measures left to each stage:** each stage judged against rivals measured late, or not at
  all, and its techniques chosen before their bar exists.
- **The measures before uwui's stages,** right after Stage 6d: uwui's first stage waits for tools
  that it does not need.
- **The stages kept whole:** milestones planned against stages twice their size, and the first game
  sold waiting for all of Stage 9b.
- **Stage 6d's milestones kept as planned:** typed styles, shortcuts, the host's API and the canvas
  made once, then made again for themes, macOS, iOS and effects.
- **Compiling left as it is until measured worse:** the budget of 2 s already missed.

## Consequences

- [`ROADMAP.md`](../ROADMAP.md) holds Stage 6e, the splits, the exit criteria and the tracks;
  [`PLAN.md`](../PLAN.md) the widened milestones of Stage 6d; [`stage-6c.md`](../stage-6c.md) what
  Stage 6c gains when it resumes.
- The budget of 2 s from a game's change to it running again is debt until compiling's changes
  land.
- Stage 6c resumes later than planned, after Stage 6e.
