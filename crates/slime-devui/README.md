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
the overlay once that editor does at least as well. It makes its list of entities again only when
the world's structure moves (`World::structure`). What it edits, and its limits, are in the rustdoc
of `WorldPanel`.

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
  is returned as a command (`slime_world::Command`), which names the entity by its persistent
  identity, and which the application sends to its host, to apply at the start of the next frame.

## Entry points

A presentation creates an `Overlay` in `start`, for the window's format and scale factor, hands it
every event, builds its panels as it extracts its frame, and draws them after its scene:

```rust,no_run
use slime_app::{AppError, Context, Presentation};
use slime_devui::{Overlay, egui};
use slime_gfx::RenderTarget;
use slime_platform::Event;
use slime_world::World;

struct Statistics {
    overlay: Option<Overlay>,
}

impl Presentation for Statistics {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        let (gpu, format, window) = (
            ctx.gpu().expect("windowed"),
            ctx.surface_format().expect("windowed"),
            ctx.window().expect("windowed"),
        );
        self.overlay = Some(Overlay::new(gpu, format, window.scale_factor())?);
        Ok(())
    }

    fn on_event(&mut self, _ctx: &mut Context, event: &Event) -> bool {
        let Some(overlay) = &mut self.overlay else {
            return false;
        };
        // Whether a field of the overlay has the keyboard, before the event: the game does not
        // see the keys typed into it.
        let typing = overlay.wants_keyboard();
        overlay.handle(event);
        typing && matches!(event, Event::Key(_))
    }

    fn extract(&mut self, ctx: &mut Context, _world: &World) {
        let (Some(overlay), Some(window)) = (&mut self.overlay, ctx.window()) else {
            return;
        };
        overlay.build(window.inner_size(), |ctx| {
            egui::Window::new("Statistics").show(ctx, |ui| {
                ui.label("60 frames per second");
            });
        });
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let gpu = ctx.gpu().expect("windowed");
        // ... draw the scene into `target` first ...
        if let Some(overlay) = &mut self.overlay {
            overlay.render(gpu, target);
        }
    }
}
```

- `Overlay::new(gpu, format, scale_factor)`: for targets of the window's format; the scale
  factor sizes the text in logical pixels. An error once the GPU device is lost, as when the run
  replaced it again while the application creates its objects.
- `Overlay::handle(&event)`: every event of the window, before the application reads it;
  `Overlay::wants_pointer()` then says whether the cursor is over a panel, as of the last frame
  built, so that the application leaves clicks and the wheel to the overlay, and
  `Overlay::wants_keyboard()` whether a text field of the overlay has the keyboard or a menu of it
  is open, so that keys typed into it, or Escape closing the menu, do not also play: a
  presentation takes such events for itself (`slime_app::Presentation::on_event`).
- `Overlay::build(size, |ctx| ...)`: builds the panels with egui's `Context`, for an image of
  `size` physical pixels, the window's, when the presentation extracts its frame, the panels
  reading the world; `slime_devui::egui` is the egui the overlay uses. egui sizes a window on the
  first frame that shows it and draws it from the next one. egui may call the closure twice in a
  frame, when it lays the frame out again (on the first frame of a grid or of a window being
  resized), so the closure builds the same panels on every call. `build` is also what hands egui
  the events given to `handle`, which the overlay keeps until then: to hide the overlay, build it
  with no panel rather than not at all.
- `Overlay::render(gpu, target)`: draws the panels built over the target, without the world; a
  frame with no panel submits no work to the GPU. Panels built in a frame that the window could not
  show are replaced by the next ones, whose drawing makes the changes of egui's textures of both.
- `WorldPanel::show(ctx, &world)`, in the closure of `build`: the world panel's window, and the
  commands its widgets ask for, which the application adds to those of the frame, since egui may
  call the closure twice, and sends to its host (`slime_app::Context::send`). The host applies
  them at the start of the next frame, and hands each refusal back (`slime_app::Context::refused`),
  which the application gives the panel with `WorldPanel::refused`, to show it. The panel keeps the
  entity selected by its identity too, and finds it again when a load or a hot reload replaced the
  world. `games/hunnawood` and the `world` example show it on F2.

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
