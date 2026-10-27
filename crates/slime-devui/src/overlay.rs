//! The dev overlay over a program's view
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): an interface in the editor's look
//! whose panels float over the view, drawn after it with the view's renderer.

use slime_gfx::{Gpu, RenderTarget};
use slime_platform::PhysicalSize;
use slime_render::{RenderError, RenderStats, Renderer};
use slime_ui::action::{Action, ActionKind, Intent};
use slime_ui::look::{self, Colors, Density};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, Layer, LayoutDirection, Place, RootSettings, Ui};
use slime_ui_render::UiRenderer;

/// The space between a panel and the window's edges, in logical pixels.
const MARGIN: f32 = 12.0;

/// The dev overlay over a program's view: an interface in the editor's look
/// (`slime_ui::look`), whose panels float over the window, each opened and closed by the program,
/// moved, sized and folded by the user, and drawn over the view's image with the view's renderer
/// ([`Overlay::draw`]).
///
/// It is the presentation's one interface (`slime_app::Presentation::ui`, [`Overlay::ui_mut`]): the
/// run gives it the window's events first, and a press over a panel, or a key that one of its
/// fields or open lists takes, does not reach the game. A focus that a press gave takes no keys,
/// and Tab reaches the game until F6 gives a panel the focus
/// (`docs/adr/0153-tab-and-f6-in-a-games-interface.md`). Agents read and use it through the
/// development channel, and screen readers, where the program builds their adapter, as any
/// interface.
///
/// Its panels hold what the program builds in each one's slot ([`Overlay::slot`]): its figures,
/// a [`ValueEditor`](crate::ValueEditor), a [`WorldPanel`](crate::WorldPanel). A panel is placed
/// in logical pixels, so that it stays where it is as the window's scale factor changes, and opens
/// again where it was when it closed.
///
/// It holds GPU objects, the atlas of its glyphs: [`Overlay::release`] drops them once the device
/// is lost, and the next draw makes them again on the new device. Its panels are kept.
pub struct Overlay {
    ui: Ui,
    /// The root as large as the window, which draws nothing, and which the panels float over.
    window: ElementId,
    renderer: UiRenderer,
    panels: Vec<Panel>,
    /// Whether [`Overlay::set_window`] gave the window's size.
    sized: bool,
}

/// One of an overlay's panels, as [`Overlay::add_panel`] gives it. The overlay's methods panic on
/// the id of another overlay's panel that it has no panel for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelId(usize);

/// The corner of the window that a panel first opens at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// The window's top left corner.
    TopStart,
    /// The window's top right corner.
    TopEnd,
}

/// A panel that an overlay opens and closes, as the program describes it ([`Overlay::add_panel`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelSpec {
    /// Its title, on its bar.
    pub title: &'static str,
    /// The chord that opens and closes it, a shortcut of the overlay's interface: `F1`.
    pub chord: &'static str,
    /// The command that its chord gives, by which an agent opens and closes it too: `statistics`.
    pub command: &'static str,
    /// The corner of the window that it first opens at.
    pub corner: Corner,
    /// Its width and its most height, in logical pixels.
    pub size: [f32; 2],
}

/// What an overlay keeps of a panel.
struct Panel {
    spec: PanelSpec,
    /// Its layer and the slot in it that the program builds in, while it is open.
    open: Option<(ElementId, ElementId)>,
    /// Whether it opens once the overlay knows the window's size: it was opened before.
    waiting: bool,
    /// Where it stood as it last closed, which it opens at again.
    place: Option<Place>,
}

impl Overlay {
    /// An overlay without panels, in the editor's dark look, whose interface leaves Tab to the game
    /// until it holds a focus that takes the keys.
    ///
    /// # Panics
    ///
    /// If the look's documents were not valid, which `slime-ui`'s tests rule out.
    pub fn new() -> Self {
        let mut fonts = Fonts::new();
        look::register_fonts(&mut fonts);
        let mut ui = Ui::new(fonts);
        ui.set_style(
            &[look::style_sheet()],
            &look::themes(Colors::Dark, Density::Comfortable),
        )
        .expect("the look's documents are valid, as slime-ui's tests check");
        ui.set_tab_enters(false);
        let window = ui.add_root(settings([1.0, 1.0], 1.0), Style::default());
        Self {
            ui,
            window,
            renderer: UiRenderer::default(),
            panels: Vec::new(),
            sized: false,
        }
    }

    /// The overlay's interface: whether one of its fields takes the keys ([`Ui::text_input`]),
    /// what it means to agents.
    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// The overlay's interface, to build its panels in, and to give its host
    /// (`slime_app::Presentation::ui`). The overlay holds its tables of shortcuts, its panels'
    /// chords ([`Overlay::add_panel`]), and reads its actions ([`Overlay::take_actions`]): a
    /// program that sets other shortcuts there replaces them.
    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    /// Adds a panel as `spec` describes it, closed, which its chord and its command open and close
    /// ([`Overlay::take_actions`]): at its corner, of its width, and at most its height, less where
    /// the window is smaller.
    ///
    /// # Panics
    ///
    /// If its chord names no key, or another panel's chord or command is the same.
    pub fn add_panel(&mut self, spec: PanelSpec) -> PanelId {
        assert!(
            self.panels.iter().all(|panel| {
                panel.spec.chord != spec.chord && panel.spec.command != spec.command
            }),
            "two panels of the chord {} or the command {}",
            spec.chord,
            spec.command
        );
        self.panels.push(Panel {
            spec,
            open: None,
            waiting: false,
            place: None,
        });
        let mut table = String::from("[global]\n");
        for panel in &self.panels {
            table.push_str(&format!(
                "{:?} = {:?}\n",
                panel.spec.chord, panel.spec.command
            ));
        }
        self.ui
            .set_shortcuts(&[Shortcuts::from_text("the dev overlay's panels", table)])
            .unwrap_or_else(|error| panic!("{error}"));
        PanelId(self.panels.len() - 1)
    }

    /// The actions that the overlay's interface queued since they were last taken, as
    /// [`Ui::take_actions`] gives them, but the commands of the panels' chords, which open and
    /// close them. Escape, the intent to cancel, while one of the panels' elements holds the focus,
    /// takes the focus away, a field committing what it holds, and gives the keys back to the
    /// game; its action stays among those given.
    pub fn take_actions(&mut self) -> Vec<Action> {
        let mut actions = self.ui.take_actions();
        let focus = self.ui.focus();
        if focus.is_some()
            && actions.iter().any(|action| {
                action.kind == ActionKind::Intent(Intent::Cancel) && action.element == focus
            })
        {
            self.ui.set_focus(None);
        }
        actions.retain(|action| {
            let ActionKind::Command(command) = &action.kind else {
                return true;
            };
            let Some(at) = self
                .panels
                .iter()
                .position(|panel| panel.spec.command == command)
            else {
                return true;
            };
            self.toggle(PanelId(at));
            false
        });
        actions
    }

    /// Whether `panel` is open, or opens once the overlay knows the window's size.
    pub fn is_open(&self, panel: PanelId) -> bool {
        let held = &self.panels[panel.0];
        held.open.is_some() || held.waiting
    }

    /// Opens `panel`, or closes it, keeping where it stood.
    pub fn toggle(&mut self, panel: PanelId) {
        if self.is_open(panel) {
            self.close(panel);
        } else {
            self.open(panel);
        }
    }

    /// Opens `panel` where it stood as it last closed, or at its corner, over the window as
    /// [`Overlay::set_window`] last gave it, or once it gives it; nothing happens if it is open.
    /// Its slot is empty until the program builds in it.
    ///
    /// # Panics
    ///
    /// If `panel` is another overlay's, of a place that this one has no panel at.
    pub fn open(&mut self, panel: PanelId) {
        if self.is_open(panel) {
            return;
        }
        if !self.sized {
            self.panels[panel.0].waiting = true;
            return;
        }
        let [width, height] = self
            .ui
            .root_settings(self.window)
            .map_or([1.0; 2], |root| root.size);
        let held = &self.panels[panel.0];
        let spec = held.spec;
        let place = held.place.unwrap_or_else(|| {
            let size = [
                spec.size[0].min(width - 2.0 * MARGIN).max(1.0),
                spec.size[1].min(height - 2.0 * MARGIN).max(1.0),
            ];
            let x = match spec.corner {
                Corner::TopStart => MARGIN,
                Corner::TopEnd => (width - MARGIN - size[0]).max(0.0),
            };
            Place::Floating {
                over: self.window,
                origin: [x, MARGIN],
                size,
            }
        });
        let layer = Layer {
            place,
            modal: false,
            dismiss: false,
        };
        let root = self.ui.open_layer(layer, Style::default());
        let mut slot = None;
        self.ui.build(root, 0, |children| {
            let panel = Element::floating_panel(spec.title, true, Style::default());
            children.add("panel", panel, |panel| {
                // What the panel holds scrolls where the panel is smaller.
                let mut content = Style::default();
                content.layout.direction = FlexDirection::Column;
                content.layout.gap = [4.0, 0.0];
                content.layout.grow = 1.0;
                content.layout.basis = Dimension::Pixels(0.0);
                slot = Some(panel.add_slot("content", Element::scroll(content)));
            });
        });
        let slot = slot.expect("built");
        self.panels[panel.0].open = Some((root, slot));
    }

    /// Closes `panel`, keeping where it stands to open it there again; nothing happens if it is
    /// closed.
    pub fn close(&mut self, panel: PanelId) {
        self.panels[panel.0].waiting = false;
        let Some((root, _)) = self.panels[panel.0].open.take() else {
            return;
        };
        self.panels[panel.0].place = self.ui.layer_place(root);
        self.ui.remove(root);
    }

    /// The slot of `panel` that the program builds what it shows in, while it is open.
    pub fn slot(&self, panel: PanelId) -> Option<ElementId> {
        self.panels[panel.0].open.map(|(_, slot)| slot)
    }

    /// Gives the overlay the window's size, in physical pixels, and its scale factor, which its
    /// panels are laid out over: each frame, before the panels are opened and built.
    pub fn set_window(&mut self, size: PhysicalSize, scale_factor: f64) {
        let scale = scale_factor as f32;
        let logical = [
            size.width.max(1) as f32 / scale,
            size.height.max(1) as f32 / scale,
        ];
        let wanted = settings(logical, scale);
        if self.ui.root_settings(self.window) != Some(wanted) {
            self.ui.set_root(self.window, wanted);
        }
        if !self.sized {
            self.sized = true;
            for at in 0..self.panels.len() {
                if std::mem::take(&mut self.panels[at].waiting) {
                    self.open(PanelId(at));
                }
            }
        }
    }

    /// Lays the overlay out and draws its panels over what `target` holds, the window's image
    /// after the view, with `renderer`, the view's, whose canvas it draws with
    /// (`slime_ui_render::UiRenderer::draw`). A frame with no panel open draws nothing.
    ///
    /// # Errors
    ///
    /// As `UiRenderer::draw`: the GPU out of memory or the device lost, nothing drawn then.
    ///
    /// # Panics
    ///
    /// As `UiRenderer::draw`: if `target` does not have the renderer's format, or holds several
    /// samples per pixel.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        renderer: &mut Renderer,
        target: &RenderTarget,
    ) -> Result<(), RenderError> {
        let (display, _) = self.ui.frame();
        if display.items().is_empty() {
            return Ok(());
        }
        self.renderer
            .draw(gpu, renderer, self.ui.fonts(), &display, |_| None, target)?;
        Ok(())
    }

    /// Drops the overlay's GPU objects, once the GPU device is lost: the next draw makes them
    /// again, with the renderer of the new device. Its panels and what they show are kept.
    pub fn release(&mut self) {
        self.renderer = UiRenderer::default();
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

/// The figures of a frame that the renderer drew, one line each, as the overlays' panels of
/// statistics show them: the CPU's time of the render, the GPU's time of each pass, or "not
/// measured" where the GPU does not time it, and the draw calls, instances and triangles.
pub fn render_figures(stats: &RenderStats) -> Vec<String> {
    let mut lines = vec![format!(
        "render on the CPU: {:.2} ms",
        stats.cpu_time.as_secs_f64() * 1000.0
    )];
    for (pass, time) in [
        ("shadow", stats.shadow_gpu_time),
        ("spot shadows", stats.spot_shadow_gpu_time),
        ("scene", stats.scene_gpu_time),
        ("sky", stats.sky_gpu_time),
        ("resolve", stats.resolve_gpu_time),
        ("bloom", stats.bloom_gpu_time),
        ("tone mapping", stats.tone_mapping_gpu_time),
    ] {
        lines.push(match time {
            Some(time) => format!("{pass} on the GPU: {:.3} ms", time.as_secs_f64() * 1000.0),
            None => format!("{pass} on the GPU: not measured"),
        });
    }
    lines.push(format!(
        "{} draw calls of {} instances, {} triangles; {} draw calls for shadows",
        stats.draw_calls,
        stats.instances,
        stats.triangles,
        stats.shadow_draw_calls + stats.spot_shadow_draw_calls
    ));
    lines
}

/// The settings of the window's root: `size` logical pixels at `scale`, from left to right.
fn settings(size: [f32; 2], scale: f32) -> RootSettings {
    RootSettings {
        size,
        scale,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    }
}

#[cfg(test)]
mod tests {
    use slime_gfx::{GfxError, RenderPassDescriptor, TextureFormat};
    use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
    use slime_render::RenderError;
    use slime_ui::tree::Role;

    use super::*;

    /// A press of the key that means `key`.
    fn press(key: NamedKey) -> KeyEvent {
        KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Named(key),
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
        }
    }

    const WINDOW: PhysicalSize = PhysicalSize {
        width: 1280,
        height: 720,
    };

    /// An overlay over a window of 1280 × 720 pixels at scale 1, with a panel of statistics on F1,
    /// at the top start, which holds a button, and a world panel on F2, at the top end.
    fn overlay() -> (Overlay, PanelId, PanelId) {
        let mut overlay = Overlay::new();
        let statistics = overlay.add_panel(PanelSpec {
            title: "Statistics",
            chord: "F1",
            command: "statistics",
            corner: Corner::TopStart,
            size: [300.0, 200.0],
        });
        let world = overlay.add_panel(PanelSpec {
            title: "World",
            chord: "F2",
            command: "world",
            corner: Corner::TopEnd,
            size: [380.0, 640.0],
        });
        overlay.set_window(WINDOW, 1.0);
        (overlay, statistics, world)
    }

    /// Takes the overlay's actions, builds a button in the panel of statistics if it is open, and
    /// lays the overlay out.
    fn frame(overlay: &mut Overlay, statistics: PanelId) {
        overlay.take_actions();
        if let Some(slot) = overlay.slot(statistics) {
            overlay.ui_mut().build(slot, 0, |children| {
                children.add("reset", Element::button("Reset", Style::default()), |_| {});
            });
        }
        overlay.ui_mut().frame();
    }

    /// The box of the open panel titled `title`, in pixels of the window.
    fn panel_box(overlay: &Overlay, title: &str) -> slime_ui::Rect {
        let meaning = overlay
            .ui()
            .meaning()
            .into_iter()
            .find(|meaning| meaning.role == Role::Pane && meaning.name == title)
            .unwrap_or_else(|| panic!("the panel {title} is open"));
        meaning.bounds.expect("laid out")
    }

    #[test]
    fn a_panel_opens_by_its_chord_or_its_command_at_its_corner_and_again_where_it_was() {
        let (mut overlay, statistics, _) = overlay();
        assert!(
            overlay.ui_mut().key(&press(NamedKey::F1)),
            "its chord is taken"
        );
        frame(&mut overlay, statistics);
        assert!(overlay.is_open(statistics));
        let open = panel_box(&overlay, "Statistics");
        assert_eq!((open.x, open.y, open.width), (12.0, 12.0, 300.0));
        // An agent's command, as the chord gives it.
        overlay.ui_mut().act(Action {
            element: None,
            kind: ActionKind::Command("world".to_owned()),
        });
        frame(&mut overlay, statistics);
        let open = panel_box(&overlay, "World");
        assert_eq!(
            (open.x, open.width, open.height),
            (1280.0 - 12.0 - 380.0, 380.0, 640.0)
        );
        // The user drags the panel of statistics by its bar; closed and opened again, it stands
        // where they left it.
        let ui = overlay.ui_mut();
        ui.pointer_moved(100.0, 20.0);
        ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
        ui.pointer_moved(200.0, 120.0);
        ui.pointer_button(MouseButton::Left, ButtonState::Released);
        frame(&mut overlay, statistics);
        overlay.toggle(statistics);
        frame(&mut overlay, statistics);
        assert!(!overlay.is_open(statistics));
        assert_eq!(overlay.slot(statistics), None);
        overlay.toggle(statistics);
        frame(&mut overlay, statistics);
        let moved = panel_box(&overlay, "Statistics");
        assert_eq!((moved.x, moved.y), (112.0, 112.0));
    }

    #[test]
    fn a_panel_opened_before_the_window_is_known_opens_once_it_is_and_keeps_its_place_at_any_scale()
    {
        let mut overlay = Overlay::new();
        let statistics = overlay.add_panel(PanelSpec {
            title: "Statistics",
            chord: "F1",
            command: "statistics",
            corner: Corner::TopEnd,
            size: [300.0, 200.0],
        });
        overlay.open(statistics);
        assert!(overlay.is_open(statistics));
        assert_eq!(
            overlay.slot(statistics),
            None,
            "not laid out over a window yet"
        );
        overlay.set_window(WINDOW, 1.0);
        frame(&mut overlay, statistics);
        let open = panel_box(&overlay, "Statistics");
        assert_eq!((open.x, open.width), (1280.0 - 12.0 - 300.0, 300.0));
        // The window moves to a screen of twice the pixels: the panel keeps its logical place.
        overlay.set_window(
            PhysicalSize {
                width: 2560,
                height: 1440,
            },
            2.0,
        );
        frame(&mut overlay, statistics);
        let doubled = panel_box(&overlay, "Statistics");
        assert_eq!((doubled.x, doubled.width), (open.x * 2.0, 600.0));
    }

    #[test]
    fn the_overlay_leaves_tab_to_the_game_and_f6_gives_its_panels_the_focus() {
        let (mut overlay, statistics, _) = overlay();
        overlay.open(statistics);
        frame(&mut overlay, statistics);
        let ui = overlay.ui_mut();
        assert!(!ui.key(&press(NamedKey::Tab)), "the game's");
        assert_eq!(ui.focus(), None);
        assert!(ui.key(&press(NamedKey::F6)));
        assert!(ui.focus().is_some(), "a panel's element");
        assert!(
            ui.key(&press(NamedKey::Tab)),
            "the panel's, once it holds the focus"
        );
        assert!(ui.key(&press(NamedKey::F6)));
        assert_eq!(ui.focus(), None, "back to the game");
        // Escape, while a panel holds the focus, gives the keys back to the game too; a held F1
        // toggles the panel once.
        assert!(ui.key(&press(NamedKey::F6)));
        assert!(ui.key(&press(NamedKey::Escape)));
        let actions = overlay.take_actions();
        assert_eq!(overlay.ui().focus(), None);
        assert!(
            actions
                .iter()
                .any(|action| action.kind == ActionKind::Intent(Intent::Cancel)),
            "{actions:?}"
        );
        let ui = overlay.ui_mut();
        assert!(ui.key(&press(NamedKey::F1)));
        assert!(ui.key(&KeyEvent {
            repeat: true,
            ..press(NamedKey::F1)
        }));
        overlay.take_actions();
        assert!(!overlay.is_open(statistics));
    }

    /// The pixels of `target`, as the GPU holds them.
    fn pixels(gpu: &Gpu, target: &RenderTarget) -> Vec<u8> {
        gpu.read_render_target(target)
            .expect("the device is not lost")
            .pixels
    }

    /// A target of the window's size, cleared to black, as a view would leave it.
    fn cleared(gpu: &Gpu) -> RenderTarget {
        let target = gpu
            .create_render_target(
                "the window",
                WINDOW.width,
                WINDOW.height,
                TextureFormat::Rgba8UnormSrgb,
            )
            .expect("the GPU has memory for it");
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(slime_color::LinearColor::BLACK),
            depth: None,
            timer: None,
            resolve: None,
        };
        gpu.render(&pass, |_| {});
        target
    }

    #[test]
    fn the_panels_are_drawn_over_the_view_and_again_with_the_device_that_replaces_a_lost_one() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let mut renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the device is not lost");
        let (mut overlay, statistics, _) = overlay();
        // No panel open: nothing drawn.
        let target = cleared(&gpu);
        let black = pixels(&gpu, &target);
        frame(&mut overlay, statistics);
        overlay
            .draw(&gpu, &mut renderer, &target)
            .expect("the device is not lost");
        assert_eq!(pixels(&gpu, &target), black);
        overlay.open(statistics);
        frame(&mut overlay, statistics);
        overlay
            .draw(&gpu, &mut renderer, &target)
            .expect("the device is not lost");
        let drawn = pixels(&gpu, &target);
        assert_ne!(drawn, black, "the panel is drawn");
        // The device lost, the draw fails without a panic; released, the overlay draws the same with
        // a new device's renderer.
        gpu.lose();
        let lost = overlay.draw(&gpu, &mut renderer, &target);
        assert!(
            matches!(lost, Err(RenderError::Gfx(GfxError::DeviceLost { .. }))),
            "{lost:?}"
        );
        overlay.release();
        let gpu = Gpu::new().expect("a new device");
        let mut renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the device is not lost");
        let target = cleared(&gpu);
        frame(&mut overlay, statistics);
        overlay
            .draw(&gpu, &mut renderer, &target)
            .expect("the device is not lost");
        assert_eq!(pixels(&gpu, &target), drawn, "drawn again as it was");
    }
}
