//! What each frame costs, in the engine's dev overlay (`slime-devui`), shown and hidden with F1:
//! tooling for whoever works on the game, not part of it.

use slime_devui::{Overlay, egui};
use slime_gfx::{Gpu, RenderTarget, TextureFormat};
use slime_platform::Event;
use slime_render::RenderStats;

/// The overlay and what it shows.
pub struct Statistics {
    overlay: Overlay,
    shown: bool,
    /// The frame's duration, in milliseconds, averaged over the last frames.
    frame_ms: f32,
}

impl Statistics {
    /// The overlay, hidden, for targets of `format` in a window of `scale_factor`.
    pub fn new(gpu: &Gpu, format: TextureFormat, scale_factor: f64) -> Self {
        Self {
            overlay: Overlay::new(gpu, format, scale_factor),
            shown: false,
            frame_ms: 0.0,
        }
    }

    /// Hands `event` to the overlay, shown or not, so that it takes events as they come.
    pub fn handle(&mut self, event: &Event) {
        self.overlay.handle(event);
    }

    pub fn toggle(&mut self) {
        self.shown = !self.shown;
    }

    /// Counts a frame of `seconds`.
    pub fn frame(&mut self, seconds: f32) {
        self.frame_ms = self.frame_ms * 0.9 + seconds * 1000.0 * 0.1;
    }

    /// Draws the overlay over `target`, with `stats`, the last frame's.
    pub fn draw(&mut self, gpu: &Gpu, target: &RenderTarget, stats: &RenderStats) {
        let (shown, frame_ms, adapter) = (self.shown, self.frame_ms, gpu.adapter_name());
        let size = (target.width(), target.height());
        // Drawn with no panel while hidden, so that egui still takes the events handed to it.
        self.overlay.render(gpu, target, |ctx| {
            if !shown {
                return;
            }
            egui::Window::new("Nightwatch")
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
                    ui.label("F1 hides this");
                });
        });
    }
}
