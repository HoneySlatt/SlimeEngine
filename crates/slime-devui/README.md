# slime-devui

The dev overlay: panels on the engine's interface, `slime-ui`, drawn over a program's view, to show
statistics and tooling over a running game
([ADR-0152](../../docs/adr/0152-the-dev-overlay-on-the-engines-interface.md)).

## Status

Tooling only. It was egui until Stage 6b, which rewrote it on the engine's interface, in the
editor's look (`slime_ui::look`), and took egui out of the engine. Its panels float over the
window, as roots of the program's one interface: the run gives that interface the window's events
first, so that a press on a panel, or a key that one of its fields takes, does not reach the game;
agents read and use it through the development channel, and screen readers in a program that
builds their adapter. A focus that a press gave takes no keys, and Tab reaches the game until F6
gives a panel the focus
([ADR-0153](../../docs/adr/0153-tab-and-f6-in-a-games-interface.md)).

Its world panel, `WorldPanel`, lists a world's entities, shows the persistent identity of the one
selected and, for a placement, its prefab (`docs/adr/0090-stage-5b-scenes-and-prefabs.md`), and
edits its components through reflection, for whoever works on a game, as
`docs/adr/0081-stage-5a-the-world.md` decided. Its list builds only the rows that show, and makes
its list of entities again only when the world's structure moves (`World::structure`). It does not
undo, add, remove or save components, which the editor of Stage 6c is to do. Its editors of
reflected values, `ValueEditor`, edit a value outside a world too, as a view's settings
(`docs/adr/0127-the-settings-of-a-view.md`); their fields change a value only when the user changed
it. What each edits, and its limits, are in their rustdoc.

## Role

- Holds the overlay's interface, in the editor's look, and its panels, which float over the
  window, each opened and closed by a chord, a shortcut of the interface, and by the command that
  the chord gives, which an agent gives too.
- Draws them with the canvas of the view's renderer, after the view, through
  `slime-ui-render`; it reaches no wgpu.
- Lists a world's entities by name and edits the components of the one selected through the
  world's type registry, without knowing their types (`slime-world`, `slime-reflect`). Each change
  is given as a command (`slime_world::Command`), which names the entity by its persistent
  identity, and which the program sends to its host, to apply at the start of the next frame.

## Entry points

A presentation makes an `Overlay`, adds its panels, gives the overlay's interface to its host,
builds the panels that are open as it extracts its frame, and draws them after its view:

```rust,no_run
use slime_app::{Context, Presentation};
use slime_devui::{Corner, Overlay, PanelId, PanelSpec};
use slime_gfx::RenderTarget;
use slime_render::Renderer;
use slime_ui::style::Style;
use slime_ui::tree::{Element, Ui};
use slime_world::World;

struct Statistics {
    overlay: Overlay,
    panel: PanelId,
    /// The view's renderer, made once the window is open.
    renderer: Option<Renderer>,
    frames: u64,
}

impl Statistics {
    fn new() -> Self {
        let mut overlay = Overlay::new();
        let panel = overlay.add_panel(PanelSpec {
            title: "Statistics",
            chord: "F1",
            command: "statistics",
            corner: Corner::TopStart,
            size: [320.0, 200.0],
        });
        Self { overlay, panel, renderer: None, frames: 0 }
    }
}

impl Presentation for Statistics {
    fn ui(&mut self) -> Option<&mut Ui> {
        Some(self.overlay.ui_mut())
    }

    fn extract(&mut self, ctx: &mut Context, _world: &World) {
        // The panels' chords open and close them; the rest is the program's to read.
        let _actions = self.overlay.take_actions();
        if let Some(window) = ctx.window() {
            self.overlay.set_window(window.inner_size(), window.scale_factor());
        }
        self.frames += 1;
        if let Some(slot) = self.overlay.slot(self.panel) {
            let text = format!("{} frames", self.frames);
            self.overlay.ui_mut().build(slot, self.frames, |children| {
                children.add("frames", Element::label(text, Style::default()), |_| {});
            });
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(gpu), Some(renderer)) = (ctx.gpu(), self.renderer.as_mut()) else {
            return;
        };
        // ... draw the view into `target` with `renderer` first ...
        if let Err(error) = self.overlay.draw(gpu, renderer, target) {
            ctx.exit_with_error(error);
        }
    }
}
```

- `Overlay::new()`: an overlay without panels, its interface in the editor's dark look, which
  leaves Tab to the game (`slime_ui::tree::Ui::set_tab_enters`). `Overlay::ui_mut()` is the
  interface that the presentation gives its host (`slime_app::Presentation::ui`).
- `Overlay::add_panel(spec)`: a panel, closed, of a title, a chord and a command that open and
  close it, a corner that it first opens at, and a size. `Overlay::open`, `Overlay::close` and
  `Overlay::toggle` open and close it from the program; a panel keeps its place while it is closed.
- `Overlay::take_actions()`: the actions of the overlay's interface since the last frame, but the
  commands of the panels' chords, which open and close them; the program reads the others, and
  gives them to its `WorldPanel` and `ValueEditor`s.
- `Overlay::set_window(size, scale_factor)`: the window's size and scale factor, each frame, before
  the panels are built; a panel opened before the first lays out once it.
- `Overlay::slot(panel)`: the slot of an open panel, which the program builds what it shows in
  (`slime_ui::tree::Ui::build`), and which scrolls where the panel is smaller.
- `Overlay::draw(gpu, &mut renderer, target)`: lays the overlay out and draws its panels over the
  target with the view's renderer; a frame with no panel open draws nothing.
  `Overlay::release()`, once the GPU device is lost, drops the atlas of its glyphs, which the next
  draw makes again with the new device's renderer; the panels stay.
- `render_figures(&stats)`: the lines of a frame's statistics that the panels of statistics show.
- `WorldPanel::build(&mut ui, parent, &world)`: the world panel built under `parent`, a panel's
  slot, again only once what it shows changed; `WorldPanel::act(&mut actions)` takes from the
  actions those of the panel's elements, and gives the commands that they ask for, which the
  program sends to its host (`slime_app::Context::send`). The host hands each refusal back
  (`slime_app::Context::refused`), which the program gives the panel with `WorldPanel::refused`.
  The panel keeps the entity selected by its identity too, and finds it again when a load or a hot
  reload replaced the world. `games/hunnawood` and the `world` example show it on F2.
- `ValueEditor::build(&mut ui, slot, label, &value)`: the editors of a value outside a world, in a
  slot; `ValueEditor::apply(&actions, &mut value)` sets the value as their actions ask, within its
  fields' ranges. `games/hunnawood`'s overlay, on F1, and the `world` example's panel of the views,
  on F3, edit their views' settings (`slime_render::ViewSettings`) with it.

## Dependencies

- `slime-ui`, with its `editor-look` feature: the interface that the panels are built on, and the
  editor's look, its fonts among it.
- `slime-ui-render` and `slime-render`: the canvas of the view's renderer, which draws the panels,
  and the statistics of a frame that they show.
- `slime-gfx` and `slime-platform`: the GPU and the target drawn over, and the window's size.
- `slime-world` and `slime-reflect`: the world that the world panel lists, and the reflection it
  edits its components and other values through.
- `slime-assets` and `slime-color`: the asset IDs and the colors that the world panel shows as
  such.

## Must not

- Be used by the engine's crates: it is a client of them, like the examples.
- Ship in a game built for shipping: a game leaves it out of that build, as *Hunna Wood* does with
  its `development` feature.
