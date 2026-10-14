# Changelog

Notable changes to SlimeEngine, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are git tags (`v0.0.1`, ...);
nothing is promised stable before 1.0 (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## [Unreleased]

### Added

- *Nightwatch*, the first game made with the engine, through its public API only, in
  `games/nightwatch` ([ADR-0055](docs/adr/0055-games-in-the-workspace.md)): so far, a courtyard at
  night under the moon, which the player, a small glowing slime, crosses with the keyboard, stopped
  by its walls and crates, under a camera that follows it, past guards with flashlights and
  floodlights that end the game when they see it, to a gate that wins it; a switch turns a
  floodlight off, and F1 shows what each frame costs. `cargo run -p nightwatch` plays it; CI builds
  it for Windows with the examples.

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
