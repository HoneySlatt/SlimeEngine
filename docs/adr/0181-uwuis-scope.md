# 0181. uwui's scope

- Status: Accepted; amends [0170](0170-applications-on-the-ui-framework.md) (uwui is the base of the
  owner's programs and of the engine's interfaces, in a repository of its own, which the engine
  depends on), [0173](0173-the-platforms-of-applications.md) (Android phones and applications on the
  web are aimed at; Android without condition; macOS and iOS within reach, without a port),
  [0178](0178-two-pilots-and-the-frameworks-name.md) (four programs ported before the API is
  declared stable), [0180](0180-one-canvas-and-the-frameworks-foundations.md) (where the shared
  foundations go), [0141](0141-style-sheets-and-themes.md) (uwui reads its documents with `serde`),
  [0146](0146-screen-readers.md) (the translation to AccessKit moves into uwui),
  [0011](0011-windowing-with-winit.md) (uwui's desktop host uses winit too),
  [0031](0031-dev-overlay-with-egui.md) (uwui's drawing uses wgpu) and
  [0080](0080-reflection-and-serialization.md) (uwui reads its documents with `serde`);
  amended by [0182](0182-the-engine-under-mit-or-apache-2-0.md) (the engine leaves MPL-2.0 too)
- Date: 2026-11-05

## Context

uwui, the engine's UI framework ([ADR-0179](0179-the-framework-named-uwui.md)), is to be the base of
every interface of the owner's: the editor's, games' and desktop applications'
([ADR-0170](0170-applications-on-the-ui-framework.md)), and also of applications on phones and on
the web. The owner decided its scope on 2026-11-05, and the questions that it left open.

What the code holds on that day, read for this decision:

- `slime-ui`, about 18 400 lines outside its tests, owns no loop and no window. A program gives it
  events, `Ui::pointer_moved`, `key`, `ime`, and asks it for a frame, `Ui::frame`; `needs_frame` and
  `next_change` say when the next one is due. Its focus moves by Tab, by the arrows to the nearest
  element, and by F6 between panes; modal roots keep it. Agents read and drive it without a window
  or a GPU ([ADR-0143](0143-what-agents-see-of-the-interface.md)).
- It reads its themes, style sheets, shortcuts and docks' layouts as TOML through `slime-reflect`
  and `slime-serialize`, in 7 of its 34 files; it depends on `slime-color` and `slime-input` too.
- Its drawing takes a whole `slime_render::Renderer`; the canvas reads its items from storage
  buffers in its vertex shader, which WebGL2 lacks, as many Mali GPUs do in GLES.
- `slime-platform` holds the loop, on winit, which `slime-app` runs; `slime-app` translates the
  interface's meaning into AccessKit's tree (1 380 lines), which `slime-platform`'s adapter,
  `accesskit_winit`, gives the system.
- No touch, gamepad or lifecycle event exists, nor any behaviour of a platform: the physics of a
  scroll, a long press, the handles of a selection, haptics, Cmd beside Ctrl.
- wgpu is built for Vulkan and Direct3D 12 alone; the renderer uses threads and the file system;
  system fonts come through fontconfig and DirectWrite, and the web has none.

## Decision

**Its role.** uwui is the base of the interfaces of all of the owner's projects: applications, and
the engine's editor and games.

**Targets.**

- Aimed at, each with a project behind it: the desktop on Linux and Windows; Android phones; web
  applications drawn in a canvas.
- Within reach, without a port: macOS and iOS. Nothing in the foundations excludes them, and no
  abstraction is built for them. uwui's CI builds and tests it, with its desktop host, on macOS, and
  builds its core for iOS.
- Out of scope: Android TV and Apple TV; consoles; Android Auto and CarPlay, interfaces that the
  system imposes, which a program's core and its native layer feed; web sites, which stay in HTML.
  uwui renders no HTML.

**uwui is embedded in a host.** Its core never owns the event loop: a host calls it, as the web
requires and as Android works. An explicit API of the host carries the surface, created, lost,
created again and resized, with its scale; input, the pointer, multi-touch, the keyboard and
gamepads; the bridge to the system's keyboard and its input methods, composition included; the
lifecycle, pause, resume and low memory; the bridge to accessibility, an AccessKit adapter for each
platform; the clipboard; and requests for frames, so that nothing is drawn at rest. A desktop host
in which uwui runs the whole program is one host among others, on winit, in uwui's repository; the
engine keeps `slime-platform` for games and the editor, so that two layers use winit, each with its
clipboard and its AccessKit adapter. On Android, the program stays a Kotlin program, which keeps its
lifecycle, services, media sessions, notifications and DRM; uwui draws in a surface that Kotlin
gives it, so that screens move one at a time. The core, its tree, layout, text, focus, meaning and
agents' surface, stays usable without a window or a GPU. A project's logic never depends on uwui.

**The same look everywhere, the platform's behaviours.** The look comes from themes in files, the
same on every platform. A small layer for each platform gives what its users expect: the physics of
a scroll and its bounce, Android's predictive back, a selection's handles and the long press,
haptics, and the conventions of shortcuts.

**Input from the foundations.** The mouse, the keyboard, touch and gamepads; one model of focus, its
order of Tab and its directions, which serves the keyboard, accessibility and a game's menus driven
by a gamepad.

**Its own repository; the engine depends on it.** uwui has a repository of its own, under the
project method: its rules, plan, roadmap with its quality bar, ADRs and CI. SlimeEngine depends on
uwui, never the reverse: what binds it to the engine, drawing over a game's image or its HUD, lives
on the engine's side or in an adapter.

- **Agents and applications, an open question.** The core keeps its agents' surface. How an agent
  reaches an application on uwui, on the desktop, Android and the web, where the engine's
  development channel and MCP server cannot come since uwui does not depend on the engine, is
  decided at Stage 6d's start: the channel moved with uwui, an adapter, or a protocol of uwui's.
- **Its documents.** uwui reads and writes its themes, style sheets, shortcuts and layouts with
  `serde` and `toml`, behind a feature; without it, a program writes its theme in code. The files
  stay TOML. `slime-reflect` and `slime-serialize` stay in the engine, which may give uwui's types
  reflection in an adapter of its own, the day its editor needs it.
- **What leaves with it.** `slime-color` and `slime-input`, which the engine then takes from uwui;
  the canvas, out of `slime-render`; the translation of meaning into AccessKit's tree, out of
  `slime-app`.
- **Its GPU layer.** uwui draws on wgpu directly: `uwui-render` takes the host's device, its queue
  and a target, as `egui-wgpu` and Zed's GPUI on Linux and the web do. `slime-gfx` stays in the
  engine, which hands uwui its device; at Stage 9c, where the engine draws through native backends
  ([ADR-0098](0098-the-games-the-engine-aims-at.md)), it hands a wgpu device made over its own,
  which that stage checks. A native renderer may be added behind the same API if a measure on a
  lower tier asks for it.
- **Its backends.** Vulkan on Android and WebGPU on the web first; GLES and WebGL2 as their
  fallbacks, through one path: the canvas gives its items as instances of vertex buffers, which
  every backend takes, its transforms with them, measured before and after at 3840 × 2160 and 240
  frames a second. The canvas uses neither threads nor files, which the web lacks.
- **Accessibility on the web.** An AccessKit adapter for the web, written in uwui, keeps a hidden
  tree of the page's elements with ARIA roles, for what is in view; the fields that the system's
  keyboard needs live in the same tree. It is offered to AccessKit once proven, with the owner's
  agreement.
- **Its license: MIT OR Apache-2.0**, the Rust ecosystem's, which GPL-2.0 and GPL-3.0 programs can
  use. The engine stays under MPL-2.0. uwui's code and that of the crates that leave with it are
  committed under the owner's identity alone, with no code copied from elsewhere found; the fonts
  and icons that it ships keep their own licenses
  ([ADR-0133](0133-font-files-under-the-open-font-license.md)); the canvas's shadows, which follow
  vello's computation ([ADR-0137](0137-the-canvas.md)), keep vello's notice if their code derives
  from it, which the move checks.
- **Its history.** The repository is made with `git filter-repo` from a clone, keeping uwui's files
  and following their former paths. The ADRs of the interface become its first ADRs; SlimeEngine's
  stay as dated.

**Trials before the API is declared stable**, short, bounded to two milestones each, and thrown
away: they are not ports to keep. Each ends with a written report and corrections of the API; one
that overflows stops and reports.

- **Android**: uwui in a Kotlin activity; the surface lost and made again, rotation; the system's
  keyboard with composition; suspend and resume; scrolling by touch; TalkBack; frame times and power
  on the owner's phone, in a release build.
- **The web**: WebAssembly in a canvas that the page gives; a loop that the host drives; the
  system's keyboard through a hidden field; a change of the pixels' density; nothing drawn at rest;
  the package's size and the start.

**Four programs before the API is declared stable**, in this order: the music player on the desktop;
Slatty, the launcher; the music player's Android prototype, measured against Jetpack Compose; a
media application. Each runs at least as well as with its present framework, on figures measured
first. On Android, uwui becomes the music player's interface whatever the prototype shows, and is
fixed until it matches Compose.

**Lower tiers of applications**, unmeasured, none being in the owner's hands:

- the desktop: a laptop of 2015, two cores, an Intel HD 520, 4 GB of memory, a 1920 × 1080 screen at
  60 Hz;
- the phone: an entry-level Android 14 phone, of the class of a Helio G99, 4 GB of memory, a screen
  at 90 Hz;
- the web: both of them in a browser, Chrome and Firefox on the laptop, Chrome on the phone, the
  first load over Chrome's "slow 4G" profile.

**The package for the web**: at most 1 MB of WebAssembly compressed with brotli, fonts apart and
loaded when needed; provisional, confirmed or corrected after the web's trial.

**When.** The crates are renamed now, in milestone M6c.N. Stage 6d decouples uwui inside this
repository, where a change of both sides is one commit, moves it, then runs the trials and the
ports. Its first milestones, as cut from the code on 2026-11-05, which the stage's plan refines:

1. **Documents with `serde`**: themes, style sheets, shortcuts and layouts read and written by
   `serde` and `toml`, the same files read as before; `slime-reflect` and `slime-serialize` gone
   from uwui.
2. **The canvas in uwui, on wgpu**: the canvas moved into `uwui-render`, drawing with a wgpu device
   and a target, its items as instances; the engine composing it over its views; measured before and
   after.
3. **The host's API, and the desktop host**: the surface, its scale, input with touch and gamepads,
   input methods, the lifecycle, the clipboard, requests for frames; the translation to AccessKit in
   uwui; a desktop host on winit; `slime-app` a host of uwui; a check in CI that a program on uwui
   compiles neither the renderer, the world nor the editor.
4. **The move**: uwui's repository, its method, its CI on Linux, Windows and macOS, its core and
   `uwui-render` built for the web, Android and iOS; its license; `slime-color` and `slime-input`
   with it; the engine depending on it.
5. **Touch, gamepads and the platforms' behaviours**: multi-touch, a scroll's physics and bounce,
   the long press and a selection's handles, haptics, a primary modifier, gamepads in the focus.
6. **Fonts without the system's**: fonts given as bytes and loaded when a text first needs them,
   with a fallback for each writing system, for Android and the web.
7. **The Android trial**, two milestones; **the web trial**, two milestones.

Then the drawing, the images, the motion and the text that ADR-0170 and
[ADR-0171](0171-effects-of-the-interface.md) give the stage, cut at its start, and the four ports.

## Alternatives considered

- **uwui owning the loop on every platform**, as GPUI does: one way to run, but the web drives its
  own loop and Android its lifecycle; a Kotlin program could not move a screen at a time.
- **Reflection and serialization leaving with uwui**, or a third repository of foundations: no
  second reader of TOML, but the engine's core changing in another repository, which a change of the
  world or of saves would cross.
- **`slime-gfx` leaving with uwui**: no second GPU layer, but the engine's, which Stage 9c reshapes,
  in a repository of the interface's.
- **Two paths in the canvas**, storage buffers and a path for WebGL, as GPUI keeps: the fastest path
  where it exists, two to keep alike.
- **Vulkan and WebGPU alone**: one path as today, but a browser without WebGPU shows nothing.
- **A tree of the page's elements of uwui's own**, built from its meaning without AccessKit: all of
  ARIA, but a second translation of the meaning to keep.
- **Android only if uwui matches Compose**, as ADR-0173 decided, in whole or screen by screen: the
  user never served worse, but Android left to Compose for as long as uwui does not match.
- **Apache-2.0 alone**: a patent grant, but GPL-2.0 programs left out, and apart from the
  ecosystem's custom. **MPL-2.0 kept**: compatible with GPL-3.0 already, but a copyleft that
  companies avoid.
- **A repository made fresh**, its history left in SlimeEngine; or `git subtree split`, which keeps
  one directory and follows no rename.
- **The crates renamed at the move**: one change instead of two, but the decoupling done under the
  old names, and the move mixing two operations.

## Consequences

- The engine's interfaces and games draw with uwui, which a repository of its own versions.
- Stage 6d grows: the decoupling, the move, two trials and four ports come before its API is
  declared stable.
- The engine keeps its own platform layer over winit for games and the editor; uwui's desktop host
  is another, for applications.
- The music player's own plan changes: it is ported during the stage.
- Until the move, two crates of this repository use winit, `slime-platform` and uwui's desktop host,
  and `serde` enters through uwui.
