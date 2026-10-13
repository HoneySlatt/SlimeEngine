# slime-devui

The dev overlay: [egui](https://github.com/emilk/egui) drawn over the engine's frames, to show
statistics and tooling while the engine has no UI framework of its own.

## Status

Temporary, tooling only (`docs/adr/0031-dev-overlay-with-egui.md`). The editor's UI framework
(Stage 6 of `docs/ROADMAP.md`) replaces it, and this crate is deleted then. It draws egui panels
over a frame and feeds egui the window's events; it does not report modifier keys, since the
engine does not yet, nor the clipboard, cursor icons or input methods. egui warns once at startup
that the framebuffer is sRGB, since it prefers a linear one for its blending: the panels look
slightly lighter than egui's own demos.

## Role

- Draws egui with egui's own wgpu renderer, over the frame the application has drawn, in a pass
  that keeps the frame. It is the only crate besides `slime-gfx` that reaches wgpu, through the
  `wgpu` feature of `slime-gfx`.
- Translates the engine's events (`slime_platform::Event`) into egui's: the cursor and mouse
  buttons in logical pixels (egui's points), the wheel, keys and the text they produce, focus, the
  scale factor.
  egui's own window integration (`egui-winit`) is not used: winit's types stay inside
  `slime-platform`.

## Entry points

An application creates an `Overlay` in `init`, for the window's format and scale factor, hands it
every event, and draws its panels after its scene:

```rust,no_run
use slime_app::{App, AppError, Context};
use slime_devui::{Overlay, egui};
use slime_gfx::RenderTarget;
use slime_platform::Event;

struct Game {
    overlay: Option<Overlay>,
}

impl App for Game {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (gpu, format, window) = (
            ctx.gpu().expect("windowed"),
            ctx.surface_format().expect("windowed"),
            ctx.window().expect("windowed"),
        );
        self.overlay = Some(Overlay::new(gpu, format, window.scale_factor()));
        Ok(())
    }

    fn on_event(&mut self, _ctx: &mut Context, event: &Event) {
        if let Some(overlay) = &mut self.overlay {
            overlay.handle(event);
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let gpu = ctx.gpu().expect("windowed");
        // ... draw the scene into `target` first ...
        if let Some(overlay) = &mut self.overlay {
            overlay.render(gpu, target, |ctx| {
                egui::Window::new("Statistics").show(ctx, |ui| {
                    ui.label("60 frames per second");
                });
            });
        }
    }
}
```

- `Overlay::new(gpu, format, scale_factor)`: for targets of the window's format; the scale
  factor sizes the text in logical pixels.
- `Overlay::handle(&event)`: every event of the window, before the application reads it;
  `Overlay::wants_pointer()` then says whether the cursor is over a panel, as of the last frame
  drawn, so that the application leaves clicks and the wheel to the overlay.
- `Overlay::render(gpu, target, |ctx| ...)`: builds the panels with egui's `Context` and draws
  them over the target; `slime_devui::egui` is the egui the overlay uses. egui sizes a window
  on the first frame that shows it and draws it from the next one. egui may call the closure
  twice in a frame, when it lays the frame out again (on the first frame of a grid or of a
  window being resized), so the closure builds the same panels on every call.

## Dependencies

- `egui` and `egui-wgpu`: the UI and its wgpu renderer, the latter without its default features,
  which would enable every wgpu backend.
- `slime-gfx`, with its `wgpu` feature: the GPU, the render pass and wgpu itself.
- `slime-platform`: the events to translate.

## Must not

- Be used by the engine's crates: it is a client of them, like the examples.
- Outlive the engine's own UI framework: delete it when Stage 6 delivers one.
