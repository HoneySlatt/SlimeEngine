# slime-devui

The dev overlay: [egui](https://github.com/emilk/egui) drawn over the engine's frames, to show
statistics and tooling over a running application.

## Status

Tooling only (`docs/adr/0031-dev-overlay-with-egui.md`). It is removed once what replaces it, the
editor of Stage 6 of `docs/ROADMAP.md`, does at least as well
(`docs/adr/0052-amend-roadmap-before-0-1.md`). It draws egui panels over a frame and feeds egui the
window's events, the modifier keys held included; it does not report the clipboard, cursor icons or
input methods. A key pressed while Ctrl is held types no text, so that a shortcut does not type its
letter into a text field. egui warns once at startup that the framebuffer is sRGB, since it prefers
a linear one for its blending: the panels look slightly lighter than egui's own demos.

Its world panel, `WorldPanel`, lists a world's entities, shows the persistent identity of the one
selected and, for a placement, its prefab (`docs/adr/0090-stage-5b-scenes-and-prefabs.md`), and
edits its components through reflection, for whoever works on a game, as
`docs/adr/0081-stage-5a-the-world.md` decided. It does not undo, add, remove or save components,
which the editor that `docs/ROADMAP.md` plans for Stage 6 is to do; the panel goes with the rest of
the overlay once that editor does at least as well. What it edits, and its limits, are in the
rustdoc of `WorldPanel`.

## Role

- Draws egui with egui's own wgpu renderer, over the frame the application has drawn, in a pass
  that keeps the frame. It is the only crate besides `slime-gfx` that reaches wgpu, through the
  `wgpu` feature of `slime-gfx`.
- Translates the engine's events (`slime_platform::Event`) into egui's: the cursor and mouse
  buttons in egui's points (logical pixels, divided by egui's zoom factor when the panels' code
  zooms with `egui::Context::set_zoom_factor`), the wheel, keys and the text they produce, focus,
  the scale factor.
  egui's own window integration (`egui-winit`) is not used: winit's types stay inside
  `slime-platform`.
- Lists a world's entities by name and edits the components of the one selected through the
  world's type registry, without knowing their types (`slime-world`, `slime-reflect`). Each change
  is returned as an `Edit`, an entity, a component, a property path and a value, which the
  application applies.

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
  drawn, so that the application leaves clicks and the wheel to the overlay, and
  `Overlay::wants_keyboard()` whether a text field of the overlay has the keyboard or a menu of it
  is open, so that keys typed into it, or Escape closing the menu, do not also play.
- `Overlay::render(gpu, target, |ctx| ...)`: builds the panels with egui's `Context` and draws
  them over the target; `slime_devui::egui` is the egui the overlay uses. egui sizes a window
  on the first frame that shows it and draws it from the next one. egui may call the closure
  twice in a frame, when it lays the frame out again (on the first frame of a grid or of a
  window being resized), so the closure builds the same panels on every call. `render` is also
  what hands egui the events given to `handle`, which the overlay keeps until then: to hide the
  overlay, call `render` with no panel rather than not at all; such a frame submits no work to
  the GPU.
- `WorldPanel::show(ctx, &world)`, in the closure of `render`: the world panel's window, and the
  edits its widgets ask for, which the application adds to those of the frame, since egui may
  call the closure twice. Once the frame is drawn, it applies each with
  `World::set_property(edit.entity, &edit.component, &edit.path, edit.value)`, and hands a
  refusal back with `WorldPanel::refused`, which the panel shows. `games/hunnawood` and the
  `world` example show it on F2.

## Dependencies

- `egui` and `egui-wgpu`: the UI and its wgpu renderer, the latter without its default features,
  which would enable every wgpu backend.
- `slime-gfx`, with its `wgpu` feature: the GPU, the render pass and wgpu itself.
- `slime-platform`: the events to translate.
- `slime-world` and `slime-reflect`: the world that the world panel lists, and the reflection it
  edits its components through.
- `slime-assets` and `slime-color`: the asset IDs and the colors that the world panel shows as
  such.

## Must not

- Be used by the engine's crates: it is a client of them, like the examples.
- Outlive what replaces it: remove it once the editor of Stage 6 shows the same at least as well.
