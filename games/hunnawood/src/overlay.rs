//! The engine's dev overlay (`slime-devui`) over the game: what each frame costs, the settings of
//! the hero's view and the game's time, shown and hidden with F1, and the world's entities, whose
//! components F2's panel edits.
//! Tooling for whoever works on the game, not part of it.

use slime_devui::{Overlay, WorldPanel, edit_reflected, egui};
use slime_gfx::{GfxError, Gpu, RenderTarget, TextureFormat};
use slime_platform::{Event, PhysicalSize};
use slime_render::{RenderStats, ViewSettings};
use slime_world::{Command, World};

/// The slowest and the fastest that the statistics' slider sets game time to.
const SPEEDS: std::ops::RangeInclusive<f64> = 0.1..=4.0;

/// The overlay and what it shows.
pub struct DevOverlay {
    /// The overlay, released from the loss of the GPU device until a new one replaces it.
    overlay: Overlay,
    statistics_shown: bool,
    panel: WorldPanel,
    panel_shown: bool,
    /// The frame's duration, in milliseconds, averaged over the last frames.
    frame_ms: f32,
}

/// The speed of the game's time, as the overlay shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSpeed {
    /// Whether it is paused.
    pub paused: bool,
    /// Its speed.
    pub speed: f64,
}

/// What the overlay asks of the game once built.
#[derive(Default)]
pub struct Asked {
    /// Whether its button asks to lose the GPU device, which the game then does once the frame is
    /// drawn, to check by hand that it goes on with a new one.
    pub lose_device: bool,
    /// The world panel's changes, which the game sends to its host, to apply at the next frame's
    /// start.
    pub commands: Vec<Command>,
    /// The speed of game time that its slider set, when it changed it.
    pub speed: Option<f64>,
}

impl DevOverlay {
    /// The overlay, hidden, for targets of `format` in a window of `scale_factor`; an error once
    /// the GPU device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat, scale_factor: f64) -> Result<Self, GfxError> {
        Ok(Self {
            overlay: Overlay::new(gpu, format, scale_factor)?,
            statistics_shown: false,
            panel: WorldPanel::default(),
            panel_shown: false,
            frame_ms: 0.0,
        })
    }

    /// Drops the overlay's GPU objects, once the GPU device is lost, and keeps what it shows and
    /// where the cursor is ([`Overlay::release`]); it draws nothing until [`DevOverlay::restore`].
    pub fn release(&mut self) {
        self.overlay.release();
    }

    /// Makes the overlay's GPU objects again with `gpu`, a device that replaced a lost one, for
    /// targets of `format`: its panels shown or hidden as they were. An error once that device is
    /// lost too.
    pub fn restore(&mut self, gpu: &Gpu, format: TextureFormat) -> Result<(), GfxError> {
        self.overlay.restore(gpu, format)
    }

    /// Hands `event` to the overlay, shown or not, so that it takes events as they come.
    pub fn handle(&mut self, event: &Event) {
        self.overlay.handle(event);
    }

    /// Whether a text field of the overlay has the keyboard ([`Overlay::wants_keyboard`]).
    pub fn wants_keyboard(&self) -> bool {
        self.overlay.wants_keyboard()
    }

    /// Whether the cursor is over a panel shown ([`Overlay::wants_pointer`]).
    pub fn wants_pointer(&self) -> bool {
        (self.statistics_shown || self.panel_shown) && self.overlay.wants_pointer()
    }

    pub fn toggle_statistics(&mut self) {
        self.statistics_shown = !self.statistics_shown;
    }

    pub fn toggle_panel(&mut self) {
        self.panel_shown = !self.panel_shown;
    }

    /// Counts a frame of `seconds`.
    pub fn frame(&mut self, seconds: f32) {
        self.frame_ms = self.frame_ms * 0.9 + seconds * 1000.0 * 0.1;
    }

    /// Shows in the world panel why the world refused one of its changes.
    pub fn refused(&mut self, error: &dyn std::fmt::Display) {
        self.panel.refused(error);
    }

    /// Builds the overlay for an image of `size`, with `stats`, the last frame's, the name of the
    /// GPU's `adapter`, the game's `time`, the settings of the hero's view, which its widgets
    /// change, and the size it is drawn at, and the entities of `world`, and returns what it asks
    /// of the game. [`DevOverlay::render`] draws it.
    pub fn build(
        &mut self,
        size: PhysicalSize,
        stats: &RenderStats,
        adapter: &str,
        time: GameSpeed,
        (settings, render_size): (&mut ViewSettings, Option<(u32, u32)>),
        world: &World,
    ) -> Asked {
        let frame_ms = self.frame_ms;
        let (statistics_shown, panel_shown) = (self.statistics_shown, self.panel_shown);
        let mut asked = Asked::default();
        let (overlay, panel) = (&mut self.overlay, &mut self.panel);
        // Built with no panel while hidden, so that egui still takes the events handed to it.
        overlay.build(size, |ctx| {
            if panel_shown {
                asked.commands.extend(panel.show(ctx, world));
            }
            if !statistics_shown {
                return;
            }
            egui::Window::new("Hunna Wood")
                .default_pos(egui::pos2(12.0, 12.0))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(adapter);
                    ui.label(format!("{} × {} pixels", size.width, size.height));
                    ui.label(format!("frame: {frame_ms:.2} ms"));
                    ui.label(format!(
                        "render on the CPU: {:.2} ms",
                        stats.cpu_time.as_secs_f64() * 1000.0
                    ));
                    for (pass, time) in [
                        ("shadow", stats.shadow_gpu_time),
                        ("spot shadows", stats.spot_shadow_gpu_time),
                        ("scene", stats.scene_gpu_time),
                        ("sky", stats.sky_gpu_time),
                        ("resolve", stats.resolve_gpu_time),
                        ("bloom", stats.bloom_gpu_time),
                        ("tone mapping", stats.tone_mapping_gpu_time),
                    ] {
                        ui.label(match time {
                            Some(time) => {
                                format!("{pass} on the GPU: {:.3} ms", time.as_secs_f64() * 1000.0)
                            }
                            None => format!("{pass} on the GPU: not measured"),
                        });
                    }
                    ui.label(format!(
                        "{} draw calls, {} triangles; {} draw calls for shadows",
                        stats.draw_calls,
                        stats.triangles,
                        stats.shadow_draw_calls + stats.spot_shadow_draw_calls
                    ));
                    ui.separator();
                    if let Some((width, height)) = render_size {
                        ui.label(format!("the view, drawn at {width} × {height} pixels"));
                    }
                    edit_reflected(ui, "view", settings);
                    ui.separator();
                    // Its value is shown as a label rather than in a field that takes the
                    // keyboard, so that the game's keys keep their meaning.
                    let mut speed = time.speed;
                    let shown = if time.paused { "paused" } else { "running" };
                    let label = format!("game time × {speed:.2}, {shown}");
                    ui.add(
                        egui::Slider::new(&mut speed, SPEEDS)
                            .logarithmic(true)
                            .show_value(false)
                            .text(label),
                    );
                    if speed != time.speed {
                        asked.speed = Some(speed);
                    }
                    ui.label("F3 pauses and runs game time, F4 runs one step of it while paused");
                    asked.lose_device |= ui.button("Lose the GPU device").clicked();
                    ui.label("F1 hides this");
                });
        });
        asked
    }

    /// Draws over `target` what [`DevOverlay::build`] made.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget) {
        self.overlay.render(gpu, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tests::every_component;

    #[test]
    fn the_world_panel_is_built_for_every_component_of_the_game() {
        let (world, _) = every_component();
        let entities: Vec<_> = world.entities().collect();
        for name in [
            "hunnawood::Collider",
            "hunnawood::Marker",
            "hunnawood::TimeOfDay",
            "hunnawood::Flickers",
            "hunnawood::Key",
            "hunnawood::Door",
            "hunnawood::Clock",
            "hunnawood::Look",
        ] {
            assert!(
                entities
                    .iter()
                    .any(|&entity| world.component_names(entity).contains(&name)),
                "an entity holds a {name}"
            );
        }
        let ctx = egui::Context::default();
        let mut panel = WorldPanel::default();
        for entity in entities {
            panel.select(entity);
            // egui sizes the window on the first frame.
            for _ in 0..2 {
                let mut commands = Vec::new();
                ctx.run_ui(egui::RawInput::default(), |ui| {
                    commands.extend(panel.show(ui.ctx(), &world));
                })
                .drop_without_applying_deltas();
                assert_eq!(commands, [], "nothing is touched");
            }
        }
    }
}
