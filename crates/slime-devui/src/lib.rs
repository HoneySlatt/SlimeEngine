#![doc = include_str!("../README.md")]

mod input;
pub mod interface;
mod world_panel;

use std::time::Instant;

use slime_gfx::wgpu;
use slime_gfx::{GfxError, Gpu, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_platform::{Event, PhysicalSize};

/// The UI library the overlay draws, re-exported so that callers build their panels with the
/// same version.
pub use egui;
pub use world_panel::{WorldPanel, edit_reflected};

/// egui drawn over the engine's frames.
///
/// Each frame, the application hands the overlay the window's events ([`handle`](Self::handle)),
/// builds its panels as it extracts its frame ([`build`](Self::build)), then draws its scene and
/// the panels over it ([`render`](Self::render)).
pub struct Overlay {
    context: egui::Context,
    /// egui's renderer, its GPU objects; `None` once released, after a lost device, until restored.
    renderer: Option<egui_wgpu::Renderer>,
    input: input::Input,
    started: Instant,
    /// The format of the targets egui's pipeline draws into.
    format: TextureFormat,
    /// The panels built and not drawn yet.
    built: Option<Built>,
}

/// Panels that [`Overlay::build`] made, which [`Overlay::render`] draws.
struct Built {
    primitives: Vec<egui::ClippedPrimitive>,
    /// The changes of egui's textures, those of panels built and not drawn first.
    textures: egui::TexturesDelta,
    pixels_per_point: f32,
}

impl Drop for Built {
    fn drop(&mut self) {
        // Panels built and never drawn, as when the run stops or a panic unwinds after a frame's
        // extraction: epaint panics at changes of textures dropped unapplied.
        self.textures.clear();
    }
}

impl Overlay {
    /// Creates an overlay for targets of `format`, the window's, on a window with `scale_factor`
    /// physical pixels per logical pixel (`slime_platform::Window::scale_factor`), which sizes
    /// the text.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If the device is lost while egui's renderer creates its buffers, with a wgpu call that
    /// panics on a lost device, between the check and the creation.
    pub fn new(gpu: &Gpu, format: TextureFormat, scale_factor: f64) -> Result<Self, GfxError> {
        gpu.check()?;
        Ok(Self {
            context: egui::Context::default(),
            renderer: Some(renderer(gpu, format)),
            input: input::Input::new(scale_factor),
            started: Instant::now(),
            format,
            built: None,
        })
    }

    /// Drops the overlay's GPU objects, once the GPU device is lost, and keeps the rest: its panels,
    /// what they hold, the menus open and where the cursor is, so that a click after the overlay is
    /// restored, without the cursor moving first, lands where the cursor is. It draws nothing until
    /// [`restore`](Self::restore); it still takes events and builds its panels.
    pub fn release(&mut self) {
        self.renderer = None;
    }

    /// Makes the overlay's GPU objects again with `gpu`, a device that replaced a lost one, for
    /// targets of `format`, after [`release`](Self::release): egui's font texture, the only texture
    /// of its panels, is uploaded whole.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once `gpu` is lost too ([`Gpu::check`]): the overlay stays
    /// released then.
    ///
    /// # Panics
    ///
    /// As [`Overlay::new`] does, if the device is lost between the check and the creation.
    pub fn restore(&mut self, gpu: &Gpu, format: TextureFormat) -> Result<(), GfxError> {
        gpu.check()?;
        let mut renderer = renderer(gpu, format);
        // egui hands its textures over as changes, which the lost renderer took: the next ones
        // change a texture that the new renderer does not have. No font exists before the first
        // frame built, whose changes give the whole texture.
        if self.context.cumulative_pass_nr() > 0 {
            let fonts = egui::epaint::ImageDelta::full(
                self.context.fonts(|fonts| fonts.image()),
                egui::epaint::TextureAtlas::texture_options(),
            );
            renderer.update_texture(
                gpu.wgpu_device(),
                gpu.wgpu_queue(),
                egui::TextureId::default(),
                &fonts,
            );
        }
        self.renderer = Some(renderer);
        self.format = format;
        Ok(())
    }

    /// Gives the overlay an event of the window. It keeps what egui understands: the cursor, the
    /// mouse buttons and wheel, keys and the text they produce, the modifier keys held, the
    /// window's focus and its scale factor. egui's "command" modifier, which its shortcuts use, is
    /// Ctrl. A key pressed while Ctrl is held types no text, so that a shortcut does not type its
    /// letter into a text field; on Windows, this includes Ctrl+Alt used in place of AltGr, whereas
    /// AltGr itself types its characters.
    ///
    /// The overlay keeps the events until the next [`build`](Self::build), which hands them to
    /// egui. To hide the overlay, an application builds it with no panel rather than not at all:
    /// otherwise the events pile up, and egui gets them all at once when the overlay is shown
    /// again, clicks included.
    pub fn handle(&mut self, event: &Event) {
        self.input.handle(event, self.context.zoom_factor());
    }

    /// Whether the cursor is over a panel of the overlay, as of the last frame built: the
    /// application then leaves clicks and the wheel to the overlay.
    pub fn wants_pointer(&self) -> bool {
        self.context.is_pointer_over_egui()
    }

    /// Whether the overlay takes the keyboard, as of the last frame built: a text field has it, a
    /// number typed into included, or a menu or a color picker is open, which Escape closes. The
    /// application then leaves the keys to the overlay, Escape among them, so that typing a name or
    /// closing a menu does not also play.
    pub fn wants_keyboard(&self) -> bool {
        self.context.text_edit_focused() || egui::Popup::is_any_open(&self.context)
    }

    /// Builds this frame's panels with `ui`, for an image of `size` physical pixels, the window's:
    /// what a presentation does when it extracts its frame, the panels reading the world, which
    /// [`render`](Self::render) then draws without it.
    ///
    /// egui may call `ui` twice in a frame: when the first layout asks to be discarded, as on the
    /// first frame of a grid or of a window being resized, egui lays the frame out again and keeps
    /// only the second layout. `ui` therefore builds the same panels on every call.
    ///
    /// Building is also what hands egui the events given to [`handle`](Self::handle). The panels
    /// built replace those that were built and not drawn, as in a frame that the window could not
    /// show, whose changes of egui's textures the next drawing still makes.
    pub fn build(&mut self, size: PhysicalSize, mut ui: impl FnMut(&egui::Context)) {
        let scale_factor = self.input.scale_factor();
        // egui draws a point as its zoom factor times the screen's scale, which the panels' code
        // can change (`egui::Context::set_zoom_factor`, `set_pixels_per_point`): the area, like
        // the cursor's position (`handle`), is in those points. A new zoom takes effect at the
        // start of a frame, in which egui rescales the area itself and the events handled before
        // it are still in the old points, as with egui's own window integration.
        let pixels_per_point = scale_factor * self.context.zoom_factor();
        let mut raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(
                    size.width as f32 / pixels_per_point,
                    size.height as f32 / pixels_per_point,
                ),
            )),
            time: Some(self.started.elapsed().as_secs_f64()),
            events: self.input.take_events(),
            focused: self.input.focused(),
            ..egui::RawInput::default()
        };
        // egui gets the scale factor as the screen's own, not as a zoom: when its zoom changes,
        // egui lays the next frame out in the last frame's area rescaled, not in `screen_rect`, as
        // if the window had kept its pixels, and moves the panels into that area for good.
        raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(scale_factor);
        let output = self.context.run_ui(raw_input, |root| ui(root.ctx()));
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        // The changes of textures of panels built and not drawn come first.
        let mut textures = self
            .built
            .take()
            .map(|mut built| std::mem::take(&mut built.textures))
            .unwrap_or_default();
        textures.append(output.textures_delta);
        self.built = Some(Built {
            primitives,
            textures,
            pixels_per_point: output.pixels_per_point,
        });
    }

    /// Draws the panels that [`build`](Self::build) made over what `target` holds, in a pass that
    /// keeps it: the scene is drawn into the target first. Nothing is drawn when no panel was
    /// built since the last call, nor while the overlay is released: the panels wait for the next
    /// call after it is restored.
    ///
    /// A frame with no panel, as while the application hides the overlay, submits no work to the
    /// GPU. A device lost before the overlay uploads the panels, for example when the scene's work
    /// found the loss, makes it skip the frame.
    ///
    /// # Panics
    ///
    /// If `target` does not have the format the overlay was created for: a bug in the caller. Also
    /// if the upload of the panels is what finds the device lost: egui's renderer panics then.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget) {
        assert!(
            target.format() == self.format,
            "the overlay draws into {:?} targets, not into `{}`, which is {:?}",
            self.format,
            target.label(),
            target.format()
        );
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let Some(mut built) = self.built.take() else {
            return;
        };
        let (primitives, mut textures) = (
            std::mem::take(&mut built.primitives),
            std::mem::take(&mut built.textures),
        );
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [target.width(), target.height()],
            pixels_per_point: built.pixels_per_point,
        };

        let (device, queue) = (gpu.wgpu_device(), gpu.wgpu_queue());
        // The deltas are taken out: epaint checks that none is dropped unapplied.
        for (id, deltas) in std::mem::take(&mut textures.set) {
            for delta in deltas {
                renderer.update_texture(device, queue, id, &delta);
            }
        }
        // egui's renderer panics when it uploads its buffers to a lost device. A loss found
        // earlier in the frame, by the scene's work or the uploads above, stops a windowed run at
        // its next check (ADR-0022), so this frame is dropped.
        if gpu.check().is_err() {
            textures.clear();
            return;
        }
        // A frame with no panel, as while the application hides the overlay, records and submits
        // no pass: the texture uploads above are queued writes, which the next submission takes.
        if !primitives.is_empty() {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dev overlay"),
            });
            let uploads =
                renderer.update_buffers(device, queue, &mut encoder, &primitives, &screen);
            // Only egui's paint callbacks, which the overlay has none of, record into the encoder
            // or return command buffers. The buffer uploads are queued writes, which wgpu submits
            // ahead of the overlay's pass below: a submission of their own would be an empty one.
            if !uploads.is_empty() {
                queue.submit(uploads.into_iter().chain([encoder.finish()]));
            }
            let pass = RenderPassDescriptor {
                target,
                clear: None,
                depth: None,
                timer: None,
                resolve: None,
            };
            gpu.render(&pass, |pass| {
                renderer.render(pass.wgpu_pass(), &primitives, &screen);
            });
        }
        for id in std::mem::take(&mut textures.free) {
            renderer.free_texture(&id);
        }
    }
}

/// egui's renderer, for targets of `format`.
fn renderer(gpu: &Gpu, format: TextureFormat) -> egui_wgpu::Renderer {
    egui_wgpu::Renderer::new(
        gpu.wgpu_device(),
        format.wgpu_format(),
        egui_wgpu::RendererOptions {
            msaa_samples: 1,
            depth_stencil_format: None,
            dithering: true,
            predictable_texture_filtering: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use slime_platform::{ButtonState, MouseButton};

    use super::*;

    #[test]
    fn an_overlay_is_not_created_on_a_lost_device() {
        // egui's renderer panicked, creating its buffers on the lost device.
        let gpu = gpu();
        gpu.lose();
        let overlay = Overlay::new(&gpu, FORMAT, 1.0);
        assert!(
            matches!(overlay, Err(GfxError::DeviceLost { .. })),
            "{:?}",
            overlay.err()
        );
    }

    /// Checked when the tests are compiled (`docs/ARCHITECTURE.md`, "Threads").
    #[test]
    fn the_overlay_can_be_shared_between_threads() {
        fn send_and_sync<T: Send + Sync>() {}
        send_and_sync::<Overlay>();
    }

    const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

    /// Builds the panels of `ui` for an image as large as `target`, and draws them into it: a
    /// frame of a presentation, which builds as it extracts and draws as it renders.
    fn show(
        overlay: &mut Overlay,
        gpu: &Gpu,
        target: &RenderTarget,
        ui: impl FnMut(&egui::Context),
    ) {
        let size = PhysicalSize {
            width: target.width(),
            height: target.height(),
        };
        overlay.build(size, ui);
        overlay.render(gpu, target);
    }

    fn gpu() -> Gpu {
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
    }

    /// Draws a frame with one window at `(200, 200)` points, egui's default 340 points wide, and
    /// returns, as egui saw them, the area it laid the frame out in and the window's rectangle.
    fn frame(overlay: &mut Overlay, gpu: &Gpu, target: &RenderTarget) -> (egui::Rect, egui::Rect) {
        let mut seen = None;
        show(overlay, gpu, target, |ctx| {
            let window = egui::Window::new("panel")
                .default_pos(egui::pos2(200.0, 200.0))
                .show(ctx, |ui| ui.label("text"))
                .expect("the window is open");
            seen = Some((ctx.content_rect(), window.response.rect));
        });
        seen.expect("the panels were built")
    }

    #[test]
    fn egui_knows_when_the_window_loses_and_regains_the_focus() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 64, 64, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        let mut focused = None;
        for (event, expected) in [(Event::Focused(false), false), (Event::Focused(true), true)] {
            overlay.handle(&event);
            show(&mut overlay, &gpu, &target, |ctx| {
                focused = Some(ctx.input(|i| i.focused))
            });
            assert_eq!(focused, Some(expected), "after {event:?}");
        }
    }

    #[test]
    fn a_new_scale_factor_keeps_the_panels_in_place() {
        let gpu = gpu();
        let small = gpu
            .create_render_target("1x target", 800, 600, FORMAT)
            .expect("the GPU has memory for it");
        let large = gpu
            .create_render_target("2x target", 1600, 1200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
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
    fn the_panels_follow_egui_zoom() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 800, 600, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        show(&mut overlay, &gpu, &target, |ctx| ctx.set_zoom_factor(2.0));
        // egui applies the zoom on the next frame, the only one it lays out in an area of its own.
        let mut area = egui::Rect::NOTHING;
        for _ in 0..2 {
            show(&mut overlay, &gpu, &target, |ctx| area = ctx.content_rect());
        }
        assert_eq!(
            area.size(),
            egui::vec2(400.0, 300.0),
            "the panels are laid out in the target, at twice the size"
        );

        overlay.handle(&Event::CursorMoved { x: 100.0, y: 100.0 });
        let mut pointer = None;
        show(&mut overlay, &gpu, &target, |ctx| {
            pointer = ctx.pointer_hover_pos()
        });
        assert_eq!(
            pointer,
            Some(egui::pos2(50.0, 50.0)),
            "the cursor is where the panels are drawn"
        );
    }

    #[test]
    fn a_frame_that_egui_lays_out_twice_still_draws_the_panels() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
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
            show(&mut overlay, &gpu, &target, panels);
        }
        assert!(overlay.wants_pointer(), "the cursor is over the window");

        passes.set(0);
        discard.set(true);
        show(&mut overlay, &gpu, &target, panels);
        assert_eq!(passes.get(), 2, "egui laid the frame out again");
        assert!(
            overlay.wants_pointer(),
            "the window was drawn in the layout egui kept"
        );
    }

    #[test]
    fn the_panels_are_drawn_over_what_the_target_holds() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        // Opaque, not black, and exact in sRGB: an overlay pass that cleared the target, to black or
        // to transparent black, would not leave this color around the panel.
        let blue = slime_color::LinearColor::rgb(0.0, 0.0, 1.0);
        let scene = RenderPassDescriptor {
            target: &target,
            clear: Some(blue),
            depth: None,
            timer: None,
            resolve: None,
        };
        // egui sizes the window on the first frame and draws it from the second.
        for _ in 0..2 {
            gpu.render(&scene, |_| {});
            show(&mut overlay, &gpu, &target, |ctx| {
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
        assert_eq!(
            pixel(190, 190),
            [0, 0, 255, 255],
            "the scene is kept around it"
        );
    }

    #[test]
    fn an_overlay_restored_on_a_new_device_draws_its_panels_and_knows_where_the_cursor_is() {
        let blue = slime_color::LinearColor::rgb(0.0, 0.0, 1.0);
        let panels = |ctx: &egui::Context| {
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            egui::Window::new("panel")
                .default_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| ui.label("text"));
        };
        let frame = |overlay: &mut Overlay, gpu: &Gpu, target: &RenderTarget| {
            let scene = RenderPassDescriptor {
                target,
                clear: Some(blue),
                depth: None,
                timer: None,
                resolve: None,
            };
            gpu.render(&scene, |_| {});
            show(overlay, gpu, target, panels);
        };
        let lost = gpu();
        let target = lost
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&lost, FORMAT, 1.0).expect("the device is not lost");
        overlay.handle(&Event::CursorMoved { x: 20.0, y: 20.0 });
        for _ in 0..2 {
            frame(&mut overlay, &lost, &target);
        }
        overlay.release();
        drop(target);
        lost.lose();

        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        overlay
            .restore(&gpu, FORMAT)
            .expect("the new device is not lost");
        // A click before the cursor moves again: where the cursor was, over the window.
        overlay.handle(&Event::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        });
        frame(&mut overlay, &gpu, &target);
        assert!(overlay.wants_pointer(), "the click is over the window");
        // Every shape that egui draws samples its font texture, which the new device was given.
        let image = gpu.read_render_target(&target).expect("read the target");
        let pixel = |x: usize, y: usize| &image.pixels[(y * 200 + x) * 4..][..4];
        assert_ne!(pixel(20, 20), [0, 0, 255, 255], "the window is drawn");
    }

    #[test]
    fn panels_shown_after_a_hidden_frame_are_drawn() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        // egui hands its font texture over on its first frame, which draws nothing here: the
        // panels drawn later sample it.
        show(&mut overlay, &gpu, &target, |_| {});
        for _ in 0..2 {
            show(&mut overlay, &gpu, &target, |ctx| {
                ctx.all_styles_mut(|style| style.animation_time = 0.0);
                egui::Window::new("panel")
                    .default_pos(egui::pos2(10.0, 10.0))
                    .show(ctx, |ui| ui.label("text"));
            });
        }

        let image = gpu.read_render_target(&target).expect("read the target");
        let pixel = &image.pixels[(20 * 200 + 20) * 4..][..4];
        assert_ne!(pixel, [0, 0, 0, 0], "the window is drawn");
    }

    #[test]
    fn panels_built_and_not_drawn_hand_their_textures_to_the_next_drawing() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        let size = PhysicalSize {
            width: 200,
            height: 200,
        };
        // The first frame, whose window cannot be shown, hands egui's font texture over: the
        // panels drawn later sample it.
        overlay.build(size, |_| {});
        for _ in 0..2 {
            overlay.build(size, |ctx| {
                ctx.all_styles_mut(|style| style.animation_time = 0.0);
                egui::Window::new("panel")
                    .default_pos(egui::pos2(10.0, 10.0))
                    .show(ctx, |ui| ui.label("text"));
            });
        }
        overlay.render(&gpu, &target);
        let image = gpu.read_render_target(&target).expect("read the target");
        let pixel = &image.pixels[(20 * 200 + 20) * 4..][..4];
        assert_ne!(pixel, [0, 0, 0, 0], "the window is drawn");
        // Drawn once: nothing is built since.
        overlay.render(&gpu, &target);
    }

    #[test]
    fn panels_built_for_a_minimized_window_of_no_size_are_kept_until_it_shows_again() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        let panels = |ctx: &egui::Context| {
            egui::Window::new("panel")
                .default_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| ui.label("text"));
        };
        // A minimized window has no size on Windows: its frames are built and not drawn.
        let none = PhysicalSize {
            width: 0,
            height: 0,
        };
        for _ in 0..3 {
            overlay.build(none, panels);
        }
        for _ in 0..2 {
            show(&mut overlay, &gpu, &target, panels);
        }
        let image = gpu.read_render_target(&target).expect("read the target");
        let pixel = &image.pixels[(20 * 200 + 20) * 4..][..4];
        assert_ne!(pixel, [0, 0, 0, 0], "the window is drawn");
    }

    #[test]
    fn a_device_lost_earlier_in_the_frame_drops_the_frame() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 200, 200, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        let panels = |ctx: &egui::Context| {
            egui::Window::new("panel").show(ctx, |ui| ui.label("text"));
        };
        show(&mut overlay, &gpu, &target, panels);

        // As when the scene's work, submitted earlier in the frame, finds the device lost.
        gpu.wgpu_device().destroy();
        let _ = gpu.wgpu_device().poll(wgpu::PollType::wait_indefinitely());
        assert!(gpu.check().is_err(), "the loss is reported");
        show(&mut overlay, &gpu, &target, panels);
    }

    #[test]
    #[should_panic(expected = "the overlay draws into Bgra8UnormSrgb targets")]
    fn a_target_of_another_format_is_refused() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 4, 4, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay =
            Overlay::new(&gpu, TextureFormat::Bgra8UnormSrgb, 1.0).expect("the device is not lost");
        show(&mut overlay, &gpu, &target, |_| {});
    }

    #[test]
    fn a_text_field_or_an_open_menu_takes_the_keyboard() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("overlay target", 400, 300, FORMAT)
            .expect("the GPU has memory for it");
        let mut overlay = Overlay::new(&gpu, FORMAT, 1.0).expect("the device is not lost");
        let mut text = String::new();
        let mut rects = (egui::Rect::NOTHING, egui::Rect::NOTHING);
        let mut frame = |overlay: &mut Overlay| {
            show(overlay, &gpu, &target, |ctx| {
                egui::Window::new("panel")
                    .default_pos(egui::pos2(20.0, 20.0))
                    .show(ctx, |ui| {
                        let field = ui.text_edit_singleline(&mut text).rect;
                        let menu = egui::ComboBox::from_id_salt("menu")
                            .selected_text("one")
                            .show_ui(ui, |ui| ui.label("two"))
                            .response
                            .rect;
                        rects = (field, menu);
                    });
            });
            rects
        };
        // egui sizes the window on the first frame and draws it from the second.
        frame(&mut overlay);
        let (field, menu) = frame(&mut overlay);
        let mut click = |overlay: &mut Overlay, at: egui::Pos2| {
            overlay.handle(&Event::CursorMoved {
                x: f64::from(at.x),
                y: f64::from(at.y),
            });
            for state in [ButtonState::Pressed, ButtonState::Released] {
                overlay.handle(&Event::MouseButton {
                    button: MouseButton::Left,
                    state,
                });
                frame(overlay);
            }
        };
        assert!(!overlay.wants_keyboard());
        click(&mut overlay, field.center());
        assert!(overlay.wants_keyboard(), "the text field has the keyboard");
        click(&mut overlay, egui::pos2(390.0, 290.0));
        assert!(!overlay.wants_keyboard(), "a click elsewhere takes it away");
        click(&mut overlay, menu.center());
        assert!(overlay.wants_keyboard(), "the open menu takes Escape");
    }
}
