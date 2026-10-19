# 0052. Amend the roadmap before 0.1

- Status: Accepted; amends [0031](0031-dev-overlay-with-egui.md) (when the dev overlay is removed);
  amended by [0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and
  [0064](0064-cascaded-shadow-maps.md) (Stage 9a's content),
  [0067](0067-hunna-wood-replaces-nightwatch.md) (the sample game replaced), and
  [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (culling and instancing
  moved to Stage 6, and the release map)
- Date: 2026-10-14

## Context

Before releasing 0.1, the known debt was audited, and two reviews of the engine by readers from
outside the project compared [`docs/ROADMAP.md`](../ROADMAP.md) with the games the engine aims
at: large narrative and open-world 3D games, but also stylized 3D games and 2D games. They found
five gaps.

1. The vision says "a 3D creation platform", and no stage mentions 2D.
2. The renderer's work left for "a later stage" has no stage: cascaded shadow maps, the shadows of
   point and spot lights, many lights, transparency, anti-aliasing, culling, instancing, fog,
   ambient occlusion, decals, a day-night cycle. Stage 10 covers rendering at very large scale
   only, and the narrative vertical slice of Stage 9 needs most of them.
3. Duties that the audit found for Stages 4 and 5 are written nowhere: recovering from a lost GPU
   device, which stops the run today ([ADR-0022](0022-losing-the-gpu-device.md)); loading threads
   that create GPU objects; the precision of world positions, `f32` in the API, which
   [ADR-0048](0048-rendering-relative-to-the-camera.md) left to Stage 5.
4. The first playable sample comes at Stage 7, after the asset formats (Stage 4), the world model
   (Stage 5) and the editor (Stage 6) are designed. A scene that is only displayed does not show
   what a game finds hard: moving and creating objects, interacting with them, restarting.
5. The roadmap fixes an engine-owned, GPU-rendered UI framework for the editor, with no criteria,
   though it is close to a product of its own (text, input methods, layout, docking, focus); and it
   never mentions the UI of games: menus, HUD, text.

The owner decided on 2026-10-14 on each, as below.

## Decision

The roadmap is amended as follows.

- **2D and stylized games.** The vision puts 3D first and makes 2D and stylized games with the same
  renderer, through pieces that the stages add where they are needed: an orthographic camera,
  sprites, transparency, unlit and custom materials, crisp sampling for pixel art. There is no
  separate 2D renderer.
- **Stage 3c, a first game**, right after 0.1: a small game that uses the public API only. The
  player moves, interacts with a few objects and reaches an objective, and the game restarts
  cleanly; no text, physics or sound, which the engine does not have yet. What the game finds hard
  guides Stages 4 and 5. From then on, the sample each stage ends with is that game, grown by the
  stage, and the exit criteria of the later stages name it. The stage has no release of its own.
- **Stage 9 is split in two.** Stage 9a, Production renderer, gathers the renderer's work above
  and the pieces for 2D and stylized games that no earlier stage needed. Stage 9b, Content
  production, is the former Stage 9. Stage 10 keeps rendering at very large scale.
- **Stage numbers.** The stages added take letters, as Stage 3's halves did, so that the stage
  numbers that documents and ADRs cite stay right; a reference to Stage 9 now means its two halves,
  as one to Stage 3 does. Releases follow the stages: 0.7 for Stage 9a, 0.8 for 9b, 0.9 for Stage
  10, and 0.10, then 1.0, for Stage 11.
- **Stage 4** recovers from a lost device, creating the GPU objects again from their sources, and
  creates GPU objects from loading threads, through the `Gpu` they share (it is `Send` and `Sync`
  already).
- **Stage 5** decides, with the transform hierarchy, whether world positions stay `f32`; rendering
  is relative to the camera already (ADR-0048).
- **The editor's UI moves from the fixed decisions to the open ones**, decided at Stage 6 on
  criteria. The editor starts on egui, which the engine's tooling uses already
  ([ADR-0031](0031-dev-overlay-with-egui.md)); an engine-owned, GPU-rendered framework is written
  only for a measured need: performance, a limit of egui that the editor meets, or reuse for the
  UI of games. Neither uses Qt or GTK. The dev overlay is removed only once what replaces it does
  at least as well.
- **The UI of games** (menus, HUD, text) is a topic of its own, built at Stage 7 with what else a
  game needs beyond rendering; whether it shares the editor's framework is decided at Stage 6, with
  the editor's.

This amends ADR-0031, whose dev overlay was to be deleted when Stage 6's UI framework replaced it.

## Alternatives considered

- **A separate 2D renderer**, with 2D physics and tiles: close to a second engine. **3D only**:
  leaves 2D games out of the project.
- **Spreading the renderer's work over the existing stages** (transparency and anti-aliasing for
  the editor at Stage 6, shadows at Stage 9): the renderer's progress is scattered and harder to
  follow. **Leaving it unplanned**: the question comes back before Stage 9 anyway.
- **Renumbering the stages** after an insertion: the numbers are cited across the documents and the
  ADRs, which would become wrong.
- **A growing game from Stage 4 on, without Stage 3c**: no delay, but the first feedback on the
  API arrives while Stage 4's choices are being made. **The first playable sample at Stage 7**, as
  before: the risk that the reviews pointed out.
- **An engine-owned UI for the editor from Stage 6**, as fixed before: Stage 6 grows by close to a
  product, and the editor waits for it. **egui for the editor for good**: gives up a UI shared by
  the editor and games before anything is measured.

## Consequences

- A few days between 0.1 and Stage 4; the game stays very simple until the engine has text,
  physics and sound.
- One more stage before 0.7; Stage 11's first release becomes 0.10.
- The editor may depend on egui for a long time, with its look and its limits, and moving to an
  engine-owned framework later would cost work; in exchange, the editor does not wait for the
  engine to write text, layout and docking of its own.
- `slime-devui` is no longer deleted at Stage 6 by default: its README, the glossary and the
  roadmap say when it is.
