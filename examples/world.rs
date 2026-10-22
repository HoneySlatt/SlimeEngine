//! Draws the examples' yard from a world (`docs/adr/0084-what-the-world-draws.md`): a scene file
//! that places the cooked sample scene and four lamps, a prefab of its own, one lamp red and one
//! without its light (`docs/adr/0095-cooked-scene-files.md`). It is spawned in the world that
//! `slime-app` keeps, through a library of the cooked scenes, its meshes loaded by an asset cache
//! in the background, and drawn through `slime-world-render`, its sun given once, to the sky and to
//! the lights. The glowing ring floats up and down at the fixed step, its light with it, and each
//! frame draws it between its last two places. The camera turns around the scene, and a second
//! view, orthographic, draws a map of it from above in the window's top right corner
//! (`docs/adr/0126-views-apart-from-the-renderer.md`). After a lost GPU
//! device, it goes on: a new asset cache loads the world's meshes again, and the world is
//! untouched. Besides the yard, the example draws into the yard's view a grid on the ground, hidden
//! by what stands in front of it, and an icon above each light, facing the camera, through an unlit
//! material; a click selects the entity drawn under the cursor, picked by the GPU
//! (`docs/adr/0130-picking.md`), and Tab the next entity that places meshes, whose box both views
//! draw over their image (`docs/adr/0129-drawing-for-the-editor.md`); a tool picks in either view
//! too. F2 shows the dev overlay's world panel, which
//! lists the entities by name and edits their components (`slime-devui`); F3 its panel of the
//! views, which shows the size each draws at and edits its settings: its render scale,
//! anti-aliasing, a mode of debugging that shows normals or shadows, and its passes
//! (`docs/adr/0127-the-settings-of-a-view.md`), which a tool driving the example changes too.
//! Escape, closing the window or Ctrl+C quits.
//!
//! When it quits, it saves its world (`docs/adr/0125-saves-per-scene.md`): what the run changed
//! since the example settled the world that it spawns at the start, what the panel changed. The
//! ring's transform is driven, as the yard marks it (`slime_world::Driven`): the save and the file
//! leave the ring where the yard puts it, and the example would float it again from there. It loads
//! the save into a world spawned anew, and the run fails if that world is not the one saved. It
//! also writes the yard back as a scene file (`docs/adr/0092-a-world-written-back-as-a-scene.md`),
//! spawns that file, and the run fails if the world spawned is not the one written.
//! `SLIME_LOG=debug` shows the save and the file.
//!
//! It reads cooked data, which `slime-cook` makes first:
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example world
//! cargo run --example world -- --frames 120      # closes after 120 frames that drew the whole world
//! cargo run --example world -- --headless --frames 600   # the world without a window, 600 frames
//! cargo run --example world -- --watch           # draws again what changes in the cooked data
//! ```
//!
//! With `--watch`, a change of the cooked data, as `slime-cook watch` makes it, is drawn: the cache
//! loads again the meshes, materials and textures that changed, and a scene that changed, the lamp
//! that the yard places among them, carries the run into the world spawned anew from it
//! (`World::carry`): every lamp follows the lamp but where the yard overrides it.
//!
//! Without `--assets`, it reads `assets` beside the program if there is one, as where CI ships it,
//! and `target/cooked/examples` otherwise. Without a window, nothing is drawn: the world runs, the
//! ring floats, and the run says how far it went.

#[path = "common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "this example takes the sample's sky, exposure and bloom, not its `SampleScene`"
)]
mod sample_scene;
#[path = "common/yard.rs"]
mod yard;

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use slime_app::{
    AppConfig, AppError, Context, PickAnswer, Presentation, Reflected, ShownView, Simulation,
    SimulationContext,
};
use slime_assets::{CookedAssets, ManifestWatch};
use slime_color::LinearColor;
use slime_devui::{Overlay, WorldPanel, edit_reflected, egui};
use slime_gfx::{AddressMode, Filter, Gpu, RenderTarget, SamplerDescriptor, TextureFormat};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, MouseButton, WindowConfig};
use slime_render::glam::{DAffine3, DMat3, DQuat, DVec3, Vec3};
use slime_render::{
    AlphaMode, AssetCache, AssetRequests, Camera, DrawList, Drawings, Instance, Line, Material,
    Mesh, MeshData, Pick, Projection, RenderStats, Renderer, SampledTexture, Scene, UnlitData,
    View, ViewDraw, ViewSettings, Viewport,
};
use slime_world::{Entity, Light, Meshes, PersistentId, SceneLibrary, Tick, Transform, World};
use slime_world_render::FrameBuilder;

use sample_scene::{BLOOM, EXPOSURE, SKY};
use yard::YARD;

const USAGE: &str = "usage: world [--assets <directory>] [--frames <count>] [--headless] [--watch]";

/// The key the yard is spawned under: its entities' identities start with it.
const KEY: &str = "yard";
/// How high the ring floats above and below where the file puts it, in metres.
const FLOAT_HEIGHT: f64 = 0.3;
/// How long the ring takes to rise and fall back.
const FLOAT_PERIOD: f64 = 3.0;
/// Radians per second at which the camera turns around the scene.
const TURN_SPEED: f32 = 0.2;
/// The side of the map, a fraction of the window's shorter side, and its margin from the window's
/// edges, a fraction of its own side.
const MAP_SIDE: f32 = 0.3;
const MAP_MARGIN: f32 = 0.05;
/// The names of the yard's view and of the map's, by which a tool captures them and changes their
/// settings, in the order of `YardView::views` and `YardView::settings`.
const VIEWS: [&str; 2] = ["yard", "map"];
/// How high the grid lies above the ground, in metres, so that the ground does not hide it, its
/// lines' color, and their width, in pixels.
const GRID_HEIGHT: f64 = 0.01;
const GRID_COLOR: LinearColor = LinearColor::rgba(0.8, 0.8, 0.8, 0.35);
const GRID_WIDTH: f32 = 1.0;
/// The side of a light's icon, in metres, which floats that high above its light, out of a lamp's
/// bulb, and the side of its texture, in texels.
const ICON_SIDE: f64 = 0.4;
const ICON_TEXELS: u32 = 32;
/// The color of the selected entity's box, and its width, in pixels.
const SELECTED_COLOR: LinearColor = LinearColor::rgb(1.0, 0.75, 0.1);
const SELECTED_WIDTH: f32 = 2.0;

/// The example's rules: the yard spawned, its ring floating at the fixed step, and, with `--watch`,
/// the run carried into the scenes that changed.
struct Yard {
    /// The cooked data the yard in the world was spawned from: with `--watch`, the library goes
    /// back to it when the scenes that changed cannot be spawned, so that it holds the scenes of
    /// the world that runs.
    assets: CookedAssets,
    /// The scenes of the cooked data, the yard's and those it places.
    scenes: SceneLibrary,
    /// The ring, and the height around which it floats, where the file puts it, if the scene has
    /// one.
    ring: Option<(Entity, f64)>,
    /// The lowest and highest the ring floated.
    floated: (f64, f64),
    /// With `--watch`, what tells when the cooked data changes.
    watch: Option<ManifestWatch>,
}

/// What the example shows of the yard: the renderer and its asset cache while there is a GPU, the
/// camera turning around the yard, the map drawn from above, and the dev overlay's world panel.
struct YardView {
    /// The renderer, while there is a GPU.
    renderer: Option<Renderer>,
    /// The views of the camera turning around the yard and of the map, while there is a GPU.
    views: Option<(View, View)>,
    /// Their settings, which the overlay and a tool change, and which their views take at each
    /// frame: those made again after a lost device keep them.
    settings: [ViewSettings; 2],
    /// Where the last frame drew them into the window's image, by their names.
    shown: Vec<ShownView>,
    /// The dev overlay, while there is a GPU, and its world panel, shown with F2.
    overlay: Option<Overlay>,
    panel: WorldPanel,
    panel_shown: bool,
    /// Whether the overlay shows the views' settings, with F3.
    settings_shown: bool,
    /// The renderer's asset cache, while there is a GPU.
    cache: Option<AssetCache>,
    /// The requests of the cache of a lost device, which the frame builder's handles hold, for the
    /// cache made with the new one to load again (`AssetCache::into_requests`).
    requests: Option<AssetRequests>,
    /// The cooked data that the cache loads from, the last that the watch gave: a cache made
    /// after a lost device loads from it.
    cached: CookedAssets,
    builder: FrameBuilder,
    /// The frame that `extract` built, which `render` draws, and the cameras of the yard's view
    /// and of the map.
    scene: Option<(Scene, Camera, Camera)>,
    /// Whether that frame draws the whole world: every mesh it names held by the cache.
    whole: bool,
    /// The point the camera looks at, how far from it the yard reaches, and the world's structure
    /// when the yard was framed, which framing it again waits for.
    framing: (DVec3, f32),
    framed: Option<Tick>,
    /// How far the camera has turned around the scene, in radians.
    yaw: f32,
    /// How many frames drew the whole world.
    drawn: u64,
    /// The number of frames that draw the whole world after which the example closes itself, if
    /// `--frames` gives one; a run that sets one fails if no frame did.
    frame_limit: Option<u64>,
    /// With `--watch`, what tells the cache when the cooked data changes.
    watch: Option<ManifestWatch>,
    /// The square and the material of the lights' icons, while there is a GPU.
    icon: Option<(Mesh, Material)>,
    /// The entity whose box both views draw over their image, and whether Tab asked for the next.
    selected: Option<Entity>,
    select_next: bool,
    /// Where the cursor is over the window, in its pixels, and the picking of the last click, whose
    /// answer selects what it found.
    cursor: (f64, f64),
    click: Option<Pick>,
    /// The pickings that tools asked for, each with where its answer goes.
    picks: Vec<(Pick, PickAnswer)>,
    /// What the example draws into each view besides the yard, built by `extract`: in the yard's, a
    /// grid on the ground and an icon above each light, facing the camera; in both, the box of the
    /// selected entity, over the image.
    drawings: [Drawings; 2],
}

impl Yard {
    /// Spawns the yard in `world`, as [`start`] does, and finds its ring.
    fn spawn(&mut self, world: &mut World) -> Result<(), AppError> {
        start(world, &mut self.scenes)?;
        self.ring = ring(world);
        Ok(())
    }

    /// With `--watch`, takes the scenes that changed in the cooked data: if one did, the run is
    /// carried into the yard spawned anew from them, saved and loaded into the new one. If they
    /// cannot be spawned, the library goes back to the scenes of the world that runs, which goes
    /// on.
    fn watch(&mut self, world: &mut World) {
        let Some(watch) = &mut self.watch else {
            return;
        };
        let assets = match watch.poll(Instant::now()) {
            Ok(Some(assets)) => assets,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(
                    "the cooked data cannot be read: {error}; the example goes on with what it \
                     loaded"
                );
                return;
            }
        };
        let changed = self.scenes.reload(assets.clone());
        if changed.is_empty() {
            self.assets = assets;
            return;
        }
        // The world as the example starts it from the scenes as they are now, which names the yard:
        // carrying spawns no scene that the example would prepare.
        let carried = made(&mut self.scenes).and_then(|after| {
            let centre = ring(&after).map(|(_, height)| height);
            let carried = world.carry(after, &mut self.scenes, &mut |_, _| {})?;
            Ok((carried, centre))
        });
        match carried {
            Ok(((carried, loaded), centre)) => {
                self.assets = assets;
                if !loaded.set_aside.is_empty() {
                    tracing::warn!(
                        set_aside = ?loaded.set_aside,
                        "the run set aside what the scenes no longer have"
                    );
                }
                *world = carried;
                self.ring = ring(world)
                    .zip(centre)
                    .map(|((ring, _), centre)| (ring, centre));
                tracing::info!(
                    scenes = changed.len(),
                    "the scenes that changed carried the run"
                );
            }
            Err(error) => {
                self.scenes.reload(self.assets.clone());
                tracing::error!(
                    "the scenes that changed cannot be spawned: {error}; the example goes on with \
                     the world it has"
                );
            }
        }
    }
}

impl YardView {
    /// Makes the renderer, its cache and the overlay with the GPU device of `ctx`.
    fn open(&mut self, ctx: &Context) -> Result<(), AppError> {
        if let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        {
            let renderer = Renderer::new(gpu, format)?;
            self.views = Some((
                renderer.create_view(gpu, "yard")?,
                renderer.create_view(gpu, "map")?,
            ));
            let requests = self.requests.take().unwrap_or_default();
            self.cache = Some(AssetCache::with_requests(
                &renderer,
                self.cached.clone(),
                requests,
            ));
            self.icon = Some(icon(gpu, &renderer)?);
            self.renderer = Some(renderer);
            self.overlay = Some(Overlay::new(gpu, format, window.scale_factor())?);
        }
        Ok(())
    }

    /// With `--watch`, once nothing loads, has the cache load again the meshes, materials and
    /// textures that changed in the cooked data, keeping the rest.
    fn watch(&mut self, ctx: &Context) {
        let (Some(watch), Some(gpu), Some(cache)) = (&mut self.watch, ctx.gpu(), &mut self.cache)
        else {
            return;
        };
        if cache.loading() > 0 || cache.reloading() {
            return;
        }
        // The yard's rules warn when the cooked data cannot be read.
        if let Ok(Some(assets)) = watch.poll(Instant::now()) {
            self.cached = assets.clone();
            cache.reload(gpu, assets);
        }
    }
}

/// Spawns the yard of `scenes` in `world` under [`KEY`], its first directional light, the sample's
/// sun, showing in the sky, and settles it: the world as the example makes it at the start of a
/// run, from which its save differs.
fn start(world: &mut World, scenes: &mut SceneLibrary) -> Result<(), AppError> {
    let yard = scenes.resolve(world.registry(), YARD)?;
    world.spawn_scene(KEY, &yard)?;
    let mut sun = false;
    world.query_mut::<&mut Light>(|_, mut light| {
        // Read first: only the lights written are stamped as changed.
        if matches!(*light, Light::Directional { .. })
            && let Light::Directional { in_sky, .. } = &mut *light
        {
            *in_sky = !sun;
            sun = true;
        }
    });
    world.settle();
    Ok(())
}

/// The ring of the yard in `world`, and its height.
fn ring(world: &World) -> Option<(Entity, f64)> {
    let ring = world.find(&PersistentId::new([KEY, "sample", "ring"]))?;
    Some((ring, world.get::<Transform>(ring)?.translation.y))
}

/// A world made as the example starts it ([`start`]).
fn made(scenes: &mut SceneLibrary) -> Result<World, AppError> {
    let mut world = World::new();
    start(&mut world, scenes)?;
    Ok(world)
}

/// Saves `world`, which ran from the yard of `scenes`, as what the run changed since the example
/// made it at the start ([`start`]), loads the save into another world made so, and checks
/// that it is the world saved, entity by entity, floats by their bits; then writes the yard back as
/// a scene file, spawns that file, and checks that the world spawned is the world written. Returns
/// the sizes of the save and of the file in bytes; logs their texts at the `debug` level.
fn round_trip(world: &mut World, scenes: &mut SceneLibrary) -> Result<(usize, usize), AppError> {
    let save = world.save(&BTreeMap::new())?;
    tracing::debug!("the save:\n{save}");
    let mut loaded = made(scenes)?;
    let report = loaded.load(&save, scenes, &mut |_, _| {})?;
    if !report.set_aside.is_empty() {
        return Err(format!("the save set aside {:?}", report.set_aside).into());
    }
    if let Some(difference) = world.first_difference(&loaded) {
        return Err(format!("the world loaded is not the world saved: {difference}").into());
    }
    // The yard written back, then spawned from the file in place of its source.
    let root = world
        .find(&PersistentId::new([KEY]))
        .ok_or("the yard is gone")?;
    let written = world.write_scene(root)?;
    tracing::debug!("the yard written back:\n{}", written.text);
    if !written.left_out.is_empty() {
        return Err(format!("the yard written back left out {:?}", written.left_out).into());
    }
    scenes.insert_file(YARD, "yard", written.text.clone());
    if let Some(difference) = world.first_difference(&made(scenes)?) {
        return Err(format!(
            "the yard spawned from the file is not the world written: {difference}"
        )
        .into());
    }
    Ok((save.len(), written.text.len()))
}

/// The point the camera looks at, and how far the scene reaches from it: the box around where the
/// world places its entities, with a metre of margin, since meshes reach beyond their origins.
fn framing(world: &World) -> (DVec3, f32) {
    let mut low = DVec3::splat(f64::INFINITY);
    let mut high = DVec3::splat(f64::NEG_INFINITY);
    for (entity, _) in world.query::<&Transform>() {
        if let Some(at) = world.global_transform(entity) {
            low = low.min(at.translation);
            high = high.max(at.translation);
        }
    }
    if low.x > high.x {
        return (DVec3::ZERO, 1.0);
    }
    (
        (low + high) / 2.0,
        ((high - low).length() / 2.0 + 1.0) as f32,
    )
}

impl Simulation for Yard {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        self.spawn(world)
    }

    fn fixed_step(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let Some((ring, base)) = self.ring else {
            return;
        };
        // The game time once this step has run.
        let game = ctx.time().game();
        let time = (game.elapsed() + game.step()).as_secs_f64();
        let height = base + FLOAT_HEIGHT * (TAU * time / FLOAT_PERIOD).sin();
        if let Some(mut transform) = world.get_mut::<Transform>(ring) {
            transform.translation.y = height;
        }
        self.floated = (self.floated.0.min(height), self.floated.1.max(height));
    }

    fn update(&mut self, _ctx: &mut SimulationContext<'_>, world: &mut World) {
        // Without a window too, so that a run without one follows the cooked data.
        self.watch(world);
    }

    fn stop(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let time = ctx.time().game().elapsed().as_secs_f64();
        tracing::info!(
            entities = world.len(),
            ring_lowest = self.floated.0,
            ring_highest = self.floated.1,
            "stop"
        );
        if self.ring.is_some() && time > FLOAT_PERIOD && self.floated.0 >= self.floated.1 {
            ctx.exit_with_error("the ring did not float");
        }
        match round_trip(world, &mut self.scenes) {
            Ok((save, file)) => tracing::info!(
                save,
                file,
                "the world, saved and loaded, is the world saved, and the yard written back spawns \
                 the world written"
            ),
            Err(error) => ctx.exit_with_error(error),
        }
    }
}

impl Presentation for YardView {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        self.open(ctx)?;
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        // Escape leaves a text field of the overlay, or closes a menu of it, rather than quit.
        let typing = self.overlay.as_ref().is_some_and(Overlay::wants_keyboard);
        if let Some(overlay) = &mut self.overlay {
            overlay.handle(event);
        }
        if let Event::Key(KeyEvent {
            physical,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        }) = event
        {
            match physical {
                KeyCode::F2 => self.panel_shown = !self.panel_shown,
                KeyCode::F3 => self.settings_shown = !self.settings_shown,
                KeyCode::Tab if !typing => self.select_next = true,
                KeyCode::Escape if !typing => ctx.request_exit(),
                _ => {}
            }
        }
        let pointed = self.overlay.as_ref().is_some_and(Overlay::wants_pointer);
        match *event {
            Event::CursorMoved { x, y } => self.cursor = (x, y),
            Event::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            } if !pointed => self.click = self.pick_at_cursor(),
            _ => {}
        }
        // The yard's rules read no input; the keys typed into the panel are its own.
        typing
    }

    fn extract(&mut self, ctx: &mut Context, world: &World) {
        self.watch(ctx);
        self.yaw += TURN_SPEED * ctx.time().real().delta().as_secs_f32();
        // Framed again when the yard's structure moves, as when the run is carried into new scenes.
        if self.framed != Some(world.structure()) {
            self.framing = framing(world);
            self.framed = Some(world.structure());
        }
        for (_, error) in ctx.refused() {
            self.panel.refused(error);
        }
        let (Some(gpu), Some(cache)) = (ctx.gpu(), self.cache.as_mut()) else {
            return;
        };
        let (center, radius) = self.framing;
        let (sin, cos) = self.yaw.sin_cos();
        let distance = f64::from(radius) * 2.2;
        let camera = Camera {
            eye: center + DVec3::new(f64::from(sin), 0.45, f64::from(cos)) * distance,
            target: center,
            near: radius / 1000.0,
            ..Camera::default()
        };
        // From above, north up: what is farther is as large, as on a map.
        let map = Camera {
            eye: center + DVec3::new(0.0, f64::from(radius) * 2.0, 0.0),
            target: center,
            up: Vec3::NEG_Z,
            projection: Projection::Orthographic {
                height: radius * 1.25,
                far: radius * 4.0,
            },
            near: radius / 1000.0,
            ..Camera::default()
        };
        let frame = match self
            .builder
            .build(world, cache, gpu, ctx.time().game().alpha())
        {
            Ok(frame) => frame,
            Err(error) => {
                ctx.exit_with_error(error);
                return;
            }
        };
        self.whole = frame.loading == 0;
        if std::mem::take(&mut self.select_next) {
            self.selected = next_with_meshes(world, self.selected);
        }
        // The pickings that came back: the click's selects what it found, or nothing; a tool's
        // is given its entity. One that cannot answer, on a lost device, gives up.
        if let Some(picked) = self.click.as_ref().and_then(|pick| pick.take(gpu)) {
            self.click = None;
            if let Ok(picked) = picked {
                self.selected = Entity::from_bits(picked.at(0, 0));
            }
        }
        let mut waiting = Vec::new();
        for (pick, answer) in self.picks.drain(..) {
            match pick.take(gpu) {
                Some(Ok(picked)) => answer.give(Entity::from_bits(picked.at(0, 0))),
                Some(Err(error)) => tracing::warn!("a tool's picking failed: {error}"),
                None => waiting.push((pick, answer)),
            }
        }
        self.picks = waiting;
        if self.selected.is_some_and(|entity| !world.contains(entity)) {
            self.selected = None;
        }
        let selected = self.selected.map_or_else(Vec::new, |entity| {
            box_lines(world, cache, entity, ctx.time().game().alpha())
        });
        let lights = frame.lights.iter().filter_map(|(_, light)| match light {
            slime_render::Light::Point(light) => Some(light.position),
            slime_render::Light::Spot(light) => Some(light.position),
            _ => None,
        });
        let icons = self
            .icon
            .as_ref()
            .map_or_else(Vec::new, |(square, material)| {
                lights
                    .map(|light| {
                        let above = light + DVec3::Y * ICON_SIDE;
                        icon_facing(square, material, above, camera.eye)
                    })
                    .collect()
            });
        self.drawings = [
            Drawings {
                in_scene: DrawList {
                    lines: grid(self.framing),
                    instances: icons,
                },
                over_image: selected.clone(),
            },
            Drawings {
                over_image: selected,
                ..Drawings::default()
            },
        ];
        let mut scene = frame.scene(camera.eye);
        scene.exposure = EXPOSURE;
        scene.bloom = BLOOM;
        self.scene = Some((scene, camera, map));
        let mut commands = Vec::new();
        if let (Some(overlay), Some(window)) = (&mut self.overlay, ctx.window()) {
            let (panel, shown) = (&mut self.panel, self.panel_shown);
            let (settings, views) = (&mut self.settings, &self.views);
            let sizes = views.as_ref().map_or([None; 2], |(yard, map)| {
                [yard.render_size(), map.render_size()]
            });
            let settings_shown = self.settings_shown;
            // Built with no panel while hidden, so that egui still takes the events handed to it.
            overlay.build(window.inner_size(), |context| {
                if shown {
                    commands.extend(panel.show(context, world));
                }
                if settings_shown {
                    egui::Window::new("Views")
                        .default_pos(egui::pos2(12.0, 12.0))
                        .resizable(false)
                        .show(context, |ui| {
                            for ((name, settings), size) in
                                VIEWS.into_iter().zip(settings.iter_mut()).zip(sizes)
                            {
                                ui.heading(name);
                                if let Some((width, height)) = size {
                                    ui.label(format!("drawn at {width} × {height} pixels"));
                                }
                                edit_reflected(ui, name, settings);
                            }
                        });
                }
            });
        }
        // Applied by the host at the next frame's start: the next frame shows them.
        for command in commands {
            ctx.send(command);
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        // What a tool captures is what this frame draws.
        self.shown.clear();
        let (Some(gpu), Some(renderer), Some((yard, map)), Some((scene, camera, map_camera))) = (
            ctx.gpu(),
            self.renderer.as_mut(),
            self.views.as_mut(),
            self.scene.take(),
        ) else {
            return;
        };
        yard.set_settings(self.settings[0]);
        map.set_settings(self.settings[1]);
        let mut draws = vec![ViewDraw::new(yard, camera, target)];
        draws[0].drawings = Some(&self.drawings[0]);
        let side = (target.width().min(target.height()) as f32 * MAP_SIDE) as u32;
        let margin = (side as f32 * MAP_MARGIN) as u32;
        // A window too small for a map draws none.
        if side > 0 {
            let mut corner = ViewDraw::new(map, map_camera, target);
            corner.viewport = Some(Viewport {
                x: target.width() - side - margin,
                y: margin,
                width: side,
                height: side,
            });
            corner.drawings = Some(&self.drawings[1]);
            draws.push(corner);
        }
        if let Err(error) = renderer.render(gpu, &scene, &mut draws) {
            ctx.exit_with_error(error);
            return;
        }
        self.shown = VIEWS
            .into_iter()
            .zip(&draws)
            .map(|(name, draw)| {
                let viewport = draw
                    .viewport
                    .unwrap_or_else(|| Viewport::whole(draw.target));
                ShownView {
                    name: name.to_owned(),
                    x: viewport.x,
                    y: viewport.y,
                    width: viewport.width,
                    height: viewport.height,
                }
            })
            .collect();
        if let Some(overlay) = &mut self.overlay {
            overlay.render(gpu, target);
        }
        if self.whole {
            self.drawn += 1;
        }
        if self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
            ctx.request_exit();
        }
    }

    fn views(&self) -> Vec<ShownView> {
        self.shown.clone()
    }

    fn pick(&mut self, view: &str, (x, y): (u32, u32), answer: PickAnswer) -> Result<(), String> {
        let index = VIEWS
            .iter()
            .position(|name| *name == view)
            .ok_or_else(|| format!("the example draws no view `{view}`"))?;
        let views = self
            .views
            .as_mut()
            .ok_or("the example has no GPU device to pick with")?;
        let view = if index == 0 {
            &mut views.0
        } else {
            &mut views.1
        };
        self.picks.push((view.pick(pixel(x, y)), answer));
        Ok(())
    }

    fn reports(&self) -> Vec<(String, Reflected)> {
        let mut reports = Vec::new();
        if let Some(cache) = &self.cache {
            reports.push(("assets".to_owned(), Reflected::new(&cache.report())));
        }
        if let Some((yard, map)) = &self.views {
            let stats: Vec<(String, RenderStats)> = VIEWS
                .into_iter()
                .map(str::to_owned)
                .zip([yard.stats(), map.stats()])
                .collect();
            reports.push(("views".to_owned(), Reflected::new(&stats)));
        }
        reports
    }

    fn view_settings(&self, name: &str) -> Option<Reflected> {
        let index = VIEWS.iter().position(|&view| view == name)?;
        Some(Reflected::new(&self.settings[index]))
    }

    fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
        let index = VIEWS
            .iter()
            .position(|&view| view == name)
            .ok_or_else(|| format!("the example has no view `{name}`"))?;
        self.settings[index] = settings.get().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every GPU object goes, the frame not drawn with them; the world stays as it is.
        self.renderer = None;
        self.views = None;
        self.shown.clear();
        self.overlay = None;
        self.requests = self.cache.take().map(AssetCache::into_requests);
        self.scene = None;
        self.builder.release();
        self.icon = None;
        self.drawings = Default::default();
        // Their views are gone: no answer comes.
        self.click = None;
        self.picks.clear();
    }

    fn gpu_restored(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        self.open(ctx)
    }

    fn stop(&mut self, ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "the yard's frames");
        if self.frame_limit.is_some_and(|limit| limit > 0) && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the whole world");
        }
    }
}

impl YardView {
    /// The picking of the pixel under the cursor in the last view that the last frame drew there;
    /// none outside the views.
    fn pick_at_cursor(&mut self) -> Option<Pick> {
        let (x, y) = self.cursor;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (x, y) = (x as u32, y as u32);
        let shown = self.shown.iter().rev().find(|shown| {
            (shown.x..shown.x + shown.width).contains(&x)
                && (shown.y..shown.y + shown.height).contains(&y)
        })?;
        let views = self.views.as_mut()?;
        let view = if shown.name == VIEWS[0] {
            &mut views.0
        } else {
            &mut views.1
        };
        Some(view.pick(pixel(x - shown.x, y - shown.y)))
    }
}

/// The area of the one pixel at `(x, y)` of a view's rectangle.
fn pixel(x: u32, y: u32) -> Viewport {
    Viewport {
        x,
        y,
        width: 1,
        height: 1,
    }
}

/// A grid on the ground around the yard that `framing` gives, its point and its reach: a line each
/// metre along each axis, as far as the yard reaches.
fn grid((center, radius): (DVec3, f32)) -> Vec<Line> {
    let reach = f64::from(radius.ceil());
    let (x, z) = (center.x.round(), center.z.round());
    let line = |from: DVec3, to: DVec3| Line {
        from,
        to,
        color: GRID_COLOR,
        width: GRID_WIDTH,
    };
    (-(reach as i32)..=reach as i32)
        .flat_map(|step| {
            let step = f64::from(step);
            [
                line(
                    DVec3::new(x + step, GRID_HEIGHT, z - reach),
                    DVec3::new(x + step, GRID_HEIGHT, z + reach),
                ),
                line(
                    DVec3::new(x - reach, GRID_HEIGHT, z + step),
                    DVec3::new(x + reach, GRID_HEIGHT, z + step),
                ),
            ]
        })
        .collect()
}

/// The square of a light's icon at `position`, turned to face a camera at `eye`, upright.
fn icon_facing(square: &Mesh, material: &Material, position: DVec3, eye: DVec3) -> Instance {
    // The square faces +Z: its +Z turned towards the camera, its +Y kept up.
    let back = (eye - position).normalize_or(DVec3::Z);
    let right = DVec3::Y.cross(back).normalize_or(DVec3::X);
    let rotation = DQuat::from_mat3(&DMat3::from_cols(right, back.cross(right), back));
    let mut instance = Instance::new(
        square,
        material,
        DAffine3::from_scale_rotation_translation(DVec3::splat(ICON_SIDE), rotation, position),
    );
    instance.casts_shadows = false;
    instance
}

/// The square of the lights' icons and its material, unlit and blended, a bright disc whose edge
/// fades over a texel, drawn by `renderer`.
fn icon(gpu: &Gpu, renderer: &Renderer) -> Result<(Mesh, Material), AppError> {
    let middle = ICON_TEXELS as f32 / 2.0;
    let texels: Vec<u8> = (0..ICON_TEXELS * ICON_TEXELS)
        .flat_map(|index| {
            let (x, y) = ((index % ICON_TEXELS) as f32, (index / ICON_TEXELS) as f32);
            let distance = (x + 0.5 - middle).hypot(y + 0.5 - middle);
            let alpha = (middle - 1.5 - distance + 0.5).clamp(0.0, 1.0);
            // A white heart in a warm ring.
            let warm = if distance < middle * 0.45 { 255 } else { 190 };
            [255, 230, warm, (alpha * 255.0).round() as u8]
        })
        .collect();
    let texture = gpu.create_texture(
        "light icon",
        ICON_TEXELS,
        ICON_TEXELS,
        TextureFormat::Rgba8UnormSrgb,
        &texels,
    )?;
    let sampler = gpu.create_sampler(
        "light icon",
        &SamplerDescriptor {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
        },
    )?;
    let material = renderer.create_unlit_material(
        gpu,
        "light icon",
        &UnlitData {
            texture: Some(SampledTexture {
                texture: &texture,
                sampler: &sampler,
            }),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..UnlitData::default()
        },
    )?;
    let square = renderer.create_mesh(gpu, "light icon", &MeshData::square())?;
    Ok((square, material))
}

/// The entity after `selected` among those that place meshes, in the world's order, the first
/// after the last; none in a world without them.
fn next_with_meshes(world: &World, selected: Option<Entity>) -> Option<Entity> {
    let placing: Vec<Entity> = world.query::<&Meshes>().map(|(entity, _)| entity).collect();
    let after = selected
        .and_then(|selected| placing.iter().position(|&entity| entity == selected))
        .map_or(0, |index| index + 1);
    placing.get(after).or(placing.first()).copied()
}

/// The edges of the box around each mesh of `entity` that `cache` holds, where the world draws it
/// at `fraction` of a fixed step, as the frame builder places it.
fn box_lines(world: &World, cache: &AssetCache, entity: Entity, fraction: f64) -> Vec<Line> {
    let (Some(at), Some(transform), Some(meshes)) = (
        world.interpolated_transform(entity, fraction),
        world.get::<Transform>(entity),
        world.get::<Meshes>(entity),
    ) else {
        return Vec::new();
    };
    let placed = at.to_affine(transform.stretch);
    let mut lines = Vec::new();
    for (mesh, _) in meshes.ids.iter().filter_map(|&id| cache.mesh(id)) {
        let (center, half_size) = mesh.bounds();
        // Corner i has bit 0 set at +x, bit 1 at +y, bit 2 at +z; an edge joins two corners that
        // differ by one bit.
        let corner = |index: usize| {
            let sign = |bit: usize| if index >> bit & 1 == 1 { 1.0 } else { -1.0 };
            placed.transform_point3(
                (center + half_size * Vec3::new(sign(0), sign(1), sign(2))).as_dvec3(),
            )
        };
        for index in 0..8 {
            for bit in 0..3 {
                if index >> bit & 1 == 0 {
                    lines.push(Line {
                        from: corner(index),
                        to: corner(index | 1 << bit),
                        color: SELECTED_COLOR,
                        width: SELECTED_WIDTH,
                    });
                }
            }
        }
    }
    lines
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let directory = options.assets.clone().unwrap_or_else(default_assets);
    let assets = match CookedAssets::open(&directory) {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!(
                "{error}\nfor the sample: cargo run -p slime-cook -- cook examples/assets \
                 target/cooked/examples"
            );
            return ExitCode::FAILURE;
        }
    };
    let config = AppConfig {
        window: (!options.headless).then(|| WindowConfig {
            title: "SlimeEngine world example".to_owned(),
            ..WindowConfig::default()
        }),
        // Without a window, every frame counts; with one, the example counts the frames that drew
        // the whole world itself.
        frame_limit: options.frame_limit.filter(|_| options.headless),
        restore_lost_device: true,
        ..AppConfig::default()
    };
    // Each half follows the cooked data: the rules its scenes, the view its meshes and materials.
    let watch = || {
        options
            .watch
            .then(|| ManifestWatch::new(&assets, Instant::now()))
    };
    let mut yard = Yard {
        scenes: SceneLibrary::of(assets.clone()),
        assets: assets.clone(),
        ring: None,
        floated: (f64::INFINITY, f64::NEG_INFINITY),
        watch: watch(),
    };
    let mut view = YardView {
        renderer: None,
        views: None,
        settings: [ViewSettings::default(); 2],
        shown: Vec::new(),
        overlay: None,
        panel: WorldPanel::default(),
        panel_shown: false,
        settings_shown: false,
        cache: None,
        requests: None,
        cached: assets.clone(),
        builder: FrameBuilder::new(Some(SKY)),
        scene: None,
        whole: false,
        framing: (DVec3::ZERO, 1.0),
        framed: None,
        yaw: 0.6,
        drawn: 0,
        frame_limit: options.frame_limit,
        watch: watch(),
        icon: None,
        selected: None,
        select_next: false,
        cursor: (0.0, 0.0),
        click: None,
        picks: Vec::new(),
        drawings: Default::default(),
    };
    match slime_app::run(&mut yard, &mut view, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The directory of cooked data read without `--assets`: `assets` beside the program, where CI
/// ships the Windows programs with the sample cooked, or else `target/cooked/examples` in the
/// workspace, where the command that the module documents cooks it.
fn default_assets() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|program| Some(program.parent()?.join("assets")));
    match beside {
        Some(directory) if directory.join(slime_assets::MANIFEST_FILE).is_file() => directory,
        _ => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/cooked/examples"
        )),
    }
}

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    assets: Option<PathBuf>,
    frame_limit: Option<u64>,
    headless: bool,
    /// Whether the example draws again what changes in the cooked data.
    watch: bool,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--assets") => {
                options.assets = Some(arguments.next().ok_or("--assets needs a directory")?.into());
            }
            Some("--headless") => options.headless = true,
            Some("--watch") => options.watch = true,
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                options.frame_limit = Some(count);
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(options)
}
