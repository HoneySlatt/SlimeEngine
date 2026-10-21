//! What the player sees of the game, as a presentation that a host runs
//! (`slime_app::Presentation`, `docs/adr/0107-the-execution-model.md`): the view of the world, the
//! camera behind the hero, and the dev overlay. It reads the world, never writes it: the overlay's
//! edits are commands, which the host applies at the start of the next frame. It holds game time
//! paused until the zone is drawn, so that the hero does not walk where the player cannot see.

use std::time::{Duration, Instant};

use slime_app::{AppError, Context, Presentation};
use slime_assets::{CookedAssets, ManifestWatch};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent};
use slime_render::glam::Vec2;
use slime_render::{RenderStats, ShaderSources, ShaderWatch};
use slime_world::World;

use crate::camera;
use crate::look::Look;
use crate::overlay::{DevOverlay, GameSpeed};
use crate::rules::{self, Game};
use crate::simulation::log_run;
use crate::view::View;
use crate::zone;

/// What the player sees of *Hunna Wood*: the view, made once the window is open and again with a
/// new GPU device, and the dev overlay.
pub struct Screen {
    /// The cooked data, which the view loads its GPU objects from once the window is open, and
    /// again with a new device.
    assets: CookedAssets,
    view: Option<View>,
    /// The dev overlay: the statistics on F1, the world panel on F2.
    overlay: Option<DevOverlay>,
    /// What the last frame cost.
    stats: RenderStats,
    /// How many frames drew the zone.
    drawn: u64,
    /// The number of frames that draw the zone after which the game closes itself, if `--frames`
    /// gives one. A run that sets one fails if no frame drew the zone: the automated checks run the
    /// game for a number of frames in a compositor without a screen, to prove that it draws.
    frame_limit: Option<u64>,
    /// The frame that draws the zone after which the game loses its GPU device on purpose, if
    /// `--lose-device-at` gives one, to check that it goes on with a new one.
    lose_device_at: Option<u64>,
    /// Whether the overlay's button asked to lose the GPU device, which the frame being drawn does.
    lose_asked: bool,
    /// The frames drawn while the zone loads; `None` once they are logged.
    loading: Option<Loading>,
    /// Whether the screen holds game time paused until the zone is drawn.
    holding: bool,
    /// With `--watch`, what tells when the cooked data changes, which the view loads again.
    watch: Option<ManifestWatch>,
    /// With `--watch`, what tells when the renderer's shader files change.
    shader_watch: Option<ShaderWatch>,
}

/// The frames drawn while the zone loads, and the longest interval between the starts of two
/// frames, until the first frame that draws the zone has been followed by another: what loading
/// costs the player, logged once it is over.
#[derive(Default)]
struct Loading {
    /// How many frames were drawn without the zone.
    frames: u64,
    /// When the last frame started.
    last: Option<Instant>,
    longest: Duration,
    /// Whether a frame drew the zone.
    shown: bool,
}

impl Loading {
    /// Logs the frames drawn while the zone loaded, and the longest interval between two of them.
    fn log(&self) {
        tracing::info!(
            frames = self.frames,
            longest_interval = ?self.longest,
            "the zone is drawn"
        );
    }
}

impl Screen {
    /// What the player sees of the world, drawn from `assets`: for `frame_limit` frames of the
    /// zone, if it gives a number, the GPU device lost after the frame `lose_device_at` of the
    /// zone, if it gives one, and the cooked data and the shaders loaded again as they change if
    /// `watch`.
    pub fn new(
        assets: CookedAssets,
        frame_limit: Option<u64>,
        lose_device_at: Option<u64>,
        watch: bool,
    ) -> Self {
        Self {
            watch: watch.then(|| ManifestWatch::new(&assets, Instant::now())),
            shader_watch: watch.then(|| ShaderWatch::new(ShaderSources::DIRECTORY, Instant::now())),
            assets,
            view: None,
            overlay: None,
            stats: RenderStats::default(),
            drawn: 0,
            frame_limit,
            lose_device_at,
            lose_asked: false,
            loading: Some(Loading::default()),
            holding: false,
        }
    }

    /// Makes the view of `world` with the GPU device of `ctx` ([`View::new`]), and holds game time
    /// paused until it draws the zone, unless something else paused it.
    fn open(&mut self, ctx: &mut Context, world: &World) -> Result<(), AppError> {
        let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) else {
            return Err("Hunna Wood needs a window".into());
        };
        let hero = rules::hero_in(world).ok_or("the world has no hero")?;
        self.view = Some(View::new(
            gpu,
            format,
            &self.assets,
            world,
            hero,
            rules::night(world),
        )?);
        if !ctx.paused() {
            ctx.pause();
            self.holding = true;
        }
        Ok(())
    }

    /// The shaders loaded again, with `--watch`, once their files changed.
    fn watch_shaders(&mut self, ctx: &Context) {
        let (Some(shader_watch), Some(gpu), Some(view)) =
            (&mut self.shader_watch, ctx.gpu(), &mut self.view)
        else {
            return;
        };
        match shader_watch.poll(Instant::now()) {
            Ok(Some(sources)) => {
                let started = Instant::now();
                match view.set_shaders(gpu, &sources) {
                    Ok(()) => tracing::info!(
                        time = ?started.elapsed(),
                        "the shaders changed: the frames are drawn with them"
                    ),
                    Err(error) => tracing::warn!(
                        "the shaders cannot be used: {}; the frames are drawn as before",
                        slime_app::error_chain(&error)
                    ),
                }
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(
                "the shaders cannot be read from {}: {error}; the game draws with those it has",
                ShaderSources::DIRECTORY
            ),
        }
    }

    /// The cooked data loaded again by the view, with `--watch`, once it changed: once the zone is
    /// shown, so that the zone loads whole first, and once nothing loads, so that no load is
    /// dropped half-way, which waits for its running tasks. The game's rules carry the run into
    /// the scenes that changed.
    fn watch_assets(&mut self, ctx: &Context) {
        let (Some(watch), Some(gpu), Some(view)) = (&mut self.watch, ctx.gpu(), &mut self.view)
        else {
            return;
        };
        if !view.zone_shown() || !view.settled() {
            return;
        }
        // The rules warn when the cooked data cannot be read.
        if let Ok(Some(assets)) = watch.poll(Instant::now()) {
            view.reload(gpu, assets.clone());
            self.assets = assets;
        }
    }
}

impl Presentation for Screen {
    fn start(&mut self, ctx: &mut Context, world: &World) -> Result<(), AppError> {
        self.open(ctx, world)?;
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        self.overlay = Some(DevOverlay::new(gpu, format, window.scale_factor())?);
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        let Some(overlay) = &mut self.overlay else {
            return false;
        };
        // As of the last frame, before the event changes it.
        let (typing, over_a_panel) = (overlay.wants_keyboard(), overlay.wants_pointer());
        overlay.handle(event);
        if let Event::Key(KeyEvent {
            physical,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        }) = event
        {
            match physical {
                KeyCode::F1 => overlay.toggle_statistics(),
                KeyCode::F2 => overlay.toggle_panel(),
                KeyCode::F3 if !typing => {
                    // The player takes game time over: the screen no longer holds it.
                    self.holding = false;
                    if ctx.paused() {
                        ctx.resume();
                    } else {
                        ctx.pause();
                    }
                }
                KeyCode::F4 if !typing => ctx.step(),
                _ => {}
            }
        }
        // The keys typed into a field of the overlay, or Escape closing a menu of it, and the
        // clicks on its panels, do not also play.
        match event {
            Event::Key(_) => typing,
            Event::MouseButton { .. } | Event::MouseWheel(_) => over_a_panel,
            _ => false,
        }
    }

    fn extract(&mut self, ctx: &mut Context, world: &World) {
        let now = Instant::now();
        if let Some(loading) = &mut self.loading {
            if let Some(last) = loading.last {
                loading.longest = loading.longest.max(now - last);
            }
            loading.last = Some(now);
            // The interval that ends here holds the first frame that drew the zone.
            if loading.shown {
                loading.log();
                self.loading = None;
            }
        }
        self.watch_shaders(ctx);
        self.watch_assets(ctx);
        if let Some(overlay) = &mut self.overlay {
            overlay.frame(ctx.time().real().delta().as_secs_f32());
            for (_, error) in ctx.refused() {
                overlay.refused(error);
            }
        }
        // A handle of the device, which the frame drops: the context changes as the frame goes.
        let (Some(gpu), Some(view)) = (ctx.gpu().cloned(), &mut self.view) else {
            return;
        };
        // What moves is drawn between its last two fixed steps, the share of a step this frame
        // comes after the last one; what glows flickers on game time, so that it stops while the
        // game is paused.
        let game = *ctx.time().game();
        let alpha = game.alpha();
        let seconds = game.elapsed().as_secs_f64() + alpha * game.step().as_secs_f64();
        let hero = rules::hero_in(world);
        let at = hero
            .and_then(|hero| world.interpolated_transform(hero, alpha))
            .map_or(Vec2::ZERO, |at| {
                Vec2::new(at.translation.x as f32, at.translation.z as f32)
            });
        let look = hero
            .and_then(|hero| world.get::<Look>(hero).copied())
            .unwrap_or(Look { yaw: 0.0 });
        let camera = camera::follow(look, at, &zone::colliders(world));
        let night = rules::night(world);
        if let Err(error) = view.extract(&gpu, world, night, alpha, seconds, camera) {
            // The zone cannot be loaded: the game stops with the error, which `run` logs with its
            // causes. The device lost, the frame is skipped: the run opens a new one before the
            // next frame, and `exit_with_error` leaves the loss to it.
            ctx.exit_with_error(error);
            return;
        }
        let (Some(overlay), Some(window)) = (&mut self.overlay, ctx.window()) else {
            return;
        };
        let speed = GameSpeed {
            paused: ctx.paused(),
            speed: ctx.speed(),
        };
        let asked = overlay.build(
            window.inner_size(),
            &self.stats,
            gpu.adapter_name(),
            speed,
            world,
        );
        self.lose_asked |= asked.lose_device;
        if let Some(speed) = asked.speed {
            ctx.set_speed(speed);
        }
        // Applied by the host at the next frame's start: the next frame shows them.
        for command in asked.commands {
            ctx.send(command);
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        // A handle of the device, which the frame drops: the context changes as the frame goes.
        let (Some(gpu), Some(view)) = (ctx.gpu().cloned(), &mut self.view) else {
            return;
        };
        match view.render(&gpu, target) {
            Ok(Some(stats)) => self.stats = stats,
            Ok(None) => return,
            // The GPU's memory cannot hold the targets of the window's new size: the game stops
            // with the error. The device lost, `exit_with_error` leaves the loss to the run.
            Err(error) => {
                ctx.exit_with_error(error);
                return;
            }
        }
        if let Some(overlay) = &mut self.overlay {
            overlay.render(&gpu, target);
        }
        let mut lose_now = std::mem::take(&mut self.lose_asked);
        if view.zone_shown() {
            self.drawn += 1;
            // The zone is drawn: the hero may walk where the player sees.
            if self.holding {
                ctx.resume();
                self.holding = false;
            }
            if let Some(loading) = &mut self.loading {
                loading.shown = true;
            }
            lose_now |= self.lose_device_at == Some(self.drawn);
        } else if let Some(loading) = &mut self.loading {
            loading.frames += 1;
        }
        if lose_now {
            // Once the frame is drawn, as a driver reset would come: the engine opens a new device
            // before the next frame, and calls `gpu_restored`.
            tracing::warn!("the GPU device is lost on purpose");
            gpu.lose();
        }
        if view.zone_shown() && self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
            ctx.request_exit();
        }
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every object of the lost device goes, the view's loads in progress with it, so that the
        // new device opens on the same adapter (ADR-0075). The world, and the run in it, are kept.
        self.view = None;
        if let Some(overlay) = &mut self.overlay {
            overlay.release();
        }
        if let Some(loading) = self.loading.take()
            && loading.shown
        {
            loading.log();
        }
    }

    fn gpu_restored(&mut self, ctx: &mut Context, world: &World) -> Result<(), AppError> {
        self.open(ctx, world)?;
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        if let Some(overlay) = &mut self.overlay {
            overlay.restore(gpu, format, window.scale_factor())?;
        }
        self.loading = Some(Loading::default());
        // A reload in progress went with the lost device: the data it loaded is given again.
        if let Some(watch) = &mut self.watch {
            watch.start_from(&self.assets);
        }
        // The new view's renderer has the embedded shaders: the files that differ are given again.
        if let Some(shader_watch) = &mut self.shader_watch {
            shader_watch.start_again();
        }
        let what =
            "a new GPU device replaces the lost one: the zone loads again, and the run goes on";
        match Game::find(world, false) {
            Ok(game) => log_run(&game, world, what),
            Err(_) => tracing::info!("{what}"),
        }
        Ok(())
    }

    fn stop(&mut self, ctx: &mut Context) {
        // A run that stops right after the zone's first frame, as the automated checks' do, logs
        // the load here, without the interval that holds that frame.
        if let Some(loading) = self.loading.take()
            && loading.shown
        {
            loading.log();
        }
        tracing::info!(
            drawn = self.drawn,
            draw_calls = self.stats.draw_calls,
            instances = self.stats.instances,
            shadow_draw_calls = self.stats.shadow_draw_calls,
            spot_shadow_draw_calls = self.stats.spot_shadow_draw_calls,
            triangles = self.stats.triangles,
            cpu_time = ?self.stats.cpu_time,
            shadow_gpu_time = ?self.stats.shadow_gpu_time,
            spot_shadow_gpu_time = ?self.stats.spot_shadow_gpu_time,
            scene_gpu_time = ?self.stats.scene_gpu_time,
            sky_gpu_time = ?self.stats.sky_gpu_time,
            resolve_gpu_time = ?self.stats.resolve_gpu_time,
            bloom_gpu_time = ?self.stats.bloom_gpu_time,
            tone_mapping_gpu_time = ?self.stats.tone_mapping_gpu_time,
            "the frames"
        );
        if self.frame_limit.is_some_and(|limit| limit > 0) && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the zone");
        }
    }
}
