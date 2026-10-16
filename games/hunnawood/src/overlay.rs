//! The engine's dev overlay (`slime-devui`) over the game: what each frame costs, shown and hidden
//! with F1, and the world's entities, whose components F2's panel edits. Tooling for whoever works
//! on the game, not part of it.

use slime_devui::{Edit, Overlay, WorldPanel, egui};
use slime_gfx::{Gpu, RenderTarget, TextureFormat};
use slime_platform::Event;
use slime_render::RenderStats;
use slime_world::World;

/// The overlay and what it shows.
pub struct DevOverlay {
    /// The overlay; `None` from the loss of the GPU device until a new one replaces it.
    overlay: Option<Overlay>,
    statistics_shown: bool,
    panel: WorldPanel,
    panel_shown: bool,
    /// The frame's duration, in milliseconds, averaged over the last frames.
    frame_ms: f32,
}

/// What the overlay asks of the game once drawn.
#[derive(Default)]
pub struct Asked {
    /// Whether its button asks to lose the GPU device, which the game then does once the frame is
    /// drawn, to check by hand that it goes on with a new one.
    pub lose_device: bool,
    /// The world panel's edits, which the game applies to its world.
    pub edits: Vec<Edit>,
}

impl DevOverlay {
    /// The overlay, hidden, for targets of `format` in a window of `scale_factor`.
    pub fn new(gpu: &Gpu, format: TextureFormat, scale_factor: f64) -> Self {
        Self {
            overlay: Some(Overlay::new(gpu, format, scale_factor)),
            statistics_shown: false,
            panel: WorldPanel::default(),
            panel_shown: false,
            frame_ms: 0.0,
        }
    }

    /// Drops the overlay's GPU objects, once the GPU device is lost; it draws nothing until
    /// [`DevOverlay::restore`].
    pub fn release(&mut self) {
        self.overlay = None;
    }

    /// Makes the overlay again with `gpu`, a device that replaced a lost one, for targets of
    /// `format` in a window of `scale_factor`: its panels shown or hidden as they were.
    pub fn restore(&mut self, gpu: &Gpu, format: TextureFormat, scale_factor: f64) {
        self.overlay = Some(Overlay::new(gpu, format, scale_factor));
    }

    /// Hands `event` to the overlay, shown or not, so that it takes events as they come.
    pub fn handle(&mut self, event: &Event) {
        if let Some(overlay) = &mut self.overlay {
            overlay.handle(event);
        }
    }

    /// Whether a text field of the overlay has the keyboard ([`Overlay::wants_keyboard`]).
    pub fn wants_keyboard(&self) -> bool {
        self.overlay.as_ref().is_some_and(Overlay::wants_keyboard)
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

    /// Shows in the world panel why the world refused one of its edits.
    pub fn refused(&mut self, error: &dyn std::fmt::Display) {
        self.panel.refused(error);
    }

    /// Draws the overlay over `target`, with `stats`, the last frame's, and the entities of
    /// `world`, and returns what it asks of the game.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        stats: &RenderStats,
        world: &World,
    ) -> Asked {
        let (frame_ms, adapter) = (self.frame_ms, gpu.adapter_name());
        let (statistics_shown, panel_shown) = (self.statistics_shown, self.panel_shown);
        let mut asked = Asked::default();
        let (Some(overlay), panel) = (&mut self.overlay, &mut self.panel) else {
            return asked;
        };
        let size = (target.width(), target.height());
        // Drawn with no panel while hidden, so that egui still takes the events handed to it.
        overlay.render(gpu, target, |ctx| {
            if panel_shown {
                asked.edits.extend(panel.show(ctx, world));
            }
            if !statistics_shown {
                return;
            }
            egui::Window::new("Hunna Wood")
                .default_pos(egui::pos2(12.0, 12.0))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(adapter);
                    ui.label(format!("{} × {} pixels", size.0, size.1));
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
                    asked.lose_device |= ui.button("Lose the GPU device").clicked();
                    ui.label("F1 hides this");
                });
        });
        asked
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
                let mut edits = Vec::new();
                ctx.run_ui(egui::RawInput::default(), |ui| {
                    edits.extend(panel.show(ui.ctx(), &world));
                })
                .drop_without_applying_deltas();
                assert_eq!(edits, [], "nothing is touched");
            }
        }
    }
}
