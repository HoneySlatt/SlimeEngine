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
    /// The format of the targets egui's pipeline draws into.
    format: TextureFormat,
}

impl Overlay {
    /// Creates an overlay for targets of `format`, the window's, on a window with `scale_factor`
    /// physical pixels per logical pixel (`slime_platform::Window::scale_factor`), which sizes
    /// the text.
    ///
    /// # Panics
    ///
    /// If the device is already lost ([`Gpu::check`]): egui's renderer creates its buffers with a
    /// wgpu call that panics then.
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
            format,
        }
    }

    /// Gives the overlay an event of the window. It keeps what egui understands: the cursor, the
    /// mouse buttons and wheel, keys and the text they produce, the window's focus and its scale
    /// factor. The engine does not report the state of the modifier keys yet, so egui gets none
    /// and sees a shortcut as its key alone: Ctrl+Z in a text field undoes nothing, Shift with an
    /// arrow moves the cursor without selecting. A key pressed while Ctrl is held types no text,
    /// so that a shortcut does not type its letter into a text field; on Windows, this includes
    /// Ctrl+Alt used in place of AltGr, whereas AltGr itself types its characters.
    ///
    /// The overlay keeps the events until the next [`render`](Self::render), which hands them to
    /// egui. To hide the overlay, an application calls `render` with no panel rather than not at
    /// all: otherwise the events pile up, and egui gets them all at once when the overlay is
    /// shown again, clicks included.
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
    ///
    /// egui may call `ui` twice in a frame: when the first layout asks to be discarded, as on the
    /// first frame of a grid or of a window being resized, egui lays the frame out again and draws
    /// only the second layout. `ui` therefore builds the same panels on every call.
    ///
    /// A device lost before the overlay uploads the panels, for example when the scene's work
    /// found the loss, makes it skip the frame.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the overlay was created for: a bug in the caller. Also
    /// if the upload of the panels is what finds the device lost: egui's renderer panics then.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, mut ui: impl FnMut(&egui::Context)) {
        assert!(
            target.format() == self.format,
            "the overlay draws into {:?} targets, not into `{}`, which is {:?}",
            self.format,
            target.label(),
            target.format()
        );
        let pixels_per_point = self.input.pixels_per_point();
        let size = [target.width(), target.height()];
        let mut raw_input = egui::RawInput {
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
        // egui gets the scale factor as the screen's own, not as a zoom: when its zoom changes,
        // egui lays the next frame out in the last frame's area rescaled, not in `screen_rect`, as
        // if the window had kept its pixels, and moves the panels into that area for good.
        raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(pixels_per_point);
        let output = self.context.run_ui(raw_input, |root| ui(root.ctx()));
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
        // egui's renderer panics when it uploads its buffers to a lost device. A loss found
        // earlier in the frame, by the scene's work or the uploads above, stops a windowed run at
        // its next check (ADR-0022), so this frame is dropped.
        if gpu.check().is_err() {
            textures.clear();
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dev overlay"),
        });
        let uploads =
            self.renderer
                .update_buffers(device, queue, &mut encoder, &primitives, &screen);
        // Only egui's paint callbacks, which the overlay has none of, record into the encoder or
        // return command buffers. The buffer uploads are queued writes, which wgpu submits ahead
        // of the overlay's pass below: a submission of their own would be an empty one.
        if !uploads.is_empty() {
            queue.submit(uploads.into_iter().chain([encoder.finish()]));
        }
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

    fn gpu() -> Gpu {
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
    }

    /// Draws a frame with one window at `(200, 200)` points, egui's default 340 points wide, and
    /// returns, as egui saw them, the area it laid the frame out in and the window's rectangle.
    fn frame(overlay: &mut Overlay, gpu: &Gpu, target: &RenderTarget) -> (egui::Rect, egui::Rect) {
        let mut seen = None;
        overlay.render(gpu, target, |ctx| {
            let window = egui::Window::new("panel")
                .default_pos(egui::pos2(200.0, 200.0))
                .show(ctx, |ui| ui.label("text"))
                .expect("the window is open");
            seen = Some((ctx.content_rect(), window.response.rect));
        });
        seen.expect("the panels were built")
    }

    #[test]
    fn a_new_scale_factor_keeps_the_panels_in_place() {
        let gpu = gpu();
        let small = gpu.create_render_target("1x target", 800, 600, FORMAT);
        let large = gpu.create_render_target("2x target", 1600, 1200, FORMAT);
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0);
        // egui sizes the window on the first frame and draws it from the second.
        frame(&mut overlay, &gpu, &small);
        let (_, before) = frame(&mut overlay, &gpu, &small);
        assert!(
            before.max.x > 400.0,
            "the window reaches past half the screen"
        );

        // The window moves to a screen with twice the pixels: its logical size stays.
        overlay.handle(&Event::ScaleFactorChanged { scale_factor: 2.0 });
        let (screen, after) = frame(&mut overlay, &gpu, &large);
        assert_eq!(screen.size(), egui::vec2(800.0, 600.0));
        assert_eq!(after.min, before.min, "the window stays where it was");
    }

    #[test]
    fn a_frame_that_egui_lays_out_twice_still_draws_the_panels() {
        let gpu = gpu();
        let target = gpu.create_render_target("overlay target", 200, 200, FORMAT);
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0);
        overlay.handle(&Event::CursorMoved { x: 20.0, y: 20.0 });
        let passes = Cell::new(0);
        let discard = Cell::new(false);
        let panels = |ctx: &egui::Context| {
            passes.set(passes.get() + 1);
            if discard.take() {
                // What egui does itself when a grid first shows or a window starts resizing.
                ctx.request_discard("test");
            }
            egui::Window::new("panel")
                .default_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| ui.label("text"));
        };
        for _ in 0..2 {
            overlay.render(&gpu, &target, panels);
        }
        assert!(overlay.wants_pointer(), "the cursor is over the window");

        passes.set(0);
        discard.set(true);
        overlay.render(&gpu, &target, panels);
        assert_eq!(passes.get(), 2, "egui laid the frame out again");
        assert!(
            overlay.wants_pointer(),
            "the window was drawn in the layout egui kept"
        );
    }

    #[test]
    fn the_panels_are_drawn_over_what_the_target_holds() {
        let gpu = gpu();
        let target = gpu.create_render_target("overlay target", 200, 200, FORMAT);
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0);
        // Opaque, not black, and exact in sRGB: an overlay pass that cleared the target, to black or
        // to transparent black, would not leave this color around the panel.
        let blue = slime_gfx::Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };
        let scene = RenderPassDescriptor {
            target: &target,
            clear: Some(blue),
            depth: None,
            timer: None,
        };
        // egui sizes the window on the first frame and draws it from the second.
        for _ in 0..2 {
            gpu.render(&scene, |_| {});
            overlay.render(&gpu, &target, |ctx| {
                // Without fading in, the window is opaque from its first frame.
                ctx.all_styles_mut(|style| style.animation_time = 0.0);
                egui::Window::new("panel")
                    .default_pos(egui::pos2(10.0, 10.0))
                    .show(ctx, |ui| ui.label("text"));
            });
        }

        let image = gpu.read_render_target(&target).expect("read the target");
        let pixel = |x: usize, y: usize| &image.pixels[(y * 200 + x) * 4..][..4];
        assert_ne!(pixel(20, 20), [0, 0, 255, 255], "the window is drawn");
        assert_eq!(pixel(190, 190), [0, 0, 255, 255], "the scene is kept around it");
    }

    #[test]
    fn a_device_lost_earlier_in_the_frame_drops_the_frame() {
        let gpu = gpu();
        let target = gpu.create_render_target("overlay target", 200, 200, FORMAT);
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0);
        let panels = |ctx: &egui::Context| {
            egui::Window::new("panel").show(ctx, |ui| ui.label("text"));
        };
        overlay.render(&gpu, &target, panels);

        // As when the scene's work, submitted earlier in the frame, finds the device lost.
        gpu.wgpu_device().destroy();
        let _ = gpu.wgpu_device().poll(wgpu::PollType::wait_indefinitely());
        assert!(gpu.check().is_err(), "the loss is reported");
        overlay.render(&gpu, &target, panels);
    }

    #[test]
    #[should_panic(expected = "the overlay draws into Bgra8UnormSrgb targets")]
    fn a_target_of_another_format_is_refused() {
        let gpu = gpu();
        let target = gpu.create_render_target("overlay target", 4, 4, FORMAT);
        let mut overlay = Overlay::new(&gpu, TextureFormat::Bgra8UnormSrgb, 1.0);
        overlay.render(&gpu, &target, |_| {});
    }
}
