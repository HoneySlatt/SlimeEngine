# Changelog

Notable changes to SlimeEngine, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are git tags (`v0.0.1`, ...);
nothing is promised stable before 1.0 (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## [Unreleased]

### Added

- What agents see of the interface (ADR-0143): `Ui::meaning` gives, when asked, each element that
  means something, its role (`slime_ui::tree::Role`, named as AccessKit names roles), its name, its
  states, what can be done to it now (`ElementAction`), where it shows and its path, `#0/bar/open`
  (`Ui::path_text`, `Ui::element_by_path`); `Ui::meaning_at` finds it at a pixel, and a builder
  gives an element its role and its name (`Element::role`, `Element::named`). A presentation gives
  its interface (`Presentation::ui`), which the development channel reads, acts on and captures,
  and the MCP server's tools `find_elements`, `element_at`, `act_on_interface` and
  `capture_element` give agents. The example `interface` counts the presses of its bar's buttons in
  its world, through commands.
- Input and focus for the interface (ADR-0142): `Ui` takes a person's input, the pointer, the
  wheel, the modifiers and the keys, and turns it into actions (`slime_ui::action`), a button
  pressed, an intent of navigation, the command of a shortcut, which the program reads from a queue
  (`Ui::take_actions`) and an agent sends (`Ui::act`). Buttons are hovered, pressed and focused; Tab
  moves the focus in the order of the tree and the arrows to the nearest element on screen. A root
  may stand at a place of its own (`RootSettings::origin`) or be modal (`RootSettings::modal`),
  keeping the input and the focus while it is open. Shortcuts are tables of chords of the keys'
  meaning that name commands in scopes (`slime_ui::shortcuts::Shortcuts`, `Ui::set_shortcuts`),
  which the cooker cooks from `.keys` files and `CookedAssets::shortcuts` reads. A new crate,
  `slime-input`, holds the engine's vocabulary of input, which `slime-platform` translates winit's
  events into. The example `interface` is used with the pointer and the keyboard, opens a modal
  dialog, and switches its theme by a shortcut; `--dialog` opens the dialog first.
- Style sheets and themes for the interface (ADR-0141): `slime_ui::theme::StyleSheet` holds rules
  that select elements by kind, classes, key, states and ancestor, in the order of their file, and
  set a part of their style; `Theme` holds the named values that the rules give as `"$name"`;
  colors are written in sRGB, `"#rrggbb"`. `Ui::set_style` styles an interface with them, each
  element's own style over the rules, and styles it again when they change, a document at fault
  refused by name and place. Elements hold classes (`Element::class`). The cooker cooks `.style`
  and `.theme` files, which `CookedAssets::style_sheet` and `CookedAssets::theme` read.
  `slime_serialize::References::value` gives the value that a text stands for. The example
  `interface` is styled by a style sheet and a dark or a light theme, switched with T, and styled
  again with `--watch` as they change; the benchmark `ui.tree.editor` measures a theme changed.
- The interface's tree of elements (ADR-0140): `slime_ui::tree::Ui` keeps an interface's roots, each
  of a size, a scale and a direction, and their elements from one frame to the next. `Ui::build`
  runs a builder when the revision it reads moved, matching the elements it adds with those before
  by their keys (`Children::add`, `Children::add_slot` for a panel built by its own revision);
  `Ui::frame` lays out by flexbox and grid, with taffy, the roots where something changed, mirrored
  for a root laid out from right to left, draws again what changed or moved, and gives the display
  list. Its elements are boxes, labels, images, buttons that show their states, boxes that scroll and
  lists that build only the rows that show; their styles are typed, in logical pixels, with start
  and end sides (`slime_ui::style`). The example `interface` builds its interface as a tree, with a
  list of 100 000 rows, and lays it out from right to left with `--right-to-left`; the benchmarks
  `ui.tree.editor` and `ui.tree.list.100k` measure a tree of the editor's size and the list.
- The interface's text drawn by the GPU (ADR-0139): `slime_ui::display::DisplayList`, what an
  interface draws in a frame as data, boxes, shadows, texts laid out and images named by keys, each
  with its clip; and `slime-ui-render`, a new crate, whose `UiRenderer` draws a display list
  through the renderer's canvas, the glyphs of its texts kept in an atlas within a budget
  (`GLYPH_ATLAS_BUDGET`), the least recently used leaving first. `Gpu::write_texture`, which writes
  a part of a texture. The example `interface` draws from a display list, with a sentence of each
  of eight writing systems; the benchmark `ui.draw.menu` draws a game's menu of real text.
- The interface's canvas (ADR-0137): `slime_render::Canvas`, which a client fills each frame with
  boxes, their corners rounded and their borders inside their edges (`CanvasBox`), the shadows that
  boxes cast (`CanvasShadow`), and parts of images (`ImageQuad`), textures of colors, textures of
  coverage drawn in a color, or the target that a view drew, each item clipped; and
  `Renderer::draw_canvas`, which draws it over a target after the views and reports its items, its
  draws and its GPU time (`CanvasStats`). Into an sRGB target, its colors mix as their sRGB values
  do, as design tools mix them. The example `interface` draws one over a window or into a PNG file,
  a view's image in a panel; the benchmark `render.canvas` draws an editor's screen and a game's
  menu at the hardware tiers' resolutions.
- `TextureFormat::R8Unorm`, a texture of one channel; `Blend::Premultiplied`, which lays colors
  already multiplied by their alpha over a target; `TextureFormat::Bgra8Unorm`,
  `TextureFormat::without_srgb` and `RenderTarget::without_srgb`, an sRGB target, a window's image
  included, seen without its encoding; `slime_color::linear_to_srgb`.
- The interface's text, without a GPU (ADR-0136): `slime-ui`, a new crate, holds it, the UI
  framework's first part. `slime_ui::text::Fonts` registers fonts from their bytes, which a style
  names in a stack, and falls back on the system's where a program asks for them
  (`Fonts::with_system_fonts`), the font found for a writing system named for it from then on;
  `Fonts::lay_out` shapes, orders, breaks and aligns a text in every writing system, with parley,
  and cuts it with an ellipsis to its box (`TextBounds`); the `TextLayout` gives its lines, its
  glyph runs, the cluster under a point, the position nearest it, carets and selections;
  `GlyphRasterizer` draws each glyph's image, coverage or color, from its `GlyphKey`, with swash;
  `TextCache` keeps laid-out text within a budget of memory. The example `text` writes a sentence of
  eight writing systems to an image. The benchmarks `ui.text.labels.1k`, `ui.text.paragraphs` and
  `ui.text.raster`. The fonts of the tests, parts of Noto fonts under the SIL Open Font License, and
  `scripts/subset-fonts.sh`, which makes them.
- The editor's program (ADR-0135): `slime_editor::run`, an application of the engine without a
  window, which opens a level, keeps its journal in the user's data directory, recovers by itself
  what the journal holds that the level's file does not, as a crash or a stop without writing
  leaves it, and answers the tools that edit the level through the development channel. The
  example `editor` runs it on the examples' yard, and `games/hunnawood-editor`, *Hunna Wood*'s
  editor package, on the game's zone. The development channel's protocol gains the editor's
  requests (`Request::Edit`); the MCP server gains `edit`, `undo`, `redo`, `select`,
  `editor_history`, `editor_journal` and `write_level`.
- `Simulation::apply`, through which a host applies the commands that it receives, and
  `Simulation::edit`, which answers an editor's requests; `AppConfig::pace_headless`, which paces a
  run without a window; `slime_app::data_directory`, `slime_app::HeldFile` and
  `slime_app::set_aside`, the user's data directory, a file that one program at a time holds, and a
  file set aside under a name that nothing has, which *Hunna Wood* had of its own.
- The editor's core, without its interface (ADR-0134): `slime-editor`, a new crate.
  `Editor::open` opens a level, a scene file, into a world, under the key that the game spawns it
  under; `Editor::apply` and `apply_text` change it through steps of commands, given as values or
  as text, each step made whole or not at all; `undo` and `redo` undo and make them again, within a
  bound of memory (`HISTORY_BUDGET`), and select again what was selected before or after them
  (`select`, `selection`); `keep_journal` writes the steps to a journal as they are made, from
  which `Editor::recover` recovers an editor that crashed, with its history; `write` writes the
  level back, whole or not at all, its comments kept, never over a file changed apart from the
  editor. The editor refuses a command that would change what is not under its level's root.
- The world's commands are the editor's (ADR-0134): `Command::Create`, `Place`, `Delete`,
  `Reparent`, `Insert`, `Remove` and `Restore` beside `SetProperty`, each naming its entities by
  their persistent identities. A deletion records what it takes out (`Deleted`), its values
  referring to entities by identity, and `Restore` creates it again under the same identities, at
  the same places, the references of other entities pointing at it again. `Command::write` and
  `Command::read` give a command's text, which `Identities` names entities in, and
  `Command::entity` the entity that a command changes.
- `World::spawn_as`, `World::place_as` and `World::new_id` create an entity, or place a prefab,
  under an identity of a scene, `zone/torch-0rdx`, as a scene file names what it holds;
  `World::set_parent_at` puts a child at a rank among its siblings, and `World::rank` gives it.
- `slime_serialize::keep_comments` puts the comments of a document written by hand back into the
  document written in its place.
- The benchmark `world.undo.1k`: a subtree of 1 000 entities deleted then restored.

### Changed

- The development channel's protocol is at version 8, with the editor's and the interface's
  requests. `slime-app` depends on `slime-ui`, and `slime_ui::tree::Element` gains `role` and
  `name`.
- `slime-platform`'s input types live in `slime-input`, which it re-exports: their paths through
  `slime-platform` are unchanged. `RootSettings` gains `origin` and `modal`.
  `slime_ui::theme::StyleError` is `slime_ui::DocumentError`, which tables of shortcuts share; the
  cooker's `CookError::Style` is `CookError::Interface`, and `STYLE_COOKER_VERSION` is
  `INTERFACE_COOKER_VERSION`. In the example `interface`, Escape closes the dialog, and stops the
  program only when nothing is open.
- A button's looks in its states are rules of a style sheet: `ButtonStates` is gone, and
  `Element::button` takes a label and a style.
- `slime_ui::Rect`, at the crate's root, gives a rectangle by its top left corner and its size, as
  the canvas does: the text's carets and selections give it so, where they gave edges.
- `TextCache::get` gives its texts shared, as `Arc<TextLayout>`; `TextLayout` can be cloned.
- The high hardware tier draws at 3840 × 2160 (ADR-0138), and the render benchmarks measure it
  there, rather than at 2560 × 1440.
- A window whose images cannot be seen without their sRGB encoding, Vulkan without
  `VK_KHR_swapchain_mutable_format`, is refused (`GfxError::SurfaceWithoutEncoding`).
- `World::apply` returns the command that undoes the one it applied.
- `slime_cook::write`, which writes a file whole or not at all, is public.
- A reference to an entity that a tool writes `""` in the development channel reads as no entity.
- The commands that a host refuses are handed back with an `AppError` (`Context::refused`).
- The commands that a host receives, from its presentation, the dev overlay or a tool's
  `set_property`, go through `Simulation::apply`, so that a simulation that applies them its own
  way, as the editor does, receives them all.
- A tool of an earlier version of the development channel's protocol is refused by a game of this
  one, and the reverse.

## [0.5.0] - 2026-10-22

The eighth release, which closes Stage 6a of the roadmap, what the editor stands on, with the two
passes that came before it. An audit of 0.4 found defects in saves and identities, in the readers
of files and in the recovery from a lost GPU device: the audit pass fixed them, made the quick wins
that it measured and put the engine's measurements in CI, and the foundations pass decided as ADRs
what the editor will freeze. Stage 6a built it. The renderer leaves out what the camera does not
see and draws the instances of one mesh and one material together; one task pool runs the engine's
work, and loads copy to the GPU within a budget, so that they no longer slow the frames; the world
knows what changed since a reader last read; a frame owns what it draws. A game is a simulation,
its rules, and a presentation, what the player sees, on a game time that pauses and steps, and an
AI agent drives a running game through its development channel and the engine's MCP server. Saves
hold what changed, scene by scene; a level unloaded keeps what the run changed of it, and an entity
adopted outlives its level. One renderer draws several views, each with its settings; assets are
asked for by requests that carry a priority, and kept within budgets of memory; the renderer draws
lines and unlit materials, which may blend, and says which object is drawn at a pixel, as the CPU
does by casting a ray. The saves of 0.4 are refused, and *Hunna Wood* sets them aside; every
texture is cooked again once. The Windows programs that CI builds are attached to the release on
GitHub.

### Added

- Picking (ADR-0130): `View::pick(area)` returns a `Pick`, whose `take` gives, a frame or more
  later, the number of the instance drawn nearest at each pixel of the area (`Picked`), each
  instance's `Instance::pick`, which `slime-world-render`'s frame builder makes its entity's handle
  (`Entity::to_bits`, `Entity::from_bits`). `slime_world_render::Picker` picks by the CPU, a ray
  that `Camera::ray` gives against the meshes that a world places. A tool picks at a pixel of a
  game's view (`Presentation::pick`, `PickAnswer`, `Request::Pick`), through the MCP server's `pick`
  tool too; the `world` example selects what a click picks.
- `TextureFormat::Rg32Uint`, render targets of integers, and `Gpu::read_render_target_later`, whose
  `Readback` gives a target's texels without waiting for the GPU.
- Unlit materials (ADR-0129): `Renderer::create_unlit_material` makes, from `UnlitData`, a material
  whose color, times its texture, shows as it is whatever the light and the exposure, masked or
  blended (`AlphaMode::Blend`), casting no shadows; its parameters are `UnlitValues`.
- What a client draws into a view besides the scene (ADR-0129): `ViewDraw::drawings` gives a view
  `Drawings`, `Line`s a width in pixels wide and instances in the scene, hidden by what is in front
  of them, and lines over the image; `RenderStats::after_sky_gpu_time` and `over_image_gpu_time`
  time them. The `world` example draws a grid and the lights' icons, and the box of the entity that
  Tab selects.
- `slime_gfx::Blend`: a pipeline lays its colors over its target by their alpha. `Mesh::bounds`
  gives a mesh's box.
- Asset requests (ADR-0113, ADR-0128): `AssetCache::request(gpu, ids, priority)` returns a
  `Request`, whose meshes arrive together, and whose handles (`Request::into_handles`,
  `AssetHandle`) keep each mesh, or keep it loading, while they live, and say what became of it
  (`AssetState`). The loads' tasks and their copies to the GPU go by class, `Priority::Now`, `Soon`
  or `Later`, then in the order the loads started, and a handle's priority changes while it lives. A
  load that nothing holds any more stops without waiting; a mesh that no handle holds stays while
  its kind's budget has room (`Budgets`, by default the handheld tier's), and goes, least recently
  held first, once it has none. `AssetCache::report` says what the cache holds, loads and drops
  (`CacheReport`). After a lost device, the cache's requests (`AssetCache::into_requests`) go to the
  new cache (`AssetCache::with_requests`): the handles stay valid. `slime-world-render`'s frame
  builder holds a handle of each mesh that the world names.
- A game's reports (ADR-0128): `Presentation::reports` gives named values, described by reflection,
  `assets` and `views` in the `world` example and *Hunna Wood*; the development channel's protocol
  lists and reads them (`Request::Report`), and the MCP server's `read_report`
  gives them. `RenderStats` and `std::time::Duration`, in seconds, are described by reflection.
- `slime_gfx::UploadRank`: the uploader submits the copies of the lowest rank first, a rank changed
  while they wait included; `Texture::bytes` gives a texture's size.
- The settings of a view (ADR-0127): `ViewSettings`, which `View::set_settings` changes, gives a
  view its render scale, from 0.25 to 2, its anti-aliasing, none or four samples
  (`AntiAliasing`), a mode of debugging that shows the normals or the shadows (`DebugMode`), and
  whether it draws the shadows, the sky and the bloom. `View::render_size` gives the size it draws
  at. The `world` example's views panel, on F3, and *Hunna Wood*'s dev overlay, on F1, change their
  views' settings, with `slime_devui::edit_reflected`, which edits any reflected value.
- A tool reads and changes a view's settings: `Presentation::view_settings` and
  `Presentation::set_view_settings` hand them over as `Reflected`; the development channel's
  protocol lists each view of the game's state with its settings (`ViewSummary`) and changes them
  (`Request::SetViewSettings`), and the MCP server's `set_view_settings` tool changes them. *Hunna
  Wood* names its view `hero`.
- Views apart from the renderer (ADR-0111, ADR-0126): `Renderer::create_view` makes a `View`, a
  camera's image drawn into an output, which a game holds from frame to frame, and
  `Renderer::render(gpu, &scene, &mut [ViewDraw])` draws a scene through several in a frame, each
  with its camera, its target and, as a `Viewport`, the rectangle of the target that it draws into.
  The views share the renderer's pipelines, meshes and shadow atlases: four views of 1280 × 720
  with the sun take about 60 % of what four renderers took (`docs/measurements.md`). A view's
  statistics are its own (`View::stats`).
- Orthographic cameras and parts of an image: `Camera::projection` is a `Projection`, in
  perspective or orthographic, and `Camera::part`, an `ImagePart`, draws a part of the camera's
  image, for a split screen or a magnifier. The `world` example draws a map of the yard from above
  in a corner of its window.
- `Gpu::memory_in_use`: the GPU memory that the device's objects take, as wgpu counts it on Vulkan
  and Direct3D 12.
- A tool captures one view of a running game: `Presentation::views` names the views that a frame
  drew, the development channel's protocol lists them in the game's state and captures one by
  its name, and the MCP server's `capture` takes the view's name.
- Scenes unloaded and adoption (ADR-0112, ADR-0125): `World::unload_scene` destroys a scene and
  keeps what the run changed of it, which spawning the same scene under its key again and settling
  it applies; `slime_world::Persistence`, on a scene's root or on an entity, makes what the run
  changed of it reset instead, `Persistence = "Reset"` in a scene file. `World::adopt` takes an
  entity of a scene out of it under a key of its own, its former identity kept, so that it outlives
  its level.
- Saving and loading a running game from a tool: `Simulation::save` and `Simulation::load`, which
  *Hunna Wood* implements, answer the development channel's `Request::Save` and `Request::Load`, and
  the MCP server's `save_game` and `load_game` tools.
- Driven components (ADR-0108, ADR-0124): an entity marks with `slime_world::Driven` the
  components that a system computes again, `Driven = ["Transform"]` in a scene file; saves and
  scene files written back leave them as their author wrote them, and `World::first_difference`
  leaves out their values. The `world` example's floating ring is marked so.
- The development channel (ADR-0110, ADR-0122): a game built with `slime-app`'s `development`
  feature, on by default in *Hunna Wood*, always on in the examples, and started with a token in
  `SLIME_DEV_TOKEN`, listens on `127.0.0.1` and answers the tools that connect with the token: its
  state and its frames' times; pause, speed, steps and stop; its entities and their components,
  read and set by persistent identity through reflection; keys put down and let go; its window's
  image; the last lines of its log. Started without a token, it opens no port; built with
  `--no-default-features`, it holds no channel. The new `slime-channel` crate holds the protocol,
  at version 6, and both of its ends: `Listener` for the game, `launch` and `Client` for the tools.
- `slime-mcp`, the engine's MCP server: an agent starts a game through it and drives it through
  its development channel, sees its image as a PNG image, cooks assets and runs the tests
  ([`crates/slime-mcp/README.md`](crates/slime-mcp/README.md)).
- `Context::hold`, a hold of game time apart from the pause, which a game sets while it is not ready
  to be played; *Hunna Wood* holds its time while its zone loads, so that a pause set meanwhile is
  kept once the zone is drawn.
- `Gpu::read_render_target` reads a window's image back where the window's system allows it
  (`GfxError::NotReadable` otherwise). `KeyCode::name` and
  `KeyCode::from_name` give a key's code as text and back; `Registry::type_at` gives the type of
  the part of a value at a path; `slime_serialize::read_binary` reads a value from the binary
  encoding.
- Game time (ADR-0107, ADR-0121): a game's simulation runs on a time of its own, which the host
  slows down or hastens (`Context::set_speed`), pauses (`Context::pause`, `Context::resume`) and
  steps while paused (`Context::step`), its fixed steps keeping their duration at every speed;
  `slime_app::time::Time` gives each phase real time and game time (`GameTime`).
- `slime_app::Host`, which owns a game's world, its two times, its input and its commands, and runs
  the first phases of a frame (`Host::frame`): a test, or one day a server, runs a game's rules with
  it, without a window or a GPU, pressing keys by script.
- `slime_platform::Input`, the input as data: the keys and mouse buttons held, and those pressed
  and released since the last frame, which the host keeps from the window's events.
- `slime_world::Command`, a change of the world that names its entity by its persistent identity,
  applied by `World::apply` (`WorldError::NotFound` when no entity has the identity); a
  presentation sends them with `Context::send`, and the host applies them at the start of the next
  frame (`Context::refused` hands back the refusals).
- *Hunna Wood*'s dev overlay pauses game time and lets it run again on F3, runs one step of it on
  F4 while it is paused, and sets its speed with a slider of the statistics (F1); the camera turns
  while the game is paused.
- Kinds of materials (ADR-0111): each material is of a kind, which gives the renderer the shader of
  its passes, its materials' group and its parameters' type, the physically based kind alone,
  which draws as before. `Material::kind` names it; `Material::parameters` and
  `Material::set_parameters` read and change a material's parameters through reflection
  (`MaterialValues` is reflected), and `Renderer::describe_materials` registers their types for
  an editor.
- The world knows what changed (ADR-0108, ADR-0119): each value of a component holds the ticks at
  which it was added and last changed, `World::tick` gives the world's tick, which a reader keeps,
  and `World::changed` the components of a type written or added since, skipping the blocks of
  values that did not change: 1 000 lights changed among 100 000 found in 18 µs, where comparing
  every light with a copy took 3.4 ms. `World::removed` and `World::destroyed` tell what left
  since, `World::structure` when entities last came, went, changed parent or name, and
  `World::ticks` a component's ticks. `Mut::set_if_differs` writes and stamps only a value that
  differs.
- `world.changes.100k`, a benchmark of readers of 100 000 entities of which 1 % changed.
- `slime-tasks`, the engine's task pool: as many threads as the machine's logical cores but one,
  which run tasks of two priorities, `Frame` and `Background`, a free thread taking a frame task
  first and background tasks never holding every thread; `slime_tasks::map` computes items in
  parallel, the calling thread taking its share.
- `slime_gfx::Uploader`, the one owner of the copies to the GPU of what loads bring: textures and
  ranges of vertex and index buffers written into staging buffers by the threads that load,
  reused from one upload to the next, and their copies submitted by a thread of the uploader's,
  within a budget of bytes per frame that `Uploader::frame` renews.
- `loading.textures`, a benchmark of sixteen textures of 4096² loaded while frames are drawn; the
  loading benchmarks give the longest frame while their meshes load.
- `World::spawn_keyed` creates an entity whose persistent identity is a key that the game chooses:
  what a game creates in code while it makes its start world, named so that saves and hot reload
  find it whatever else the game creates, and in whichever order. A run that creates one under a
  key saves it, and loading creates it again.
- `Range::holds` checks a number against a field's range in the number's own type, and `Number`
  holds a number so, which `Value::number` gives of a value that is one.
- `SceneLight::check` says whether the engine draws a light, and `slime_render::Light::check`
  whether the renderer draws one where it is placed: the one rule of a valid light. The limits that
  bound what a file can make a reader take: `slime_assets::MAX_SIDE`, the longest side of a cooked
  texture; `slime_serialize::MAX_VALUES`, the most values of a document, with
  `ReadProblem::TooMany`; `slime_world::MAX_SCENE_SIZE`, the most bytes of a resolved scene.
- `SLIME_TRACE=<file>` (`slime_app::TRACE_ENV_VAR`) makes a program that calls `init_diagnostics`
  write every span it enters and leaves into the file, in the Chrome Trace Event format, which
  Perfetto and `chrome://tracing` open: the phases of each frame, and each submission to the GPU.
- The engine's benchmarks, `cargo run --release -p slime-benches`, and `scripts/measure.sh`, which
  times the build and runs them; `docs/measurements.md` keeps their figures from one milestone to
  the next.
- `scripts/check.sh headless` and `scripts/check.sh windowed` run each part of the checks, the
  checks that need no compositor and the runs in headless Weston compositors, which CI runs as two
  jobs side by side (ADR-0123); without an argument, it runs both.
- The programs that draw the dev overlay, the examples that have one and *Hunna Wood*, embed
  egui's fonts, and travel with their licences, as the Ubuntu Font Licence and the Open Font
  License ask: a game that ships the overlay ships them too (ADR-0105).
- `Gpu::write_uniform_buffer_start` writes the start of a uniform buffer and leaves the rest as it
  is.
- The renderer leaves out of the scene pass the instances whose boxes the camera does not see
  (frustum culling), and draws the instances of one pipeline, material and mesh that a pass keeps
  with one draw call (instancing): a frame's CPU time follows what each pass sees
  ([`docs/measurements.md`](docs/measurements.md)). `RenderStats::instances` counts the instances
  that the scene pass drew. Past what the device's storage buffers hold the values of, the last
  instances are left out with a warning, where `render` panicked inside wgpu.
- `slime_gfx::StorageBuffer`, which shaders read at a `BindingType::StorageBuffer` binding;
  `Gpu::create_zeroed_vertex_buffer`, `create_zeroed_index_buffer`, `write_vertex_buffer`,
  `write_index_buffer` and `write_storage_buffer`, which write buffers by ranges; and
  `Gpu::max_storage_buffer_size`.
- The render benchmarks draw at the screens of the hardware tiers, 1280 × 800, 1920 × 1080 and
  2560 × 1440, and give the sum of the GPU times of a frame's passes.

### Changed

- `Instance` has a `pick`, the number that picking gives of it.
- `FragmentState` has a `blend`; `ShaderSources` holds `unlit.wgsl` and
  `lines.wgsl`; `AlphaMode` has `Blend`, which a physically based material refuses until Stage 9a.
- A mesh that the asset cache cannot read, or whose material or texture it cannot, fails alone: the
  others asked for with it arrive. `AssetCache::poll` takes the GPU and returns each failed
  mesh with its error, as `Frame::errors` holds them; `AssetCache::request` takes a priority and
  returns the request to keep. The asset cache's meshes take pages of their own.
- `Scene::new(instances)` takes no camera: a view gives it (`ViewDraw`), and every view draws the
  scene's lights, those that the renderer can draw from every view's camera.
  `Camera::fov_y` is `Projection::Perspective { fov_y }`. `Renderer::render` returns no statistics:
  they are each view's. `slime_world_render::Frame::scene` takes the point that the lights
  beyond those drawn are nearest to, rather than a camera.
- A view whose size changes keeps drawing into its targets, its image stretched over its
  rectangle, until the new size has held still for `RESIZE_DELAY`, 200 ms; it then frees its old
  targets, once the GPU has finished the frames that use them, before it makes new ones: a window's
  edge dragged makes them once, the GPU never holds both, and a size that does not fit leaves the
  view without targets.
- Saves per scene (ADR-0125): a game settles its world once it made the start of a run, and again
  once it prepared a scene that it spawned since (`World::settle`); `World::save(about)` writes what
  the run changed since, without a start world, and costs what changed: about a hundred times less
  for a level of 100 000 entities of which 1 % moved (`docs/measurements.md`). `World::load` takes a
  callback that prepares the scenes that a save spawns, and `World::carry` no longer takes the start
  of the scenes before they changed. What the run created outside scenes takes, when a save loads,
  the numbers that follow the world's count, so that an update of the game that creates one more
  entity at the start no longer makes a save refused, nor moves or destroys another entity. Saves
  take layout 5: the saves of layouts 1 to 4, those of 0.3, of 0.4 and of the versions since, are
  refused (`DocumentError::Layout`), and *Hunna Wood* sets such a save aside as
  `save.toml.earlier-version` and starts a new run; its fixture is made again.
- `World::spawn_scene` under a key that holds a scene unloads that one first, rather than refusing,
  with `WorldError::IdentityInUse`, while an entity of it lives; a level that keeps something of the
  one before adopts it.
- Saving refuses a start that created, without a key, entities that saves hold
  (`DocumentError::UnkeyedAtStart`, which names them; ADR-0109, ADR-0124): a start names what it
  creates with `World::spawn_keyed`, a prefab with `World::spawn_scene`, or marks it `Unsaved`.
- A game is a simulation and a presentation (ADR-0107, ADR-0121): `slime_app::App` gave way to
  `Simulation`, a game's rules, whose `start`, `fixed_step`, `update` and `stop` receive the world
  to change and a `SimulationContext` (the input, both times, the run's stop), and `Presentation`,
  what the player sees, whose `start`, `extract` and `gpu_restored` receive the world as `&World`,
  and `on_event`, `render`, `gpu_lost` and `stop` do not. `run(&mut simulation, &mut presentation,
  &config)` runs both; `()` stands for the part a program lacks, and a headless run calls no method
  of the presentation. `Presentation::on_event` returns whether the presentation takes the event,
  which the simulation's input then does not see. The world is the host's: `Context::world`,
  `Context::world_mut` and `Context::fixed_step_alpha` are gone, the fraction of a step is
  `GameTime::alpha`, and the profiling span `fixed_update` is `fixed_step`, with `extract` before
  `acquire`.
- The dev overlay builds its panels with `Overlay::build(size, ui)`, as a presentation extracts its
  frame, and draws them with `Overlay::render(gpu, target)`. The world panel returns commands
  (`slime_world::Command`) in place of `Edit`, and finds the entity it shows again by its identity
  when the world is replaced.
- `slime_world_render::Frame::lights` pairs each light with its entity.
- *Hunna Wood* is a library and its program (ADR-0110): its rules, `HunnaWood`, and what the player
  sees, `Screen`. Where the player looks is a component of the hero's, `hunnawood::Look`, which its
  saves hold; the fire flickers in the frame drawn, not in the world, and its saves no longer put
  its light back; its game time is held (`Context::hold`) until the zone is drawn.
- A frame owns what it draws (ADR-0111): `Instance` holds clones of its mesh and material, counted
  references to the same GPU objects, and `Scene` and `slime_world_render::Frame` their instances,
  lights and environment, without lifetimes, so that a frame built on one thread is drawn on
  another. `Scene::new` takes the instances (a slice is cloned), `scene.lights` is a `Vec`,
  `scene.environment` an `Option<Environment>`, and `Frame::scene` takes the frame. Meshes and
  materials compare equal when one is a clone of the other.
- `AssetCache::new(&renderer, assets)` keeps what its loads take of the renderer, so that
  `AssetCache::request`, `AssetCache::reload(gpu, assets)` and
  `FrameBuilder::build(world, cache, gpu, fraction)` take no renderer. `AssetCache::clear` is gone:
  after a lost device, a game makes a new cache with the renderer that replaces the lost one.
- `Environment::new(gpu, &sky)` makes an environment without a renderer, in place of
  `Renderer::create_environment`.
- `slime_gfx::Gpu` opens its device with every limit that its adapter offers, where it asked for
  WebGPU's default limits, which are now the engine's floor: an adapter below it is refused with
  `GfxError::BelowFloor`, which names the limit, rather than failing later. A frame draws as many
  instances as the device's storage buffers hold, 16 777 215 on the RX 6900 XT where it drew
  1 048 576; textures are still written, and images read back, through buffers of at most 256 MiB.
- `World::get_mut` and `World::query_mut`'s `&mut T` give a guard, `slime_world::Mut`, rather than
  `&mut T`: it reads as the value, and its first write stamps it as changed. Code that wrote
  through `&mut T` mostly compiles as it was; a closure of `query_mut` that writes through a guard
  declares it `mut`, a function that takes `&mut T` is given `&mut guard`, and a match that binds
  parts of the value to change them matches `&mut *guard`. A `Transform` reached without being
  written no longer marks where its entity is in the world to be computed again.
- `Component` requires `Clone`: every component can be cloned, so that a world can be copied.
- `World::is_hidden` reads a flag that adding or removing `Hidden`, and changing parent, keep,
  rather than walking the entity's ancestors, which builds a frame of 100 000 entities about a tenth
  faster.
- The dev overlay's world panel makes its list of entities again only when the world's structure
  moves.
- The loads of cooked scenes and of the asset cache's batches run as background tasks of the
  engine's pool, where they ran on a pool of `slime-render`'s own of at most four threads; the
  sky's environment and the BRDF table are computed by frame tasks of the pool and the calling
  thread, where they started a thread per core; the cooker encodes textures on the pool.
- A load no longer slows the frames: its tasks write what they read into staging buffers, and the
  renderer's uploader submits their copies after each frame, at most 8 MiB for each frame or
  each 16.7 ms without one, so that a load of sixteen textures of 4096² keeps every frame under
  11 ms on the owner's GPU, where frames took up to 31 ms. Before the first frame, and 100 ms after
  the last, the copies go whatever the budget, so that a load that a program waits for without
  drawing is not held back. The tasks of the process's loads run two at a time; a load ends with
  its last task or copy, without a thread of its own, and starting one takes about a microsecond.
- `RenderPass::draw_indexed` takes a base vertex and a range of instances.
- `RenderStats::draw_calls` and the shadow passes' draw calls count draws of groups of instances,
  where they counted one per instance; `triangles` counts those of the instances drawn.
- A renderer's meshes share its buffers: `Renderer::create_mesh` and the loads write a mesh's
  vertices and indices into the first of the renderer's pages that has room, and capture tools
  name the pages' buffers, not each mesh's.
- `Problem::OutOfRange` carries the number out of range as a `Number`, in its own type, where it
  held an `f64`.
- `GltfFile::open` takes the directory of sources, from inside which alone it reads a `.gltf`
  file's buffers.
- The longest side of an image that the cooker takes is `slime_assets::MAX_SIDE`, the longest side
  of a cooked texture, where it was `slime_cook::image::MAX_SIDE`.
- `Overlay::new` returns `GfxError::DeviceLost` on a lost device, where egui's renderer panicked:
  it returns a `Result`.
- A run that replaces lost GPU devices gives up at a loss that comes after three new devices within
  five minutes, where it gave up after three in a row without a frame shown: a device lost again a
  few frames after each new one was replaced without end.
- An error of a lost GPU device that the application passes to `Context::exit_with_error`, or
  returns from `Presentation::gpu_restored`, once the run's device is lost, leaves the loss to the
  run, which replaces the device as for any loss, or stops with `RunError::Gfx`: it stopped the run
  as the application's error. *Hunna Wood* and the examples skip the frame that a lost device
  failed.
- `Gpu::wait_idle` and the readbacks wait for the GPU without polling the device, waking up to 1 ms
  after it finishes; every creation of a GPU object asks wgpu whether the device is still valid,
  with a buffer of 4 bytes, a few microseconds more on lavapipe.
- Debug builds compile each crate of the workspace once, where `cargo test` after `cargo build`
  compiled again every crate that depends on the reflection's derive, and give the dependencies
  their line tables only, not the rest of their debug information: the target directory is about a
  third smaller.
- `World::children` returns `Children`, an iterator over an entity's children in their order, read
  from either end, where it returned a slice: a child now leaves its parent in constant time.
- When a world places more lights than the renderer draws, `Frame::scene` gives first the sun, then
  the other directional lights, then the point and spot lights nearest the point that it is given,
  where the lamps beyond the first 16 in the world's order stayed dark.
- The cooker stretches an image whose sides are at least 4 texels but not multiples of 4 to the next
  multiples, and compresses it as any other, where it kept its 8-bit texels: a texture of
  2 048 × 2 043 texels takes a quarter of the memory. `TEXTURE_COOKER_VERSION` is 2: every texture
  is cooked again once.
- The renderer draws by pipeline, then material, then mesh, and binds a draw's material and mesh
  only when they differ from the draw before; it sends the GPU only the instances' slots that a
  frame uses.
- A frame of a large world is built faster: the world looks its component storages up once where a
  frame asked for them at each entity, and each ancestor.
- Starting a load of cooked data no longer copies its manifest: `CookedAssets`' clones share it.

### Fixed

- After a lost GPU device, a click on the dev overlay before the cursor moved again landed in the
  window's corner, its overlay made again without knowing where the cursor was: `Overlay::release`
  and `Overlay::restore` keep the overlay, its panels and the cursor's place, and make its GPU
  objects again, egui's font texture uploaded whole. *Hunna Wood* and the `world` example keep
  their overlay so.
- `error_chain` no longer writes a cause that the error's message already holds, as a cooked file's
  errors write what is wrong with it after its path: "the mesh is not one the cooker writes: a text
  is not UTF-8" appeared twice.
- The tooltips of the dev overlay's widgets show a value's documentation as plain text, its lines
  joined into paragraphs and its links written as their text, where they showed its doc comments
  as written, line breaks and Markdown included.
- The ring of the `world` example rises from where it rests and falls back, where it sank into the
  ground for half of each rise and fall.
- The lamps of the `world` example's yard have their light: the light of the lamp's prefab had no
  transform, which the frame builder needs to place a light, and was never drawn.
- When creating the spot lights' shadow atlas ran out of GPU memory after the cascades' atlas was
  created, the sun lost its shadows for good; the atlases' group is now made again whenever the
  atlases change.
- A multisampled output made `Renderer::render` panic inside wgpu; it panics with a message of the
  renderer's own, which its documentation lists.
- An entity that a run created, or a prefab that it placed, written into its level by
  `World::write_scene`, exists once when the run is carried into the file cooked again
  (`World::carry`), a level that the run changed to included, the file's entity given what the run
  holds of it; it existed twice, the run's and the file's (ADR-0109, ADR-0124).
- An alias of a field or a variant that is also the name, or an alias, of another field or variant
  beside it is refused: `#[derive(Reflect)]` fails to compile, `Registry::register_dynamic` returns
  an error, and `Registry::register` panics on a type described so by hand. Data that gave it read
  back as the other field or variant, without a word.
- A number on its field's bound is accepted: a range's bounds are compared with a 32-bit float
  rounded to the nearest 32-bit float, so that `range(min = 0.7)` holds `0.7_f32`, and with an
  integer exactly, beyond 2⁵³ too (`Range::holds`). A save, a scene file or a level written back
  that held such a value failed whole, and a type whose default sat on its bound panicked when
  registered. The derive refuses an integer bound that a 64-bit float does not hold exactly, written
  as a number or given as a constant.
- A section of a save that a save does not hold, misspelled or of a later program, is a warning
  in `Loaded::warnings`, and a section, or an entry of one, that is not a table refuses the save, as
  `World::load`'s rustdoc said: both were left out without a word.
- `World::save` compares a reference between entities with the start world's by the identity that
  each names: when the start world was made in another order than the run's, a reference that the
  run changed was compared by its handle, and could be left out of the save.
- `World::write_scene` writes `Unsaved` as any other component, for the root, the scene's entities
  and its placements': it left out what a scene file marks `Unsaved`, with what was under it, and
  wrote a placement's entity that holds it as the file had it, its changes since lost, without a
  report. It leaves out only the entities created outside scenes that hold it, or that are under an
  entity that does, and reports them.
- A saved component of a type, or whose value names a variant, that the program does not know is
  left out with a warning in `Loaded::warnings`, as `World::load`'s rustdoc said of what it does
  not know: it was set aside, as a value that its type refuses, so that a game could not tell a save
  that a later version of it wrote from one whose entities an update removed.
- The cooker decodes JPEG images in CMYK and YCCK to their colors: zune-jpeg 0.5.15 converted them
  to RGBA with the stride of RGB, which shifted the colors of every texel and left alpha wrong,
  without a word. It asks the decoder for RGB, and adds an opaque alpha itself; the images in YCbCr
  and gray, as all of *Hunna Wood*'s, cook to the same bytes.
- Reading a document leniently, as a save is read, costs time in proportion with its size however
  many warnings it gives: each warning's column was counted from its line's start, so that a line
  of 1 150 000 fields unknown took 488 s. A reading keeps the first `MAX_WARNINGS` warnings, then
  one that counts the rest (`ReadProblem::MoreLeftOut`), and places the first thousand errors and
  warnings: in a text each from the last placed, in the binary encoding through its tables and
  arrays, each walked once, where each was placed from the first byte again. `Document::warn` adds a
  warning of a reader that reads the document's layout itself, as a world reading a save does.
- A panic in a type's check or upgrade, a game's code, refuses the value that it was called on, as
  its error does, rather than crash the program, unless it aborts on panics: a save edited to give
  a value that a game's check did not expect left the world half loaded.
- The readers of cooked data check a file before they use it, and refuse a damaged one with an
  error, never a panic, a hang or memory exhausted. The manifest names its cooked files inside its
  directory only, by ASCII letters, digits, `-`, `_` and `.`, where `../` or an absolute path was
  read; a cooked file that is not a file, as a pipe or a device, or that is of another size than
  the manifest says, is refused before it is read, and read up to that size, where it was read
  whole first. `CookedTexture::from_ktx2` reads KTX2 files with a reader of the engine's own, which
  checks the header, the sides, up to `MAX_SIDE`, the count of levels and each level's place and
  size before it copies them: one byte of the data format descriptor made the `ktx2` crate panic,
  and a file of 1 MiB could announce levels of gigabytes. A cooked material that reads a texture of
  the wrong kind for its slot, data for colors or the reverse, is refused with
  `RenderError::InvalidMaterial`, where the load panicked. `AssetCache::poll` ends a batch whose
  load panicked, and does not ask for its meshes again, where the cache stayed loading them.
- The cooker cooks again a cooked file that it finds deleted, emptied or cut short, as a power cut
  or a full disk leaves one: it reuses a file only if its kind's reader reads it, of the size that
  the record of its import says, and a scene file's encoding only byte for byte, where it took any
  file under the expected name as good, so that the cook that a game advised repaired nothing. It
  forces what it writes to the disk before it renames it, and writes a new metadata file through a
  file beside it, as it writes cooked files.
- The glTF importer reads a `.gltf` file's buffers only from files inside the directory cooked,
  each up to the length that the file declares, where a path with `..`, an absolute one, a pipe or
  `/dev/zero` was read, and whole before its size was compared; and it refuses positions so far
  apart that the tangents computed from them are not finite numbers, where the cooker panicked.
- Reading a document takes memory bounded by its limits: a document holds at most `MAX_VALUES`
  values, 2²⁰, a text's counted before it is parsed by one for each `=`, `,`, `[`, `{` and `.` that
  it holds, the binary encoding's as they are read (`ReadProblem::TooMany`), so that a save of
  16 MiB of empty arrays, which the parser took gigabytes to read, is refused at once; and a writer
  refuses a document that a reader would. A save then holds about 22 000 entities that have a name
  and a transform, where 16 MiB held 57 000. A resolved scene takes at most `MAX_SCENE_SIZE` bytes,
  256 MiB, its placements' copies of their prefabs counted before any is made, where a few small
  files that placed a large prefab many times made a scene of gigabytes under `MAX_ENTITIES`; so do
  the scenes that a save spawns while it is loaded, together, the others set aside.
- A light that the renderer cannot draw is left out, with a warning that names it, where
  `Renderer::render` panicked: a damaged light ended a game at its first frame. One rule says what
  a light may be, `SceneLight::check`, which the cooker, the reader of cooked scenes, the world's
  `Light`, the renderer's `Light::check` and the sky's sun apply; it also refuses a color times an
  intensity that a 32-bit float does not hold. The frame builder leaves out a light that its
  entity's transforms, each finite, place where no number reaches, and the scene shader bounds the
  light that each light gives a point, and the sky its sun's disc, to 10³⁰, so that a light far
  brighter than any no longer makes pixels that are not numbers, which the bloom spread over the
  screen.
- A save that gives one key both to a scene that it spawns and to an entity that it creates, as a
  save edited by hand may, is refused with `DocumentError::IdentityInUse`: it was loaded, two
  entities claiming one identity, and the next save left one of them out.
- Reports say why they set aside: `World::write_scene` reports an entity that a key names, which
  a scene file does not hold, as such, not as an entity of another scene; loading a save sets aside
  a reference or a parent to an entity of the start, when the start created other entities than
  the save's, for that reason, not as one that the world does not have.
- Loading a save of a run that changed level, another scene spawned under a key, or carrying the
  run through hot reload, destroys only the old scene's entities: what the start placed in the
  level, the hero's prefab at a spawn point, stays where the run left it. It was destroyed with the
  level, and the save loaded without it.
- *Hunna Wood* compares the zone's layout that a save was made on with its own number by number,
  within a ten-thousandth of a metre or of a radian, whatever the order of the colliders, and so
  does its hot reload: a fingerprint of the numbers rounded to ten-thousandths changed when one of
  the seven of them that lie within one unit in the last place of a rounding's boundary moved by
  that unit, as an update that does not change the zone may make it, and every save was set aside.
  A save says its layout's numbers, under `zone`.
- *Hunna Wood* checks a save before it resumes it. It finds its zone and its hero again in the world
  loaded, by their identities, where it kept handles to entities that loading may have destroyed,
  so that a save edited to remove the key made it panic at its first step, at each launch. A save
  whose run cannot be played, that a later version of the game wrote, as the version that a save
  now says tells, or that holds what this version does not know, is set aside as one of another
  layout is, rather than resumed and written back without what this version did not know. It reads
  a save up to the size of the largest one, rather than whole before checking it, and neither reads
  nor moves what is not a file in the save's place, where it renamed a directory.
- *Hunna Wood* sets a save aside under a name of its own, `save.toml.<why>.2` and on, rather than
  in place of the one set aside before.
- One copy of *Hunna Wood* at a time holds a save, through a lock on a file beside it,
  `save.toml.lock`: another copy started with the same save says so, and plays a new run that it
  does not save, where both wrote the save and the last to quit wrote over the other's run. Each
  copy writes through a file of its own, `save.toml.<process id>.new`, where two saves at once could
  leave a file mixed from both.
- A panic in `Presentation::render` after the frame drew ends the process as a panic does, with exit
  status 101: on Vulkan, the window's surface, dropped while the panic unwound, panicked again in
  wgpu, which aborted the process. A lost device that wgpu refused to present the image to, the
  loss found by another thread after the engine's check, no longer makes `Surface::restore` or the
  surface's drop panic: the surface releases its last image before its swap chain goes.
- A loss of the GPU device found by a loading thread, a wait or a readback is `GfxError::DeviceLost`:
  creating a texture panicked in wgpu's unmap of its uploads, reading a target back in its mapping,
  and `Gpu::wait_idle` and the readbacks in `Device::poll`, which wgpu treats as fatal; a readback
  also failed as `GfxError::Readback`.
- After a restore of the GPU device that failed, `run` returns the error that the game passed to
  `exit_with_error`, as its rustdoc says, rather than the restore's.
- Emptying a parent of many children, by giving them other parents or none or by destroying them,
  took time quadratic in their number; resolving a scene file took time quadratic in its
  placements.
- A sun set where it already is, at each fixed step, kept the sky's environment: interpolating two
  equal transforms gives them back exactly, where the frame builder made the sky again at most
  frames.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) and what the engine does not do yet
in the [`README.md`](README.md#what-it-does-not-do-yet). In short: no editor yet, which Stage 6b
brings; a physically based material does not blend, and blended surfaces are not sorted by depth; a
picking draws all that its view sees, and picking by the CPU passes through nothing that a mask cuts
out; a client draws into a view lines and instances in the scene, and lines over the image only; the
asset cache's requests leave work and memory behind, and a page of meshes keeps the ranges of
dropped meshes until new ones fill them; every instance of a scene costs CPU time at each frame,
whether a pass draws it or not; the development channel scripts keys only, and captures windowed
runs only; the hardware tiers' budgets are checked on one GPU only. Limitations of 0.4 remain: a
scene file written back loses its comments, a large scene spawns slower than as many entities
spawned in code, a node renamed in Blender loses what saves held of it, saves stay text, of limited
size, the world has no systems or schedule, at most 16 lights are drawn, point lights cast no
shadow, frames are not synchronized with the display's refresh, and input methods, touch, gestures
and gamepads are not reported.

## [0.4.0] - 2026-10-18

The seventh release, which closes Stage 5b of the roadmap: scenes and prefabs. Worlds are written in
files: a scene file places entities and other scenes as prefabs, each placement with overrides of
its own, which follow the changes of their prefab; a world, or a part of it, is written back as a
scene file that keeps its placements, and what a game spawns while it runs is saved as placements
of its scenes. Scene files are cooked into a binary encoding of the engine's own, and the nodes of
glTF scenes made in Blender carry a game's components as typed properties. *Hunna Wood*'s zone is a
scene file that places its land and six lanterns of one prefab, and the `world` example spawns a
yard of prefabs. Cooked data of 0.3 must be cooked again, and the saves of 0.3 that *Hunna Wood*
made are set aside. The Windows programs that CI builds are attached to the release on GitHub.

### Added

- Scene files and prefabs ([ADR-0089](docs/adr/0089-scenes-and-prefabs.md),
  [ADR-0091](docs/adr/0091-scene-documents-and-placements.md)). A scene file is TOML: its entities
  under IDs of their own, each naming its parent and holding its components, a table of the assets
  that it uses by short names, and *placements* of other scenes, glTF scenes or scene files, as
  prefabs, nested. The file that places a prefab writes what each placement changes of it in the
  sections of saves: parts of components changed, components added and removed, entities removed.
  `slime_world::SceneLibrary` holds a game's scenes by asset ID and resolves one, with the prefabs
  that it places, into a `ResolvedScene`, a handle that clones at the cost of a count; an override
  whose entity or component the prefab no longer has is set aside and reported, and a file that says
  something wrong is refused, with the line and the column; a reference or a parent into a placement
  whose prefab no longer has its entity is reported too, and an entity of the file under one that it
  removes stays, the root's child. Placements nest at most `MAX_NESTING` scenes deep, and a scene
  holds at most `MAX_ENTITIES` entities. The entities of a placement are named through it:
  `zone/lamp 3/light`. `World::placement` says what the world remembers of a placement: its prefab,
  and the overrides set aside.
- A world, or a subtree of it, written back as a scene file
  ([ADR-0092](docs/adr/0092-a-world-written-back-as-a-scene.md)): `World::write_scene` keeps
  placements as placements, with what differs from their prefab and the overrides that the scene
  wrote, those that hold the prefab's value and those set aside included; entities created since
  get IDs that they keep; the same world gives the same bytes, and what a file cannot hold is
  reported (`Written::left_out`). `World::spawn_placement` places a prefab while a world runs;
  `World::placement_root` and `World::differs_from_prefab` say which placement an entity belongs to,
  and whether a component of it differs from the prefab's.
- Saves hold what a world spawned while it ran, a scene under a key or a placement made with
  `World::spawn_placement`, as its scene and what differs from it
  ([ADR-0093](docs/adr/0093-saves-and-hot-reload-with-placements.md)): loading spawns the scene as
  it is then, so that a fix of a prefab reaches the saves made before; a scene that the game no
  longer has is set aside. Such a save loads, where loading refused it before. `World::carry`
  carries a run into a start world made from files that changed, through a save, as hot reload
  does.
- The engine's binary encoding of documents
  ([ADR-0094](docs/adr/0094-the-binary-encoding-of-documents.md)): the document that a text holds,
  without its types, in bytes that are read back without parsing text, through the same reader,
  strict and lenient alike. `slime_serialize::encode` encodes a text, a composer's too, and
  `Document::parse_binary` reads it; its errors name the byte (`ReadError::byte`). The module
  `format` lays out its bytes.
- `slime_serialize::Composer::without_defaults` leaves out the fields that hold their defaults, and
  `Composer::table` makes a table that holds nothing yet; `slime_reflect::Value::same` compares
  values with their floats by their bits.
- `slime_serialize::References` names assets too (`asset_name`, `asset`), by their IDs unless a
  document names them otherwise; `Document::is_table` and `Document::error_at` let a program read a
  document's layout itself, and place what it finds wrong at its line and column.
- Scene files are cooked ([ADR-0095](docs/adr/0095-cooked-scene-files.md)): a `.scene` file with its
  metadata file is a source, which the cooker checks, `slime_world::outline` reading its layout
  without the game's types, and writes in the binary encoding, under the manifest's kind
  `SceneFile`, which `CookedAssets::scene_file` reads. It refuses, with the file and the line, an
  asset of the table that no source makes, a prefab that is not a scene, a parent missing, parents
  that would be each other's ancestors and scene files that place each other; a stale path of the
  table is a warning. `slime_cook::scan::SOURCE_EXTENSIONS` holds `scene` too, and
  `slime_cook::cook::SCENE_FILE_COOKER_VERSION` names what a scene file's cooked file is made by.
- Typed properties on glTF nodes: a custom property named `slime:<type name>`, a text that holds a
  component's value written as in a scene file, or a boolean or a number, gives the node that
  component when a world resolves the scene, read strictly through its registry, at the version of
  its type that the node's `slime:$versions` gives, as a document's `$versions` does. The cooker
  refuses a property of another kind, or whose text is not one value on one line
  (`slime_cook::gltf_import::TYPED_PROPERTY`, `TYPED_VERSIONS`).
- `SceneLibrary::of` reads the glTF scenes and the scene files of cooked data, each when it is first
  resolved, and `SceneLibrary::reload` takes new cooked data and gives the scenes that changed;
  every scene is resolved again, so that one that places a changed prefab follows it.
- The `world` example spawns a yard of four lamps, prefabs of a scene file, one of them red and one
  without its light; it writes the yard back when it quits, and checks the file; with `--watch`, it
  carries its run when a scene changes. The dev overlay's world panel shows the persistent identity
  of the entity selected and, for a placement, the prefab that it places.
- *Hunna Wood*'s zone is made of prefabs: a scene file written by hand places the land and six
  lanterns of one prefab, the third dimmer and the fifth without its light and its glass, and shows
  the moon in the sky. The zone's Blender script exports the land and the lantern apart; its nodes
  carry the colliders, the markers, the lights' times of day and the campfire's flicker as typed
  components, each node has a name of its own, and the script fails when Blender numbers one. The
  land and the lantern come out of the script the same from one run to the next. Its list of
  sources names the new files, on the mirror's release `hunnawood-7`: fetch them with
  `scripts/fetch-assets.sh zone/`, then cook the zone again.

### Changed

- `World::load` takes a `SceneLibrary`, through which it spawns again what the world spawned while
  it ran; a game that spawns nothing while it runs passes `&mut SceneLibrary::new()`. A save gives
  the version of its layout, `layout = 2`; the saves of 0.3, of layout 1, still load.
- `World::spawn_scene` spawns a `ResolvedScene`: a game that spawns a glTF scene alone resolves it
  first, `ResolvedScene::from_cooked(id, &scene, world.registry())`, which reads the typed
  properties of its nodes and fails with `SceneError::Node` for one that the registry refuses, or
  through a library of its cooked data, `SceneLibrary::of(assets).resolve(world.registry(), id)`. A
  value that its type's own check refuses makes spawning fail with `WorldError::RefusedInScene`, and
  nothing is spawned.
- `Document::keys(&[])` lists the keys of a document's root, where it listed none.
- A `Transform` read from data that leaves a field out gives that field the identity's value.
- The text writer keeps the order of a table's tables: once a value of a table goes under a header,
  the tables after it do too, small or empty, where they were written on their keys' lines before
  the headers. A scene file written back kept the order of siblings only so.
- A rotation that changed is a difference whole, its four coordinates, in a save as in a scene file
  written back (`Registry::difference`), so that it stays a rotation when the scene's turns later.
  Saves of 0.3, which hold the coordinates that changed, still load.
- The cooked scene's format is at version 2, with the typed components of each node, and so is the
  cooker's import of glTF files: cooked data of 0.3 must be cooked again. `slime_assets::SceneNode`
  has a new field, `components`, its typed components in the binary encoding: code that builds
  nodes adds `components: Vec::new()`. The `slime:` properties of a node are no longer among its
  `extras`.
- *Hunna Wood*'s hot reload carries the run through a save into the scenes that changed, and starts
  it again when the layout of a new game of them is not the layout of a new game of the scenes
  before: a collider changed through the world panel no longer starts the run again at the next
  reload. Its saves of 0.3 are set aside as saves of another layout, and a new run starts: the
  zone's entities have other identities, `zone/land/oak 12 wood`.
- The look of *Hunna Wood*'s zone changes a little: its script takes each model's meshes in the
  order of their names, which gives some rocks and ferns other variants, and the lanterns' bodies
  turn with their posts. A zone's layout no longer depends on the order of its scene file's records.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) and what the engine does not do yet
in the [`README.md`](README.md#what-it-does-not-do-yet). In short: no editor writes scene files yet,
which Stage 6 brings, and a scene file written back loses its comments and writes what spawning set
aside as the world holds it; a large scene resolves and spawns far slower than as many entities
spawned in code; a node renamed in Blender loses what saves held of it; saves stay text, of limited
size; the cooked glTF scene is a format apart from the binary encoding; and some meshes of *Hunna
Wood*'s zone keep names that Blender numbered. Limitations of 0.3 remain: the world has no systems
or schedule, the order of children and of queries may differ after loading, *Hunna Wood* does not
save its camera, the renderer has no transparency, culling or instancing, at most 16 lights are
drawn, point lights cast no shadow, frames are not synchronized with the display's refresh, and
input methods, touch, gestures and gamepads are not reported.

## [0.3.0] - 2026-10-17

The sixth release, which closes Stages 4b and 5a of the roadmap: live assets and the world. Scenes
load in the background while a game draws, a game can go on after a lost GPU device, a material's
values change while it runs, and a texture or a shader changed on disk changes in the running game.
The engine has a world of its own, which keeps a game's entities, their components and their
hierarchy, positions in double precision, and draws them; its own reflection, through which every
component is reached by name and edited in a panel of the dev overlay; and its own serialization,
through which values are written to TOML documents and read back, from earlier versions of their
types too. A world saves what changed since the start of a run, and loads it back. *Hunna Wood*
runs on the world, and saves and resumes its runs. The Windows programs that CI builds, the game
and the new `world` example among them, are attached to the release on GitHub.

### Added

- Scenes load in the background
  ([ADR-0073](docs/adr/0073-loading-scenes-in-the-background.md)):
  `Renderer::load_scene_in_background` returns a `SceneLoad` at once, which a game polls at each
  frame with `take` until it gives the whole scene. Meshes and textures are read and uploaded on a
  task pool of the renderer's own; dropping a `SceneLoad` cancels its load.
- `Gpu` is `Clone`: its clones share the device, so that several threads create GPU objects
  through it. Its submissions, from any thread, wait while a window's surface is configured, which
  wgpu fails to do when another thread submits work meanwhile (ADR-0074).
- `slime_render::scene_lights` gives the lights that a cooked scene's nodes place without loading
  the scene, so that a game knows its sun and its moon before its scene is loaded.
- *Hunna Wood* draws while its zone loads: its window shows the hero under the sky two to three
  times sooner, and the zone appears, whole, once loaded; the hero waits for it. The log says after
  how many frames, and the longest interval between two of them
  ([ADR-0074](docs/adr/0074-uploading-textures-without-holding-up-frames.md)).
- An application can go on after a lost GPU device
  ([ADR-0075](docs/adr/0075-recovering-from-a-lost-gpu-device.md)): with
  `AppConfig::restore_lost_device`, the run calls `App::gpu_lost`, where the application drops its
  GPU objects, opens a new device between frames, and calls `App::gpu_restored`, where the
  application creates them again; it gives up after three new devices in a row without a frame
  shown. Without `slime-app`, `Gpu::restore` and `Surface::restore` open the new device;
  `Gpu::lose` loses one on purpose, for tests and tools.
- *Hunna Wood* goes on after a lost GPU device: its world keeps the run where it was, and what the
  world draws loads again with the new device. `--lose-device-at <frame>`, and a button of its dev
  overlay, lose the device on purpose, to try it.
- A material's values change while a game runs: `Material::set_values` takes `MaterialValues`,
  all of a material's data but its textures and its sidedness, and the frames submitted after it
  draw with them; `Material::values` reads them. `LoadedScene::material` finds a loaded scene's
  materials by their names.
- *Hunna Wood*'s campfire flickers, its flames, its embers and its light together, and the key
  glows in pulses at night. The `scene` example's overlay has a slider of how strongly its ring
  glows.
- Hot reload of cooked data ([ADR-0076](docs/adr/0076-hot-reload-of-cooked-data.md)):
  `slime-cook watch <sources> <output>` cooks again each source that changes;
  `Renderer::reload_scene_in_background` loads a scene again, keeping the meshes and textures whose
  cooked file did not change; `slime_assets::ManifestWatch` tells a game when its manifest changes;
  *Hunna Wood* and the `scene` and `world` examples take `--watch`, which loads again what changed.
  *Hunna Wood* spawns its zone or its hero again when its scene changed, and starts the run again
  when the zone's layout changed.
- The renderer's shaders change while a game runs
  ([ADR-0077](docs/adr/0077-hot-reload-of-shaders.md)): `Renderer::set_shaders` makes its pipelines
  again from new WGSL sources, all or none, keeping the old ones on an error that names the
  shader's file, line and column; `ShaderSources` and `ShaderWatch` read the shaders' files, which
  `--watch` in *Hunna Wood* and the `scene` example gives the renderer when they change.
- `slime_app::error_chain`: an error's message followed by the message of each of its causes,
  the line that `run` logs for a failed run, for programs that do not use `run`.
- `slime-world`, the engine's world ([ADR-0079](docs/adr/0079-the-engines-own-world.md),
  [ADR-0082](docs/adr/0082-the-worlds-storage-and-handles.md)): a `World` gives out `Entity`
  handles, keeps their components, one of each type, and finds the entities that hold the
  components a query names, `World::query` reading them through an iterator and
  `World::query_mut` changing them through a closure. A component is reflected, under a stable
  name, and says it is one: `#[derive(Reflect)] #[reflect(name = "my_game::Lantern")]` and
  `impl Component for Lantern {}` ([ADR-0085](docs/adr/0085-reflection.md)); each world keeps the
  registry of its components' types. `WorldError` says why the world refused something.
- The world keeps a hierarchy and where its entities are
  ([ADR-0083](docs/adr/0083-transforms-and-the-hierarchy.md)): `World::set_parent`, `parent` and
  `children`; `Transform`, an `f64` translation, a rotation, a uniform scale that children inherit
  and a stretch that they do not; `World::global_transform`, always current, and
  `World::update_transforms`. `World::spawn_scene` spawns a cooked scene under a key as entities
  with their `Name`, `Meshes`, `Light` and `Extras`, each with a persistent identity, `zone/door`;
  a node whose name holds `/` takes its index, `#12`. An entity that `World::spawn` creates is
  `created/<number>`, from a count that saves keep. `World::find` and `World::persistent_id` go from
  one to the other, and `PersistentId` parses from its text.
- `slime_render::AssetCache` keeps the meshes that a game draws by their asset IDs, with their
  materials and textures ([ADR-0084](docs/adr/0084-what-the-world-draws.md)): `request` starts
  loading, as one batch in the background, those it neither holds nor loads, `poll` takes the
  batches that finished, each whole, and `mesh` gives a mesh and its material once held. `reload`
  loads again what changed in new cooked data, keeping the rest and dropping what it no longer
  lists, and `clear` drops everything after a lost device. Scene loads and the cache share one
  loader.
- `slime_app::Context::world` and `Context::world_mut` give the game's world, which the engine owns
  and tells before and after each fixed step: what `fixed_update` moves is drawn between its two
  transforms, at the frame's `fixed_step_alpha`, and what `update` changes is drawn at once.
- The world says what is drawn, without GPU objects
  ([ADR-0084](docs/adr/0084-what-the-world-draws.md)): `Meshes` holds the meshes an entity places,
  by asset ID, and whether they cast shadows; `Hidden` leaves an entity and its descendants out;
  `Light` is the world's own light, with a `LinearColor`, and a directional one says whether it
  shows in the sky. `World::begin_fixed_step` and `World::end_fixed_step` keep the transforms before
  a fixed step of what moves during it, `World::interpolated_transform` gives where it is drawn
  between the two, and `World::teleport` makes an entity be drawn at once where it goes; what a
  moving entity holds stays on it. `World::is_hidden` says whether an entity is hidden, by itself
  or through an ancestor.
- `slime-world-render`, the bridge from a world to the renderer
  ([ADR-0084](docs/adr/0084-what-the-world-draws.md)): `FrameBuilder::build` gives, at each frame,
  the instances of the meshes that the world's entities place, where the world draws them between
  fixed steps, from the meshes that an `AssetCache` holds, asking it for the others; the lights
  they place; and the sky's environment around the world's sun, the directional light that shows
  in the sky, given once to the sky and to the lights, first among them so that it casts the
  shadows. A new example, `world`, draws the examples' cooked scene from a world, its ring
  floating at the fixed step, runs headless too, and takes `--watch`; CI's Windows programs hold
  `world.exe`.
- `slime-reflect`, the engine's reflection
  ([ADR-0080](docs/adr/0080-reflection-and-serialization.md),
  [ADR-0085](docs/adr/0085-reflection.md)): `#[derive(Reflect)]` describes a struct or an enum
  under a stable name, `#[reflect(name = "...")]`, with aliases, a version, defaults, skipped and
  transient fields, ranges of finite numbers, units, and a check that refuses the values breaking
  the rules between fields, as `Light` and `Transform` declare; `Value` holds its values as plain
  data; `Registry` registers types and the types their values hold, types with no Rust type behind
  them too, checks values, sets their parts by `Path` within their fields' ranges, and lists their
  leaves; `diff` and `apply` compare values and give one back from the other. Descriptions hold
  their defaults' values (`TypeInfo::default`, `FieldInfo::default`), which the registry checks
  within their fields' ranges when it registers a type; it refuses names that start with `$`, a
  transient field without a default, and a transient field of a tuple. `Registry::difference`
  gives what changed of a value as a part of it, and `Registry::overlay` sets a part back;
  `Registry::field_default` gives what data that leaves a field out gives it. The standard types,
  glam's `Vec3`, `DVec3` and `Quat`, `LinearColor`, `AssetId`, `Extra` and `Entity` are reflected.
- A world reaches its components by name, without their types: `World::registry`,
  `World::register`, `World::entities`, `World::component_names`, `World::component_value`,
  `World::set_component_value` and `World::set_property`, which respects the ranges of the fields.
- The dev overlay's world panel, `slime_devui::WorldPanel`
  ([ADR-0081](docs/adr/0081-stage-5a-the-world.md)): a world's entities listed by name, with a
  filter, and the components of the one selected edited through reflection, whatever their types:
  booleans, numbers with their units, text, enum variants and options, and vectors, rotations and
  colors whole. It returns each change as an `Edit`, which the application applies with
  `World::set_property`; the world refuses a value out of its range, and the panel shows why.
  *Hunna Wood* and the `world` example show it on F2. `Overlay::wants_keyboard` says when a text
  field of the overlay has the keyboard or a menu of it is open, so that keys typed into it, or
  Escape closing the menu, do not also play. Tooling: it does not undo, add, remove or save
  components.
- `slime-serialize`, the engine's serialization ([ADR-0086](docs/adr/0086-serialization.md)):
  `write` and `write_partial` write a value of a registered type, or a part of one, as a TOML
  document, the same value giving the same bytes on every platform, and refuse what could not be
  read back, a document larger than `MAX_SIZE` or deeper than `MAX_DEPTH` included, a part's option
  fields as arrays, `[]` for one emptied; `read`, `read_partial` and `read_file` read it back, whole
  or in part, strictly for sources and leniently for saves, with errors by file, line, column and
  property path, and never panic; `read_as` and `write_as` do so for a Rust type. The module
  `format` says how a document holds enum variants, absent values, large integers and floats,
  which come back to their bits. A type past version 1 names an upgrade,
  `#[reflect(version = 2, upgrade = path)]`, which brings values of earlier versions, whole or
  partial, to its version as they are read; a later version is refused. `Composer` and `Document`
  write and read documents of several values, each at its place, references to entities named by
  their `References`.
- A world saves and loads ([ADR-0087](docs/adr/0087-the-worlds-documents.md)):
  `World::save(start, about)` writes, as a TOML document, what differs between the world and
  `start`, the world that the game makes at the start of a run: entities removed and created,
  components added, removed and changed, the changed ones by the fields that changed, parents
  changed, references to entities as persistent identities, and a reference to an entity destroyed
  or left out as `""`, no entity; the same worlds give the same bytes. It begins with `about`, what
  the game says of it, which `World::about` reads without loading the save
  ([ADR-0088](docs/adr/0088-saving-and-resuming-a-run.md)). `World::load` applies a save to such a
  world, made from the scenes as they are now, reports in a `Loaded` the differences that it could
  not apply, as `SetAside`s with their reasons, and the scenes that changed since; a save that is
  not one leaves the world as it was (`DocumentError`). Entities that hold `Unsaved`, with their
  descendants, and fields marked transient are left out. `World::first_difference` compares two
  worlds as saves see them; `World::insert_component_value` and `remove_component_value` attach and
  detach a component by its type's name. The `world` example saves its world when it quits, loads
  the save into a world spawned anew, and fails if the two differ.
- *Hunna Wood* saves its run ([ADR-0088](docs/adr/0088-saving-and-resuming-a-run.md)): F5 saves it,
  F9 loads the save in place of the run, the game saves when it quits and resumes the save when it
  starts. The save is `hunnawood/save.toml` in the user's data directory, or the file that
  `--save` names; `--new` starts a new run. A save that is not resumed, made on another layout of
  the zone or that cannot be read, is set aside, and one that can be neither read nor set aside is
  never written over; a save is written whole or not at all. The log says what was saved or loaded.

### Changed

- `Renderer::load_scene` starts a load in the background and waits for it: it loads on several
  threads, about twice as fast as before on the sample game's zone (ADR-0073). A panic of a
  loading thread is raised again on the thread that waits. When several assets of a scene fail, the
  error returned is the first that a loading thread reports, which can change from one run to the
  next.
- Textures reach the GPU through a buffer of their own and a copy submitted at once, rather than
  through wgpu's queue writes, so that a thread that loads them does not hold up the thread that
  submits frames (ADR-0074). Creating a texture costs a submission.
- `--frames` counts the frames that draw the zone in *Hunna Wood*, and the frames that draw the
  scene in the `scene` example, which loads it in the background too.
- `GfxError::DeviceLost` says why the device was lost (`LossKind`: the driver, the program, or
  memory that ran out), and `GfxError::CannotRestore` says why no device replaces a lost one.
- `Renderer::create_material`, and the loads of cooked scenes, refuse material values out of the
  ranges that glTF keeps them in, or that are not finite, with `RenderError::InvalidMaterial`:
  such a value drew nothing that a surface can look like, and one that is not a number spread
  through the bloom over the whole frame. `slime-cook` refuses such materials in glTF files, as it
  refuses the lights that glTF forbids: cook again data that was cooked before.
- The cooker writes its temporary files under names of the process's own, so that two cooks of
  the same sources at once leave the cooked data whole, and writes the manifest only when what it
  lists changed.
- `BindingType::UniformBuffer` declares the size of the values its shaders read (`size`): a
  pipeline whose shader reads more is refused when it is made, rather than failing when it draws
  (ADR-0077).
- *Hunna Wood*'s zone is refined: the oaks' leaf cards, smaller and four times as many, are lit as
  masses of leaves rather than card by card; the ground, the rocks and the stone are graded toward
  the zone's cold grey palette; the capital's picture is 2048 × 768 texels, within the sizes
  decided for the zone, the city farther and hazier. Its list of sources names the new files, on
  the mirror's release `hunnawood-6`: `scripts/fetch-assets.sh zone/` fetches them, then cook the
  zone again.
- `LinearColor` moved from `slime-gfx` to a crate of its own, `slime-color`, below every crate that
  holds a color, so that the world holds colors without the GPU layer (ADR-0079): `use
  slime_gfx::LinearColor` becomes `use slime_color::LinearColor`. The crate also gives the sRGB
  transfer function, `srgb8_to_linear` and `linear_to_srgb8`, which the GPU layer and the cooker
  each had a copy of.
- The renderer's positions are `f64`s, as the world's are (ADR-0079): `Camera::eye` and
  `Camera::target`, `PointLight::position` and `SpotLight::position` are `DVec3`s,
  `Instance::transform` is a `DAffine3`, and `LoadedScene::bounds` holds `DVec3`s;
  `Camera::view` and `Camera::view_projection` give `DMat4`s. What the camera sees is drawn as
  precisely 10 000 km from the origin as at it: the renderer still subtracts the eye on the CPU
  before positions reach the GPU as `f32`s (ADR-0048). `Mat4::IDENTITY` given to `Instance::new`
  becomes `DAffine3::IDENTITY`, and `Vec3::new(…)` given to a camera or a light becomes
  `DVec3::new(…)`.
- `slime-app` owns the game's world, and so depends on `slime-world`, which brings
  `slime-reflect`, `slime-serialize` and `toml_edit` into every game built on it. `slime-assets`
  and `slime-color` depend on `slime-reflect`: `AssetId`, `Extra` and `LinearColor` are reflected.
- *Hunna Wood* runs on the world (ADR-0079): its zone and its hero are entities, and its colliders,
  markers and lights the game's own components, read from the zone's extras. The run is in the
  world: the key taken, the door open and the time of day are components of the game's, `Key`,
  `Door` and `Clock`, from which it hides the key, the door and its colliders, and the lights of
  the other time of day; the world draws the hero between fixed steps, and the game draws the world
  through `slime-world-render` and an `AssetCache`, which loads the hero first. The game looks and
  plays as before.
- The cooker reads and writes metadata files through `slime-serialize`, as documents of a
  `Metadata` value: every committed one is unchanged, and an error says its line and column.
  `metadata::read` returns a `slime_serialize::ReadError`, which `ScanError::Metadata` holds.
  `.gitattributes` keeps `*.toml` files with LF endings.

### Removed

- `LoadedScene::instance_nodes`, which a game read to leave out what some nodes place: a game
  hides them in its world (`slime_world::Hidden`).
- `slime_cook::metadata::MetadataError`: metadata files are read through `slime-serialize`.

### Fixed

- A light refused for a negative value says that it is negative or not finite, rather than "not
  finite and not negative".

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) and what the engine does not do yet
in the [`README.md`](README.md#what-it-does-not-do-yet). In short: the world has no systems or
schedule, and no scene files or prefabs, which Stage 5b brings; a save is text of limited size,
one cut short at the boundary of a section loads in part, the order of children and of queries
may differ after loading, a node renamed in Blender loses what saves held of it, and *Hunna Wood*
does not save its camera, nor, when Windows ends the session, the run since the last F5. A lost GPU
device still ends the run of an application that does not ask for a new one. Limitations of 0.2
remain: the renderer has no transparency, culling or instancing, edges that masked materials cut
stay aliased, at most 16 lights are drawn, point lights cast no shadow, and the sky's light is
neither shadowed nor occluded; *Hunna Wood*'s zone takes seconds per frame on CPU drivers such as
lavapipe; frames are not synchronized with the display's refresh, frames pause on Windows while the
window is moved, wgpu's errors other than running out of memory panic in release builds too,
upgrading wgpu waits for egui, and input methods, touch, gestures and gamepads are not reported.

## [0.2.0] - 2026-10-15

The fifth release, which closes Stage 4a of the roadmap: cooked assets. Every source file has an
asset ID that survives a move, kept in a metadata file beside it; the cooker, `slime-cook`, imports
glTF scenes and PNG and JPEG images and cooks them into meshes, materials, scenes and textures
compressed in BC7 and BC5, which a game reads by ID and the renderer loads, each once. The sample
game is now *Hunna Wood*, a role-playing game seen in the third person: its first zone, a forest
path to a ruined chapel built in Blender from free assets, is played by day and by night. Stages 3c
and 3d, which came between 0.1 and this release, are in it too: anti-aliasing, the shadows of spot
lights, the sun's shadows in cascades, and DXC beside the Windows programs. The Windows programs
that CI builds, the game with its zone cooked among them, are attached to the release on GitHub.

### Added

- *Hunna Wood*, the sample game that every stage grows, in `games/hunnawood`
  ([ADR-0055](docs/adr/0055-games-in-the-workspace.md),
  [ADR-0067](docs/adr/0067-hunna-wood-replaces-nightwatch.md)): a role-playing
  game seen in the third person, whose hero walks the zone below, stopped by what stands in it and
  sliding along it, under a camera that follows it and comes nearer when something stands in
  between. `cargo run -p hunnawood` plays it; CI builds it for Windows with the examples.
- The asset pipeline's first two crates ([ADR-0069](docs/adr/0069-the-asset-pipeline.md)):
  `slime-assets`, whose `AssetId` names an asset by a UUID that stays the same when its source file
  moves, and the cooker, `slime-cook`, whose `scan` command gives each glTF, PNG or JPEG source file
  an ID in a metadata file beside it, `<file>.meta`, and whose `check` command, which the checks
  run, refuses a source file without one, a metadata file left without its source, and two that
  hold one ID.
- `slime-gfx` creates textures from blocks of BC7 and BC5 compression, a quarter of the memory of
  8-bit texels for BC7: `TextureFormat::Bc7RgbaUnormSrgb`, `Bc7RgbaUnorm` and `Bc5RgUnorm`, from
  levels the caller gives (`Gpu::create_texture_with_levels`). The device asks for BC compression
  when the GPU has it; `Gpu::can_sample_bc` tells, and a compressed texture on a GPU without it is
  refused with `GfxError::UnsupportedTexture`, which names the GPU.
- The cooker cooks PNG and JPEG images into textures ([ADR-0071](docs/adr/0071-cooked-textures.md)):
  `slime-cook cook <sources> <output>` computes every level of detail of each image, in linear
  light for colors, compresses them in BC7 blocks, and writes them to KTX2 files in `output`, with
  a manifest; a second cook cooks only what changed. A game opens that directory with
  `slime_assets::CookedAssets`, reads a texture by its image's ID, and uploads it with
  `Renderer::create_texture`. A texture of 2048 × 2048 texels takes about a second to cook in a
  release build.
- The cooker imports glTF files, `.gltf` with their buffers and images beside them, and GLB: each
  primitive becomes a mesh of only the vertices it names, with flat normals and MikkTSpace tangents
  where the file has none, the same tangents as Blender's; each material a material whose textures
  are cooked in the role its slots read them in; each scene its nodes, with their transforms, meshes,
  lights and `extras`, which a game reads its own data from. `slime_assets::CookedAssets` reads
  them by ID, a file's default scene by the file's own ID. The importer refuses what it cannot read
  right with a message that says what and where, and warns about what it leaves out.
- `Renderer::load_scene` loads a cooked scene onto the GPU, each mesh, material and texture once:
  its `LoadedScene` gives the instances to draw, the lights in world space, the box around them,
  and the scene's nodes with their `extras`.
- `scripts/fetch-assets.sh` fetches the source assets of *Hunna Wood*, which
  `games/hunnawood/assets/sources.tsv` lists with their authors and licences, all CC0, from their
  mirror, the immutable releases of github.com/HoneySlatt/SlimeEngine-assets
  ([ADR-0070](docs/adr/0070-the-sample-games-source-assets.md)), and checks each file's SHA-256.
- *Hunna Wood*'s first zone, a forest path past a gully, a campfire and a ruined chapel whose sealed
  door opens on a terrace, with its hero: built by a Blender script, `games/hunnawood/zone/`, from
  the source assets, exported to glTF and mirrored with them. Its colliders, markers (the start, the
  item, the door, the campfire) and lights of day and night are custom properties of its nodes,
  which the game reads from the cooked data. `scripts/check.sh` fetches and cooks it, and checks
  that the hero can reach the item and the campfire, but not what the door closes, and that the
  zone keeps within its budgets.
- *Hunna Wood* is played in that zone: it reads its colliders, its places and its lights from the
  cooked scene, and draws the zone and the hero as cooked. The hero takes the key on walking up to
  it, the key opens the sealed door, and R starts again from the same state. Resting by the
  campfire, with E, turns the day to night and the night to day: by night, the moon, the lanterns
  whose spot lights cast shadows and the campfire light the zone; `--night` starts the game at
  night. The game needs its zone fetched and cooked first, which its README explains; `--assets`
  reads another directory of cooked data, and the Windows programs that CI builds hold the zone
  cooked beside `hunnawood.exe`.
- `LoadedScene::instance_nodes` gives the node that places each instance, so that a game can leave
  out what some nodes place, such as a door that opens.
- `MeshData::cube`, `MeshData::sphere` and `MeshData::square` make simple shapes in code, one metre
  across, for a game or a test that has no model to load.
- `slime-gfx` draws into targets of four samples per pixel, for multisample anti-aliasing:
  `Gpu::create_multisampled_render_target` and `Gpu::create_multisampled_depth_target`, pipelines
  built for them, passes that resolve them into ordinary targets, and shaders that read them sample
  by sample (`BindingType::MultisampledTexture`;
  [ADR-0059](docs/adr/0059-multisampling-in-the-gpu-layer.md),
  [ADR-0060](docs/adr/0060-resolving-the-samples-weighed-by-their-brightness.md)).
- The renderer smooths the edges of what it draws: the scene and the sky are drawn at four samples
  per pixel, then each pixel is the average of its samples, each weighed by its brightness, so that
  the edges of bright surfaces are smooth too (multisample anti-aliasing,
  [ADR-0056](docs/adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md),
  [ADR-0060](docs/adr/0060-resolving-the-samples-weighed-by-their-brightness.md)).
  `RenderStats::resolve_gpu_time` gives the cost of that average, which the dev overlays show. A
  renderer holds about 87 MiB more of GPU memory at 1920 × 1080.
- `RenderPass::set_viewport` draws into part of a pass's targets, so that one pass can draw several
  views side by side.
- Spot lights cast shadows: every one drawn that casts them, as `SpotLight::casts_shadows` says (on
  by default), through a tile of one shadow atlas, sized by how many cast them, 2048 or 1024 texels
  wide ([ADR-0058](docs/adr/0058-shadows-for-every-spot-light-in-one-atlas.md),
  [ADR-0061](docs/adr/0061-projecting-the-shadows-of-spot-lights.md),
  [ADR-0062](docs/adr/0062-looking-spot-shadows-up-off-the-triangle.md)). The atlas takes 64 MiB,
  from the first frame in which a spot light casts shadows. `RenderStats::spot_shadow_draw_calls`
  and `spot_shadow_gpu_time` give the cost of its pass, which the dev overlays show.

### Changed

- The renderer reads a normal texture's x and y only, from red and green, and rebuilds z, the
  normal being of length one as glTF asks: two-channel BC5 textures serve as normal textures, and
  their blue is not read ([ADR-0071](docs/adr/0071-cooked-textures.md)).
- The `scene` example draws cooked data, which the cooker makes first:
  `cargo run -p slime-cook -- cook examples/assets target/cooked/examples` cooks the sample, which
  it reads by default. `--assets <directory>` reads another directory of cooked data, and
  `--scene <id>` another of its scenes, by its ID, where `--scene` took a GLB file. The Windows
  programs that CI builds come with the sample cooked, in `assets` beside `scene.exe`.
- `RenderPipelineDescriptor` has a new field, `multisampled`, and `RenderPassDescriptor` one,
  `resolve`: a literal written in full names them, `false` and `None` for what it drew before.
- `SpotLight` has a new field, `casts_shadows`, on by default: a literal written in full names it.
- Shadows are filtered over 5 × 5 texels, as Castaño proposed, where they were filtered over 3 × 3:
  their edges fade smoothly over about six texels, and a shadow moves off its caster by up to 4.5
  texels ([ADR-0063](docs/adr/0063-a-smoother-shadow-filter.md)).
- The directional light casts its shadows through four cascades, where it had one shadow map: what
  the camera sees up to its `shadow_distance` is split into four slices, each twice as deep as the
  one before, and each is shadowed by a map of 2048 × 2048 texels, the nearer ones finer
  ([ADR-0064](docs/adr/0064-cascaded-shadow-maps.md)). A point is shadowed by the finest cascade
  that holds it, which fades into the next across a band at its edge, and shadows fade out at the
  edge of the last ([ADR-0065](docs/adr/0065-reading-the-cascades.md)); a point is looked up off
  its triangle, as under spot lights. The cascades take 64 MiB of GPU memory, from the first
  frame in which a directional light casts shadows, where the map took 16 MiB from the renderer's
  creation; `RenderStats::shadow_draw_calls` counts an instance once in each cascade it reaches.
  In a courtyard lit by the moon, the scene pass costs about 10 % more on an RX 6900 XT.
- On Direct3D 12, shaders are compiled with DXC when its library, `dxcompiler.dll`, lies beside the
  program, and with FXC otherwise, with a warning in the log
  ([ADR-0066](docs/adr/0066-shipping-dxc-beside-the-programs.md)): DXC's code draws a
  courtyard's scene pass about twice as fast. The Windows programs that CI builds come in one
  directory with DXC and its licence; `scripts/fetch-dxc.sh` downloads it. `WGPU_DX12_COMPILER=fxc`
  asks for FXC.

### Removed

- The examples' temporary glTF loader, `examples/common/gltf.rs`: the cooker imports glTF files,
  and `Renderer::load_scene` loads what it cooked ([ADR-0069](docs/adr/0069-the-asset-pipeline.md),
  which supersedes [ADR-0029](docs/adr/0029-temporary-gltf-loading.md)).

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) and what the engine does not do yet
in the [`README.md`](README.md#what-it-does-not-do-yet). In short: the asset pipeline is half done,
so a game loads its assets whole, on the calling thread, without hot reload, and a lost GPU device
still ends the run; the renderer has no transparency, culling or instancing, edges that masked
materials cut stay aliased, at most 16 lights are drawn, point lights cast no shadow, and the sky's
light is neither shadowed nor occluded; *Hunna Wood*'s zone takes seconds per frame on CPU drivers
such as lavapipe; and limitations of 0.1 remain: frames are not synchronized with the display's
refresh, frames pause on Windows while the window is moved, wgpu's errors other than running out of
memory panic in release builds too, upgrading wgpu waits for egui, and input methods, touch,
gestures and gamepads are not reported.

## [0.1.0] - 2026-10-14

The fourth release, which closes Stage 3b of the roadmap: the physically based renderer. The engine
draws glTF's metallic-roughness materials under directional, point and spot lights in physical
units, with the shadows of a sun around the camera and the light of a procedural sky, in high
dynamic range, then exposes, blooms, tone maps and dithers it, its passes ordered by a render graph.
It draws relative to the camera and with reversed depth, so that large scenes keep their precision;
every creation of a GPU object can fail with an error that a game can handle; and image tests cover
it on lavapipe, WARP and an AMD GPU. The Windows examples that CI builds are attached to the
release on GitHub.

### Added

- The README says how a game depends on the engine (through git, pinned to a release tag, with
  the renderer optimized in debug builds) and what the engine does not do yet.
- `Instance::casts_shadows` ([ADR-0049](docs/adr/0049-instances-that-cast-no-shadow.md)): an
  instance that casts no shadow, a vast floor or a sky dome, is left out of the shadow pass and of
  the depths the shadow map spans, so that it costs no draw there and does not coarsen those depths;
  it still receives shadows. What lies beyond the map is lit.
- `DirectionalLight::shadow_distance`, 50 m by default
  ([ADR-0050](docs/adr/0050-a-shadow-region-that-follows-the-camera.md)): the shadow map covers
  what the camera sees up to that distance, instead of every instance, its texels on a grid fixed
  in the world, so that shadows work in levels and open worlds and their edges no longer shimmer
  as the camera or the instances move. A point is looked up in the map moved off its surface along
  its normal, which keeps curved surfaces from shadowing themselves, instead of the slope-scaled
  depth bias. The samples' shadows are slightly softer, and their reference images changed.
- Dithering of 8-bit outputs ([ADR-0051](docs/adr/0051-dithering-8-bit-outputs.md)): the
  tone-mapping pass adds noise of up to one step, the same at every frame, before the output rounds
  the frame, so that slow gradients such as the sky's show an even grain instead of bands; black
  and white stay as they are. The reference images changed.
- `Event::ModifiersChanged`: the modifier keys held as a whole (`Modifiers`: Shift, Control, Alt,
  Super), once per change, released when the window loses the focus. The dev overlay feeds them
  to egui, so its shortcuts with Ctrl, and selecting text with Shift, work.
- `Context::fixed_step_alpha`: the fraction of a fixed step accumulated but not run, so that an
  application draws what `fixed_update` moves between its last two states, smoothly when the frame
  rate and the step rate differ.
- `Context::exit_with_error`: an application fails a run from any method, `shutdown` included; the
  run stops as after an exit request and `run` returns the new `RunError::App`
  ([ADR-0043](docs/adr/0043-application-errors-during-a-run.md)). Before, only `init` could fail,
  and a game that hit an error later exited with success.
- Which types may cross threads is stated in `docs/ARCHITECTURE.md` ("Threads") and checked when
  the tests compile: the `Gpu` and every GPU object, meshes, materials, scenes, windows, events
  and the `Context` are `Send` and `Sync`; a `GpuTimer` and the `Renderer` are `Send`.
- Multi-pass rendering in `slime-gfx`
  ([ADR-0033](docs/adr/0033-gpu-layer-for-multi-pass-rendering.md)): render targets in the new
  `TextureFormat::Rgba16Float`, 16-bit floating-point colors beyond 0 to 1, read back with
  `Gpu::read_float_render_target` as a `FloatImage`; render targets and depth targets that later
  passes sample (`BindingResource::RenderTarget`, and `BindingResource::DepthTarget` at the new
  `BindingType::DepthTexture`); pipelines without a vertex buffer, for a fullscreen triangle built
  from the vertex index; depth-only pipelines and passes (`Gpu::render_depth`); comparison samplers
  (`Gpu::create_comparison_sampler`, `BindingType::ComparisonSampler`), for shadow lookups with
  percentage-closer filtering; and a depth bias on pipelines (`DepthBias`). Several passes can be
  recorded into one submission (`Gpu::submit` and `Commands`,
  [ADR-0041](docs/adr/0041-render-graph.md)), which costs the CPU less than a submission per pass.
- `Gpu::max_texture_dimension()` and `Gpu::max_buffer_size()`: the largest width or height of a
  texture, a render target, a depth target or a window's image (8192 with the default limits),
  and the largest buffer (256 MiB). A loader checks the images and meshes of a file against them
  before creating them, as the examples' glTF loader does.
- HDR rendering and tone mapping in `slime-render`
  ([ADR-0034](docs/adr/0034-hdr-rendering-and-tone-mapping.md)): the scene is drawn into a 16-bit
  floating-point target with its depth target, both following the output's size, its light
  multiplied by `Scene::exposure`, then a tone-mapping pass maps it to the output with Khronos PBR
  Neutral, which keeps hues and compresses highlights instead of clipping them.
  `RenderStats::scene_gpu_time` and `RenderStats::tone_mapping_gpu_time` give the
  GPU time of each pass. The `scene` example shows both and has an exposure slider.
- Physically based materials in `slime-render`
  ([ADR-0035](docs/adr/0035-physically-based-materials.md)): glTF's metallic-roughness model with
  the formulas of the Khronos glTF Sample Viewer. `MaterialData` gains metallic and roughness with
  their texture, a normal texture and its scale, an occlusion texture and its strength, an
  emission with its strength and texture, `AlphaMode` (opaque, or masked with a cutoff) and
  double-sidedness; double-sided materials are drawn without culling and shaded from the side
  seen. `MeshData::tangents` gives normal textures their frame.
- The examples' glTF loader reads all of it, tangents included (computed per triangle when a file
  has none), and `KHR_materials_emissive_strength`. The sample asset has normal, occlusion,
  metallic-roughness and emissive textures and tangents; the shapes of the image test have six
  spheres of increasing roughness, dielectric and metal.
- Lights in physical units in `slime-render`
  ([ADR-0037](docs/adr/0037-lights-in-physical-units-and-pre-exposure.md)): `Scene::lights` holds
  up to `MAX_LIGHTS` (16) lights of glTF's `KHR_lights_punctual`, each a `Light`: a
  `DirectionalLight` with an illuminance in lux, or a `PointLight` or `SpotLight` with an intensity
  in candelas and an optional range, and for spot lights inner and outer cone angles; each with its
  color apart from its strength. Point and spot lights fall off with the inverse square of the
  distance, brought smoothly to zero at their range, and spot lights fade between their cones, as
  the extension recommends.
- The examples' glTF loader reads `KHR_lights_punctual`: the lights that the scene's nodes place,
  at their world position and along their −Z axis. The sample asset has a sun of 100 000 lux, a
  point light in its glowing ring and a spot light on its wall, and requires the extension. The
  sample draws in physical units at the exposure of a sunny day, EV100 14.7, and lights a file
  without lights with its sun; the `scene` example's slider moves in exposure values.

- Shadows in `slime-render` ([ADR-0038](docs/adr/0038-shadow-mapping.md)): the first directional
  light of a scene that casts shadows (`DirectionalLight::casts_shadows`, true by default) casts
  them, through a 2048 × 2048 shadow map drawn each frame by a depth-only pass from an orthographic
  projection over the region around the camera that `shadow_distance` sets, and read by the scene
  pass at 3 × 3 points, so that their edges fade over three of the map's texels. Masked materials
  cast the shadow of what they keep. `RenderStats::shadow_draw_calls` and
  `RenderStats::shadow_gpu_time` describe the shadow pass, which the `scene` example's overlay
  shows.
- `FragmentState::target_format` in `slime-gfx` is optional: a fragment stage without a target
  format writes no color and may discard fragments, in a pass without a render target.
- Image-based lighting in `slime-render`
  ([ADR-0039](docs/adr/0039-image-based-lighting-computed-on-the-cpu.md)):
  `Renderer::create_environment` computes a `Sky` on the CPU, a clear-sky gradient over hazy
  ground with an optional sun, into an `Environment`: a cube map prefiltered for four roughnesses,
  which `Scene::environment` names. Surfaces reflect it as the Khronos glTF Sample Viewer does,
  smooth ones sharply and rough ones blurred, metals in their color, and a sky pass draws it behind
  the scene with the sun's disc (`RenderStats::sky_gpu_time`). The sample and the shapes are drawn
  under a clear blue sky, and the `scene` example's overlay shows the sky pass and the time the
  sky took to compute at startup.
- In `slime-gfx`: textures whose levels the caller gives, in any format
  (`Gpu::create_texture_with_levels`), cube textures (`Gpu::create_cube_texture`,
  `BindingType::CubeTexture`), and `f32_to_f16`, which converts to the half-precision floats of
  `Rgba16Float` texels.
- A bloom in `slime-render` ([ADR-0040](docs/adr/0040-bloom.md)): `Scene::bloom` spreads that share
  of each pixel's light into a halo that falls with the square of the distance, before tone
  mapping, without adding light; 0 skips it. `RenderStats::bloom_gpu_time` gives its passes' time.
  The samples bloom at 0.04, so that the sun glares and the ring glows, and the `scene` example's
  overlay shows the bloom's time and has a strength slider.

### Changed

- The image tests let 0.2 % of the pixels differ by more than 8 out of 255 on drivers other than
  lavapipe, which renders the references, where they let 0.1 %
  ([ADR-0054](docs/adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)): the drivers
  filter a texture seen at a grazing angle differently. On lavapipe, nothing changes.
- One color type, `slime_gfx::LinearColor`, for every color of the API
  ([ADR-0045](docs/adr/0045-one-linear-color-type.md)): red, green, blue and alpha in linear light,
  as `f32`s. It replaces `slime_gfx::Color` for clears, the arrays of `MaterialData::base_color`
  and `MaterialData::emissive`, and the `Vec3` of the lights' `color` and of `Sky::zenith`,
  `Sky::horizon` and `Sky::ground`. `LinearColor::from_srgb8` decodes the sRGB-encoded bytes of a
  color picker or a web color, which were easy to pass as linear values by mistake. Images do not
  change.
- `Instance` and `Scene` are made with `Instance::new(mesh, material, transform)` and
  `Scene::new(camera, &instances)`, then their public fields are set
  ([ADR-0046](docs/adr/0046-constructors-for-the-structs-a-game-fills-in.md)): both are
  `#[non_exhaustive]`, so that the fields they gain later do not break games. A new scene has no
  lights, no environment, an exposure of 1 and no bloom.
- Depth is reversed, from 1 at the near plane to 0 at infinity
  ([ADR-0047](docs/adr/0047-reversed-depth-with-an-infinite-far-plane.md)): 32-bit depths resolve
  distances to about one part in 8 million at any distance, where far surfaces fought before, and
  nothing is clipped by distance. `Camera::far` is removed. In `slime-gfx`, depth targets are
  cleared to 0, `DepthCompare` has `Greater` and `GreaterEqual` instead of `Less` and `LessEqual`,
  and a negative `DepthBias` pushes a surface away. Images do not change.
- The renderer draws relative to the camera
  ([ADR-0048](docs/adr/0048-rendering-relative-to-the-camera.md)): it subtracts the camera's eye
  from every position in `f64` before the GPU sees it, so that a scene 10 km from the origin is
  drawn exactly as at the origin, where its edges, shadows and lights moved before. The API's
  positions are still `f32`s, held in steps of 1 mm at 10 km.
- Every method of `Gpu` that creates a GPU object returns a `Result`
  ([ADR-0044](docs/adr/0044-running-out-of-gpu-memory.md)): the new `GfxError::OutOfMemory` when
  the GPU's memory cannot hold it, where the engine panicked, so that a game can free memory or ask
  for less; and `GfxError::DeviceLost` on a lost device, where it returned an object that could not
  be used. Memory that runs out in a creation the engine does not make, in egui's renderer for
  example, gives the device up and stops the run cleanly. The `SLIME_GPU_MEMORY_BUDGET`
  environment variable lowers the memory that creations may use, to try a game with less, on
  Direct3D 12 and on the Vulkan drivers whose memory budget wgpu reads (not RADV). The
  renderer's `create_environment`, `create_material` and `render` return a `Result` too: a frame
  whose new targets do not fit draws nothing, and the next one tries again.
- On Direct3D 12, shaders are compiled with FXC, which is part of Windows, on every machine
  ([ADR-0036](docs/adr/0036-direct3d-12-shader-compiler.md)). wgpu used any `dxcompiler.dll` that
  Windows found, whatever its version, so the same program could compile its shaders differently,
  or not start, from one machine to another. `WGPU_DX12_COMPILER=dxc` still asks for DXC.
- The Windows examples that CI publishes link the C runtime statically: they start on a Windows
  without the Visual C++ Redistributable, such as a fresh install or Windows Sandbox.
- `MeshData::tangents` is required, and a vertex takes 48 bytes: the largest mesh is 5 592 405
  vertices with the default limits. A white matte surface facing a light and the camera reflects
  0.97/π of the illuminance it receives, glTF's BRDF. `Renderer::create_material` panics on a
  texture in the wrong kind of format for its slot (sRGB for colors, `Rgba8Unorm` for data). Images
  change. The examples' loader leaves unsupported required extensions to the `gltf` crate's
  validation, draws `BLEND` materials opaque with a warning, and decodes only the images that
  materials read, each once.
- The scene pass applies the exposure before it writes the HDR target, so that light in physical
  units fits half precision (pre-exposure,
  [ADR-0037](docs/adr/0037-lights-in-physical-units-and-pre-exposure.md)), divided by 64, so that
  half precision holds exposed light up to 64 × 65 504, and writes brighter light as that
  ([ADR-0040](docs/adr/0040-bloom.md)). The tone-mapping pass no longer multiplies by the exposure,
  but by 64. `Scene::light` is replaced by `Scene::lights`; `DirectionalLight` has a `color` and an
  `illuminance` in lux, and glTF's default strength, 1 lux. A scene under a directional light of π
  lux at an exposure of 1 looks as it did under the former light of π.
- A frame of `Renderer::render` takes these passes: the shadow map, the scene, the sky, the bloom's
  chain, then tone mapping; `Renderer::new` builds nine pipelines and a 16 MiB shadow map; `Scene`
  has a `bloom` field; a `DirectionalLight` written in full names `casts_shadows`.
  `Scene::environment` replaces `Scene::clear` and the renderer's fixed ambient light: a scene
  without an environment is black around its surfaces and lit by its lights alone. Images change:
  scenes lit by a directional light show its shadows, and the samples show a sky, which blues their
  shadows and which their metals reflect, and a bloom.
- `RenderStats::gpu_time` is replaced by `scene_gpu_time` and `tone_mapping_gpu_time`; `Scene`
  has a new field, `exposure`; `Renderer::new` takes the output's format, any render-target
  format. The output is opaque, where the background of an offscreen image kept the clear color's
  alpha. Images change: a white surface lit by a light of 1 shows as 240 instead of 255, mid-tones
  lose 0.04 (0.5 shows as 181 instead of 188), highlights no longer clip, and dark colors are
  darker and more saturated.
- `RenderPipelineDescriptor::vertex_layout` is an `Option`, and `fragment: Option<FragmentState>`
  replaces `fragment_entry_point` and `target_format`. `DepthState` gains `bias` and is no longer
  `Eq` or `Hash`. Offscreen render targets and depth targets carry the usage that lets shaders
  sample them. `Gpu::create_texture` panics on `Rgba16Float`, whose textures take the levels their
  caller gives, and `Gpu::read_render_target` on its targets, read with
  `Gpu::read_float_render_target`.
- A `GpuTimer` times the passes of one submission, each apart: `Gpu::create_timer(label, passes)`
  takes how many passes of a submission it times, and `GpuTimer::latest(pass)` takes a pass's
  number in the order they were recorded instead of the GPU; it no longer polls the device. Times
  come back when later work is submitted (the next frame) or in `Gpu::wait_idle`.
- `Renderer::render` orders its passes with a render graph and records them four to a
  submission, each timed by one timer, instead of a submission and a timer per pass
  ([ADR-0041](docs/adr/0041-render-graph.md)): on an RX 6900 XT it takes about half the CPU time
  (at 1920 × 1080, 0.81 → 0.38 ms with the bloom, 0.22 → 0.11 ms without). Its GPU times can be
  `None` for a few frames after the output's size changes or the shadow pass or the bloom starts
  or stops, and never come back for the tone-mapping and bloom passes of a renderer that draws
  into outputs of two sizes in turn: use one renderer per output size. The bloom's levels are
  freed while the bloom is off. Images do not change.
- The `submit` profiling span covers only the queue submission. wgpu's validation and encoding of
  a pass's commands now count in the caller's span (`render` in a windowed run).
- The second termination signal no longer logs a warning before it ends the process; exit code
  130 says what happened.
- `slime_devui::Overlay::render` takes an `FnMut` closure instead of an `FnOnce`, since egui may
  call it twice in a frame. A closure that moves a captured value into egui (for example
  `ui.label(name)` with an owned `String`) must borrow it instead.
- `slime_devui::Overlay::render` panics with a clear message when the target does not have the
  overlay's format. It used to panic only on frames that drew something, with a wgpu validation
  error.
- The dev overlay makes one queue submission per frame instead of two.
- When a step of the Windows CI job fails, the job publishes the images that the failing image
  tests rendered on WARP, as the artifact `image-test-failures-windows`. Its test step no longer
  stops at the first test program that fails.
- `scripts/check.sh` also runs clippy on the windowed lifecycle test of `slime-app`, on Linux and
  for Windows. Because it is outside the default test run (`test = false`), `--all-targets` used
  to leave it out.
- `scripts/check.sh` checks that a window's images follow its size when it is resized during a
  run, which nothing checked: it starts a second headless Weston, at scale 2, which doubles the
  window after its first frame, and runs the new `resized` scenario of the windowed lifecycle test
  there ([ADR-0024](docs/adr/0024-windowed-lifecycle-check-in-weston.md)).
- `KeyEvent::text` is the same for a named key on every platform. Only Enter, Tab, Backspace,
  Escape and Space give text. On Wayland and X11, Delete gave `"\u{7f}"` and Clear `"\u{b}"`,
  where Windows gave none; Shift+Tab gave no text there, where Windows gave `"\t"`. Space still
  gives the character of a dead key pressed before it.
- `Renderer::render`, `Camera::view`, `Camera::projection` and `Camera::view_projection` document
  the values for which they panic when glam's assertions are on (glam's `glam-assert` feature, or
  `debug-glam-assert` in a debug build). `DirectionalLight::direction` says what a zero direction
  does, and `Camera::up` says which lengths do not work.
- The examples' glTF loader, its error for a file without a binary chunk, the `scene` example's
  help and the examples' READMEs say that it reads GLB files that hold their buffer and PNG images
  in their binary chunk, rather than "embedded buffers", which in glTF also covers the data URIs
  it refuses.

### Fixed

- The bricks of the examples' scene lie along its wall, three across its 3 m. The script that
  makes the asset gave each side of a box its texture in the order Blender lists the side's
  corners, which turned the bricks by 90° on the wall's large sides and squeezed them into its
  height. The reference images of the glTF scene changed.
- A light, or a sky's sun, whose direction is very short or very long but finite, as its
  documentation allows, lights as one of length one. Its direction was scaled to a length of one in
  `f32`, whose square of a length below about 4 × 10⁻²³ is zero and above 2 × 10¹⁹ infinite: the
  light made its pixels NaN, or gave nothing.
- `Renderer::render` returns `GfxError::DeviceLost` once the device is lost, also for a frame that
  creates nothing, of the same size as the one before; it returned the statistics of a frame that
  was never drawn.
- The dev overlay tells egui when the window loses the keyboard focus, so that a text field's
  caret stops blinking then.
- `Gpu::create_texture` of a texture with a side of zero panics with a message that says so, not
  with "takes 1 to 0 levels".
- The `scene` example keeps its camera above the floor, from which the sample's walls and crates
  flickered where they touch it.
- `scripts/check.sh` checks the links of an archive of the repository too, outside a git checkout.
- A failed `slime_app::run` is logged at the `error` level with each of its causes (`the run
  failed: cannot draw into the window: no GPU adapter is available: ...`); the README's pattern,
  which printed only the most general message, now only exits with a failure.
- `Renderer::render` draws the first `MAX_LIGHTS` (16) lights of a scene that has more, and logs a
  warning once per renderer, instead of panicking, in release builds too, on data a game loads
  ([ADR-0042](docs/adr/0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)). It panics
  with a message naming the value on an exposure, a bloom or a light value outside the range its
  documentation gives, where it drew a white image, negative light or a light from no direction.
- A renderer that drew many instances once no longer pays for them in every later frame: its
  instance buffer shrinks back once a frame uses a quarter of it or less (a 10-instance frame after
  a 100 000-instance one took 21.4 ms of CPU on lavapipe, 0.054 ms now).
- `Context::exit_requested` reports a termination signal as soon as it arrives, during
  `App::init` and in the middle of a frame too, so that a long loading or update that checks it
  can stop early on Ctrl+C. It reported it only once the run was already stopping between frames.
- Mouse buttons still down when the window loses the keyboard focus are released just before
  `Event::Focused(false)`, as keys are (`slime-platform`); a later real release, and a second press
  of a button already down, are not reported. On Windows, a drag carried across Alt+Tab left the
  button down for good, and the `scene` example's camera kept turning.
- On Windows, a key-up of either Shift key releases both: Windows sends no key-up for the first of
  two held Shift keys to be let go of, so that key stayed down until the window lost the focus.
- The dev overlay follows egui's zoom factor: panels zoomed with `set_zoom_factor` or
  `set_pixels_per_point` were laid out past the window, and the cursor missed them. A hidden
  overlay no longer submits an empty render pass every frame (2 µs per call instead of 14 on
  lavapipe and 38 on RADV, at 1920 × 1080).
- The examples' glTF loader refuses, rather than panics on, a GLB header that declares a length
  shorter than itself and a primitive whose positions name an accessor the file does not have, and
  refuses a vertex placed at a point that is not finite, which made the box around the scene NaN.
- The `scene` example's camera keeps orbiting when left running for days: its yaw stays within one
  turn instead of growing until each frame's step rounded to nothing.
- Documentation corrected: logging off or shutting down Windows never reaches the engine's console
  handler; using a GPU object with another `Gpu` does not reliably panic; on Windows with Vulkan, a
  wait of more than one second for the window's image loses the device; a key pressed after a dead
  key, or just after the focus is gained, is reported as `KeyEvent` now says.
- A window's surface that the graphics API refuses to configure (a window or a display gone, a
  swap chain that cannot be created) no longer panics: it counts as lost, is created again, and is
  given up with wgpu's message after the losses that ADR-0025 allows, which stops the run with
  `GfxError::SurfaceLost`. Acquiring an image while the previous frame is still held now panics
  with a message of the engine's.
- On Windows, a program started from Windows Terminal into a window of the older console host no
  longer prints its log's colors as escape sequences. `slime-app` asks the console to interpret
  them, through the new Windows-only dependency `nu-ansi-term`, and colors the log only when it
  accepts; it used to rely on the `WT_SESSION` variable, which such a program inherits. The older
  console host now shows colors too.
- Closing the terminal of a run in the foreground no longer cuts its clean stop short. The shell and
  the kernel each send SIGHUP, less than a millisecond apart, and the second one ended the process
  as a second signal does. A signal that arrives less than half a second after the first now repeats
  it and is ignored ([ADR-0012](docs/adr/0012-clean-shutdown-on-termination-signals.md)).
- Mirrored instances are no longer drawn inside out: the renderer keeps their front faces, and
  the examples' loader no longer copies mirrored meshes with their triangles wound the other way.
  A mesh flattened between two rotations is no longer lit from the wrong side. The sample scene's
  floor no longer disappears when seen from below.
- Key events pair up the same way on every platform (`slime-platform`). A key still down when the
  window loses the keyboard focus is released just before `Event::Focused(false)`. This is new on
  Wayland, where such a key stayed down for applications that track held keys. A key already down
  when the window gains the focus is no longer reported as a new press on X11 and Windows; it is
  not reported at all, its release included, until it is pressed again. Before, holding Escape
  while a launcher or a dialog closed quit the `window` and `scene` examples. On Windows, which
  reports only the release of Print Screen, that key now gives no event.
- On Windows, ending the event loop no longer waits for the last frame's deadline, and no longer
  calls `EventHandler::about_to_wait` once more before `exiting`. After a failed window creation,
  the loop no longer waits for a message without limit before returning the error: closing the
  `window` example stops it in about 150 ms in the owner's Windows 11 virtual machine.
- On Windows, the window is hidden when the event loop ends, instead of staying on screen,
  unresponsive, until the process exits.
- On X11, a window whose size times the scale factor exceeds 65 535 physical pixels no longer
  panics inside winit. That is 16 384 logical pixels at a scale factor of 4, and 65 535 is the
  most the protocol allows. The size is now limited to 65 535 physical pixels per side.
- The documentation of `slime-platform` was wrong in three places, now corrected:
  - On X11, losing the X server ends the process; only on Wayland does the loop end cleanly.
  - A missing libxkbcommon panics instead of returning `PlatformError::Connect`.
  - Mouse positions can lie outside the window during a drag and, on X11, when the window gains
    the focus.
- A `SLIME_LOG` value with a stray comma or a space around a directive, such as `slime_app=trace,`
  or ` debug`, no longer silences the modules it does not name. Empty directives and the spaces
  around directives are dropped before the value is read. A value with no directive left uses the
  default filter.
- A second termination signal now ends the process with exit code 130 even while standard error
  is blocked, for example a pipe that nobody reads. The thread that handles signals no longer
  writes the log line itself.
- Frames that get no window image (a minimized window, an image not ready in time) now give wgpu
  a submission, as frames with an undrawn image already did. GPU writes an application makes
  outside `App::render` (`Gpu::write_uniform_buffer`, `Gpu::create_texture`) no longer pile up for
  as long as the window is minimized.
- Mipmaps of textures with an odd width or height now include the last column and row of each
  odd-sized level. Each level used to be computed without them, so the smallest levels of a
  texture whose sides are not powers of two showed only its top-left part, stretched. Along an odd
  side longer than one pixel, each pixel of the next level now weights the three pixels it
  overlaps by how much of each it covers
  ([ADR-0032](docs/adr/0032-mipmaps-computed-at-texture-creation.md)). Textures whose sides are
  powers of two are unchanged.
- Creating a vertex, index or uniform buffer no longer panics once the GPU device is lost. Before,
  a mesh, a material or a larger instance buffer created after a driver reset panicked inside
  wgpu.
- On Vulkan, `Gpu::create_render_pipeline` and `Gpu::create_shader_module` return
  `GfxError::DeviceLost` when the driver's failure to create the object loses the device. Before,
  they returned `Ok` with an unusable object.
- A renderer with a GPU timer no longer panics inside wgpu in the frame where a Vulkan driver
  reports the device lost: the run stops with `GfxError::DeviceLost`, as documented.
- A windowed run whose GPU device is lost before the window's first image, or before the first
  image after its surface was created again, stops with `GfxError::DeviceLost` instead of
  panicking inside wgpu.
- `slime-render` lights an instance flattened along one axis (a scale of 0 on one axis, such as a
  glTF node with scale [1, 1, 0]), whose normals were NaN. Normals are now transformed by the
  cofactor matrix of the instance's transform, which also stays finite at extreme scales.
- `slime-render` writes an alpha of 1 for every surface it draws, as documented for opaque
  surfaces. It wrote the material's alpha, so an offscreen image read back with
  `Gpu::read_render_target` showed translucent surfaces.
- `Renderer::create_mesh` returns `RenderError::InvalidMesh` for a mesh whose vertices or indices
  need a buffer larger than the device accepts (`Gpu::max_buffer_size`, 256 MiB with the default
  limits: more than 5 592 405 vertices or 67 108 864 indices), instead of panicking in wgpu, and
  for a vertex value (position, normal, uv or tangent) that is not finite, which drew nothing and,
  for a position, took every shadow from the scene.
- An instance whose transform puts it at no finite place no longer takes the shadows of the whole
  scene away: the shadow map leaves it out.
- `Camera`'s documentation names the inputs that give a matrix that is not finite, such as a
  `target` at `eye`.
- The dev overlay keeps its panels in place when the window's scale factor changes. Moving the
  window to a screen with a higher scale used to push them toward the top-left, for good.
- The dev overlay no longer draws an empty frame when egui lays a frame out twice, as on the first
  frame of a grid or of a window being resized.
- The dev overlay no longer panics when the GPU device was lost earlier in the frame. It skips the
  frame, and the windowed run stops with `GfxError::DeviceLost`.
- The examples' glTF loader returns an error instead of panicking or aborting on what the `gltf`
  crate's validation lets through:
  - images larger than the GPU accepts, checked before they are decoded;
  - primitives whose vertices or indices need a larger buffer than the GPU accepts, checked before
    the loader builds them;
  - image data that lies outside the binary chunk, has no MIME type, or is missing;
  - accessors stored in types glTF does not allow, sparse, empty, overlapping, or outside the
    binary chunk. Before, indices it could not read were silently drawn as non-indexed triangles;
  - node hierarchies that are not trees. A cycle used to overflow the stack;
  - an index that names no vertex, in a primitive without normals.
- The loader places node hierarchies of any depth. A valid chain of a few thousand nodes
  overflowed the stack and aborted.
- The loader gives primitives without normals flat normals, as glTF requires, instead of
  smoothing them across shared vertices.
- The loader draws mirrored nodes (a negative determinant) with their front faces outward instead
  of inside-out.
- The loader warns about vertex colors, which it ignores.
- The `scene` example draws large and small GLB files: its camera's near and far planes now fit
  the scene, and the wheel keeps the camera between 0.1 and 20 radii of it.
- In the `scene` example, a drag that ends over the dev overlay no longer leaves the camera
  turning with the mouse.
- The `scene` example no longer replays, when F1 shows its overlay again, all the input received
  while the overlay was hidden: a click on the scene could press a button of the overlay.
- A renderer failure in the `scene` example is reported with its cause.
- `CONTRIBUTING.md` tells contributors on WARP to run the tests one at a time
  (`--test-threads=1`), as the Windows CI job does.
- A GPU device lost while a frame was being drawn into a window no longer makes the end of the
  run panic on Vulkan ("Trying to destroy a SwapchainAcquireSemaphore that is still in use by a
  SurfaceTexture"). Once the device is lost, `SurfaceFrame::present` releases the image instead of
  presenting it, and `run` returns `RunError::Gfx(GfxError::DeviceLost)`. Checked by hand with a
  simulated loss.
- The documentation of `SurfaceFrame` said that dropping a frame without presenting it skips the
  frame. On Vulkan, wgpu cannot give such an image back: each drop takes one of the window's
  images until the surface is configured again, and once none is left, `Surface::acquire` waits
  one second and returns `None`. It now says that every frame acquired must be presented.
- When the engine cannot handle termination signals because `ctrlc` cannot start its thread, its
  warning says that the signals can then be ignored, and gives the system's error, instead of
  saying that they end the process.
- The dev overlay no longer types the letter of a Ctrl shortcut (Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+Z)
  into a focused text field: a key pressed while Ctrl is held types no text, as with egui's own
  window integration.
- The `ScrollDelta` documentation said that touchpads report pixels. It now says that only Wayland
  reports pixels, that X11 and Windows report touchpads in lines, and that a `Lines` value can be
  a fraction of a line, so one event is not one wheel notch.
- `Camera::view` normalizes `up` before handing it to glam, whose `look_at_mat4` asserts an up of
  length one when glam's assertions are on: a game that turned them on, through glam's
  `glam-assert` feature or `debug-glam-assert` in a debug build, saw a camera whose up was not of
  length one, such as (0, 2, 0), panic on its first frame.
- The examples' glTF loader refuses a buffer or an image given as a data URI with a message that
  says so. It called such data "an external file" and quoted the whole base64 payload.
- The rustdoc of `Renderer::create_mesh` lists every reason it rejects a mesh, including a mesh
  with no vertex and a number of indices that is not a multiple of three, and that of
  `RenderError::InvalidMesh` points to that list instead of keeping an incomplete copy.
- `FixedStep::advance` says that a limited call drops the fraction of a step too.
- Three tests could pass with what they check broken: the mipmap test of `slime-gfx` passed on a
  texture without mipmaps, the timing tests accepted a pass that took no time, and the windowed
  lifecycle test never checked the time each frame represents. The profiling spans of the main
  loop, decided in ADR-0010 and ADR-0020, are now checked too.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) and what the engine does not do yet
in the [`README.md`](README.md#what-it-does-not-do-yet). In short: the renderer has no
anti-aliasing, transparency, culling or instancing, one shadow map without cascades and no shadows
from point and spot lights, at most 16 lights that every pixel evaluates, and a sky whose light is
neither shadowed nor occluded; glass is drawn opaque; glTF files are read by a temporary loader in
the examples, and textures get their levels of detail on the CPU; a lost GPU device ends the run;
upgrading wgpu waits for egui; and limitations of 0.0.3 remain: frames are not synchronized with
the display's refresh, presenting can still stall where Mailbox is not offered, frames pause on
Windows while the window is moved, wgpu's errors other than running out of memory panic in release
builds too, and input methods, touch, gestures and gamepads are not reported.

## [0.0.3] - 2026-10-13

The third release, which closes Stage 3a of the roadmap: the first renderer. The engine draws a
glTF scene with textured materials, a depth test and a directional light under a camera the mouse
moves, measures what each frame costs on the CPU and on the GPU and shows it in a dev overlay, and
covers the renderer with image tests that run on lavapipe, WARP and hardware GPUs.

### Added

- `slime-render`, the first renderer: `Renderer` draws a `Scene` (a camera, a directional light,
  a clear color and instances of meshes with their materials) into any render target, with a
  depth test and back-face culling, and returns `RenderStats` (draw calls, triangles, the CPU time
  of the call and, on adapters that can time their work, the GPU time of its pass); meshes come
  from positions, normals, texture coordinates and indices, materials have a base color, which a
  texture may modulate, `Camera` gives its matrices in WebGPU's convention, and `glam` is
  re-exported for the math ([ADR-0027](docs/adr/0027-the-render-crate.md)). `DirectionalLight`
  lights surfaces in proportion to the cosine between their normal and the direction to the light,
  plus an ambient light; normals follow the rotation of their instance and stay perpendicular to
  surfaces scaled unevenly.
- The `scene` example, the sample of Stage 3a: a glTF scene drawn by the renderer, lit by a
  directional light, seen from a camera that orbits it until the mouse takes over (dragging turns
  it, the wheel zooms). It draws `examples/assets/scene.glb`, made in Blender by
  `examples/assets/make_scene.py` and released under CC0, or any GLB file with embedded buffers
  and PNG images given with `--scene`, through a temporary loader in the examples
  ([ADR-0029](docs/adr/0029-temporary-gltf-loading.md)). `--frames <count>` closes it after that
  many frames and fails if none of them drew the scene, which lets `scripts/check.sh` run it in
  the headless Weston. Two image tests, compared with
  references rendered with lavapipe: the sample scene from two camera poses, and shapes drawn by
  the renderer (cubes and a smooth sphere, two of them with a checker texture).
- `slime-devui`, a temporary crate: `Overlay` draws egui panels over the engine's frames, fed
  with the engine's events, through a new `wgpu` feature of `slime-gfx` that gives the engine's
  tooling the wgpu objects behind its types ([ADR-0031](docs/adr/0031-dev-overlay-with-egui.md)).
  The `scene` example shows the frame time, the CPU and GPU times of its render, its draw calls
  and triangles in an overlay; F1 hides it.
- GPU timing: `Gpu::create_timer` gives a `GpuTimer` on adapters with timestamp queries, which a
  render pass uses to measure how long it takes on the GPU, read back a few frames later without
  waiting; `Gpu::can_time` tells.
- The GPU resources a renderer needs, in `slime-gfx`: index buffers and indexed draws, uniform
  buffers written through `Gpu::write_uniform_buffer`, bind group layouts and bind groups that
  pipelines declare and render passes bind, with dynamic offsets so that one uniform buffer holds
  the values of many draws (`Gpu::uniform_offset_alignment` gives their alignment), depth targets
  with a depth test and face culling declared by the pipeline, and render passes that keep the
  target's content instead of clearing it
  ([ADR-0026](docs/adr/0026-resource-binding-and-depth.md)). `RenderTarget::label` gives a
  target's label back.
- Textures and samplers in `slime-gfx`: `Gpu::create_texture` uploads an image that shaders
  sample, in an sRGB format (decoded when read) or in the new linear `TextureFormat::Rgba8Unorm`,
  which `TextureFormat::is_srgb` tells apart; `Gpu::create_sampler` sets the filters and the
  address modes that glTF samplers express; both are bound through bind groups.
- Textures have mipmaps, computed on the CPU when they are created, in linear light for the sRGB
  formats; samplers read the level of detail that fits, and linear filtering blends two levels
  ([ADR-0032](docs/adr/0032-mipmaps-computed-at-texture-creation.md)).
- `Surface::check` and `GfxError::SurfaceLost`: a window's surface that is lost for good (it cannot
  be created again, or is lost again each time) stops the run like a lost device, instead of an
  error logged at every frame while the run went on without drawing
  ([ADR-0025](docs/adr/0025-giving-up-a-lost-surface.md)).

### Changed

- The image tests keep the tolerance of 2 out of 255, exceeded by no pixel, on lavapipe, which
  rendered the references; on other drivers, which filter textures and place edges differently,
  a pixel may differ by up to 8, and 0.1 % of the pixels by more
  ([ADR-0028](docs/adr/0028-image-tests-tolerate-a-few-pixels.md),
  [ADR-0030](docs/adr/0030-image-tests-strict-on-the-reference-driver.md)). The tests print what
  they measured with `--nocapture` (how many pixels differ by more than 2, 4, 8 and 16), and the
  Windows CI job runs them that way.
- `Gpu::render` takes a `RenderPassDescriptor` (the target, whether to clear it, an optional depth
  target, an optional `GpuTimer`) instead of a target and a clear color.
  `RenderPipelineDescriptor` gains `bind_group_layouts`, `depth` and `cull`, and pipeline layouts
  are always explicit: a shader binding that no layout holds is rejected as `GfxError::Pipeline`.
- The default log filter also hides the error that winit's client-side decorations log when no
  session bus or desktop portal answers (`sctk_adwaita::config=off`,
  [ADR-0019](docs/adr/0019-default-log-filter.md)).
- The Windows CI artifact also contains `offscreen.exe` and `scene.exe`; the Windows job runs the
  release build of the headless samples, and runs the tests one at a time, since WARP's shader
  compiler crashes now and then when several threads compile shaders at once.
- `scripts/check.sh` also runs the `scene` example and the windowed lifecycle test of `slime-app`
  in the headless Weston ([ADR-0024](docs/adr/0024-windowed-lifecycle-check-in-weston.md)), and
  `cargo deny` checks the dependencies of the supported targets only
  ([ADR-0023](docs/adr/0023-dependency-checks-for-the-supported-targets.md)).
- The `window` example exits with a failure when a run limited by `--frames` drew nothing, so
  that the windowed check in a headless Weston proves that frames are drawn, as
  [ADR-0013](docs/adr/0013-windowed-smoke-test-in-weston.md) says.

### Removed

- `GfxError::Wait`, which no method could return: wgpu's indefinite wait reports no error.

### Fixed

- On Windows, the log uses colors only in Windows Terminal: the older console host printed the
  escape sequences as text.
- The examples report a command-line argument that is not valid text as a usage error instead of
  panicking, and the `offscreen` example names the file it could not write.
- A windowed run without any usable graphics driver reports `no GPU adapter is available` with
  wgpu's explanation, instead of `cannot create a surface for the window` with an empty cause.
- A `SLIME_LOG` value without a bare level, such as `slime_app=trace`, no longer silences every
  other module: `info` applies to the modules it does not name, as the documentation said. A
  misspelt level, which `tracing` reads as a module name, no longer silences everything either.
- On Wayland and X11, a key release no longer carries the text of the press in `KeyEvent::text`,
  which the documentation promised to be `None` and which Windows already left empty. The
  documentation now says that named keys such as Enter carry their control character.
- `Gpu::wait_idle`, `Gpu::create_shader_module`, `Gpu::create_render_pipeline` and
  `Gpu::read_render_target` return `GfxError::DeviceLost` once the device is lost, as documented,
  instead of `Ok` with objects that cannot be used.
- A `WindowConfig` with a zero width or height, a size above 16 384 logical pixels, or a title
  containing a NUL byte is rejected with a clear message before the window is created, instead of
  panicking inside the windowing library (a zero size on Wayland, a NUL byte on Wayland and X11) or
  overflowing in its decorations.
- A shader module or render pipeline that the graphics driver's compiler rejects, after wgpu's
  own validation accepted it, is returned as `GfxError::Shader` or `GfxError::Pipeline` instead
  of panicking.
- `Gpu::create_vertex_buffer` rejects empty data with a message naming the buffer, instead of
  letting the first `set_vertex_buffer` panic inside wgpu; its rustdoc now lists its panics.
- A windowed run whose `render` draws nothing into the window's image no longer grows at every
  frame (about 180 kB per frame, measured with lavapipe): the engine gives wgpu a submission for
  such frames, without which it never reclaimed the work of the earlier ones.
- Frames that the windowing system asks for on its own (during a resize) or that the application
  requests through `Window::request_redraw` no longer postpone the engine's schedule: after a burst
  of them, the loop went to sleep until the schedule caught up, for seconds, without running frames
  or noticing termination signals.
- A window larger than the images the GPU accepts (8192 pixels in a dimension with the default
  limits) no longer panics the engine: the images are limited to that size, with a warning.
- `FrameClock` no longer panics when the elapsed time would overflow (a maximum frame delta
  near `Duration::MAX`): it saturates.
- Logging to a standard error that is gone (a pipe whose reader exited, a closed terminal) no
  longer aborts the process: the events are dropped and the run goes on.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: the renderer is a first
one (no physically based materials, shadows, HDR, post-processing, multisampling, transparency,
animation or glTF extensions; they come with Stage 3b and later); glTF files are read by a
temporary loader in the examples, which takes GLB files with embedded buffers and PNG images only;
the dev overlay is temporary tooling that gets no modifier keys; mipmaps are computed on the CPU;
upgrading wgpu waits for egui; and the limitations of 0.0.2 remain: frames are not synchronized
with the display's refresh, presenting can still stall where Mailbox is not offered, frames pause
on Windows while the window is moved, wgpu's errors panic in release builds too, a lost GPU device
is not replaced, and modifier state, input methods, touch, gestures and gamepads are not reported.

## [0.0.2] - 2026-10-12

The second release, which closes Stage 2 of the roadmap: the GPU layer. The engine draws a triangle
in its window on Linux (Vulkan) and Windows (Direct3D 12), renders offscreen images that CI
compares with a reference on software drivers, paces frames without stalling when the window is
hidden, names its GPU work for capture tools, and stops cleanly when the GPU device is lost.

### Added

- Windows support for what exists: CI runs clippy, the tests and the headless samples on a Windows
  runner, publishes the examples built for Windows, and the Linux checks compile the code for
  Windows too ([ADR-0015](docs/adr/0015-platforms-and-ci.md)).
- `slime-gfx`, the GPU layer: opens a GPU device without a window, with Vulkan on Linux and
  Direct3D 12 on Windows; the checks run its tests on lavapipe (Linux) and WARP (Windows)
  ([ADR-0016](docs/adr/0016-gpu-layer-with-wgpu.md)).
- Offscreen rendering in `slime-gfx`: render targets, vertex buffers, WGSL shader modules and
  render pipelines as engine-owned types with a label each, a render pass, and reading an image
  back to the CPU. A shader or pipeline that wgpu rejects is returned as an error with the
  compiler's message ([ADR-0017](docs/adr/0017-gpu-resource-types.md)).
- The `offscreen` example, which writes a triangle rendered without a window to a PNG file, and an
  image test that compares the same render with a reference image, within a tolerance, on
  lavapipe, WARP and hardware GPUs ([ADR-0018](docs/adr/0018-reference-image-tests.md)).
- Drawing into the window: windowed runs open the GPU for their window (`Gpu::new_for_window`,
  `Surface`), and the new `App::render` draws each frame into the window's image, which the engine
  then presents. `Context::gpu` and `Context::surface_format` let the application create its GPU
  objects in `init`. The `window` example draws the samples' triangle
  ([ADR-0020](docs/adr/0020-rendering-in-the-main-loop.md)).
- `TextureFormat::Bgra8UnormSrgb`, the usual format of windows; render targets in this format
  read back as RGBA, like the others.
- `AppConfig::present_mode` (`PresentMode::Mailbox` or `PresentMode::Fifo`): by default, presenting
  an image never waits, so a hidden window keeps running at the scheduled rate; the frame scheduler
  paces frames on every platform ([ADR-0021](docs/adr/0021-frame-pacing.md)). The `window` example
  takes `--present-mode`.
- Profiling spans for the drawing part of a frame: `acquire`, `render` with `submit` inside, and
  `present`.
- Names for GPU capture tools such as RenderDoc: `RenderPass::debug_group` names groups of
  commands, and the window's image of each frame is named `window, frame <index>`.
- A lost GPU device (driver crash or reset, GPU removed, out of memory) stops the run cleanly:
  `run` returns `RunError::Gfx` with `GfxError::DeviceLost`, instead of the engine panicking
  ([ADR-0022](docs/adr/0022-losing-the-gpu-device.md)). `Gpu::check` tells whether the device
  is still usable.

### Changed

- The log filter used when `SLIME_LOG` is unset is now `slime_app::DEFAULT_LOG_FILTER`: `info` as
  before, except for wgpu-hal's details about the GPU and the Vulkan loader's messages about
  drivers, whose errors still appear ([ADR-0019](docs/adr/0019-default-log-filter.md)).
- `App::init` returns `Result<(), AppError>`: when it fails, no frame runs, `shutdown` is not
  called, and `run` returns the error. `run` returns a `RunError` (`Init`, `Platform` or `Gfx`)
  instead of a `PlatformError`.
- With a window, frames run on redraw requests: the engine asks for them at most
  `AppConfig::max_frame_rate` times per second, and the windowing system's own requests, during a
  resize for example, run frames too. In `slime-platform`, `Window::request_redraw` asks for one,
  and `EventHandler` has a new required method, `redraw_requested`, through which the event loop
  reports them.
- The windowed check in a headless Weston draws with lavapipe.

### Removed

- `softbuffer`, with which the `window` example filled its window from the CPU.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: no renderer yet (no
depth, textures, lighting or meshes; they come with Stage 3), frames are not synchronized with the
display's refresh, presenting can still stall where Mailbox is not offered, frames pause on
Windows while the window is moved, wgpu's errors panic in release builds too, a lost GPU device is
not replaced, and modifier state, input methods, touch, gestures and gamepads are not reported.

## [0.0.1] - 2026-10-12

The first release, which closes Stage 1 of the roadmap: a very small application that is
architecturally sound. It was tagged `v0.0.1` once keyboard and mouse input had been checked by
hand; its CI run is green.

### Added

- `slime-app`: the `App` trait and `run`, which runs an application headless (frames back to back
  with simulated time, deterministic) or with one window (real time between frames, on average at
  most 60 frames per second by default). The engine owns the main loop
  ([ADR-0010](docs/adr/0010-main-loop-ownership.md)).
- Frame timing: `FrameClock` limits how much time one frame represents, `FixedStep` splits frame
  time into fixed steps and drops the backlog instead of spiralling
  ([ADR-0009](docs/adr/0009-frame-timing.md)).
- Diagnostics: logging through `tracing` filtered by `SLIME_LOG`, panics logged before the
  standard hook runs, a startup line describing the engine and the machine, profiling spans for
  each frame ([ADR-0008](docs/adr/0008-errors-logging-and-assertions.md)).
- Clean shutdown on SIGINT, SIGTERM and SIGHUP; a second signal ends the process at once
  ([ADR-0012](docs/adr/0012-clean-shutdown-on-termination-signals.md)).
- `slime-platform`: one window on Wayland or X11 through winit, with its window, keyboard and mouse
  events as engine-owned types, and the `raw-window-handle` traits
  ([ADR-0011](docs/adr/0011-windowing-with-winit.md)).
- Examples: `hello` (diagnostics), `headless` (a run without a window), `window` (a window filled
  from the CPU, its events logged; also runs headless with `--headless`).
- Development: a Nix shell with a pinned toolchain, `scripts/check.sh` running every check
  including the samples without a display and inside a headless Weston compositor, a GitHub
  Actions workflow running that script, and the decisions recorded in `docs/adr/`.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: no rendering yet (the
`window` example fills its window from the CPU), the frame rate is limited by sleeping, `App`
methods cannot return errors, modifier state, input methods, touch, gestures and gamepads are not
reported, Linux only.
