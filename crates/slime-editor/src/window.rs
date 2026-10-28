//! The editor's window: a presentation of its program, on the engine's interface in the editor's
//! look, which reads what the core shares and asks it through requests
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`): a menu bar, fixed regions, the dev
//! overlay's world panel in the first, a status bar, and the window's title; and the system's dialog
//! to open a level, a modal layer waiting over the window meanwhile
//! (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`); the viewport, through which
//! the user flies and picks what to select (`docs/adr/0158-the-viewport.md`,
//! `docs/adr/0159-picking-and-the-selection.md`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slime_app::{
    AppConfig, AppError, Context, Host, PickAnswer, Presentation, Reflected, ShownView,
};
use slime_color::LinearColor;
use slime_devui::WorldPanel;
use slime_gfx::{Gpu, Image, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_platform::{ColorScheme, DialogId, Event, FileChoice, MouseButton, ScrollDelta};
use slime_render::{CanvasImage, DebugMode, Pick, RenderError, Renderer, ViewSettings};
use slime_ui::Rect;
use slime_ui::action::{Action, ActionKind, FieldValue, SurfaceInput};
use slime_ui::display::{BoxPrimitive, DisplayList};
use slime_ui::look::{self, Colors, Density};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Dimension, FlexDirection, Justify, Sides, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, ItemMark, Key, Layer, LayoutDirection, Notification,
    NotificationKind, PaletteCommand, Place, Role, RootSettings, Side, Ui,
};
use slime_ui_render::UiRenderer;
use slime_world::{Entity, World};

use crate::navigation::Navigation;
use crate::operation::{self, OPERATIONS, SHORTCUTS};
use crate::picking::{self, Selecting, Wanted};
use crate::program::Program;
use crate::session::Session;
use crate::shared::{Notice, Request, Shared};
use crate::viewport::{self, Framing, Marks, Viewport};

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
            Entry::Operation("file.choose"),
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
        "View",
        &[
            Entry::Operation("view.frame_selection"),
            Entry::Separator,
            Entry::Operation("view.shade_lit"),
            Entry::Operation("view.shade_base_color"),
            Entry::Operation("view.shade_light"),
            Entry::Operation("view.shade_normals"),
            Entry::Operation("view.shade_shadows"),
            Entry::Separator,
            Entry::Operation("view.wireframe"),
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

/// The width of the layer that waits for the system's dialog to open a level, in logical pixels.
const CHOOSING_WIDTH: f32 = 460.0;

/// The marquee's fill and border over the viewport (`docs/adr/0159-picking-and-the-selection.md`).
const MARQUEE_FILL: LinearColor = LinearColor::rgba(1.0, 1.0, 1.0, 0.08);
const MARQUEE_BORDER: LinearColor = LinearColor::rgba(1.0, 1.0, 1.0, 0.7);

/// The mode of the viewport's shading that the operation `name` shows, if it is one of the View
/// menu's (`view.shade_lit`, `view.shade_base_color`, `view.shade_light`, `view.shade_normals`,
/// `view.shade_shadows`).
fn shading(name: &str) -> Option<DebugMode> {
    match name {
        "view.shade_lit" => Some(DebugMode::Off),
        "view.shade_base_color" => Some(DebugMode::BaseColor),
        "view.shade_light" => Some(DebugMode::Light),
        "view.shade_normals" => Some(DebugMode::Normals),
        "view.shade_shadows" => Some(DebugMode::Shadows),
        _ => None,
    }
}

/// What the layer that waits for the system's dialog says while it waits.
const CHOOSE: &str = "Choose a level in the system's dialog, or type its path.";

/// What it says where no dialog showed.
const NO_DIALOG_SHOWED: &str = "No dialog of the system showed: type the level's path.";

/// What it says in a program that opens no dialog of the system, which a tool started.
const TYPE_THE_PATH: &str = "Type the level's path.";

/// How soon a dialog of the system that answers no file answers before a person could have: none
/// showed, where neither the desktop's portal nor `zenity` answers. The layer that waits stays
/// then, for the level's path to be typed, where a cancel closes it.
const NO_DIALOG: Duration = Duration::from_millis(500);

/// What an item of an open menu does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    /// Runs an operation with its arguments.
    Run(&'static str, Vec<String>),
    /// Opens the submenu of the recent levels.
    Recent,
}

/// The modal layer that waits over the window for the system's dialog to open a level, where the
/// level's path can be typed too.
struct Choosing {
    layer: ElementId,
    field: ElementId,
    open: ElementId,
    cancel: ElementId,
    /// Whether the window is yet to open the system's dialog ([`Model::dialog_wanted`]).
    wanted: bool,
    /// The system's dialog that the window opened, and when, until it answers.
    dialog: Option<(DialogId, Instant)>,
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
    /// The layer that waits for the system's dialog, while it is open.
    choosing: Option<Choosing>,
    /// The viewport's surface, once built.
    surface: Option<ElementId>,
    /// How the viewport's camera moves, and where it is.
    pub(crate) navigation: Navigation,
    /// Whether the viewport is to frame the selection at the next frame (`view.frame_selection`).
    pub(crate) frame_wanted: bool,
    /// The left button held over the viewport without Alt, while it is.
    selecting: Option<Selecting>,
    /// Where Alt and the left button went down over the viewport, while they orbit: let go before
    /// the mouse moved, they go through what lies behind.
    orbit_from: Option<[f32; 2]>,
    /// What the left button asked for over the viewport that the window has not asked the
    /// viewport yet ([`Model::picked`] takes its answer).
    pub(crate) wanted: Vec<Wanted>,
    /// How the viewport draws, which the View menu and tools change.
    pub(crate) settings: ViewSettings,
    /// Whether the viewport draws the wireframe of what is selected (`view.wireframe`).
    pub(crate) wireframe: bool,
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
            choosing: None,
            surface: None,
            navigation: Navigation::default(),
            // The level, framed as it opens.
            frame_wanted: true,
            selecting: None,
            orbit_from: None,
            wanted: Vec::new(),
            settings: ViewSettings::default(),
            wireframe: false,
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
            if let ActionKind::Surface(input) = action.kind {
                self.navigate(input);
                continue;
            }
            if let Some(request) = self.request_of(&action) {
                requests.push(request);
            }
        }
        let made = !requests.is_empty();
        self.shared.borrow_mut().requests.extend(requests);
        took || made
    }

    /// Moves the viewport's camera, or picks in it, as the pointer's `input` on its surface asks.
    fn navigate(&mut self, input: SurfaceInput) {
        match input {
            SurfaceInput::Pressed {
                button,
                modifiers,
                at,
            } => {
                // A gesture that starts before the level opening is framed keeps the camera where
                // the user takes it. A marquee drawn holds the other buttons back.
                if self.selecting.is_none() && self.navigation.press(button, modifiers, at) {
                    self.frame_wanted = false;
                    if button == MouseButton::Left {
                        self.orbit_from = Some(at);
                    }
                } else if button == MouseButton::Left
                    && self.selecting.is_none()
                    && !self.navigation.holds_cursor()
                {
                    self.selecting = Some(Selecting::new(at, modifiers));
                }
            }
            SurfaceInput::Released { button, at } => {
                let left = button == MouseButton::Left;
                if left && let Some(selecting) = self.selecting.take() {
                    self.wanted.push(selecting.release(at));
                }
                let from = if left { self.orbit_from.take() } else { None };
                if self.navigation.release(button)
                    && let Some(at) = from
                {
                    self.wanted.push(Wanted::Through { at });
                }
            }
            SurfaceInput::Wheel { delta, .. } => self.navigation.wheel(match delta {
                ScrollDelta::Lines { y, .. } => f64::from(y),
                // A touchpad's pixels, a line some 40 of them.
                ScrollDelta::Pixels { y, .. } => y / 40.0,
            }),
            // Moves come where the cursor cannot be held; where it is, the raw motion turns it.
            SurfaceInput::Moved { at } => {
                if let Some(selecting) = &mut self.selecting {
                    selecting.moved(at);
                }
                self.navigation.pointer(at);
            }
        }
    }

    /// Selects what `found` holds, the entities that `wanted` found, the window having asked the
    /// viewport (`picking::select`), as the core's selection.
    pub(crate) fn picked(&mut self, world: &World, wanted: &Wanted, found: &[Entity]) {
        let selection = picking::select(world, wanted, found, &self.selected_entities(world));
        let selection = selection
            .into_iter()
            .filter_map(|entity| world.persistent_id(entity))
            .collect();
        self.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(selection));
    }

    /// The marquee being drawn over the viewport, in pixels of the screen.
    fn marquee(&self, scale: f32) -> Option<Rect> {
        let (from, to) = self.selecting?.marquee()?;
        let surface = self.viewport_rect()?;
        let left = from[0].min(to[0]) * scale;
        let top = from[1].min(to[1]) * scale;
        Some(Rect::new(
            surface.x + left,
            surface.y + top,
            (from[0] - to[0]).abs() * scale,
            (from[1] - to[1]).abs() * scale,
        ))
    }

    /// The viewport's surface's box, in pixels of the screen, once laid out.
    pub(crate) fn viewport_rect(&self) -> Option<slime_ui::Rect> {
        self.ui.rect(self.surface?)
    }

    /// The size of the viewport's image, in pixels of the screen: its surface's, at least 1 by 1.
    pub(crate) fn viewport_size(&self) -> Option<(u32, u32)> {
        let rect = self.viewport_rect()?;
        let side = |length: f32| (length.round() as u32).max(1);
        Some((side(rect.width), side(rect.height)))
    }

    /// What the viewport marks of `world`, at `scale` pixels of the screen per logical pixel: what
    /// the core says is selected, and its wireframe if asked for.
    pub(crate) fn marks(&self, world: &World, scale: f32) -> Marks {
        Marks {
            selection: self.selected_entities(world),
            wireframe: self.wireframe,
            scale,
        }
    }

    /// The entities that the core says are selected, which `world` holds.
    pub(crate) fn selected_entities(&self, world: &World) -> Vec<Entity> {
        self.shared
            .borrow()
            .selection
            .iter()
            .filter_map(|identity| world.find(identity))
            .collect()
    }

    /// The request that `action` makes, after the menus and the layers that it opens or closes.
    fn request_of(&mut self, action: &Action) -> Option<Request> {
        if let Some(choosing) = &self.choosing
            && let Some(id) = action.element
        {
            match &action.kind {
                ActionKind::Press if id == choosing.open => return self.open_typed(),
                // Enter, not the focus leaving the field for Cancel.
                ActionKind::Committed(FieldValue::Text(_))
                    if id == choosing.field && self.ui.focus() == Some(id) =>
                {
                    return self.open_typed();
                }
                ActionKind::Press if id == choosing.cancel => {
                    self.close_choosing();
                    return None;
                }
                ActionKind::Dismissed if id == choosing.layer => {
                    self.choosing = None;
                    return None;
                }
                _ => {}
            }
        }
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
                        // The viewport's modes, ticked where they are on.
                        let mark = match (name, shading(name)) {
                            ("view.wireframe", _) => ItemMark::Check(self.wireframe),
                            (_, Some(debug)) => ItemMark::Check(self.settings.debug == debug),
                            _ => ItemMark::None,
                        };
                        (
                            label.to_owned(),
                            chord,
                            mark,
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

    /// Opens the layer that waits over the window, `size` logical pixels, for the system's dialog
    /// to open a level, which the window opens ([`Model::dialog_wanted`]); one at a time.
    fn choose_level(&mut self, size: [f32; 2]) {
        if self.choosing.is_some() {
            return;
        }
        let layer = Layer {
            place: self.choosing_place(size),
            modal: true,
            dismiss: true,
        };
        let layer = self.ui.open_layer("choose", layer, Style::default());
        let dialogs = self.shared.borrow().dialogs;
        let said = if dialogs { CHOOSE } else { TYPE_THE_PATH };
        let (field, open, cancel) = self.build_choosing(layer, said, 0);
        self.ui.set_focus(Some(field));
        self.choosing = Some(Choosing {
            layer,
            field,
            open,
            cancel,
            wanted: dialogs,
            dialog: None,
        });
    }

    /// Where the layer that waits for the system's dialog stands over a window of `size` logical
    /// pixels: in the middle across, a quarter of the way down.
    fn choosing_place(&self, size: [f32; 2]) -> Place {
        let point = [
            ((size[0] - CHOOSING_WIDTH) / 2.0).max(0.0).round(),
            (size[1] / 4.0).round(),
        ];
        Place::At {
            over: self.root,
            point,
        }
    }

    /// Builds the layer that waits for the system's dialog, `layer`, for `revision`, saying `said`:
    /// its field and its buttons Open and Cancel.
    fn build_choosing(
        &mut self,
        layer: ElementId,
        said: &str,
        revision: u64,
    ) -> (ElementId, ElementId, ElementId) {
        let mut built = None;
        self.ui.build(layer, revision, |children| {
            let mut dialog = column();
            dialog.layout.size[0] = Dimension::Pixels(CHOOSING_WIDTH);
            let dialog = Element::container(dialog)
                .class("dialog")
                .role(Role::Dialog)
                .named("Open a level");
            children.add("dialog", dialog, |dialog| {
                let title = Element::label("Open a level", Style::default()).class("heading");
                dialog.add("title", title, |_| {});
                dialog.add("said", Element::label(said, Style::default()), |_| {});
                let field = Element::text_field("", Style::default()).named("Level's path");
                let field = dialog.add("path", field, |_| {});
                let mut row = Style::default();
                row.layout.justify = Justify::End;
                row.layout.gap = [0.0, 8.0];
                let mut buttons = None;
                dialog.add("buttons", Element::container(row), |row| {
                    let cancel = Element::button("Cancel", Style::default());
                    let cancel = row.add("cancel", cancel, |_| {});
                    let open = row.add("open", Element::button("Open", Style::default()), |_| {});
                    buttons = Some((open, cancel));
                });
                let (open, cancel) = buttons.expect("built above");
                built = Some((field, open, cancel));
            });
        });
        built.expect("built above")
    }

    /// The request to open the level whose path the waiting layer's field holds, which closes the
    /// layer; none while the field is empty.
    fn open_typed(&mut self) -> Option<Request> {
        let field = self.choosing.as_ref()?.field;
        let path = self.ui.field_text(field)?.trim().to_owned();
        if path.is_empty() {
            return None;
        }
        self.close_choosing();
        Some(Request::Run {
            name: "file.open".to_owned(),
            arguments: vec![path],
        })
    }

    fn close_choosing(&mut self) {
        if let Some(choosing) = self.choosing.take() {
            self.ui.remove(choosing.layer);
        }
    }

    /// The system's dialog that the window is to open for the layer that waits, once: a level's
    /// scene file, from the open level's directory.
    pub(crate) fn dialog_wanted(&mut self) -> Option<FileChoice> {
        let choosing = self.choosing.as_mut().filter(|choosing| choosing.wanted)?;
        choosing.wanted = false;
        let directory = self
            .shared
            .borrow()
            .level
            .as_deref()
            .and_then(Path::parent)
            // A level named without a directory lies in the current one.
            .map(|directory| match directory.as_os_str().is_empty() {
                true => Path::new("."),
                false => directory,
            })
            .and_then(|directory| std::path::absolute(directory).ok());
        Some(FileChoice {
            title: "Open a level".to_owned(),
            filters: vec![("Levels".to_owned(), vec!["scene".to_owned()])],
            directory,
        })
    }

    /// Notes that the window opened the system's dialog `dialog`, at `now`, for the layer that
    /// waits.
    pub(crate) fn dialog_opened(&mut self, dialog: DialogId, now: Instant) {
        if let Some(choosing) = &mut self.choosing {
            choosing.dialog = Some((dialog, now));
        }
    }

    /// Hears the system's dialog `dialog` answer `file`, at `now`: the file chosen opens, as
    /// `file.open` opens a path, and no file closes the layer that waits, but where the dialog
    /// answered sooner than a person could have ([`NO_DIALOG`]): the layer says so, and waits for
    /// the path typed. A dialog that the layer did not open, or that it closed without, is not
    /// heard.
    pub(crate) fn file_chosen(&mut self, dialog: DialogId, file: Option<PathBuf>, now: Instant) {
        let Some(choosing) = &mut self.choosing else {
            return;
        };
        let Some((_, opened)) = choosing.dialog.filter(|&(asked, _)| asked == dialog) else {
            return;
        };
        match file {
            Some(file) => {
                self.close_choosing();
                let mut shared = self.shared.borrow_mut();
                match file.into_os_string().into_string() {
                    Ok(path) => shared.requests.push_back(Request::Run {
                        name: "file.open".to_owned(),
                        arguments: vec![path],
                    }),
                    Err(file) => shared.notices.push(Notice {
                        failure: true,
                        text: format!(
                            "{}: the editor opens a level whose path is Unicode text",
                            Path::new(&file).display()
                        ),
                    }),
                }
            }
            None if now.saturating_duration_since(opened) < NO_DIALOG => {
                choosing.dialog = None;
                let (layer, field) = (choosing.layer, choosing.field);
                self.build_choosing(layer, NO_DIALOG_SHOWED, 1);
                self.ui.set_focus(Some(field));
            }
            None => self.close_choosing(),
        }
    }

    /// Runs the window's operations that the core was asked to run, over a window of `size`
    /// logical pixels: the palette, the sheet of shortcuts, the system's dialog to open a level.
    fn run_window_operations(&mut self, size: [f32; 2]) {
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
                "file.choose" => self.choose_level(size),
                "view.frame_selection" => self.frame_wanted = true,
                "view.wireframe" => self.wireframe = !self.wireframe,
                name => match shading(name) {
                    Some(debug) => self.settings.debug = debug,
                    None => tracing::warn!(name, "the window has no operation of that name"),
                },
            }
        }
    }

    /// Lays the window out at `size`, in logical pixels, and `scale` pixels of the screen per
    /// logical pixel, from what the core shares and from `world`, and gives what to draw.
    pub(crate) fn frame(&mut self, world: &World, size: [f32; 2], scale: f32) -> DisplayList {
        self.ui.set_root(self.root, settings(size, scale));
        // The layer that waits stays in the middle of a window whose size changes.
        if let Some(layer) = self.choosing.as_ref().map(|choosing| choosing.layer) {
            let place = self.choosing_place(size);
            self.ui.set_layer_place(layer, place);
        }
        self.run_window_operations(size);
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
        let mut display = self.ui.frame().0;
        // Over the viewport, under nothing: the pointer is the surface's while it is drawn.
        if let Some(rect) = self.marquee(scale) {
            display.push(
                BoxPrimitive {
                    rect,
                    radii: [0.0; 4],
                    fill: MARQUEE_FILL,
                    border_width: scale,
                    border: MARQUEE_BORDER,
                },
                self.viewport_rect(),
            );
        }
        display
    }

    /// Follows what the core changed: a level opened in place of the other resets the world panel
    /// and is framed in the viewport; a selection that the core changed, an undo or an agent's,
    /// selects in the panel.
    fn follow_the_core(&mut self, world: &World) {
        let shared = self.shared.borrow();
        if shared.opened != self.opened {
            self.opened = shared.opened;
            self.panel = WorldPanel::default();
            self.selected = None;
            // Another level, framed as it opens.
            self.frame_wanted = true;
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
        let mut surface = None;
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
                    let center = Element::surface(viewport::IMAGE, center).named("viewport");
                    surface = Some(regions.add("viewport", center, |_| {}));
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
        if surface.is_some() {
            self.surface = surface;
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
/// level is open and the operations named `operations` run, without arguments, on the GPU that
/// [`Gpu::new`] opens, without a window of the system: what the `editor` example's `--output`
/// draws, and what its image tests compare. The level is opened as the program opens it, its
/// journal held and written, and the recent levels left as they are; the status bar shows the
/// level's path as `--level` gives it. `file.choose` shows the layer that waits for the system's
/// dialog, which no window opens here. Once the level is framed, the entities of `selection`, by
/// their persistent identities, are selected, which the viewport outlines.
///
/// # Errors
///
/// When the arguments are not the program's, the level cannot be opened, an operation fails, an
/// identity of `selection` is not one, no GPU adapter is available, or the window cannot be drawn:
/// why, as a sentence.
pub fn window_image(
    program: &Program,
    arguments: impl IntoIterator<Item = OsString>,
    operations: &[&str],
    selection: &[&str],
    scale: f32,
) -> Result<Image, String> {
    let options = crate::program::parse(program, arguments)?.ok_or("--help draws no image")?;
    let shared = Shared::default();
    // Drawn as a person sees it, waiting for the dialog that no window opens here.
    shared.borrow_mut().window = true;
    shared.borrow_mut().dialogs = true;
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
    for name in operations {
        let run = Request::Run {
            name: (*name).to_owned(),
            arguments: Vec::new(),
        };
        session
            .handle(host.world_mut(), run)
            .map_err(|error| slime_app::error_chain(&*error))?;
    }
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
    let (assets, lighting) = {
        let shared = model.shared.borrow();
        (shared.assets.clone(), shared.lighting)
    };
    let mut viewport = assets
        .zip(lighting)
        .map(|(assets, lighting)| Viewport::new(&gpu, &drawing.renderer, assets, lighting, format))
        .transpose()
        .map_err(|error| error.to_string())?;
    // The level framed as it opens, then the selection made.
    model.frame(host.world(), size, scale);
    if let (Some(viewport), Some(size)) = (&mut viewport, model.viewport_size()) {
        frame_whole(&gpu, viewport, &mut model, host.world(), size)?;
    }
    if !selection.is_empty() {
        let identities = selection
            .iter()
            .map(|identity| {
                identity
                    .parse()
                    .map_err(|error| format!("{identity}: {error}"))
            })
            .collect::<Result<_, String>>()?;
        session
            .handle(host.world_mut(), Request::Select(identities))
            .map_err(|error| slime_app::error_chain(&*error))?;
    }
    // Twice: the world panel's list builds its rows once the first frame laid it out.
    for _ in 0..2 {
        let display = model.frame(host.world(), size, scale);
        if let (Some(viewport), Some(size)) = (&mut viewport, model.viewport_size()) {
            draw_whole(
                &gpu,
                &mut drawing.renderer,
                viewport,
                &model.marks(host.world(), scale),
                &model,
                host.world(),
                size,
            )?;
        }
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
        let image = viewport.as_ref().and_then(Viewport::image);
        drawing
            .bridge
            .draw(
                &gpu,
                &mut drawing.renderer,
                model.ui.fonts(),
                &display,
                |key| match (key, image) {
                    (viewport::IMAGE, Some(image)) => Some(CanvasImage::Target(image)),
                    _ => None,
                },
                &target,
            )
            .map_err(|error| error.to_string())?;
    }
    gpu.read_render_target(&target)
        .map_err(|error| error.to_string())
}

/// How long [`window_image`] waits at most for the meshes that the level places to load.
const LOADING: std::time::Duration = std::time::Duration::from_secs(60);

/// Frames the level in `viewport` of `size` pixels as it opens, once the meshes that `world`
/// places have loaded, for [`window_image`].
fn frame_whole(
    gpu: &Gpu,
    viewport: &mut Viewport,
    model: &mut Model,
    world: &World,
    size: (u32, u32),
) -> Result<(), String> {
    let started = Instant::now();
    while model.frame_wanted {
        match viewport
            .bounds(gpu, world, &model.selected_entities(world))
            .map_err(|error| error.to_string())?
        {
            Framing::Loading if started.elapsed() > LOADING => {
                return Err("the level's meshes did not load within a minute".to_owned());
            }
            Framing::Loading => std::thread::sleep(std::time::Duration::from_millis(5)),
            Framing::Nothing => model.frame_wanted = false,
            Framing::Sphere(center, radius) => {
                model
                    .navigation
                    .frame(center, radius, f64::from(size.0) / f64::from(size.1));
                model.frame_wanted = false;
            }
        }
    }
    Ok(())
}

/// Draws `viewport` of `size` pixels as the window shows it, marked as `marks` say, once the
/// meshes that `world` places have loaded, for [`window_image`].
fn draw_whole(
    gpu: &Gpu,
    renderer: &mut Renderer,
    viewport: &mut Viewport,
    marks: &Marks,
    model: &Model,
    world: &World,
    size: (u32, u32),
) -> Result<(), String> {
    let started = Instant::now();
    viewport.set_settings(model.settings);
    loop {
        let loading = viewport
            .prepare(
                gpu,
                world,
                model.navigation.camera(),
                size,
                marks,
                Instant::now(),
            )
            .map_err(|error| error.to_string())?;
        viewport
            .draw(gpu, renderer)
            .map_err(|error| error.to_string())?;
        if !loading {
            return Ok(());
        }
        if started.elapsed() > LOADING {
            return Err("the level's meshes did not load within a minute".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
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

/// Who asked the viewport what is drawn where.
enum Asker {
    /// The user, with the left button.
    User(Wanted),
    /// A tool, through the development channel, given the entity drawn at a pixel.
    Tool(PickAnswer),
}

/// The area of the viewport's image, in its pixels, that `wanted` asks about, at `scale` pixels per
/// logical pixel, in an image of `size` pixels: the pixel under a click, the rectangle between a
/// marquee's corners, at least a pixel, within the image; `None` for what the CPU answers.
fn area(
    wanted: &Wanted,
    scale: f32,
    (width, height): (u32, u32),
) -> Option<slime_render::Viewport> {
    let pixel = |at: [f32; 2]| {
        [
            ((at[0] * scale).floor().max(0.0) as u32).min(width - 1),
            ((at[1] * scale).floor().max(0.0) as u32).min(height - 1),
        ]
    };
    let (from, to) = match *wanted {
        Wanted::Click { at, .. } => (pixel(at), pixel(at)),
        Wanted::Marquee { from, to, .. } => {
            let (from, to) = (pixel(from), pixel(to));
            (
                [from[0].min(to[0]), from[1].min(to[1])],
                [from[0].max(to[0]), from[1].max(to[1])],
            )
        }
        Wanted::Through { .. } => return None,
    };
    Some(slime_render::Viewport {
        x: from[0],
        y: from[1],
        width: to[0] - from[0] + 1,
        height: to[1] - from[1] + 1,
    })
}

/// The entities drawn in what a picking found, each once, in the order of its pixels, those that
/// `world` still holds.
fn found(world: &World, picked: &slime_render::Picked) -> Vec<Entity> {
    let mut found: Vec<Entity> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &number in &picked.numbers {
        if number != 0
            && seen.insert(number)
            && let Some(entity) = Entity::from_bits(number).filter(|&entity| world.contains(entity))
        {
            found.push(entity);
        }
    }
    found
}

/// The editor's window, the presentation of its program.
pub(crate) struct Window {
    shared: Shared,
    model: Option<Model>,
    drawing: Option<Drawing>,
    /// The viewport, once a level is open and the GPU there.
    viewport: Option<Viewport>,
    /// The pickings asked of the viewport's view that have not come back, each with who asked:
    /// the user, for what, or a tool, where to answer it
    /// (`docs/adr/0159-picking-and-the-selection.md`).
    picks: Vec<(Pick, Asker)>,
    /// What the last extraction laid out, which the frame draws.
    display: Option<DisplayList>,
    /// The title that the window shows.
    title: String,
    /// Whether the windowing system refused to hold the cursor, which the user was told once.
    cursor_refused: bool,
    /// Whether the window tried to hold the cursor for the gesture under way, once for each.
    cursor_tried: bool,
    /// How many levels the core had opened in place of the first when the viewport last took the
    /// cooked data.
    viewport_opened: u64,
}

impl Window {
    pub(crate) fn new(shared: Shared) -> Self {
        Self {
            shared,
            model: None,
            drawing: None,
            viewport: None,
            picks: Vec::new(),
            display: None,
            title: String::new(),
            cursor_refused: false,
            cursor_tried: false,
            viewport_opened: 0,
        }
    }

    /// Holds the cursor while a gesture of the viewport moves the camera, once for each gesture,
    /// and lets it go after; tells the user once if the windowing system refuses, the cursor left
    /// free and the camera following it.
    fn hold_cursor(&mut self, window: &slime_platform::Window, held: bool) {
        if !held {
            self.cursor_tried = false;
            if window.cursor_held() {
                let _ = window.set_cursor_held(false);
            }
            return;
        }
        if window.cursor_held() || std::mem::replace(&mut self.cursor_tried, true) {
            return;
        }
        if let Err(error) = window.set_cursor_held(true)
            && !std::mem::replace(&mut self.cursor_refused, true)
        {
            self.shared.borrow_mut().notices.push(Notice {
                failure: true,
                text: format!(
                    "{}: the cursor stays free, and the camera follows it",
                    slime_app::error_chain(&error)
                ),
            });
        }
    }

    /// Makes the viewport ready to draw this frame, made first once a level is open: what it
    /// shows changed, or its meshes still load, which asks for the next frame.
    fn prepare_viewport(&mut self, ctx: &mut Context, world: &World) {
        let (Some(model), Some(drawing), Some(gpu), Some(format)) = (
            &mut self.model,
            &self.drawing,
            ctx.gpu(),
            ctx.surface_format(),
        ) else {
            return;
        };
        let shared = self.shared.borrow();
        let (Some(assets), Some(lighting), opened) =
            (shared.assets.clone(), shared.lighting, shared.opened)
        else {
            return;
        };
        drop(shared);
        match &mut self.viewport {
            None => match Viewport::new(gpu, &drawing.renderer, assets, lighting, format) {
                Ok(viewport) => self.viewport = Some(viewport),
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            },
            // A level opened since: the cooked data as the core read it then, which may hold what
            // was cooked since the first.
            Some(viewport) if opened != self.viewport_opened => viewport.reload(gpu, assets),
            Some(_) => {}
        }
        self.viewport_opened = opened;
        let (Some(viewport), Some(size)) = (&mut self.viewport, model.viewport_size()) else {
            return;
        };
        if model.frame_wanted {
            match viewport.bounds(gpu, world, &model.selected_entities(world)) {
                Ok(Framing::Loading) => {}
                Ok(framing) => {
                    model.frame_wanted = false;
                    if let Framing::Sphere(center, radius) = framing {
                        let aspect = f64::from(size.0) / f64::from(size.1);
                        model.navigation.frame(center, radius, aspect);
                    }
                }
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            }
        }
        let camera = model.navigation.camera();
        let scale = ctx
            .window()
            .map_or(1.0, |window| window.scale_factor() as f32);
        // What the left button asked: by the GPU, at the frame that draws the viewport next; by
        // the CPU, at once.
        for wanted in std::mem::take(&mut model.wanted) {
            match area(&wanted, scale, size) {
                Some(area) => self.picks.push((viewport.pick(area), Asker::User(wanted))),
                None => {
                    let Wanted::Through { at } = wanted else {
                        continue;
                    };
                    let at = [at[0] * scale, at[1] * scale];
                    let met = viewport.cast_through(world, &camera, size, at);
                    model.picked(world, &wanted, &met);
                }
            }
        }
        viewport.set_settings(model.settings);
        let marks = model.marks(world, scale);
        match viewport.prepare(gpu, world, camera, size, &marks, Instant::now()) {
            Ok(true) => ctx.request_frame(),
            Ok(false) => {}
            Err(error) => ctx.exit_with_error(error),
        }
    }

    /// Takes the answers of the pickings that came back, each to whom asked; asks for the next
    /// frame while some wait, and for the one at which the core selects what a person's found.
    fn take_picks(&mut self, ctx: &mut Context, world: &World) {
        let (Some(model), Some(gpu)) = (&mut self.model, ctx.gpu()) else {
            return;
        };
        let mut waiting = Vec::new();
        let mut selected = false;
        for (pick, asker) in std::mem::take(&mut self.picks) {
            match (pick.take(gpu), asker) {
                (None, asker) => waiting.push((pick, asker)),
                (Some(Ok(picked)), Asker::User(wanted)) => {
                    model.picked(world, &wanted, &found(world, &picked));
                    selected = true;
                }
                (Some(Ok(picked)), Asker::Tool(answer)) => {
                    answer.give(found(world, &picked).first().copied());
                }
                // The tool is told that no answer came, as the answer is dropped.
                (Some(Err(error)), _) => {
                    tracing::warn!("a picking in the viewport failed: {error}")
                }
            }
        }
        if selected || !waiting.is_empty() {
            ctx.request_frame();
        }
        self.picks = waiting;
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
        let Some(model) = &mut self.model else {
            return false;
        };
        match event {
            // The mouse's raw motion turns the camera, and the keys fly it, while the cursor is held.
            Event::RawMotion { delta } => {
                model.navigation.motion(*delta);
            }
            Event::Key(key) => {
                let down = key.state == slime_platform::ButtonState::Pressed;
                return model.navigation.key(key.physical, down, Instant::now());
            }
            Event::ModifiersChanged(modifiers) => model.navigation.set_fast(modifiers.shift),
            Event::ColorSchemeChanged(scheme) => model.set_colors(match scheme {
                ColorScheme::Dark => Colors::Dark,
                ColorScheme::Light => Colors::Light,
            }),
            // What it asks, the core answers at the frame that the event runs.
            Event::FileChosen { dialog, file } => {
                model.file_chosen(*dialog, file.clone(), Instant::now());
            }
            _ => {}
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
            if let Some(choice) = model.dialog_wanted() {
                model.dialog_opened(window.choose_file(choice), Instant::now());
            }
        }
        // The keys that fly the camera move it at every frame while they are held.
        again |= model.navigation.advance(Instant::now());
        let held = model.navigation.holds_cursor();
        let title = model.title();
        if again {
            ctx.request_frame();
        }
        if let Some(window) = ctx.window().cloned() {
            self.hold_cursor(&window, held);
            if title != self.title {
                window.set_title(&title);
                self.title = title;
            }
        }
        self.take_picks(ctx, world);
        self.prepare_viewport(ctx, world);
        // What the left button asked where no viewport could answer, without a level or a GPU
        // device, is not answered later, against another image.
        if let Some(model) = &mut self.model {
            model.wanted.clear();
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(model), Some(drawing), Some(display), Some(gpu)) = (
            &self.model,
            &mut self.drawing,
            &self.display,
            ctx.gpu().cloned(),
        ) else {
            return;
        };
        let gpu = &gpu;
        if let Some(viewport) = &mut self.viewport {
            match viewport.draw(gpu, &mut drawing.renderer) {
                // A new size that has not held still yet is drawn once it has.
                Ok(Some(due)) => ctx.request_frame_at(due),
                Ok(None) => {}
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            }
        }
        let image = self.viewport.as_ref().and_then(Viewport::image);
        if let Err(error) = drawing.bridge.draw(
            gpu,
            &mut drawing.renderer,
            model.ui.fonts(),
            display,
            |key| match (key, image) {
                (viewport::IMAGE, Some(image)) => Some(CanvasImage::Target(image)),
                _ => None,
            },
            target,
        ) {
            ctx.exit_with_error(error);
        }
    }

    fn views(&self) -> Vec<ShownView> {
        let (Some(model), Some(_)) = (&self.model, &self.viewport) else {
            return Vec::new();
        };
        model
            .viewport_rect()
            .map(|rect| ShownView {
                name: viewport::NAME.to_owned(),
                x: rect.x.round() as u32,
                y: rect.y.round() as u32,
                width: rect.width.round() as u32,
                height: rect.height.round() as u32,
            })
            .into_iter()
            .collect()
    }

    fn reports(&self) -> Vec<(String, Reflected)> {
        self.viewport
            .as_ref()
            .map(|viewport| {
                (
                    viewport::NAME.to_owned(),
                    Reflected::new(&viewport.report()),
                )
            })
            .into_iter()
            .collect()
    }

    fn view_settings(&self, name: &str) -> Option<Reflected> {
        let model = self.model.as_ref()?;
        (name == viewport::NAME).then(|| Reflected::new(&model.settings))
    }

    fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
        if name != viewport::NAME {
            return Err(format!("the editor has no view `{name}`"));
        }
        let model = self
            .model
            .as_mut()
            .ok_or("the editor's window is not open yet")?;
        model.settings = settings.get().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn pick(&mut self, view: &str, (x, y): (u32, u32), answer: PickAnswer) -> Result<(), String> {
        if view != viewport::NAME {
            return Err(format!("the editor has no view `{view}`"));
        }
        let viewport = self
            .viewport
            .as_mut()
            .ok_or("the editor's viewport waits for a level and a GPU device")?;
        let area = slime_render::Viewport {
            x,
            y,
            width: 1,
            height: 1,
        };
        self.picks.push((viewport.pick(area), Asker::Tool(answer)));
        Ok(())
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every GPU object goes; the viewport is made again once the device is. The pickings of
        // the lost device go with it, a tool's told that no answer came.
        self.picks.clear();
        self.viewport = None;
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
    use slime_ui::display::Primitive;
    use slime_world::{Command, PersistentId};

    use super::*;
    use crate::operation::Changes;
    use crate::program::Lighting;

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
                lighting: Lighting::default(),
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
            shared.borrow_mut().dialogs = true;
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

    /// The left button over the viewport: a click, a drag that draws a marquee, and Alt and a click,
    /// each asking what is drawn there, and what the viewport found selected in the core
    /// (`docs/adr/0159-picking-and-the-selection.md`).
    #[test]
    fn the_left_button_over_the_viewport_asks_what_is_drawn_there_and_selects_it() {
        let mut yard = Yard::new("pick");
        let rect = yard.model.viewport_rect().expect("laid out");
        let (x, y) = (rect.x + 100.0, rect.y + 80.0);
        let click = |yard: &mut Yard, modifiers: Modifiers| {
            yard.model.ui.modifiers(modifiers);
            yard.model
                .ui
                .pointer_button(MouseButton::Left, ButtonState::Pressed);
            yard.model
                .ui
                .pointer_button(MouseButton::Left, ButtonState::Released);
            yard.model.ui.modifiers(Modifiers::default());
            yard.frame();
        };
        yard.model.ui.pointer_moved(x, y);
        click(&mut yard, Modifiers::default());
        let wanted = Wanted::Click {
            at: [100.0, 80.0],
            adding: false,
        };
        assert_eq!(std::mem::take(&mut yard.model.wanted), [wanted]);
        // What the viewport found there, selected in the core, and the world panel with it.
        let well: PersistentId = "yard/well".parse().expect("an identity");
        let entity = yard.host.world().find(&well).expect("the well");
        yard.model.picked(yard.host.world(), &wanted, &[entity]);
        yard.frame();
        assert_eq!(yard.shared.borrow().selection, [well]);
        assert_eq!(yard.model.selected, Some(entity));
        // A drag: the marquee drawn over the viewport while the button is held, its area asked for
        // as it is let go.
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.model.ui.pointer_moved(x + 50.0, y + 30.0);
        yard.frame();
        let display = yard.model.frame(yard.host.world(), SIZE, 1.0);
        let marquee = Rect::new(x, y, 50.0, 30.0);
        assert!(
            display.items().iter().any(|item| matches!(
                &item.primitive,
                Primitive::Box(drawn) if drawn.rect == marquee && drawn.border == MARQUEE_BORDER
            )),
            "the marquee drawn"
        );
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.frame();
        assert_eq!(
            std::mem::take(&mut yard.model.wanted),
            [Wanted::Marquee {
                from: [100.0, 80.0],
                to: [150.0, 110.0],
                adding: false
            }]
        );
        // Alt and a click, where the mouse did not move: through what lies behind.
        let mut alt = Modifiers::default();
        alt.alt = true;
        click(&mut yard, alt);
        assert_eq!(
            std::mem::take(&mut yard.model.wanted),
            [Wanted::Through { at: [150.0, 110.0] }]
        );
        // The click's pixel and the marquee's area in the viewport's image, at scale 2.
        let area = |wanted| super::area(&wanted, 2.0, (800, 600)).expect("by the GPU");
        assert_eq!(
            area(wanted),
            slime_render::Viewport {
                x: 200,
                y: 160,
                width: 1,
                height: 1
            }
        );
        let marquee = Wanted::Marquee {
            from: [150.0, 110.0],
            to: [100.0, 380.0],
            adding: false,
        };
        assert_eq!(
            area(marquee),
            slime_render::Viewport {
                x: 200,
                y: 220,
                width: 101,
                height: 380
            }
        );
    }

    #[test]
    fn the_view_menu_shades_the_viewport_and_draws_the_wireframe_of_the_selection() {
        let mut yard = Yard::new("shade");
        let run = |yard: &mut Yard, name: &str| {
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: name.to_owned(),
                        arguments: Vec::new(),
                    },
                )
                .expect("run");
            yard.frame();
        };
        run(&mut yard, "view.shade_normals");
        assert_eq!(yard.model.settings.debug, DebugMode::Normals);
        run(&mut yard, "view.shade_light");
        assert_eq!(yard.model.settings.debug, DebugMode::Light);
        run(&mut yard, "view.shade_lit");
        assert_eq!(yard.model.settings.debug, DebugMode::Off);
        run(&mut yard, "view.wireframe");
        assert!(yard.model.wireframe);
        assert!(yard.model.marks(yard.host.world(), 1.0).wireframe);
        run(&mut yard, "view.wireframe");
        assert!(!yard.model.wireframe);
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

    impl Yard {
        /// Runs `file.choose`, as its shortcut or its item runs it: the layer that waits opens, and
        /// the window is to open the system's dialog, as the number `dialog`, at `opened`.
        fn choose(&mut self, dialog: u64, opened: Instant) -> FileChoice {
            self.shared.borrow_mut().requests.push_back(Request::Run {
                name: "file.choose".to_owned(),
                arguments: Vec::new(),
            });
            self.frame();
            let choice = self
                .model
                .dialog_wanted()
                .expect("the system's dialog wanted");
            assert!(self.model.dialog_wanted().is_none(), "wanted once");
            self.model.dialog_opened(DialogId(dialog), opened);
            choice
        }

        /// The element of the layer that waits for the system's dialog at `path`, below its dialog.
        fn waiting(&self, path: &str) -> Option<ElementId> {
            self.model
                .ui
                .element_by_path(&format!("choose/dialog/{path}"))
        }

        fn level(&self) -> PathBuf {
            self.shared.borrow().level.clone().expect("a level open")
        }
    }

    #[test]
    fn the_level_chosen_in_the_system_s_dialog_opens_as_its_path_given_to_file_open() {
        let mut yard = Yard::new("chosen");
        let start = Instant::now();
        let choice = yard.choose(7, start);
        assert_eq!(
            choice.filters,
            [("Levels".to_owned(), vec!["scene".to_owned()])]
        );
        assert_eq!(choice.directory.as_deref(), yard.level().parent());
        assert!(yard.waiting("path").is_some());
        assert_eq!(
            yard.model.ui.focus(),
            yard.waiting("path"),
            "the path can be typed at once"
        );
        // Another dialog's answer is not heard; the layer's opens the level, as `file.open` would.
        let garden = yard.directory.join("garden.scene");
        let later = start + Duration::from_secs(3);
        yard.model
            .file_chosen(DialogId(8), Some(garden.clone()), later);
        assert!(yard.waiting("path").is_some());
        yard.model
            .file_chosen(DialogId(7), Some(garden.clone()), later);
        yard.frame();
        yard.frame();
        assert_eq!(yard.level(), garden);
        assert!(yard.model.ui.element_by_path("choose").is_none());
    }

    #[test]
    fn a_program_that_a_tool_started_opens_no_dialog_and_waits_for_the_path() {
        let mut yard = Yard::new("driven");
        yard.shared.borrow_mut().dialogs = false;
        // Run by its name through the development channel, as an agent runs it.
        yard.session
            .edit(
                yard.host.world_mut(),
                &slime_channel::Edit::Run {
                    name: "file.choose".to_owned(),
                    arguments: Vec::new(),
                },
            )
            .expect("run");
        yard.frame();
        assert!(
            yard.model.dialog_wanted().is_none(),
            "no dialog of the system"
        );
        let said = yard.waiting("said").expect("what the layer says");
        assert!(
            matches!(&yard.model.ui.element(said).expect("a label").kind,
                slime_ui::tree::Kind::Label(text) if text == TYPE_THE_PATH)
        );
        let field = yard.waiting("path").expect("the field");
        let garden = yard.directory.join("garden.scene");
        assert!(
            yard.model
                .ui
                .set_value(field, &garden.display().to_string())
        );
        yard.frame();
        yard.frame();
        assert_eq!(yard.level(), garden);
    }

    #[test]
    fn where_no_dialog_shows_the_level_s_path_is_typed() {
        let mut yard = Yard::new("typed");
        let start = Instant::now();
        yard.choose(1, start);
        // No file, sooner than a person answers: no dialog showed, and the layer says so.
        yard.model
            .file_chosen(DialogId(1), None, start + Duration::from_millis(20));
        yard.frame();
        let said = yard.waiting("said").expect("what the layer says");
        assert!(
            matches!(&yard.model.ui.element(said).expect("a label").kind,
                slime_ui::tree::Kind::Label(text) if text == NO_DIALOG_SHOWED)
        );
        let field = yard.waiting("path").expect("the field");
        assert_eq!(yard.model.ui.focus(), Some(field));
        // Open, while the field is empty, opens nothing.
        let open = yard.waiting("buttons/open").expect("Open");
        yard.model.ui.act(Action {
            element: Some(open),
            kind: ActionKind::Press,
        });
        yard.frame();
        assert!(yard.waiting("path").is_some());
        // The path given, as Enter gives it, opens the level.
        let garden = yard.directory.join("garden.scene");
        assert!(
            yard.model
                .ui
                .set_value(field, &garden.display().to_string())
        );
        yard.frame();
        yard.frame();
        assert_eq!(yard.level(), garden);
        assert!(yard.model.ui.element_by_path("choose").is_none());
    }

    #[test]
    fn a_dialog_cancelled_opens_nothing_and_one_left_is_not_heard() {
        let mut yard = Yard::new("cancelled");
        let yard_level = yard.level();
        let start = Instant::now();
        yard.choose(1, start);
        // No file, later than a person answers: cancelled, the layer closes.
        yard.model
            .file_chosen(DialogId(1), None, start + Duration::from_secs(2));
        yard.frame();
        assert!(yard.model.ui.element_by_path("choose").is_none());
        // Escape closes the layer too; the dialog's answer after it is not heard.
        yard.choose(2, start);
        yard.key(Key::Named(NamedKey::Escape), false);
        assert!(yard.model.ui.element_by_path("choose").is_none());
        let garden = yard.directory.join("garden.scene");
        yard.model
            .file_chosen(DialogId(2), Some(garden), start + Duration::from_secs(2));
        yard.frame();
        yard.frame();
        assert_eq!(yard.level(), yard_level);
        // Cancel does as Escape.
        yard.choose(3, start);
        let cancel = yard.waiting("buttons/cancel").expect("Cancel");
        yard.model.ui.act(Action {
            element: Some(cancel),
            kind: ActionKind::Press,
        });
        yard.frame();
        assert!(yard.model.ui.element_by_path("choose").is_none());
    }
}
