#![doc = include_str!("../README.md")]

mod input;

use std::time::Instant;

use slime_gfx::wgpu;
use slime_gfx::{Gpu, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_platform::Event;

/// The UI library the overlay draws, re-exported so that callers build their panels with the
/// same version.
pub use egui;

/// egui drawn over the engine's frames.
///
/// Each frame, the application hands the overlay the window's events
/// ([`handle`](Self::handle)), draws its scene, then builds its panels and draws them over the
/// scene ([`render`](Self::render)).
pub struct Overlay {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    input: input::Input,
    started: Instant,
}

impl Overlay {
    /// Creates an overlay for targets of `format`, the window's, on a window with `scale_factor`
    /// physical pixels per logical pixel (`slime_platform::Window::scale_factor`), which sizes
    /// the text.
    pub fn new(gpu: &Gpu, format: TextureFormat, scale_factor: f64) -> Self {
        let renderer = egui_wgpu::Renderer::new(
            gpu.wgpu_device(),
            format.wgpu_format(),
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: true,
                predictable_texture_filtering: false,
            },
        );
        Self {
            context: egui::Context::default(),
            renderer,
            input: input::Input::new(scale_factor),
            started: Instant::now(),
        }
    }

    /// Gives the overlay an event of the window. It keeps what egui understands: the cursor, the
    /// mouse buttons and wheel, keys and the text they produce, the window's focus and its scale
    /// factor. Modifier keys are not reported by the engine yet, so shortcuts with them do not
    /// reach the overlay.
    pub fn handle(&mut self, event: &Event) {
        self.input.handle(event);
    }

    /// Whether the cursor is over a panel of the overlay, as of the last frame drawn: the
    /// application then leaves clicks and the wheel to the overlay.
    pub fn wants_pointer(&self) -> bool {
        self.context.is_pointer_over_egui()
    }

    /// Builds this frame's panels with `ui` and draws them over what `target` holds, in a pass
    /// that keeps it. The scene is drawn into the target first.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, ui: impl FnOnce(&egui::Context)) {
        let pixels_per_point = self.input.pixels_per_point();
        let size = [target.width(), target.height()];
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(
                    size[0] as f32 / pixels_per_point,
                    size[1] as f32 / pixels_per_point,
                ),
            )),
            time: Some(self.started.elapsed().as_secs_f64()),
            events: self.input.take_events(),
            ..egui::RawInput::default()
        };
        self.context.set_pixels_per_point(pixels_per_point);
        let mut ui = Some(ui);
        let output = self.context.run_ui(raw_input, |root| {
            if let Some(ui) = ui.take() {
                ui(root.ctx());
            }
        });
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: output.pixels_per_point,
        };

        let (device, queue) = (gpu.wgpu_device(), gpu.wgpu_queue());
        // The deltas are taken out: epaint checks that none is dropped unapplied.
        let mut textures = output.textures_delta;
        for (id, deltas) in std::mem::take(&mut textures.set) {
            for delta in deltas {
                self.renderer.update_texture(device, queue, id, &delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dev overlay"),
        });
        let uploads =
            self.renderer
                .update_buffers(device, queue, &mut encoder, &primitives, &screen);
        queue.submit(uploads.into_iter().chain([encoder.finish()]));
        let pass = RenderPassDescriptor {
            target,
            clear: None,
            depth: None,
            timer: None,
        };
        gpu.render(&pass, |pass| {
            self.renderer.render(pass.wgpu_pass(), &primitives, &screen);
        });
        for id in std::mem::take(&mut textures.free) {
            self.renderer.free_texture(&id);
        }
    }
}
