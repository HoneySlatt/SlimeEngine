//! The editor's window: a presentation of its program, on the engine's interface in the editor's
//! look, which reads what the core shares and asks it through requests
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`): a menu bar, fixed regions, the dev
//! overlay's world panel in the first, a status bar, and the window's title.

use std::ffi::OsString;
use std::path::Path;

use slime_app::{AppConfig, AppError, Context, Host, Presentation};
use slime_color::LinearColor;
use slime_devui::WorldPanel;
use slime_gfx::{Gpu, Image, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_platform::{ColorScheme, Event};
use slime_render::{RenderError, Renderer};
use slime_ui::action::{Action, ActionKind};
use slime_ui::display::DisplayList;
use slime_ui::look::{self, Colors, Density};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Align, Dimension, FlexDirection, Justify, Sides, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, ItemMark, Key, Layer, LayoutDirection, Notification,
    NotificationKind, PaletteCommand, Place, RootSettings, Side, Ui,
};
use slime_ui_render::UiRenderer;
use slime_world::{Entity, World};

use crate::operation::{self, OPERATIONS, SHORTCUTS};
use crate::program::Program;
use crate::session::Session;
use crate::shared::{Request, Shared};

/// An entry of a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// An operation without parameters, by its name.
    Operation(&'static str),
    /// The submenu of the levels opened last, each running `file.open` with its path.
    Recent,
    /// A line that parts the entries before it from those after.
    Separator,
}

/// The menus of the bar, each its title and its entries.
pub(crate) const MENUS: &[(&str, &[Entry])] = &[
    (
        "File",
        &[
            Entry::Recent,
            Entry::Operation("file.save"),
            Entry::Separator,
            Entry::Operation("file.quit"),
        ],
    ),
    (
        "Edit",
        &[
            Entry::Operation("edit.undo"),
            Entry::Operation("edit.redo"),
            Entry::Separator,
            Entry::Operation("edit.select_none"),
        ],
    ),
    (
        "Help",
        &[
            Entry::Operation("view.palette"),
            Entry::Operation("view.shortcuts"),
        ],
    ),
];

/// The width of the region on the window's start, which holds the world panel, in logical pixels.
const SIDE_WIDTH: f32 = 340.0;

/// What the center of the window says until it holds a viewport.
const NO_VIEWPORT: &str = "No viewport yet: the level shows in the world panel.";

/// What an item of an open menu does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    /// Runs an operation with its arguments.
    Run(&'static str, Vec<String>),
    /// Opens the submenu of the recent levels.
    Recent,
}

/// What the window shows and how it turns what the user does into the core's requests, without a
/// GPU: what the tests run.
pub(crate) struct Model {
    shared: Shared,
    pub(crate) ui: Ui,
    root: ElementId,
    /// The slot of the start region that the world panel builds into.
    slot: Option<ElementId>,
    panel: WorldPanel,
    /// The items of the menu bar, each with the place of its menu in [`MENUS`].
    bar: Vec<(ElementId, usize)>,
    /// The items of the menus open, each with what it does.
    items: Vec<(ElementId, Item)>,
    /// The revision of the shared value that the window last built from.
    built: u64,
    /// How many levels the core opened in place of the first when the world panel was last made
    /// for its world: one opened since resets it.
    opened: u64,
    /// The entity that the world panel last selected, which the core's selection follows.
    selected: Option<Entity>,
    colors: Colors,
}

impl Model {
    /// The window of the core that shares `shared`, in the editor's look.
    pub(crate) fn new(shared: Shared) -> Self {
        let mut fonts = Fonts::new();
        look::register_fonts(&mut fonts);
        let mut ui = Ui::new(fonts);
        ui.set_shortcuts(&[Shortcuts::from_text("editor.keys", SHORTCUTS)])
            .expect("the editor's shortcuts are valid");
        let root = ui.add_root("editor", settings([1.0, 1.0], 1.0), column());
        let mut model = Self {
            shared,
            ui,
            root,
            slot: None,
            panel: WorldPanel::default(),
            bar: Vec::new(),
            items: Vec::new(),
            built: u64::MAX,
            opened: 0,
            selected: None,
            colors: Colors::Dark,
        };
        model.style();
        model
    }

    /// Shows the look in `colors`.
    pub(crate) fn set_colors(&mut self, colors: Colors) {
        self.colors = colors;
        self.style();
    }

    fn style(&mut self) {
        let themes = look::themes(self.colors, Density::Comfortable);
        self.ui
            .set_style(&[look::style_sheet()], &themes)
            .expect("the look's documents are valid");
    }

    /// Turns what the user did since the last frame into the core's requests: the world panel's
    /// changes and selection, the menus' items, the shortcuts', the palette's and the
    /// notifications' operations. Gives whether it took an action or made a request, which the
    /// next frame shows or answers.
    pub(crate) fn act(&mut self, world: &World) -> bool {
        let mut actions = self.ui.take_actions();
        let took = !actions.is_empty();
        let mut requests = Vec::new();
        for command in self.panel.act(&mut actions) {
            let label = format!("{} {}", command.kind(), command.entity());
            requests.push(Request::Apply {
                label,
                commands: vec![command],
            });
        }
        if self.panel.selected() != self.selected {
            self.selected = self.panel.selected();
            let selection = self
                .selected
                .and_then(|entity| world.persistent_id(entity))
                .into_iter()
                .collect();
            requests.push(Request::Select(selection));
        }
        for action in actions {
            if let Some(request) = self.request_of(&action) {
                requests.push(request);
            }
        }
        let made = !requests.is_empty();
        self.shared.borrow_mut().requests.extend(requests);
        took || made
    }

    /// The request that `action` makes, after the menus that it opens or closes.
    fn request_of(&mut self, action: &Action) -> Option<Request> {
        match (&action.kind, action.element) {
            (ActionKind::Command(name), _) => Some(Request::Run {
                name: name.clone(),
                arguments: Vec::new(),
            }),
            (ActionKind::Press, Some(id)) => {
                if let Some(&(_, menu)) = self.bar.iter().find(|&&(item, _)| item == id) {
                    self.open_menu(id, Side::Below, Menu::Bar(menu));
                    return None;
                }
                let item = self
                    .items
                    .iter()
                    .find(|(each, _)| *each == id)
                    .map(|(_, item)| item.clone())?;
                match item {
                    Item::Run(name, arguments) => Some(Request::Run {
                        name: name.to_owned(),
                        arguments,
                    }),
                    Item::Recent => {
                        self.open_menu(id, Side::End, Menu::Recent);
                        None
                    }
                }
            }
            // A menu closed: its items are forgotten.
            (ActionKind::Dismissed, _) => {
                let ui = &self.ui;
                self.items.retain(|&(each, _)| ui.element(each).is_some());
                None
            }
            _ => None,
        }
    }

    /// Opens `menu` beside `element`, on its `side`, in a layer that closes as the user leaves it.
    fn open_menu(&mut self, element: ElementId, side: Side, menu: Menu) {
        let entries: Vec<(String, String, ItemMark, Option<Item>)> = match menu {
            Menu::Bar(menu) => MENUS[menu]
                .1
                .iter()
                .map(|entry| match *entry {
                    Entry::Operation(name) => {
                        let label =
                            operation::operation(name).map_or(name, |operation| operation.label);
                        let chord = self.ui.command_chord(name).unwrap_or_default();
                        (
                            label.to_owned(),
                            chord,
                            ItemMark::None,
                            Some(Item::Run(name, Vec::new())),
                        )
                    }
                    Entry::Recent => (
                        "Open a recent level".to_owned(),
                        String::new(),
                        ItemMark::Submenu,
                        Some(Item::Recent),
                    ),
                    Entry::Separator => (String::new(), String::new(), ItemMark::None, None),
                })
                .collect(),
            Menu::Recent => self
                .shared
                .borrow()
                .recent
                .iter()
                .map(|level| {
                    (
                        level.display().to_string(),
                        String::new(),
                        ItemMark::None,
                        Some(Item::Run("file.open", vec![level.display().to_string()])),
                    )
                })
                .collect(),
        };
        let layer = Layer {
            place: Place::Beside {
                element,
                side,
                along: Along::Start,
            },
            modal: false,
            dismiss: true,
        };
        let key = match menu {
            Menu::Bar(_) => "menu",
            Menu::Recent => "recent",
        };
        let layer = self.ui.open_layer(key, layer, Style::default());
        let mut items = Vec::new();
        // A recent level's path holds `/`, which a key of a path cannot: its item is keyed by its
        // place, and named by its path.
        let by_place = matches!(menu, Menu::Recent);
        self.ui.build(layer, 0, |children| {
            children.add("menu", Element::menu(Style::default()), |menu| {
                for (place, (label, chord, mark, item)) in entries.into_iter().enumerate() {
                    let Some(item) = item else {
                        let line = Element::container(Style::default()).class("separator");
                        menu.add(format!("separator {place}"), line, |_| {});
                        continue;
                    };
                    let element = Element::menu_item(label.as_str(), chord, mark, Style::default());
                    let key = if by_place {
                        Key::from(place)
                    } else {
                        Key::from(label.as_str())
                    };
                    items.push((menu.add(key, element, |_| {}), item));
                }
            });
        });
        self.items.extend(items);
    }

    /// Runs the window's operations that the core was asked to run: the palette, the sheet of
    /// shortcuts.
    fn run_window_operations(&mut self) {
        let operations = std::mem::take(&mut self.shared.borrow_mut().window_operations);
        for name in operations {
            let commands: Vec<PaletteCommand> = OPERATIONS
                .iter()
                .filter(|operation| operation.parameters.is_empty())
                .map(|operation| PaletteCommand {
                    name: operation.label.to_owned(),
                    command: operation.name.to_owned(),
                })
                .collect();
            match name {
                "view.palette" => self.ui.open_palette("Find an operation", commands),
                "view.shortcuts" => self.ui.open_shortcut_sheet("Shortcuts", &commands),
                name => tracing::warn!(name, "the window has no operation of that name"),
            }
        }
    }

    /// Lays the window out at `size`, in logical pixels, and `scale` pixels of the screen per
    /// logical pixel, from what the core shares and from `world`, and gives what to draw.
    pub(crate) fn frame(&mut self, world: &World, size: [f32; 2], scale: f32) -> DisplayList {
        self.ui.set_root(self.root, settings(size, scale));
        self.run_window_operations();
        let notices = std::mem::take(&mut self.shared.borrow_mut().notices);
        for notice in notices {
            self.ui.notify(Notification {
                kind: if notice.failure {
                    NotificationKind::Error
                } else {
                    NotificationKind::Information
                },
                text: notice.text,
                progress: None,
                buttons: Vec::new(),
            });
        }
        let revision = self.shared.borrow().revision;
        if revision != self.built {
            self.built = revision;
            self.follow_the_core(world);
        }
        self.build();
        if let Some(slot) = self.slot {
            self.panel.build(&mut self.ui, slot, world);
        }
        self.ui.frame().0
    }

    /// Follows what the core changed: a level opened in place of the other resets the world panel;
    /// a selection that the core changed, an undo or an agent's, selects in the panel.
    fn follow_the_core(&mut self, world: &World) {
        let shared = self.shared.borrow();
        if shared.opened != self.opened {
            self.opened = shared.opened;
            self.panel = WorldPanel::default();
            self.selected = None;
        }
        let selected = shared
            .selection
            .first()
            .and_then(|identity| world.find(identity));
        if selected != self.selected {
            match selected {
                Some(entity) => self.panel.select(entity),
                None => self.panel.deselect(),
            }
            self.selected = selected;
        }
    }

    /// Builds the bar of menus, the regions and the status bar again, when what they show changed.
    fn build(&mut self) {
        let status = self.status();
        let mut bar = Vec::new();
        let mut slot = None;
        self.ui.build(self.root, self.built, |children| {
            let mut window = column();
            window.layout.grow = 1.0;
            window.layout.min_size[1] = Dimension::Pixels(0.0);
            let window = Element::container(window).class("window");
            children.add("window", window, |children| {
                children.add("menus", Element::menu_bar(Style::default()), |menus| {
                    for (place, (title, _)) in MENUS.iter().enumerate() {
                        let item =
                            Element::menu_item(*title, "", ItemMark::Submenu, Style::default());
                        bar.push((menus.add(*title, item, |_| {}), place));
                    }
                });
                let mut regions = Style::default();
                regions.layout.grow = 1.0;
                regions.layout.min_size[1] = Dimension::Pixels(0.0);
                regions.layout.gap = [8.0, 8.0];
                regions.layout.padding = Sides::all(8.0);
                children.add("regions", Element::container(regions), |regions| {
                    let mut side = column();
                    side.layout.size[0] = Dimension::Pixels(SIDE_WIDTH);
                    side.layout.shrink = 0.0;
                    side.layout.gap = [8.0, 0.0];
                    side.layout.padding = Sides::all(8.0);
                    let side = Element::container(side).class("panel").named("world");
                    regions.add("world", side, |side| {
                        let title = Element::label("World", Style::default()).class("heading");
                        side.add("title", title, |_| {});
                        let mut content = column();
                        content.layout.grow = 1.0;
                        slot = Some(side.add_slot("panel", Element::scroll(content)));
                    });
                    let mut center = Style::default();
                    center.layout.grow = 1.0;
                    center.layout.justify = Justify::Center;
                    center.layout.align_items = Align::Center;
                    let center = Element::container(center).class("panel").named("viewport");
                    regions.add("viewport", center, |center| {
                        let word = Element::label(NO_VIEWPORT, Style::default()).class("muted");
                        center.add("word", word, |_| {});
                    });
                });
                let mut line = Style::default();
                line.layout.padding = Sides::all(6.0);
                line.layout.gap = [0.0, 16.0];
                let line = Element::container(line).class("bar").named("status");
                children.add("status", line, |line| {
                    for (key, text) in status {
                        let label = Element::label(text, Style::default());
                        line.add(key, label, |_| {});
                    }
                });
            });
        });
        if !bar.is_empty() {
            self.bar = bar;
        }
        if slot.is_some() {
            self.slot = slot;
        }
    }

    /// The texts of the status bar, each with its key: the level's file, the steps that it does
    /// not hold, and what is selected.
    fn status(&self) -> Vec<(&'static str, String)> {
        let shared = self.shared.borrow();
        let level = shared.level.as_deref().map_or_else(
            || "no level".to_owned(),
            |level| level.display().to_string(),
        );
        vec![
            ("level", level),
            ("unwritten", steps_not_written(shared.unwritten)),
            (
                "selected",
                match shared.selection.len() {
                    0 => "nothing selected".to_owned(),
                    1 => "1 entity selected".to_owned(),
                    count => format!("{count} entities selected"),
                },
            ),
        ]
    }

    /// The window's title: the level's file's name, the steps that the file does not hold, and the
    /// program.
    pub(crate) fn title(&self) -> String {
        let shared = self.shared.borrow();
        let level = shared
            .level
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned());
        let mut title = String::new();
        if let Some(level) = level {
            title.push_str(&level);
            if shared.unwritten > 0 {
                title.push_str(" — ");
                title.push_str(&steps_not_written(shared.unwritten));
            }
            title.push_str(" — ");
        }
        title.push_str(shared.program);
        title
    }
}

/// Which menu [`Model::open_menu`] opens.
#[derive(Clone, Copy)]
enum Menu {
    /// The menu of the bar at this place of [`MENUS`].
    Bar(usize),
    /// The levels opened last.
    Recent,
}

/// How many steps the level's file does not hold, as the status bar and the title say it.
fn steps_not_written(count: usize) -> String {
    match count {
        0 => "all written".to_owned(),
        1 => "1 step not written".to_owned(),
        count => format!("{count} steps not written"),
    }
}

fn settings(size: [f32; 2], scale: f32) -> RootSettings {
    RootSettings {
        size,
        scale,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    }
}

/// A style that lays its children out in a column.
fn column() -> Style {
    let mut style = Style::default();
    style.layout.direction = FlexDirection::Column;
    style
}

/// The size of the window that [`window_image`] draws, in logical pixels: a laptop's, at its
/// smallest.
pub const IMAGE_SIZE: [u32; 2] = [1280, 800];

/// The window of the editor's `program` opened on its level as `arguments` say, as the program
/// takes them ([`run`](crate::run): `--level`, `--assets`, `--journal`, `--discard`), drawn into an
/// image of [`IMAGE_SIZE`] logical pixels at `scale` pixels per logical pixel, as it shows once the
/// level is open, on the GPU that [`Gpu::new`] opens, without a window of the system: what the
/// `editor` example's `--output` draws, and what its image tests compare. The level is opened as the
/// program opens it, its journal held and written, and the recent levels left as they are; the
/// status bar shows the level's path as `--level` gives it.
///
/// # Errors
///
/// When the arguments are not the program's, the level cannot be opened, no GPU adapter is
/// available, or the window cannot be drawn: why, as a sentence.
pub fn window_image(
    program: &Program,
    arguments: impl IntoIterator<Item = OsString>,
    scale: f32,
) -> Result<Image, String> {
    let options = crate::program::parse(program, arguments)?.ok_or("--help draws no image")?;
    let shared = Shared::default();
    let mut session = Session::new(
        program,
        options,
        shared.clone(),
        None,
        crate::program::journals(),
    );
    let mut host = Host::new(&AppConfig::default());
    host.start(&mut session)
        .map_err(|error| slime_app::error_chain(&*error))?;
    let mut model = Model::new(shared);
    model.ui.set_reduced_motion(true);
    let gpu = Gpu::new().map_err(|error| slime_app::error_chain(&error))?;
    let format = TextureFormat::Rgba8UnormSrgb;
    let mut drawing = Drawing::new(&gpu, format).map_err(|error| error.to_string())?;
    let physical = IMAGE_SIZE.map(|side| (side as f32 * scale).round() as u32);
    let target = gpu
        .create_render_target("editor's window", physical[0], physical[1], format)
        .map_err(|error| error.to_string())?;
    let size = IMAGE_SIZE.map(|side| side as f32);
    // Twice: the world panel's list builds its rows once the first frame laid it out.
    for _ in 0..2 {
        let display = model.frame(host.world(), size, scale);
        gpu.render(
            &RenderPassDescriptor {
                target: &target,
                clear: Some(LinearColor::BLACK),
                depth: None,
                timer: None,
                resolve: None,
            },
            |_| {},
        );
        drawing
            .bridge
            .draw(
                &gpu,
                &mut drawing.renderer,
                model.ui.fonts(),
                &display,
                |_| None,
                &target,
            )
            .map_err(|error| error.to_string())?;
    }
    gpu.read_render_target(&target)
        .map_err(|error| error.to_string())
}

/// What draws the window's interface.
struct Drawing {
    renderer: Renderer,
    bridge: UiRenderer,
}

impl Drawing {
    fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        Ok(Self {
            renderer: Renderer::new(gpu, format)?,
            bridge: UiRenderer::default(),
        })
    }
}

/// The editor's window, the presentation of its program.
pub(crate) struct Window {
    shared: Shared,
    model: Option<Model>,
    drawing: Option<Drawing>,
    /// What the last extraction laid out, which the frame draws.
    display: Option<DisplayList>,
    /// The title that the window shows.
    title: String,
}

impl Window {
    pub(crate) fn new(shared: Shared) -> Self {
        Self {
            shared,
            model: None,
            drawing: None,
            display: None,
            title: String::new(),
        }
    }
}

impl Presentation for Window {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        let mut model = Model::new(self.shared.clone());
        if ctx.window().and_then(slime_platform::Window::color_scheme) == Some(ColorScheme::Light) {
            model.set_colors(Colors::Light);
        }
        self.model = Some(model);
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.drawing = Some(Drawing::new(gpu, format)?);
        }
        Ok(())
    }

    fn on_event(&mut self, _ctx: &mut Context, event: &Event) -> bool {
        if let (Event::ColorSchemeChanged(scheme), Some(model)) = (event, &mut self.model) {
            model.set_colors(match scheme {
                ColorScheme::Dark => Colors::Dark,
                ColorScheme::Light => Colors::Light,
            });
        }
        false
    }

    fn ui(&mut self) -> Option<&mut Ui> {
        self.model.as_mut().map(|model| &mut model.ui)
    }

    fn extract(&mut self, ctx: &mut Context, world: &World) {
        let Some(model) = &mut self.model else {
            return;
        };
        // What the user asked is answered, and what it opened shown, at the next frame.
        let mut again = model.act(world);
        if let Some(window) = ctx.window() {
            let scale = window.scale_factor() as f32;
            let size = window.inner_size();
            let size = [size.width as f32 / scale, size.height as f32 / scale];
            self.display = Some(model.frame(world, size, scale));
            // What the interface did as it laid out, a submenu opened by the pointer's rest.
            again |= model.act(world);
        }
        if again {
            ctx.request_frame();
        }
        let Some(window) = ctx.window() else {
            return;
        };
        let title = model.title();
        if title != self.title {
            window.set_title(&title);
            self.title = title;
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(model), Some(drawing), Some(display), Some(gpu)) =
            (&self.model, &mut self.drawing, &self.display, ctx.gpu())
        else {
            return;
        };
        if let Err(error) = drawing.bridge.draw(
            gpu,
            &mut drawing.renderer,
            model.ui.fonts(),
            display,
            |_| None,
            target,
        ) {
            ctx.exit_with_error(error);
        }
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        self.drawing = None;
    }

    fn gpu_restored(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.drawing = Some(Drawing::new(gpu, format)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use slime_app::Simulation;
    use slime_platform::{ButtonState, Key, KeyCode, KeyEvent, Modifiers, NamedKey};
    use slime_world::Command;

    use super::*;
    use crate::operation::Changes;

    const SIZE: [f32; 2] = [1280.0, 800.0];

    /// A yard of its own, without prefabs, and empty cooked data, opened by a session whose window
    /// is a model without a GPU, both sharing a value, as the program runs them.
    struct Yard {
        directory: PathBuf,
        host: Host,
        session: Session,
        model: Model,
        shared: Shared,
    }

    impl Yard {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir()
                .join(format!("slime-editor-window-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&directory);
            fs::create_dir_all(directory.join("cooked")).expect("a directory");
            fs::write(
                directory.join("cooked").join(slime_assets::MANIFEST_FILE),
                slime_assets::Manifest::new().to_bytes(),
            )
            .expect("an empty manifest");
            for level in ["yard", "garden"] {
                fs::write(
                    directory.join(format!("{level}.scene")),
                    "[entities]\nwell = { components = { Name = \"well\" } }\n",
                )
                .expect("the level");
                fs::write(
                    directory.join(format!("{level}.scene.meta")),
                    format!(
                        "id = \"00000000-0000-0000-0000-00000000000{}\"\n",
                        if level == "yard" { 1 } else { 2 }
                    ),
                )
                .expect("its metadata");
            }
            let program = Program {
                name: "editor",
                level: directory.join("yard.scene"),
                key: "yard",
                assets: directory.join("cooked"),
                sources: None,
                register: |_| {},
            };
            let arguments: Vec<OsString> = vec![
                "--journal".into(),
                directory.join("yard.journal").into_os_string(),
            ];
            let options = crate::program::parse(&program, arguments)
                .expect("options")
                .expect("not --help");
            let shared = Shared::default();
            shared.borrow_mut().window = true;
            let mut session = Session::new(
                &program,
                options,
                shared.clone(),
                Some(directory.join("editor.recent")),
                Some(directory.join("journals")),
            );
            let mut host = Host::new(&AppConfig::default());
            host.start(&mut session).expect("the yard opens");
            let mut model = Model::new(shared.clone());
            model.ui.set_reduced_motion(true);
            let mut yard = Self {
                directory,
                host,
                session,
                model,
                shared,
            };
            yard.frame();
            yard
        }

        /// A frame of the program: the core answers what the window asked, then the window acts
        /// and lays out.
        fn frame(&mut self) {
            self.host
                .frame(&mut self.session, std::time::Duration::from_millis(16));
            self.model.act(self.host.world());
            self.model.frame(self.host.world(), SIZE, 1.0);
        }

        /// A step that renames the well, made as an agent makes it.
        fn rename(&mut self, name: &str) {
            let command = format!(
                "[SetProperty]\nentity = \"yard/well\"\ncomponent = \"Name\"\npath = \"0\"\nvalue = \"{name}\"\n"
            );
            self.session
                .handle(
                    self.host.world_mut(),
                    Request::Step {
                        label: format!("Name the well {name}"),
                        commands: vec![command],
                        selection: None,
                    },
                )
                .expect("made");
            self.frame();
        }

        fn history(&self) -> (Vec<String>, Vec<String>) {
            let shared = self.shared.borrow();
            (shared.done.clone(), shared.undone.clone())
        }

        fn key(&mut self, logical: Key, control: bool) {
            let mut modifiers = Modifiers::default();
            modifiers.control = control;
            self.model.ui.modifiers(modifiers);
            let text = match &logical {
                Key::Character(text) => Some(text.clone()),
                _ => None,
            };
            self.model.ui.key(&KeyEvent {
                physical: KeyCode::Unidentified,
                logical,
                text,
                state: ButtonState::Pressed,
                repeat: false,
            });
            self.model.ui.modifiers(Modifiers::default());
            self.frame();
        }
    }

    impl Drop for Yard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn every_entry_of_the_menus_and_of_the_shortcuts_names_an_operation() {
        for (title, entries) in MENUS {
            for entry in *entries {
                if let Entry::Operation(name) = entry {
                    let operation = operation::operation(name)
                        .unwrap_or_else(|| panic!("{title}: `{name}` is no operation"));
                    assert!(
                        operation.parameters.is_empty(),
                        "{title}: `{name}` takes arguments, which an item cannot give"
                    );
                }
            }
        }
        let mut ui = Ui::new(Fonts::new());
        ui.set_shortcuts(&[Shortcuts::from_text("editor.keys", SHORTCUTS)])
            .expect("valid shortcuts");
        let scopes = ui.shortcuts();
        assert!(!scopes.is_empty());
        for (scope, chords) in scopes {
            for (chord, command) in chords {
                assert!(
                    operation::operation(&command).is_some(),
                    "{scope}, {chord}: `{command}` is no operation"
                );
            }
        }
    }

    #[test]
    fn an_operation_run_by_its_name_by_its_shortcut_and_from_the_palette_does_the_same() {
        let mut yard = Yard::new("same");
        for name in ["a", "b", "c"] {
            yard.rename(name);
        }
        assert_eq!(yard.history().0.len(), 3);
        // By its name, as an agent runs it through the development channel.
        yard.session
            .edit(
                yard.host.world_mut(),
                &slime_channel::Edit::Run {
                    name: "edit.undo".to_owned(),
                    arguments: Vec::new(),
                },
            )
            .expect("undone");
        yard.frame();
        assert_eq!(yard.history().1, ["Name the well c"]);
        // By its shortcut, Ctrl+Z.
        yard.key(Key::Character("z".to_owned()), true);
        yard.frame();
        assert_eq!(yard.history().1, ["Name the well b", "Name the well c"]);
        // From the palette, found by what is typed.
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Run {
                    name: "view.palette".to_owned(),
                    arguments: Vec::new(),
                },
            )
            .expect("opened");
        yard.frame();
        assert!(yard.model.ui.palette_open());
        for character in "undo".chars() {
            yard.key(Key::Character(character.to_string()), false);
        }
        yard.key(Key::Named(NamedKey::Enter), false);
        yard.frame();
        assert!(!yard.model.ui.palette_open());
        assert_eq!(
            yard.history().1,
            ["Name the well a", "Name the well b", "Name the well c"]
        );
        // And the menu's item, pressed.
        yard.rename("d");
        let edit = yard
            .model
            .ui
            .element_by_path("editor/window/menus/Edit")
            .expect("the Edit menu");
        yard.model.ui.act(Action {
            element: Some(edit),
            kind: ActionKind::Press,
        });
        yard.frame();
        let undo = yard
            .model
            .ui
            .element_by_path("menu/menu/Undo")
            .expect("the item Undo");
        yard.model.ui.act(Action {
            element: Some(undo),
            kind: ActionKind::Press,
        });
        yard.frame();
        yard.frame();
        assert_eq!(
            yard.history(),
            (Vec::new(), vec!["Name the well d".to_owned()])
        );
    }

    #[test]
    fn the_operations_of_the_selection_and_of_the_window_enter_no_history() {
        let mut yard = Yard::new("history");
        yard.rename("a");
        let well = yard
            .host
            .world()
            .find(&"yard/well".parse().expect("an identity"))
            .expect("the well");
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/well".parse().expect("an identity")]),
            )
            .expect("selected");
        yard.frame();
        assert_eq!(yard.model.panel.selected(), Some(well), "the panel follows");
        let before = yard.history();
        for operation in OPERATIONS
            .iter()
            .filter(|operation| matches!(operation.changes, Changes::Selection | Changes::Window))
        {
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: operation.name.to_owned(),
                        arguments: Vec::new(),
                    },
                )
                .unwrap_or_else(|error| panic!("{}: {error}", operation.name));
            yard.frame();
        }
        assert_eq!(yard.history(), before);
        assert!(
            yard.shared.borrow().selection.is_empty(),
            "selected nothing"
        );
        assert_eq!(yard.model.panel.selected(), None, "the panel follows");
        // The window ran its operations: the sheet of shortcuts is open, the palette closed by it.
        assert!(yard.model.ui.element_by_path("shortcuts").is_some());
    }

    #[test]
    fn the_window_and_the_development_channel_ask_the_core_alike() {
        let mut yard = Yard::new("alike");
        // The world panel's change, a step that the window asks for.
        let well = yard
            .host
            .world()
            .find(&"yard/well".parse().expect("an identity"))
            .expect("the well");
        yard.model.panel.select(well);
        yard.shared.borrow_mut().requests.push_back(Request::Apply {
            label: "Rename".to_owned(),
            commands: vec![Command::SetProperty {
                entity: "yard/well".parse().expect("an identity"),
                component: "Name".to_owned(),
                path: "0".parse::<slime_reflect::Path>().expect("a path"),
                value: slime_reflect::Value::Text("old well".to_owned()),
            }],
        });
        yard.frame();
        yard.frame();
        let (done, _) = yard.history();
        assert_eq!(done, ["Rename"]);
        assert_eq!(yard.shared.borrow().unwritten, 1);
        assert_eq!(
            yard.shared.borrow().selection,
            ["yard/well".parse().expect("an identity")],
            "the world panel's selection is the core's"
        );
        assert!(
            yard.model
                .title()
                .starts_with("yard.scene — 1 step not written — editor")
        );
        // Written through the development channel, as an agent writes it: the title follows.
        let reply = yard
            .session
            .edit(yard.host.world_mut(), &slime_channel::Edit::Write)
            .expect("written");
        assert!(
            matches!(reply, slime_channel::Reply::Written { .. }),
            "{reply:?}"
        );
        yard.frame();
        assert_eq!(yard.model.title(), "yard.scene — editor");
        // Another level opened by its path, as a recent level's item opens it: the world is the
        // new level's, and the list of recent levels has both, the last first.
        let garden = yard.directory.join("garden.scene");
        yard.shared.borrow_mut().requests.push_back(Request::Run {
            name: "file.open".to_owned(),
            arguments: vec![garden.display().to_string()],
        });
        yard.frame();
        yard.frame();
        let shared = yard.shared.borrow();
        assert_eq!(shared.level.as_deref(), Some(garden.as_path()));
        assert_eq!(shared.recent.len(), 2);
        assert!(
            shared.recent[0].ends_with("garden.scene"),
            "{:?}",
            shared.recent
        );
        assert!(shared.done.is_empty());
        let kept = crate::recent::read(&yard.directory.join("editor.recent")).expect("written");
        assert_eq!(kept, shared.recent);
        drop(shared);
        assert!(
            yard.directory
                .join("journals")
                .read_dir()
                .expect("the garden's journal")
                .next()
                .is_some(),
            "the garden's journal is the session's directory's"
        );
        // The first level opened again keeps the journal that `--journal` names.
        let reopen = Request::Run {
            name: "file.open".to_owned(),
            arguments: vec![yard.directory.join("yard.scene").display().to_string()],
        };
        yard.shared.borrow_mut().requests.push_back(reopen);
        yard.frame();
        let reply = yard
            .session
            .edit(yard.host.world_mut(), &slime_channel::Edit::History)
            .expect("the history");
        let slime_channel::Reply::History { history } = reply else {
            panic!("{reply:?}");
        };
        assert_eq!(
            Path::new(&history.journal),
            yard.directory.join("yard.journal")
        );
    }
}
