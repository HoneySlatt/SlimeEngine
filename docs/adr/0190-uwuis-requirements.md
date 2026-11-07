# 0190. uwui's requirements

- Status: Accepted; amends [0170](0170-applications-on-the-ui-framework.md) (the system's
  integrations, media keys among them, in an optional layer of uwui; drawing without a GPU
  decided; Stage 6d starts once M6c.Q2e is done, Stage 6c paused; uwui's stages to its 1.0 before
  Stage 7), [0171](0171-effects-of-the-interface.md)
  (motion interrupted and driven by gestures, shared elements; effects lightened on battery),
  [0173](0173-the-platforms-of-applications.md) (uwui's own tool packages applications before Stage
  9b), [0175](0175-the-quality-bar.md) (applications' budgets of start, memory, size and changes
  seen; the owner's Steam Deck measures the handheld tier) and [0181](0181-uwuis-scope.md)
  (applications' lower tiers; Android without Kotlin; how agents reach an application; publication
  and contributions at 1.0); amended by [0194](0194-stage-6d-uwui-leaving-the-engine.md) (what Stage 6d
  builds; its decisions taken in M6d.1), [0197](0197-uwuis-repository-public-from-now.md) (the repository public
  from now; the name kept on crates.io), [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (the owner's desktop no
  longer a target; `uwui bundle` delegates; updates to be evaluated),
  [0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) (four state models, today's as
  the control), [0203](0203-pilots-replaced-pluto-and-a-daw.md) (Pluto's client and a DAW among the pilots; the
  state model chosen once three programs said what they ask of uwui) and
  [0205](0205-signals-change-uwuis-interface.md) (signals change the interface; live values in
  its tree) and [0209](0209-platforms-the-editor-and-agents.md) (games' interfaces on uwui's core
  with a layer of the engine's, confirmed at M6d.10)
- Date: 2026-11-06

## Context

[ADR-0181](0181-uwuis-scope.md) made uwui the base of every interface of the owner's, on the
desktop, Android and the web, and [`ROADMAP.md`](../ROADMAP.md#stage-6d--the-interface-for-applications--08)
aims Stage 6d at writing an application better than with GPUI and its library of components,
gpui-kit, across the whole framework. What uwui must be in the end, beyond the four programs of
its pilots, had not been asked.

The owner answered on 2026-11-06, one theme at a time, after a survey of their other projects:
Opium, the music player, whose inventory lists 113 needs; Slatty, the launcher; Magnolia, a media
client; the native client of pluto, a harness of agents; and their desktop's shell, Amane, today
on an outside framework that they patch.

## Decision

**What uwui is.** A public framework, adopted widely, judged against GPUI and gpui-kit, Flutter and
Jetpack Compose, SwiftUI with Liquid Glass, Electron and Tauri, Qt and Slint: better as a whole,
never on one feature or one benchmark alone. It is published, its repository, crates.io and a site,
at its 1.0; outside contributions open then.

**What it runs.** Beyond the four programs of the pilots: whole Android applications; web
applications; heavy creative tools; pluto's native client; and the owner's desktop, its shell, its
login and lock screens, and what the compositor Umbriel draws itself. Applications to come that
its design keeps in view: a client of messaging, an editor of code and notes, a manager of files, a
terminal, system tools and settings, 2D games and playful applications, tools of drawing and music.
Each target is a layer, a host or a crate of its own: none weighs on the core.

**How an application is written.** In Rust for its logic, and in a format of files for its layout,
both reloaded at once, Rust's code included, the state kept. How state changes the interface,
signals, views that say they changed as in GPUI, or messages as in Elm, is decided at Stage 6d's
start, the three measured on the screens of reference. A live inspector, as a browser's tools,
picks an element, shows and changes its style and layout, and measures its frames; agents use it
too. No visual designer.

**Its looks.** A design system of uwui's own, the same on every platform, the default. Shipped
themes: uwui's, light, dark and of high contrast; Pro, the editor's dense look; Material 3
Expressive; Glass. Three densities apply over any theme: compact, comfortable, and couch, for a
screen across the room driven by a gamepad. No imitation of each system's look; a gallery of the
community's themes comes later. Colours follow the system's light or dark, not its accent. A theme
may build a component differently, its shape and structure, not only recolour it. Geist and Geist
Mono are its fonts; Lucide its icons, those used alone embedded. No sounds.

**The system around it.** Notifications with an image, an icon in the system's tray, the system's
dialogs to choose and save files, dragging and dropping between applications both ways, Wayland
included, one instance brought to the front, deep links, files associated, a start with the
session, media keys and the system's player (MPRIS, Windows' media controls), shortcuts that work
without the focus, several windows, menus and tooltips as windows of the system, menus drawn by
uwui with the system's own as an option. Decorations: the system's by default, or a bar drawn by
the application. Transparent windows, blurred over the desktop where the compositor allows it,
opaque otherwise.

**Mobile and the web.** On Android, an application may be written wholly in Rust, in an activity
that uwui gives, Kotlin staying possible for the system's services; tablets and foldables, their
postures and rotations. iOS stays within reach, without a port. On the web: an application that
installs itself and works offline, with notifications; one that works without WebGPU, through
WebGL2; its text selected and copied as a page's, its links opened in a tab; uwui embedded in an
existing page, not only filling one. One interface's code serves the desktop, the phone and the
web, adapting to size and input, with screens of one platform where needed.

**Input and text.** Beyond the mouse, the keyboard, touch and gamepads: the stylus, its pressure,
tilt and eraser; the touchpad's gestures, pinching, rotating, swiping, scrolling with inertia; a
keyboard on screen driven by a gamepad; shortcuts that the user remaps, with chords. Every writing
system, colour emoji and fallback fonts, layouts mirrored for right-to-left, the system's size of
text honoured; translation built in, through Fluent, with plurals and formats of dates and numbers,
the language changed live. Text shown may be selected and copied, across elements, as the
application chooses. Edited text: a whole editor of code (Tree-sitter, LSP, several carets,
folding, a minimap), rich text, Markdown shown, and spelling checked.

**Images, media and drawing.** JPEG, PNG, WebP, GIF and SVG, animated images played, AVIF, JPEG XL
and HEIC, and Lottie's animations; video decoded by the GPU (VA-API, Direct3D 11, MediaCodec); HDR
and wide gamuts for images and videos from Stage 6d. A canvas on which an application draws paths,
shapes and images; charts, live ones too; a view drawn by the application's shader; a scene of
SlimeEngine in an element, built on the engine's side.

**Components.** A table of millions of rows, its columns sized, moved, sorted and filtered, its
header kept, rows selected and edited in place; a tree; a grid of thousands of thumbnails; shelves
and carousels; dates and times; colours; a search with suggestions; forms validated, masked fields
and numbers with units; docking of panels; an application's navigation, a sidebar, tabs at the
bottom of a phone, a stack of screens with its back and its transitions; a palette of commands;
drawers, sheets, dialogs, popovers, toasts and menus; a terminal; PDF shown; a map of tiles; the
system's web view; components for conversations with AIs, text that streams in, blocks of code,
attachments.

**Motion.** Beyond ADR-0171: animations interrupted without a jump, keeping their speed;
animations that a finger or the touchpad drives, as Android's predictive back; an element that
flies from one screen to the next; the physics of scrolling of each platform, at the display's
rate. On battery or in the system's saving mode, effects lighten by themselves, blur and motion
reduced and the rate capped, unless the application refuses.

**Data, work and extensions.** Settings kept where each system keeps them, written atomically,
versioned and migrated; the window's state restored, its size, place, panels, tabs and scroll; undo
and redo for applications. Work runs on two executors: one of uwui's own for the interface's tasks,
bound to the components that start them, cancelled with them, and run on time simulated in tests;
and an adapter, neutral, for heavy work, with an official one for tokio; the web and Android run
their own loops. Extensions in WebAssembly, isolated, add screens and components.

**Safety.** Content from outside, images, SVG, PDF, Markdown and videos, is read within limits of
size, memory and time, fuzzed in CI, and never crashes; decoders written in C run in a process of
their own. Applications ask for the least that they need, through the system's portals, Flatpak's
sandbox and Android's permissions. A secret is never copied, logged, nor shown in an agent's
capture, and the system's keyring keeps it.

**Accessibility and checks.** WCAG 2.2 at level AA, checked: screen readers on every platform, Orca,
Narrator, TalkBack and the web's; the whole application by the keyboard; contrasts checked in CI;
touch targets of 48 dp at least. An application is tested without a window by the paths of its
elements, by images of reference in CI for each theme and density, and by counters in CI, as the
engine's ([ADR-0186](0186-counters-and-ratios-in-ci.md)). Agents reach any application through a
channel and an MCP server of uwui's own, on the desktop, Android and the web; the engine keeps its
own for games. Agents write screens in the format of layouts and see them at once, and judge an
image against its reference or a mock-up.

**Tools and delivery.** A tool of uwui's own: `uwui new`, from templates; `uwui run` for Android and
the web; `uwui dev`, which reloads at once; `uwui bundle`, the packages; `uwui doctor`, the
environment checked. Packages for Linux, Flatpak, AppImage and Nix; for Windows, MSIX or a signed
installer, winget and the Store; for Android, APK and AAB; for the web, a static site with its
service worker. Updates signed and installed by the application, but in stores and Flatpak. A
crash leaves a report on the machine ([ADR-0176](0176-fuzzing-and-crash-reports.md)). uwui sends
nothing over the network of its own accord, ever: no telemetry. Its documentation: a book from
nothing to a whole application, a site whose demonstrations run in the browser, and small
applications finished, to read and copy.

**Its promises.** After its 1.0, semantic versioning kept strictly; a 2.0 comes with a tool that
migrates code and themes, after a deprecation announced. It builds with the stable Rust of the
last six months. Games built with SlimeEngine draw their whole interface with uwui, menus, HUD,
inventories and dialogues, driven by gamepads, and interfaces within the world too.

**Machines and budgets.** The lowest machines that applications must stay smooth on: on the desktop,
a Raspberry Pi 5 or an old computer without a Vulkan GPU, through a light renderer on the CPU, its
effects reduced; on phones, the entry-level phone of ADR-0181. The owner's very modest laptop is
measured first, before anything is bought; the owner's Pixel serves for phones; a Raspberry Pi is
decided later. The owner's Steam Deck measures the handheld tier. Frames follow the display, 60 to
240 a second and beyond, none at rest, the budget measured at 240 on the high tier. A typical
application on the high tier: warm to its first useful frame within 100 ms, at most 100 MB at rest,
a binary of at most 20 MB, and a change of its interface seen within 2 s.

**The core and the rest.** The core holds what every application uses and what may never be
missing: the tree, layout, styles and themes, text, focus, every kind of input, shortcuts, motion,
accessibility, the agents' surface and the interface's executor; the limits on content, the saving
of power and nothing drawn at rest cannot be turned off. By default, an application also has the
GPU's drawing, its platform's host, the library of components, a crate apart from the core, the
shipped themes and densities, the common images, translation, its settings, window's state and
undo, the canvas, the shader's view and the crash report. Optional, each a feature or a crate of
uwui's, held to the same bar and checked in CI: the renderer on the CPU; the hosts of a shell, a
compositor and login and lock screens; AVIF, JPEG XL, HEIC and Lottie; video and HDR; one crate
for the system's integrations; one crate for each heavy component, the editor of code, rich text,
Markdown, the terminal, PDF, the map, the web view, charts and conversations with AIs; the adapter
of tokio; extensions; updates. The inspector, the reloading and the MCP server are tools of
development, absent from a build that ships. The core never depends on an optional part.

**Order.** uwui is now the owner's main project, and SlimeEngine steps back. Stage 6c pauses once
M6c.Q2d and M6c.Q2e are done, each finished, its CI green; its other milestones stay as planned,
and resume after uwui's 1.0, the editor's on uwui's components. Stage 6d starts then: it builds
the foundations and what the four programs of its pilots need, the editor and the game kept
working, their checks and budgets run at each push. uwui's own stages then take the rest, in its
repository, to its 1.0. SlimeEngine's Stage 7 waits for the end of Stage 6c, after uwui's 1.0.
Quality comes first, whatever it takes. Before its 1.0, the same small application,
whole, not a benchmark, is written with GPUI, Flutter, Compose, Electron or Tauri, Slint and uwui,
and measured on the owner's machines: its lines, start, memory, frames and power.

## Alternatives considered

- **uwui for the owner's projects alone**: no cost of a public product, but nothing that forces it
  to be easy and complete for others.
- **Publishing at Stage 6d's move**: feedback sooner, but an API that breaks under strangers.
- **Rust alone, or a language of interfaces alone**: one way to write, but either no layout that
  reloads without compiling, or a language to learn for logic too.
- **The look of each system**: native, but never quite right, and work without end.
- **The system's integrations in the core**, or left to each application: the first weighs on the
  web and on applications that need none; the second rewrites MPRIS in each application.
- **Every component in the core**: one dependency, but a large core that every application pays for.
- **An executor of uwui's alone, or none**: the first leaves tokio's libraries needing a second, the
  second rewrites the glue in each application and loses time simulated in tests.
- **uwui's stages in parallel with the engine's**: both progress, but two tracks for the owner to
  follow; the owner chose uwui first.
- **Stage 6c finished first, release 0.7 included**: the editor whole sooner, but uwui, now the main
  project, months later, and the editor's last panels built on widgets of its own that uwui's
  components would then replace.
- **Measuring rivals by micro-benchmarks**: easy, and easy to tune for; a whole application is not.

## Consequences

- The work grows past Stage 6d: uwui's stages are cut in uwui's roadmap, once its repository
  exists, and SlimeEngine's release map moves the end of Stage 6c, release 0.7, and Stage 7 after
  uwui's 1.0.
- The engine's last release stays 0.6 meanwhile; the editor and Hunna Wood stay as M6c.Q2e leaves
  them, their checks by hand waiting with them. When Stage 6c resumes, the editor moves to uwui's
  1.0.
- Each new target, host or heavy component adds CI and checks by hand on its platform.
- HEIC needs a decoder of HEVC, whose patents are checked before it is added; video and HDR depend
  on each platform's decoders.
- Applications' budgets of start, memory, size and changes seen join the quality bar.
- Opium, Magnolia and other applications stop writing their own MPRIS and media controls once the
  system's crate exists.
- Decisions left for Stage 6d's start: how state changes the interface; the format of layouts,
  which settles the notation of UI documents that Stage 7 was to decide.
