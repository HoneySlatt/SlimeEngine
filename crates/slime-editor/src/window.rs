//! The editor's window: a presentation of its program, on the engine's interface in the editor's
//! look, which reads what the core shares and asks it through requests
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`): a menu bar, its panels in regions of
//! tabs (`docs/adr/0172-regions-of-tabs.md`), the dev overlay's world panel among them, a status
//! bar, and the window's title; and the system's dialog to open a level, a modal layer waiting over
//! the window meanwhile (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`); the
//! viewport, through which the user flies and picks what to select (`docs/adr/0158-the-viewport.md`,
//! `docs/adr/0159-picking-and-the-selection.md`); and the history panel
//! (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slime_app::{
    AppConfig, AppError, Context, Host, PickAnswer, Presentation, Reflected, ShownView,
};
use slime_color::LinearColor;
use slime_devui::WorldPanel;
use slime_gfx::{Gpu, Image, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_platform::{ColorScheme, DialogId, Event, FileChoice};
use slime_render::glam::{DVec3, Vec2};
use slime_render::{CanvasImage, DebugMode, Pick, RenderError, Renderer};
use slime_world::{Entity, Tick, World};
use uwui::Rect;
use uwui::action::{Action, ActionKind, FieldValue, SurfaceInput};
use uwui::display::{BoxPrimitive, DisplayList, ImageKey};
use uwui::dock::{Dock, Target};
use uwui::look::{self, Colors, Density};
use uwui::shortcuts::Shortcuts;
use uwui::style::{Dimension, FlexDirection, Justify, Sides, Style};
use uwui::text::Fonts;
use uwui::tree::{
    Along, Element, ElementId, FrameStats, ItemMark, Key, Layer, LayoutDirection, Notification,
    NotificationKind, PaletteCommand, Place, Role, RootSettings, Side, Ui,
};
use uwui_render::UiRenderer;

use crate::handles::{Drag, Gizmo, Space, Tool};
use crate::hierarchy::Hierarchy;
use crate::operation::{self, BLENDER_SHORTCUTS, OPERATIONS, SHORTCUTS};
use crate::pane::{Bound, Pane};
use crate::panels::{self, FLOAT, LayoutFile, PANELS, panel_move, title};
use crate::picking::{self, Wanted};
use crate::program::Program;
use crate::session::Session;
use crate::shared::{BuildMark, Notice, Playing, Request, Shared};
use crate::speeds;
use crate::viewport::{self, Framing, Level, Marks, Viewport};

/// An entry of a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// An operation without parameters, by its name.
    Operation(&'static str),
    /// The submenu of the levels opened last, each running `file.open` with its path.
    Recent,
    /// The submenu of the window's panels, each of which shows its panel, opened where it was or
    /// its tab chosen (`view.open_panel`).
    Panels,
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
            Entry::Operation("edit.repeat"),
            Entry::Separator,
            Entry::Operation("edit.create"),
            Entry::Operation("edit.choose_prefab"),
            Entry::Operation("edit.duplicate"),
            Entry::Operation("edit.copy"),
            Entry::Operation("edit.paste"),
            Entry::Operation("edit.delete"),
            Entry::Operation("edit.rename"),
            Entry::Operation("edit.group"),
            Entry::Separator,
            Entry::Operation("edit.select_none"),
            Entry::Separator,
            Entry::Operation("edit.handles_move"),
            Entry::Operation("edit.handles_turn"),
            Entry::Operation("edit.handles_scale"),
            Entry::Operation("edit.handles_local"),
            Entry::Operation("edit.snap"),
            Entry::Separator,
            Entry::Operation("edit.drop"),
            Entry::Operation("edit.drop_bottom"),
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
            Entry::Separator,
            Entry::Operation("view.split"),
            Entry::Separator,
            Entry::Panels,
            Entry::Operation("view.reset_layout"),
            Entry::Separator,
            Entry::Operation("view.keys_default"),
            Entry::Operation("view.keys_blender"),
        ],
    ),
    (
        "Play",
        &[
            Entry::Operation("play.start"),
            Entry::Operation("play.pause"),
            Entry::Operation("play.step"),
            Entry::Operation("play.stop"),
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

/// What the palette's command that places a prefab starts with, its asset ID following.
const PLACE: &str = "edit.place:";

/// The width of the layer that waits for the system's dialog to open a level, in logical pixels.
const CHOOSING_WIDTH: f32 = 460.0;

/// The scope of the table of shortcuts that binds what the buttons of the mouse do over the
/// viewport (`docs/adr/0160-two-viewports-and-blenders-way.md`).
const VIEWPORT_SCOPE: &str = "viewport";

/// What the file of the user's choice of keys holds: Blender's way, or the editor's own, the keys
/// of most editors.
const BLENDER: &str = "blender";
/// The editor's own keys, in the file of the user's choice.
const DEFAULT_KEYS: &str = "default";

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

/// How long the layout stands still, once it settled, before the window writes it: a border moved
/// by a held arrow, or tabs chosen one after another, write it once.
const KEEP_AFTER: Duration = Duration::from_millis(500);

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
    /// Opens the submenu of the window's panels.
    Panels,
    /// Opens the submenu of where the panel of this key moves
    /// (`docs/adr/0172-regions-of-tabs.md`).
    MoveTo(String),
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
    /// The panels laid out in regions of tabs and floating, and the slot that it builds into
    /// (`docs/adr/0172-regions-of-tabs.md`,
    /// `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`).
    dock: Dock,
    dock_slot: Option<ElementId>,
    /// The program's default layout, which *Reset layout* shows; when the layout that settled last
    /// is to be written, once it has stood still for [`KEEP_AFTER`]; and whether the user was told
    /// that the layout cannot be kept, which is told once.
    default_layout: LayoutFile,
    layout_due: Option<Instant>,
    layout_unkept: bool,
    /// The box of the components' panel that the world panel builds into, while it shows.
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
    /// The panes that show the level, one viewport each, two side by side once *View* > *Two
    /// viewports* asks (`view.split`, `docs/adr/0160-two-viewports-and-blenders-way.md`).
    pub(crate) panes: Vec<Pane>,
    /// The pane that the pointer last pressed in, which the View menu changes and F frames.
    pub(crate) active: usize,
    /// The speed at which a viewport flies, Shift's included, while one does, as the status bar
    /// says it.
    flight: Option<f64>,
    /// What the handles do, the axes that they follow, and whether they snap
    /// (`docs/adr/0161-the-open-step-and-the-handles.md`): the user's, never the level's.
    pub(crate) tool: Tool,
    pub(crate) space: Space,
    pub(crate) snap: bool,
    /// Pixels of the screen per logical pixel, as the window last laid out.
    scale: f32,
    /// Whether the keys are Blender's way, laid over the editor's (`view.keys_blender`).
    blender: bool,
    /// How many copies the core made when the window last put one on the system's clipboard.
    copies: u64,
    /// The history panel's box of rows, while it shows, and its rows, each with the count of steps
    /// done that it goes to (`edit.go_to_step`).
    history_slot: Option<ElementId>,
    history: Vec<(ElementId, usize)>,
    /// The hierarchy, and its panel's box, while it shows (`docs/adr/0167-the-hierarchy.md`).
    hierarchy: Hierarchy,
    hierarchy_slot: Option<ElementId>,
    /// The buttons of Play's toolbar, each with the operation that it runs, and its control of the
    /// speeds of game time.
    toolbar: Vec<(ElementId, &'static str)>,
    speeds: Option<ElementId>,
    /// The console's box of lines, while it shows and Play has run.
    console_slot: Option<ElementId>,
    /// The entities that what the user hid and locked cover, made again when the world's structure,
    /// the eyes or the level change, and counted up each time
    /// (`docs/adr/0169-the-eye-and-the-lock.md`).
    eyes: Option<(Tick, u64, u64)>,
    hidden: HashSet<Entity>,
    locked: HashSet<Entity>,
    eyes_made: u64,
}

impl Model {
    /// The window of the core that shares `shared`, in the editor's look.
    pub(crate) fn new(shared: Shared) -> Self {
        // The look ships Latin and Cyrillic: a name in another writing system takes the system's
        // font for it (`docs/adr/0133-font-files-under-the-open-font-license.md`).
        let mut fonts = Fonts::with_system_fonts();
        look::register_fonts(&mut fonts);
        let mut ui = Ui::new(fonts);
        let blender = shared
            .borrow()
            .keys
            .as_deref()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .is_some_and(|chosen| chosen.trim() == BLENDER);
        // The layout kept for the user, or the program's default, and what the user is told. A
        // project's default that cannot be read leaves the layout kept against it alone, and a file
        // that could be neither read nor set aside is not written over.
        let (default_layout, layout) = {
            let mut shared = shared.borrow_mut();
            let (default, told) = panels::default(shared.default_layout);
            if told.is_some() {
                shared.layout = None;
            }
            let kept = match &shared.layout {
                Some(file) => panels::kept(file, &default),
                None => panels::Kept {
                    layout: default.layout.clone(),
                    notice: None,
                    writable: true,
                },
            };
            if !kept.writable {
                shared.layout = None;
            }
            shared.notices.extend(told.into_iter().chain(kept.notice));
            (default, kept.layout)
        };
        let root = ui.add_root("editor", settings([1.0, 1.0], 1.0), column());
        let mut model = Self {
            shared,
            ui,
            root,
            dock: Dock::new(layout),
            dock_slot: None,
            default_layout,
            layout_due: None,
            layout_unkept: false,
            slot: None,
            panel: WorldPanel::alone(),
            bar: Vec::new(),
            items: Vec::new(),
            built: u64::MAX,
            opened: 0,
            selected: None,
            colors: Colors::Dark,
            choosing: None,
            // The level, framed as it opens.
            panes: vec![Pane::default()],
            active: 0,
            flight: None,
            tool: Tool::default(),
            space: Space::default(),
            snap: false,
            scale: 1.0,
            blender,
            copies: 0,
            history_slot: None,
            hierarchy: Hierarchy::default(),
            hierarchy_slot: None,
            history: Vec::new(),
            toolbar: Vec::new(),
            speeds: None,
            console_slot: None,
            eyes: None,
            hidden: HashSet::new(),
            locked: HashSet::new(),
            eyes_made: 0,
        };
        model.set_keys(blender);
        model.style();
        model
    }

    /// Takes the editor's shortcuts, Blender's way laid over them if `blender`.
    fn set_keys(&mut self, blender: bool) {
        self.blender = blender;
        let mut tables = vec![Shortcuts::from_text("editor.keys", SHORTCUTS)];
        if blender {
            tables.push(Shortcuts::from_text(
                "editor-blender.keys",
                BLENDER_SHORTCUTS,
            ));
        }
        self.ui
            .set_shortcuts(&tables)
            .expect("the editor's shortcuts are valid");
    }

    /// Chooses the keys, Blender's way or not, and keeps the choice in the user's state directory,
    /// telling the user when it cannot.
    fn choose_keys(&mut self, blender: bool) {
        self.set_keys(blender);
        let file = self.shared.borrow().keys.clone();
        let Some(file) = file else {
            return;
        };
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&file, if blender { BLENDER } else { DEFAULT_KEYS }));
        if let Err(error) = written {
            self.shared.borrow_mut().notices.push(Notice {
                failure: true,
                text: format!(
                    "the choice of keys is not kept for the next start: {}: {error}",
                    file.display()
                ),
            });
        }
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
        let selected = self.selected_entities(world);
        let modifiers = self.ui.held_modifiers();
        self.dock.act(&mut actions);
        let mut requests =
            self.hierarchy
                .act(&mut actions, world, &selected, modifiers, Instant::now());
        for command in self.panel.act(&mut actions) {
            let label = format!("{} {}", command.kind(), command.entity());
            requests.push(Request::Apply {
                label,
                commands: vec![command],
                selection: None,
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
                let pane = self
                    .panes
                    .iter()
                    .position(|pane| pane.surface.is_some() && pane.surface == action.element);
                if let Some(pane) = pane {
                    if matches!(input, SurfaceInput::Pressed { .. }) {
                        self.active = pane;
                    }
                    // A handle dragged, or pressed, takes the pointer from the rest.
                    if let Some(request) = self.handle(pane, input, world) {
                        requests.extend(request);
                        continue;
                    }
                    let ui = &self.ui;
                    let bound = |button, modifiers| {
                        ui.button_command(VIEWPORT_SCOPE, button, modifiers)
                            .and_then(Bound::named)
                    };
                    self.panes[pane].input(input, bound);
                }
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

    /// Selects what `found` holds, the entities that `wanted` found, the window having asked the
    /// viewport (`picking::select`), as the core's selection.
    pub(crate) fn picked(&mut self, world: &World, wanted: &Wanted, found: &[Entity]) {
        let found = self.pickable(found);
        let selection = picking::select(world, wanted, &found, &self.selected_entities(world));
        let selection = selection
            .into_iter()
            .filter_map(|entity| world.persistent_id(entity))
            .collect();
        self.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(selection));
    }

    /// What of `found` may be picked: none that the user hid or locked
    /// (`docs/adr/0169-the-eye-and-the-lock.md`).
    pub(crate) fn pickable(&self, found: &[Entity]) -> Vec<Entity> {
        found
            .iter()
            .copied()
            .filter(|entity| !self.locked.contains(entity) && !self.hidden.contains(entity))
            .collect()
    }

    /// Makes again the entities that what the user hid and locked cover, when the world's
    /// structure, the eyes or the level changed, and gives the hierarchy what it shows of them.
    fn follow_the_eyes(&mut self, world: &World) {
        let shared = self.shared.borrow();
        let key = Some((world.structure(), shared.eyes.revision(), shared.opened));
        if self.eyes == key {
            return;
        }
        self.eyes = key;
        self.hidden = shared.eyes.hidden_entities(world);
        self.locked = shared.eyes.locked_entities(world);
        self.eyes_made += 1;
        self.hierarchy
            .set_eyes(shared.eyes.clone(), self.hidden.clone(), self.eyes_made);
    }

    /// The marquee being drawn over a viewport, in pixels of the screen, and the viewport's box.
    fn marquee(&self, scale: f32) -> Option<(Rect, Rect)> {
        let (pane, (from, to)) = self
            .panes
            .iter()
            .enumerate()
            .find_map(|(index, pane)| Some((index, pane.marquee()?)))?;
        let surface = self.viewport_rect(pane)?;
        let left = from[0].min(to[0]) * scale;
        let top = from[1].min(to[1]) * scale;
        let marquee = Rect::new(
            surface.x + left,
            surface.y + top,
            (from[0] - to[0]).abs() * scale,
            (from[1] - to[1]).abs() * scale,
        );
        Some((marquee, surface))
    }

    /// The box of the surface of the viewport of `pane`, in pixels of the screen, once laid out.
    pub(crate) fn viewport_rect(&self, pane: usize) -> Option<uwui::Rect> {
        self.ui.rect(self.panes.get(pane)?.surface?)
    }

    /// The size of the image of the viewport of `pane`, in pixels of the screen: its surface's, at
    /// least 1 by 1.
    pub(crate) fn viewport_size(&self, pane: usize) -> Option<(u32, u32)> {
        let rect = self.viewport_rect(pane)?;
        let side = |length: f32| (length.round() as u32).max(1);
        Some((side(rect.width), side(rect.height)))
    }

    /// What the viewport of `pane` marks of `world`, at `scale` pixels of the screen per logical
    /// pixel: what the core says is selected, its wireframe if asked for, and its handles.
    pub(crate) fn marks(&self, pane: usize, world: &World, scale: f32) -> Marks {
        let dragged = self
            .panes
            .get(pane)
            .and_then(|pane| pane.drag.as_ref())
            .map(|(drag, _)| drag.part());
        Marks {
            selection: self.selected_entities(world),
            eyes: self.eyes_made,
            wireframe: self.panes.get(pane).is_some_and(|pane| pane.wireframe),
            scale,
            handles: self
                .gizmo(pane, world, scale)
                .map(|gizmo| (self.tool, gizmo, dragged)),
        }
    }

    /// Where the handles of what is selected stand in the viewport of `pane`, at `scale` pixels of
    /// the screen per logical pixel; none without a selection.
    fn gizmo(&self, pane: usize, world: &World, scale: f32) -> Option<Gizmo> {
        let rect = self.viewport_rect(pane)?;
        let camera = self.panes[pane].navigation.camera();
        let selection = self.selected_entities(world);
        Gizmo::of(world, &selection, self.space, &camera, rect.height / scale)
    }

    /// The ray from the camera of `pane` through `at`, in logical pixels from its viewport's
    /// corner.
    fn ray(&self, pane: usize, at: [f32; 2]) -> Option<(DVec3, DVec3)> {
        let rect = self.viewport_rect(pane)?;
        let (width, height) = (rect.width / self.scale, rect.height / self.scale);
        let point = Vec2::new(at[0] / width.max(1.0), at[1] / height.max(1.0));
        Some(
            self.panes[pane]
                .navigation
                .camera()
                .ray(width / height.max(1.0), point),
        )
    }

    /// Drags the handles of what is selected over the viewport of `pane` as `input` asks: a press
    /// of a button that selects over a handle starts a drag, the pointer's moves give the core
    /// the drag's commands, which it applies live, and the release ends it as one step
    /// (`docs/adr/0161-the-open-step-and-the-handles.md`). Gives the requests made, `None` for an
    /// input that is not the handles'.
    fn handle(&mut self, pane: usize, input: SurfaceInput, world: &World) -> Option<Vec<Request>> {
        match input {
            SurfaceInput::Pressed {
                button,
                modifiers,
                at,
            } if self.panes[pane].drag.is_none() => {
                let selects = self
                    .ui
                    .button_command(VIEWPORT_SCOPE, button, modifiers)
                    .and_then(Bound::named)
                    == Some(Bound::Select);
                if !selects || self.panes[pane].navigation.holds_cursor() {
                    return None;
                }
                let gizmo = self.gizmo(pane, world, self.scale)?;
                let ray = self.ray(pane, at)?;
                let part = gizmo.hit(self.tool, ray)?;
                let selection = self.selected_entities(world);
                let drag = Drag::start(world, &selection, (self.tool, part, gizmo), ray)?;
                self.panes[pane].drag = Some((drag, button));
                // The gesture opened at once, empty, so that nothing changes what the drag
                // started from before its first move: an agent's step waits from now on.
                Some(vec![Request::Gesture(Vec::new())])
            }
            SurfaceInput::Moved { at } => {
                let ray = self.ray(pane, at)?;
                let (drag, _) = self.panes[pane].drag.as_ref()?;
                // Where the ray reaches nothing that the part moves along, as along its own axis,
                // what is dragged stays where it was last taken.
                let commands = drag.commands(ray, self.snap);
                if commands.is_empty() {
                    return Some(Vec::new());
                }
                Some(vec![Request::Gesture(commands)])
            }
            SurfaceInput::Released { button, .. } => {
                let pane = &mut self.panes[pane];
                let (_, held) = pane.drag.as_ref()?;
                if *held != button {
                    return Some(Vec::new());
                }
                let (drag, _) = pane.drag.take()?;
                Some(vec![Request::EndGesture(drag.label().to_owned())])
            }
            // The wheel and other buttons, while a handle is dragged, do nothing.
            _ if self.panes[pane].drag.is_some() => Some(Vec::new()),
            _ => None,
        }
    }

    /// Cancels the drag of a handle under way, as Escape does: what it applied is undone. Gives
    /// whether one was under way.
    pub(crate) fn cancel_drag(&mut self) -> bool {
        let dragged = self.panes.iter_mut().any(|pane| pane.drag.take().is_some());
        if dragged {
            self.shared
                .borrow_mut()
                .requests
                .push_back(Request::CancelGesture);
        }
        dragged
    }

    /// The pane whose gesture holds the cursor, if one does: the raw motion turns its camera, and
    /// the keys that fly reach it.
    pub(crate) fn navigated(&mut self) -> Option<&mut Pane> {
        self.panes
            .iter_mut()
            .find(|pane| pane.navigation.holds_cursor())
    }

    /// The entities that the core says are selected, which `world` holds.
    pub(crate) fn selected_entities(&self, world: &World) -> Vec<Entity> {
        model_selection(&self.shared, world)
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
            // A prefab chosen in the palette, by its asset ID.
            (ActionKind::Command(name), _) if name.starts_with(PLACE) => Some(Request::Run {
                name: "edit.place".to_owned(),
                arguments: vec![name[PLACE.len()..].to_owned()],
            }),
            (ActionKind::Command(name), _) => Some(Request::Run {
                name: name.clone(),
                arguments: Vec::new(),
            }),
            (ActionKind::Press, Some(id))
                if let Some((_, name)) = self.toolbar.iter().find(|(button, _)| *button == id) =>
            {
                Some(Request::Run {
                    name: (*name).to_owned(),
                    arguments: Vec::new(),
                })
            }
            (ActionKind::Committed(FieldValue::Chosen(place)), Some(id))
                if Some(id) == self.speeds =>
            {
                Some(Request::Run {
                    name: "play.speed".to_owned(),
                    arguments: vec![
                        SPEEDS
                            .get(*place)
                            .map_or(1.0, |&(_, value)| value)
                            .to_string(),
                    ],
                })
            }
            (ActionKind::Press, Some(id))
                if let Some(&(_, steps)) = self.history.iter().find(|&&(row, _)| row == id) =>
            {
                Some(Request::Run {
                    name: "edit.go_to_step".to_owned(),
                    arguments: vec![steps.to_string()],
                })
            }
            // A panel's context menu, the second button or Shift+F10 on its tab or on its floating
            // panel's bar: where to move it, and closing it.
            (ActionKind::ContextMenu { at }, Some(id))
                if let Some(panel) = self.dock.panel_of(id) =>
            {
                let panel = panel.to_owned();
                let place = Place::At {
                    over: id,
                    point: *at,
                };
                self.open_menu(place, Menu::Panel(panel));
                None
            }
            (ActionKind::Press, Some(id)) => {
                if let Some(&(_, menu)) = self.bar.iter().find(|&&(item, _)| item == id) {
                    self.open_menu(beside(id, Side::Below), Menu::Bar(menu));
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
                        self.open_menu(beside(id, Side::End), Menu::Recent);
                        None
                    }
                    Item::Panels => {
                        self.open_menu(beside(id, Side::End), Menu::Panels);
                        None
                    }
                    Item::MoveTo(panel) => {
                        self.open_menu(beside(id, Side::End), Menu::MoveTo(panel));
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

    /// Opens `menu` where `place` says, in a layer that closes as the user leaves it.
    fn open_menu(&mut self, place: Place, menu: Menu) {
        let entries: Vec<(String, String, ItemMark, Option<Item>)> = match &menu {
            Menu::Bar(menu) => MENUS[*menu]
                .1
                .iter()
                .map(|entry| match *entry {
                    Entry::Operation(name) => {
                        let label =
                            operation::operation(name).map_or(name, |operation| operation.label);
                        let chord = self.ui.command_chord(name).unwrap_or_default();
                        // The modes of the active viewport and of the window, ticked where they are
                        // on.
                        let pane = &self.panes[self.active];
                        let mark = match (name, shading(name)) {
                            ("view.wireframe", _) => ItemMark::Check(pane.wireframe),
                            ("view.split", _) => ItemMark::Check(self.panes.len() > 1),
                            ("view.keys_default", _) => ItemMark::Check(!self.blender),
                            ("edit.handles_move", _) => ItemMark::Check(self.tool == Tool::Move),
                            ("edit.handles_turn", _) => ItemMark::Check(self.tool == Tool::Turn),
                            ("edit.handles_scale", _) => ItemMark::Check(self.tool == Tool::Scale),
                            ("edit.handles_local", _) => {
                                ItemMark::Check(self.space == Space::Selection)
                            }
                            ("edit.snap", _) => ItemMark::Check(self.snap),
                            ("view.keys_blender", _) => ItemMark::Check(self.blender),
                            (_, Some(debug)) => ItemMark::Check(pane.settings.debug == debug),
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
                    Entry::Panels => (
                        "Panels".to_owned(),
                        String::new(),
                        ItemMark::Submenu,
                        Some(Item::Panels),
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
            // Each shown, opened where it was or its tab chosen, as an editor's menu of its windows
            // shows one.
            Menu::Panels => PANELS
                .iter()
                .map(|&(panel, title)| {
                    (
                        title.to_owned(),
                        String::new(),
                        ItemMark::None,
                        Some(Item::Run("view.open_panel", vec![panel.to_owned()])),
                    )
                })
                .collect(),
            Menu::Panel(panel) => vec![
                (
                    "Move to".to_owned(),
                    String::new(),
                    ItemMark::Submenu,
                    Some(Item::MoveTo(panel.clone())),
                ),
                (
                    "Close".to_owned(),
                    String::new(),
                    ItemMark::None,
                    Some(Item::Run("view.close_panel", vec![panel.clone()])),
                ),
            ],
            Menu::MoveTo(panel) => {
                let move_to = |to: &str| {
                    Some(Item::Run(
                        "view.move_panel",
                        vec![panel.clone(), to.to_owned()],
                    ))
                };
                let layout = self.dock.layout();
                // The other stacks that show, each by the titles of its panels open.
                let mut entries: Vec<_> = layout
                    .stacks()
                    .into_iter()
                    .filter(|stack| !stack.iter().any(|each| each == panel))
                    .filter_map(|stack| {
                        let open: Vec<&String> =
                            stack.iter().filter(|each| layout.is_open(each)).collect();
                        let titles: Vec<String> = open.iter().map(|each| title(each)).collect();
                        Some((
                            titles.join(", "),
                            String::new(),
                            ItemMark::None,
                            move_to(open.first()?),
                        ))
                    })
                    .collect();
                if !entries.is_empty() {
                    entries.push((String::new(), String::new(), ItemMark::None, None));
                }
                for (edge, label) in [
                    ("left", "The window's left edge"),
                    ("right", "The window's right edge"),
                    ("top", "The window's top"),
                    ("bottom", "The window's bottom"),
                ] {
                    entries.push((
                        label.to_owned(),
                        String::new(),
                        ItemMark::None,
                        move_to(edge),
                    ));
                }
                if !layout.floats(panel) {
                    entries.push((String::new(), String::new(), ItemMark::None, None));
                    entries.push((
                        "A floating panel".to_owned(),
                        String::new(),
                        ItemMark::None,
                        move_to(FLOAT),
                    ));
                }
                entries
            }
        };
        let layer = Layer {
            place,
            modal: false,
            dismiss: true,
        };
        let key = match menu {
            Menu::Bar(_) | Menu::Panel(_) => "menu",
            Menu::Recent => "recent",
            Menu::Panels => "panels",
            Menu::MoveTo(_) => "move to",
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
    fn run_window_operations(&mut self, size: [f32; 2], world: &World) {
        let operations = std::mem::take(&mut self.shared.borrow_mut().window_operations);
        for (name, arguments) in operations {
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
                "edit.choose_prefab" => {
                    let prefabs: Vec<PaletteCommand> = self
                        .shared
                        .borrow()
                        .prefabs
                        .iter()
                        .map(|(id, name)| PaletteCommand {
                            name: name.clone(),
                            command: format!("{PLACE}{id}"),
                        })
                        .collect();
                    if prefabs.is_empty() {
                        let mut shared = self.shared.borrow_mut();
                        shared.notices.push(Notice {
                            failure: false,
                            text: "The cooked data holds no prefab that the level can place."
                                .to_owned(),
                        });
                        shared.revision += 1;
                    } else {
                        self.ui.open_palette("Find a prefab", prefabs);
                    }
                }
                "view.shortcuts" => {
                    // What the buttons do over a viewport too, by their names.
                    let mut commands = commands;
                    commands.extend(Bound::NAMES.iter().map(|&(command, name)| PaletteCommand {
                        name: name.to_owned(),
                        command: command.to_owned(),
                    }));
                    self.ui.open_shortcut_sheet("Shortcuts", &commands);
                }
                "file.choose" => self.choose_level(size),
                "view.frame_selection" => self.panes[self.active].frame_wanted = true,
                "view.wireframe" => {
                    let pane = &mut self.panes[self.active];
                    pane.wireframe = !pane.wireframe;
                }
                "view.split" => self.split(),
                "view.move_panel" => {
                    // The core checked the names: it is refused only into its own stack, into a
                    // floating panel, or afloat again.
                    if let [panel, to] = arguments.as_slice()
                        && let Ok(target) = panel_move(panel, to)
                        && !self.dock.move_panel(panel, &target)
                    {
                        let text = if target == Target::Float {
                            format!("`{panel}` floats already.")
                        } else if self.dock.layout().floats(to) {
                            format!("`{to}` floats alone: a panel moves into a stack of tabs.")
                        } else {
                            format!("`{panel}` lies with `{to}` already.")
                        };
                        let mut shared = self.shared.borrow_mut();
                        shared.notices.push(Notice {
                            failure: true,
                            text,
                        });
                        shared.revision += 1;
                    }
                }
                // Closed where it is, to come back there; nothing for a panel closed already.
                "view.close_panel" => {
                    if let [panel] = arguments.as_slice() {
                        self.dock.close(panel);
                    }
                }
                "view.open_panel" => {
                    if let [panel] = arguments.as_slice() {
                        self.dock.show(panel);
                    }
                }
                "view.reset_layout" => self.dock.set_layout(self.default_layout.layout.clone()),
                "edit.rename" | "hierarchy.previous" | "hierarchy.next" | "hierarchy.fold"
                | "hierarchy.unfold" => {
                    // The hierarchy shows, its tab chosen, where the rename or the arrows act.
                    self.dock.show("hierarchy");
                    let selected = self.selected_entities(world);
                    if let Some(requests) = self.hierarchy.run(name, world, &selected) {
                        self.shared.borrow_mut().requests.extend(requests);
                    }
                }
                "edit.handles_move" => self.tool = Tool::Move,
                "edit.handles_turn" => self.tool = Tool::Turn,
                "edit.handles_scale" => self.tool = Tool::Scale,
                "edit.handles_local" => {
                    self.space = match self.space {
                        Space::World => Space::Selection,
                        Space::Selection => Space::World,
                    };
                }
                "edit.snap" => self.snap = !self.snap,
                // What the system's clipboard holds, or, without one, what the editor copied.
                "edit.paste" => {
                    // A copy that the core made since the last frame first.
                    self.put_copy();
                    let text = self
                        .ui
                        .clipboard_text()
                        .or_else(|| self.shared.borrow().clipboard.clone())
                        .unwrap_or_default();
                    let label =
                        operation::operation("edit.paste").map_or("Paste", |paste| paste.label);
                    self.shared.borrow_mut().requests.push_back(Request::Paste {
                        label: label.to_owned(),
                        text,
                    });
                }
                "view.keys_default" => self.choose_keys(false),
                "view.keys_blender" => self.choose_keys(true),
                name => match shading(name) {
                    Some(debug) => self.panes[self.active].settings.debug = debug,
                    None => tracing::warn!(name, "the window has no operation of that name"),
                },
            }
        }
    }

    /// Lays the window out at `size`, in logical pixels, and `scale` pixels of the screen per
    /// logical pixel, from what the core shares and from `world`, and gives what to draw.
    pub(crate) fn frame(
        &mut self,
        world: &World,
        size: [f32; 2],
        scale: f32,
    ) -> (DisplayList, FrameStats) {
        self.scale = scale;
        self.ui.set_root(self.root, settings(size, scale));
        // The layer that waits stays in the middle of a window whose size changes.
        if let Some(layer) = self.choosing.as_ref().map(|choosing| choosing.layer) {
            let place = self.choosing_place(size);
            self.ui.set_layer_place(layer, place);
        }
        self.run_window_operations(size, world);
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
        self.follow_flight();
        let revision = self.shared.borrow().revision;
        if revision != self.built {
            self.built = revision;
            self.follow_the_core(world);
        }
        self.build();
        let now = Instant::now();
        if self.dock.take_changed() {
            self.layout_due = Some(now + KEEP_AFTER);
        }
        self.keep_layout_by(now);
        self.follow_the_eyes(world);
        if let Some(slot) = self.slot {
            self.panel.build(&mut self.ui, slot, world);
        }
        if let Some(slot) = self.hierarchy_slot {
            let selected = self.selected_entities(world);
            self.hierarchy
                .build(&mut self.ui, slot, world, &selected, Instant::now());
        }
        let (mut display, stats) = self.ui.frame();
        // Over the viewport, under nothing: the pointer is the surface's while it is drawn.
        if let Some((rect, viewport)) = self.marquee(scale) {
            display.push(
                BoxPrimitive {
                    rect,
                    radii: [0.0; 4],
                    fill: MARQUEE_FILL,
                    border_width: scale,
                    border: MARQUEE_BORDER,
                },
                Some(viewport),
            );
        }
        (display, stats)
    }

    /// Writes the layout that settled, if it has stood still for [`KEEP_AFTER`] by `now`.
    pub(crate) fn keep_layout_by(&mut self, now: Instant) {
        if self.layout_due.is_some_and(|due| due <= now) {
            self.layout_due = None;
            self.keep_layout();
        }
    }

    /// When the layout that settled is to be written, for the loop to wake then.
    pub(crate) fn layout_waits(&self) -> Option<Instant> {
        self.layout_due
    }

    /// Writes the dock's layout to the file of the user's state directory that keeps it, if the
    /// program has one, telling the user once when it cannot.
    fn keep_layout(&mut self) {
        let Some(file) = self.shared.borrow().layout.clone() else {
            return;
        };
        let kept = panels::keep(&file, &self.default_layout.default, self.dock.layout());
        if let Err(error) = kept {
            tracing::warn!(%error, "the layout is not kept");
            if !std::mem::replace(&mut self.layout_unkept, true) {
                self.ui.notify(Notification {
                    kind: NotificationKind::Error,
                    text: format!("The layout is not kept for the next start: {error}"),
                    progress: None,
                    buttons: Vec::new(),
                });
            }
        }
    }

    /// Shows two viewports side by side, the second seeing as the first does, or one again, the
    /// first (`view.split`); the window is built again.
    fn split(&mut self) {
        if self.panes.len() > 1 {
            // A drag in the viewport that goes is cancelled with it.
            if self.panes[1..].iter().any(|pane| pane.drag.is_some()) {
                self.cancel_drag();
            }
            self.panes.truncate(1);
            self.active = 0;
        } else {
            let beside = self.panes[0].beside();
            self.panes.push(beside);
        }
        self.shared.borrow_mut().revision += 1;
    }

    /// Puts what the core copied last on the system's clipboard, once.
    fn put_copy(&mut self) {
        let shared = self.shared.borrow();
        if shared.copies != self.copies {
            self.copies = shared.copies;
            if let Some(text) = &shared.clipboard {
                self.ui.set_clipboard_text(text);
            }
        }
    }

    /// Follows what the core changed: a level opened in place of the other resets the world panel
    /// and is framed in the viewport; a selection that the core changed, an undo or an agent's,
    /// selects in the panel.
    fn follow_the_core(&mut self, world: &World) {
        self.put_copy();
        let shared = self.shared.borrow();
        if shared.opened != self.opened {
            self.opened = shared.opened;
            self.panel = WorldPanel::alone();
            self.hierarchy.opened();
            self.selected = None;
            // Another level, framed as it opens, in every viewport; a drag of the other level's
            // entities ends with it, whose core held its gesture.
            for pane in &mut self.panes {
                pane.frame_wanted = true;
                pane.speed_wanted = true;
                pane.drag = None;
            }
        }
        let selected = shared
            .selection
            .first()
            .and_then(|identity| world.find(identity));
        if selected != self.selected {
            match selected {
                Some(entity) => {
                    self.panel.select(entity);
                    // A selection made elsewhere shows in the hierarchy.
                    self.hierarchy.reveal(entity);
                }
                None => self.panel.deselect(),
            }
            self.selected = selected;
        }
    }

    /// Builds the bar of menus, the regions and the status bar again, when what they show changed,
    /// and the dock's panels that show.
    fn build(&mut self) {
        let status = self.status();
        let mut bar = Vec::new();
        let mut dock_slot = None;
        let panes = self.panes.len();
        let (playing, mark, played) = {
            let shared = self.shared.borrow();
            (shared.play, shared.build, shared.played)
        };
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
                // The panels in regions of tabs, which the dock builds
                // (`docs/adr/0172-regions-of-tabs.md`).
                let mut regions = Style::default();
                regions.layout.grow = 1.0;
                regions.layout.min_size[1] = Dimension::Pixels(0.0);
                regions.layout.padding = Sides::all(8.0);
                dock_slot = Some(children.add_slot("dock", Element::container(regions)));
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
        if dock_slot.is_some() {
            self.dock_slot = dock_slot;
        }
        if let Some(slot) = self.dock_slot {
            self.dock.build(&mut self.ui, slot, &title);
        }
        // Each panel that shows, in the box that its slot of the dock holds.
        let content = |gap: f32| {
            let mut content = column();
            content.layout.grow = 1.0;
            content.layout.min_size[1] = Dimension::Pixels(0.0);
            content.layout.gap = [gap, 0.0];
            content
        };
        self.hierarchy_slot = self.panel_box("hierarchy", || Element::container(content(6.0)));
        self.slot = self.panel_box("components", || Element::scroll(content(0.0)));
        self.history_slot = self.panel_box("history", || Element::scroll(content(0.0)));
        self.build_viewport_panel(panes, playing, mark);
        self.console_slot = None;
        if let Some(slot) = self.dock.slot("console") {
            self.ui.build(slot, u64::from(played), |children| {
                if played {
                    children.add_slot("lines", Element::scroll(content(0.0)));
                } else {
                    let waiting =
                        Element::label("Play has not run yet.", Style::default()).class("muted");
                    children.add("waiting", waiting, |_| {});
                }
            });
            if played {
                self.console_slot = self.ui.children(slot).first().copied();
            }
        }
        self.build_history();
        self.build_console();
    }

    /// The box that the panel `panel` holds in its slot of the dock, made by `element` once for
    /// each slot; none while the panel does not show, its tab not chosen.
    fn panel_box(&mut self, panel: &str, element: impl FnOnce() -> Element) -> Option<ElementId> {
        let slot = self.dock.slot(panel)?;
        self.ui.build(slot, 0, |children| {
            children.add_slot("content", element());
        });
        self.ui.children(slot).first().copied()
    }

    /// Builds the viewport's panel, while it shows, again when the window's revision changed or
    /// its slot is new: Play's toolbar, as `playing` and `mark` say, and the viewports of `panes`
    /// side by side, whose surfaces the panes are given.
    fn build_viewport_panel(&mut self, panes: usize, playing: Playing, mark: BuildMark) {
        let Some(slot) = self.dock.slot("viewport") else {
            self.set_surfaces(&[]);
            return;
        };
        let mut toolbar = None;
        let mut surfaces = Vec::new();
        self.ui.build(slot, self.built, |center| {
            toolbar = Some(play_toolbar(center, playing, mark));
            let mut viewports = Style::default();
            viewports.layout.grow = 1.0;
            viewports.layout.min_size = [Dimension::Pixels(0.0), Dimension::Pixels(0.0)];
            viewports.layout.gap = [8.0, 8.0];
            let row = Element::container(viewports);
            center.add("viewports", row, |viewports| {
                for pane in 0..panes {
                    let mut surface = Style::default();
                    surface.layout.grow = 1.0;
                    surface.layout.min_size[0] = Dimension::Pixels(0.0);
                    let name = viewport::name(pane);
                    let surface =
                        Element::surface(viewport::image_key(pane), surface).named(name.as_str());
                    surfaces.push(viewports.add(name.as_str(), surface, |_| {}));
                }
            });
        });
        if let Some(toolbar) = toolbar {
            (self.toolbar, self.speeds) = toolbar;
            self.set_surfaces(&surfaces);
        }
    }

    /// Gives the panes `surfaces`, in their order, none to those past them: a pane whose surface
    /// goes, behind its tab, or is made anew, its panel moved, ends what its buttons held, and the
    /// handle that it dragged is cancelled, since their release would reach no surface.
    fn set_surfaces(&mut self, surfaces: &[ElementId]) {
        let mut dragged = false;
        for (at, pane) in self.panes.iter_mut().enumerate() {
            let surface = surfaces.get(at).copied();
            if pane.surface.is_some() && pane.surface != surface {
                pane.stop();
                dragged |= pane.drag.is_some();
            }
            pane.surface = surface;
        }
        if dragged {
            self.cancel_drag();
        }
    }

    /// Builds the history panel's rows again when the history changed: a row before the first
    /// step kept, then a row for each step done, the oldest first, and for each step undone, the
    /// next to do again first, those dimmed, each going to the count of steps done that it shows;
    /// the row of the steps done marked (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    fn build_history(&mut self) {
        let Some(slot) = self.history_slot else {
            return;
        };
        let shared = self.shared.borrow();
        let mut rows = None;
        self.ui.build(slot, shared.history, |steps| {
            let labels = std::iter::once(("Before the first step kept", false))
                .chain(shared.done.iter().map(|label| (label.as_str(), false)))
                .chain(shared.undone.iter().map(|label| (label.as_str(), true)));
            let mut built = Vec::new();
            for (count, (label, undone)) in labels.enumerate() {
                let mut row = Element::button(label, Style::default()).class("row");
                if undone {
                    row = row.class("undone");
                }
                built.push((steps.add(count, row, |_| {}), count));
            }
            rows = Some(built);
        });
        let Some(rows) = rows else {
            return;
        };
        let done = shared.done.len();
        drop(shared);
        for &(row, steps) in &rows {
            if let Some(mut state) = self.ui.state(row)
                && state.selected != (steps == done)
            {
                state.selected = steps == done;
                self.ui.set_state(row, state);
            }
        }
        self.history = rows;
    }

    /// Builds the console's lines again when lines came: the last lines of the game's log, the
    /// oldest first, in the editor's monospaced text, scrolled to the last
    /// (`docs/adr/0164-play.md`).
    fn build_console(&mut self) {
        let Some(slot) = self.console_slot else {
            return;
        };
        let shared = self.shared.borrow();
        let mut built = false;
        self.ui.build(slot, shared.console_revision, |lines| {
            built = true;
            for (number, line) in shared.console.iter().enumerate() {
                let label = Element::label(line.as_str(), Style::default()).class("fixed");
                lines.add(number, label, |_| {});
            }
        });
        // The last line in sight, as lines come.
        if built {
            drop(shared);
            self.ui.scroll_to(slot, [0.0, f32::MAX]);
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
        let mut status = vec![
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
        ];
        if let Some(speed) = self.flight {
            status.push(("flying", flying(speed)));
        }
        status
    }

    /// Frames, in pane `pane`, a sphere around `center` of `radius` metres, the viewport `aspect`
    /// times as wide as it is tall; a level just opened sets the pane's flying speed too, the speed
    /// last used in it, which the user's state keeps, or one that suits its size
    /// (`navigation::level_speed`).
    pub(crate) fn framed(&mut self, pane: usize, center: DVec3, radius: f64, aspect: f64) {
        let kept = if self.panes[pane].speed_wanted {
            let shared = self.shared.borrow();
            let speeds = shared
                .speeds
                .as_deref()
                .and_then(|file| speeds::read(file).ok());
            shared
                .level
                .as_deref()
                .and_then(|level| speeds::of(&speeds?, level))
        } else {
            None
        };
        let pane = &mut self.panes[pane];
        pane.navigation.frame(center, radius, aspect);
        if std::mem::take(&mut pane.speed_wanted) {
            let speed = kept.unwrap_or_else(|| crate::navigation::level_speed(radius));
            pane.navigation.set_speed(speed);
        }
    }

    /// Follows the flight of the viewports: the status bar says the speed while one flies, built
    /// again as it changes, and the speed is kept for the level, in the user's state, once the
    /// flight ends.
    fn follow_flight(&mut self) {
        let flight = self
            .panes
            .iter()
            .find_map(|pane| pane.navigation.flying_speed());
        if flight == self.flight {
            return;
        }
        let ended = flight.is_none();
        self.flight = flight;
        self.shared.borrow_mut().revision += 1;
        let flown = self
            .panes
            .get(self.active)
            .map(|pane| pane.navigation.speed());
        let (file, level) = {
            let shared = self.shared.borrow();
            (shared.speeds.clone(), shared.level.clone())
        };
        let (true, Some(speed), Some(file), Some(level)) = (ended, flown, file, level) else {
            return;
        };
        let kept = speeds::read(&file)
            .and_then(|speeds| speeds::write(&file, &speeds::flown(&speeds, &level, speed)));
        if let Err(error) = kept {
            self.shared.borrow_mut().notices.push(Notice {
                failure: true,
                text: format!(
                    "the flying speed is not kept for the next start: {}: {error}",
                    file.display()
                ),
            });
        }
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

/// Adds to `center` Play's toolbar as Play stands, `playing`: Play, Pause or Resume, Step, Stop,
/// and the speeds of game time, one of four segments, the one in use chosen, 1× while no game runs
/// (`docs/adr/0164-play.md`); then the mark of the game's build, `mark`, while cargo builds it or
/// once it failed (`docs/adr/0166-building-the-game-again.md`). Gives its buttons, each with the
/// operation that it runs, and its control of speeds.
fn play_toolbar(
    center: &mut uwui::tree::Children<'_>,
    playing: Playing,
    mark: BuildMark,
) -> (Vec<(ElementId, &'static str)>, Option<ElementId>) {
    let mut buttons = Vec::new();
    let mut speeds = None;
    let mut bar = Style::default();
    bar.layout.gap = [0.0, 6.0];
    center.add("play", Element::container(bar).named("play"), |bar| {
        let (paused, speed) = match playing {
            Playing::Running { paused, speed } => (paused, speed),
            Playing::Starting | Playing::Stopped => (false, 1.0),
        };
        let pause = if paused { "Resume" } else { "Pause" };
        for (key, label, name) in [
            ("start", "Play", "play.start"),
            ("pause", pause, "play.pause"),
            ("step", "Step", "play.step"),
            ("stop", "Stop", "play.stop"),
        ] {
            let button = bar.add(key, Element::button(label, Style::default()), |_| {});
            buttons.push((button, name));
        }
        let chosen = SPEEDS
            .iter()
            .position(|&(_, value)| value == speed)
            .unwrap_or(2);
        let segments = Element::segments(chosen, Style::default()).named("speed of game time");
        speeds = Some(bar.add("speeds", segments, |segments| {
            for (label, _) in SPEEDS {
                segments.add(label, Element::segment(label, Style::default()), |_| {});
            }
        }));
        let said = match mark {
            BuildMark::None => None,
            BuildMark::Building => Some("Building the game…"),
            BuildMark::Failed => Some("The game's code does not build"),
            BuildMark::BuiltAgain => Some("Built again"),
        };
        if let Some(said) = said {
            bar.add("build", Element::label(said, Style::default()), |_| {});
        }
    });
    (buttons, speeds)
}

/// The speeds of game time that Play's toolbar offers, each with its label.
const SPEEDS: [(&str, f64); 4] = [("¼×", 0.25), ("½×", 0.5), ("1×", 1.0), ("2×", 2.0)];

/// Which menu [`Model::open_menu`] opens.
#[derive(Clone)]
enum Menu {
    /// The menu of the bar at this place of [`MENUS`].
    Bar(usize),
    /// The levels opened last.
    Recent,
    /// The context menu of the panel of this key, from its tab or its floating panel's bar.
    Panel(String),
    /// The window's panels, each of which shows its panel.
    Panels,
    /// Where the panel of this key moves: into the stack of another panel, or to an edge of the
    /// window (`view.move_panel`).
    MoveTo(String),
}

impl Drop for Model {
    /// Writes the layout that settled last, if it waits to be: the window closes, or quits, within
    /// [`KEEP_AFTER`] of a change.
    fn drop(&mut self) {
        if self.layout_due.take().is_some() {
            self.keep_layout();
        }
    }
}

/// The place of a menu beside `element`, on its `side`, along its start.
fn beside(element: ElementId, side: Side) -> Place {
    Place::Beside {
        element,
        side,
        along: Along::Start,
    }
}

/// The speed at which a viewport flies, in metres per second, as the status bar says it.
fn flying(speed: f64) -> String {
    if speed < 10.0 {
        format!("flying at {speed:.1} m/s")
    } else {
        format!("flying at {speed:.0} m/s")
    }
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
/// level is open and `operations` run, each an operation's name followed by its arguments, parted
/// by spaces, `view.move_panel console right`, on the GPU that
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
    let gpu = Gpu::new().map_err(|error| slime_app::error_chain(&error))?;
    let mut window = Offscreen::open(program, arguments, &gpu, IMAGE_SIZE, scale)?;
    for operation in operations {
        window.run(operation)?;
    }
    window.show_level()?;
    if !selection.is_empty() {
        window.select(selection)?;
    }
    window.image()
}

/// The editor's window without a window of the system, on a GPU, frame by frame: the program's
/// core and its window's interface, as [`run`](crate::run) runs them, which a benchmark of the
/// interface times and counts (`docs/adr/0186-counters-and-ratios-in-ci.md`), and which
/// [`window_image`] draws. A frame of the program is the core's part ([`Offscreen::update`]), which
/// answers what the interface asked, then the window's ([`Offscreen::frame`]), which builds,
/// styles, lays out and gathers the interface, and its draw ([`Offscreen::draw`]); the viewports
/// are drawn only by [`window_image`]. Its journal is the one that `--journal` names, or the
/// user's, and it reads and writes none of the user's state: neither the layout, nor the keys, nor
/// the recent levels.
pub struct Offscreen {
    session: Session,
    host: Host,
    model: Model,
    gpu: Gpu,
    drawing: Drawing,
    target: RenderTarget,
    /// The window's size in logical pixels, and its pixels per logical pixel.
    size: [f32; 2],
    scale: f32,
    /// What the last frame drew.
    display: DisplayList,
    /// When the last frame ran, which the next one advances the program's time from.
    last: Instant,
    /// What the viewports draw, once [`Offscreen::show_level`] made them.
    level: Option<Level>,
    viewports: Vec<Viewport>,
}

/// What a frame of the window's interface did ([`Offscreen::frame`]).
#[derive(Clone, Copy, Debug)]
pub struct InterfaceFrame {
    /// What the interface's frame did: the elements built, styled, laid out, drawn again and
    /// visited.
    pub stats: FrameStats,
    /// The items of its display list.
    pub items: usize,
}

impl Offscreen {
    /// The window of `program` opened on its level as `arguments` say, as the program takes them
    /// ([`run`](crate::run)), `size` logical pixels at `scale` pixels per logical pixel, drawn on
    /// `gpu`; its interface shown without motion, its first frame not run.
    ///
    /// # Errors
    ///
    /// When the arguments are not the program's, the level cannot be opened, or the window's
    /// drawing cannot be made on `gpu`: why, as a sentence.
    pub fn open(
        program: &Program,
        arguments: impl IntoIterator<Item = OsString>,
        gpu: &Gpu,
        size: [u32; 2],
        scale: f32,
    ) -> Result<Self, String> {
        let options = crate::program::parse(program, arguments)?.ok_or("--help opens nothing")?;
        let shared = Shared::default();
        // As a person sees it, waiting for the dialog that no window opens here.
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
        let mut model = Model::new(shared);
        model.ui.set_reduced_motion(true);
        let format = TextureFormat::Rgba8UnormSrgb;
        let drawing = Drawing::new(gpu, format).map_err(|error| error.to_string())?;
        let physical = size.map(|side| (side as f32 * scale).round() as u32);
        let target = gpu
            .create_render_target("editor's window", physical[0], physical[1], format)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            session,
            host,
            model,
            gpu: gpu.clone(),
            drawing,
            target,
            size: size.map(|side| side as f32),
            scale,
            display: DisplayList::new(),
            last: Instant::now(),
            level: None,
            viewports: Vec::new(),
        })
    }

    /// Runs `operation`, an operation's name followed by its arguments, parted by spaces,
    /// `view.move_panel console right`, as a tool asks it.
    ///
    /// # Errors
    ///
    /// When the operation fails: why, as a sentence.
    pub fn run(&mut self, operation: &str) -> Result<(), String> {
        let mut words = operation.split_whitespace().map(str::to_owned);
        let run = Request::Run {
            name: words.next().unwrap_or_default(),
            arguments: words.collect(),
        };
        self.session
            .handle(self.host.world_mut(), run)
            .map(drop)
            .map_err(|error| slime_app::error_chain(&*error))
    }

    /// Selects the entities of `identities`, by their persistent identities, as a tool selects.
    ///
    /// # Errors
    ///
    /// When an identity is not one: why, as a sentence.
    pub fn select(&mut self, identities: &[&str]) -> Result<(), String> {
        let identities = identities
            .iter()
            .map(|identity| {
                identity
                    .parse()
                    .map_err(|error| format!("{identity}: {error}"))
            })
            .collect::<Result<_, String>>()?;
        self.session
            .handle(self.host.world_mut(), Request::Select(identities))
            .map(drop)
            .map_err(|error| slime_app::error_chain(&*error))
    }

    /// The core's part of a frame of the program: it answers what the window asked, and runs the
    /// fixed steps of the time since its last frame, as the program runs them before the
    /// window's.
    pub fn update(&mut self) {
        let now = Instant::now();
        self.host
            .frame(&mut self.session, now.duration_since(self.last));
        self.last = now;
    }

    /// The window's part of a frame of the program, its viewports apart: the window acts on what
    /// the user did and builds, styles, lays out and gathers its interface into the display list
    /// that [`Offscreen::draw`] draws. [`Offscreen::update`] runs the core's part first, as the
    /// program does.
    pub fn frame(&mut self) -> InterfaceFrame {
        let world = self.host.world();
        self.model.act(world);
        let (display, stats) = self.model.frame(world, self.size, self.scale);
        // What the interface did as it laid out, a submenu opened by the pointer's rest.
        self.model.act(world);
        let items = display.len();
        self.display = display;
        InterfaceFrame { stats, items }
    }

    /// The window's interface, as the last frame left it: what a test reads of its elements and
    /// their styles (`docs/adr/0229-styles-typed-as-declarations.md`).
    pub fn ui(&self) -> &Ui {
        &self.model.ui
    }

    /// Draws the display list of the last frame into the window's image through the canvas, on
    /// the GPU, the viewports' images as they were last drawn, without waiting for the GPU.
    ///
    /// # Errors
    ///
    /// When the canvas cannot be drawn: why, as a sentence.
    pub fn draw(&mut self) -> Result<(), String> {
        let viewports = &self.viewports;
        self.drawing
            .bridge
            .draw(
                &self.gpu,
                &mut self.drawing.renderer,
                self.model.ui.fonts(),
                &self.display,
                |key| viewport_image(viewports, key),
                &self.target,
            )
            .map(drop)
            .map_err(|error| error.to_string())
    }

    /// Frames the level in each viewport that the operations left, as it opens, once its meshes
    /// have loaded, for [`window_image`].
    fn show_level(&mut self) -> Result<(), String> {
        self.update();
        self.frame();
        let (assets, lighting) = {
            let shared = self.model.shared.borrow();
            (shared.assets.clone(), shared.lighting)
        };
        let format = TextureFormat::Rgba8UnormSrgb;
        self.level = assets
            .zip(lighting)
            .map(|(assets, lighting)| {
                Level::new(&self.gpu, &self.drawing.renderer, assets, lighting)
            })
            .transpose()
            .map_err(|error| error.to_string())?;
        let Some(level) = &mut self.level else {
            return Ok(());
        };
        for pane in 0..self.model.panes.len() {
            let name = viewport::name(pane);
            let viewport = Viewport::new(&self.gpu, &self.drawing.renderer, &name, format)
                .map_err(|error| error.to_string())?;
            self.viewports.push(viewport);
            if let Some(size) = self.model.viewport_size(pane) {
                frame_whole(
                    &self.gpu,
                    level,
                    &mut self.model,
                    pane,
                    self.host.world(),
                    size,
                )?;
            }
        }
        Ok(())
    }

    /// The window drawn whole, its viewports once their meshes have loaded, for [`window_image`].
    fn image(&mut self) -> Result<Image, String> {
        // Twice: the world panel's list builds its rows once the first frame laid it out.
        for _ in 0..2 {
            self.update();
            self.frame();
            for (pane, viewport) in self.viewports.iter_mut().enumerate() {
                if let (Some(size), Some(level)) = (self.model.viewport_size(pane), &mut self.level)
                {
                    draw_whole(
                        &self.gpu,
                        &mut self.drawing.renderer,
                        (level, viewport),
                        &self.model.marks(pane, self.host.world(), self.scale),
                        &self.model.panes[pane],
                        self.host.world(),
                        size,
                    )?;
                }
            }
            self.gpu.render(
                &RenderPassDescriptor {
                    target: &self.target,
                    clear: Some(LinearColor::BLACK),
                    depth: None,
                    timer: None,
                    resolve: None,
                },
                |_| {},
            );
            self.draw()?;
        }
        self.gpu
            .read_render_target(&self.target)
            .map_err(|error| error.to_string())
    }
}

/// How long [`window_image`] waits at most for the meshes that the level places to load.
const LOADING: std::time::Duration = std::time::Duration::from_secs(60);

/// Frames `level` in the viewport of the pane `pane`, of `size` pixels, as it opens, once the
/// meshes that `world` places have loaded, for [`window_image`].
fn frame_whole(
    gpu: &Gpu,
    level: &mut Level,
    model: &mut Model,
    pane: usize,
    world: &World,
    size: (u32, u32),
) -> Result<(), String> {
    let started = Instant::now();
    let selection = model.selected_entities(world);
    while model.panes[pane].frame_wanted {
        match level
            .bounds(gpu, world, &selection)
            .map_err(|error| error.to_string())?
        {
            Framing::Loading if started.elapsed() > LOADING => {
                return Err("the level's meshes did not load within a minute".to_owned());
            }
            Framing::Loading => std::thread::sleep(std::time::Duration::from_millis(5)),
            Framing::Nothing => model.panes[pane].frame_wanted = false,
            Framing::Sphere(center, radius) => {
                model.framed(pane, center, radius, f64::from(size.0) / f64::from(size.1));
                model.panes[pane].frame_wanted = false;
            }
        }
    }
    Ok(())
}

/// Draws `level` into `viewport` of `size` pixels as the window shows its pane `pane`, marked as
/// `marks` say, once the meshes that `world` places have loaded, for [`window_image`].
fn draw_whole(
    gpu: &Gpu,
    renderer: &mut Renderer,
    (level, viewport): (&mut Level, &mut Viewport),
    marks: &Marks,
    pane: &Pane,
    world: &World,
    size: (u32, u32),
) -> Result<(), String> {
    let started = Instant::now();
    viewport.set_settings(pane.settings);
    loop {
        let loading = viewport
            .prepare(
                level,
                gpu,
                world,
                pane.navigation.camera(),
                size,
                marks,
                false,
                Instant::now(),
            )
            .map_err(|error| error.to_string())?
            .is_some();
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
    /// The user, with a button that selects.
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
    /// What the viewports share, once a level is open and the GPU there, and the viewport of each
    /// pane, in their order.
    level: Option<Level>,
    viewports: Vec<Viewport>,
    /// The pickings asked of the viewports' views that have not come back, each with its pane and
    /// who asked: the user, for what, or a tool, where to answer it
    /// (`docs/adr/0159-picking-and-the-selection.md`).
    picks: Vec<(Pick, usize, Asker)>,
    /// What the last extraction laid out, which the frame draws.
    display: Option<DisplayList>,
    /// The title that the window shows.
    title: String,
    /// Whether the windowing system refused to hold the cursor, which the user was told once.
    cursor_refused: bool,
    /// Whether the window tried to hold the cursor for the gesture under way, once for each.
    cursor_tried: bool,
    /// How many levels the core had opened in place of the first when the viewports last took the
    /// cooked data.
    viewport_opened: u64,
    /// Whether the viewports drew the first level whole, every mesh loaded, which the log said.
    drawn_whole: bool,
}

impl Window {
    pub(crate) fn new(shared: Shared) -> Self {
        Self {
            shared,
            model: None,
            drawing: None,
            level: None,
            viewports: Vec::new(),
            picks: Vec::new(),
            display: None,
            title: String::new(),
            cursor_refused: false,
            cursor_tried: false,
            viewport_opened: 0,
            drawn_whole: false,
        }
    }

    /// Holds the cursor while a gesture of a viewport moves the camera, once for each gesture,
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

    /// Makes the viewports ready to draw this frame, one per pane, made first once a level is
    /// open: what each shows changed, or its meshes still load, which asks for the next frame; the
    /// one navigated at the full rate, the others resting.
    fn prepare_viewports(&mut self, ctx: &mut Context, world: &World) {
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
        let level = match &mut self.level {
            Some(level) => {
                // A level opened since: the cooked data as the core read it then, which may hold
                // what was cooked since the first.
                if opened != self.viewport_opened {
                    level.reload(gpu, assets);
                    for viewport in &mut self.viewports {
                        viewport.redraw();
                    }
                }
                level
            }
            None => match Level::new(gpu, &drawing.renderer, assets, lighting) {
                Ok(level) => self.level.insert(level),
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            },
        };
        self.viewport_opened = opened;
        // The pickings of a viewport that goes, which its view would never answer, go with it: a
        // tool's told that no answer came.
        self.viewports.truncate(model.panes.len());
        // So do those of a viewport behind its tab, which draws nothing.
        self.picks.retain(|&(_, pane, _)| {
            model
                .panes
                .get(pane)
                .is_some_and(|pane| pane.surface.is_some())
        });
        while self.viewports.len() < model.panes.len() {
            let name = viewport::name(self.viewports.len());
            match Viewport::new(gpu, &drawing.renderer, &name, format) {
                Ok(viewport) => self.viewports.push(viewport),
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            }
        }
        let scale = ctx
            .window()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let navigated = model
            .panes
            .iter()
            .position(|pane| pane.navigation.holds_cursor() || pane.navigation.flying());
        level.hide(&model.hidden, model.eyes_made);
        // The soonest that a viewport asks for a frame.
        let mut again: Option<Instant> = None;
        for (index, viewport) in self.viewports.iter_mut().enumerate() {
            let Some(size) = model.viewport_size(index) else {
                continue;
            };
            if model.panes[index].frame_wanted {
                match level.bounds(gpu, world, &model_selection(&self.shared, world)) {
                    Ok(Framing::Loading) => {}
                    Ok(framing) => {
                        model.panes[index].frame_wanted = false;
                        if let Framing::Sphere(center, radius) = framing {
                            let aspect = f64::from(size.0) / f64::from(size.1);
                            model.framed(index, center, radius, aspect);
                        }
                    }
                    Err(error) => {
                        ctx.exit_with_error(error);
                        return;
                    }
                }
            }
            let pane = &mut model.panes[index];
            let camera = pane.navigation.camera();
            // What its buttons asked: by the GPU, at the frame that draws the viewport next; by
            // the CPU, at once.
            for wanted in std::mem::take(&mut pane.wanted) {
                match area(&wanted, scale, size) {
                    Some(area) => {
                        self.picks
                            .push((viewport.pick(area), index, Asker::User(wanted)));
                    }
                    None => {
                        let Wanted::Through { at } = wanted else {
                            continue;
                        };
                        let at = [at[0] * scale, at[1] * scale];
                        let met = level.cast_through(world, &camera, size, at);
                        model.picked(world, &wanted, &met);
                    }
                }
            }
            let pane = &model.panes[index];
            viewport.set_settings(pane.settings);
            let marks = model.marks(index, world, scale);
            let resting = navigated.is_some_and(|navigated| navigated != index);
            match viewport.prepare(
                level,
                gpu,
                world,
                camera,
                size,
                &marks,
                resting,
                Instant::now(),
            ) {
                Ok(due) => again = again.into_iter().chain(due).min(),
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            }
        }
        // The hierarchy's search, once the typing paused.
        let again = again.into_iter().chain(model.hierarchy.waits()).min();
        if let Some(due) = again {
            ctx.request_frame_at(due);
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
        for (pick, pane, asker) in std::mem::take(&mut self.picks) {
            match (pick.take(gpu), asker) {
                (None, asker) => waiting.push((pick, pane, asker)),
                (Some(Ok(picked)), Asker::User(wanted)) => {
                    model.picked(world, &wanted, &found(world, &picked));
                    selected = true;
                }
                (Some(Ok(picked)), Asker::Tool(answer)) => {
                    answer.give(model.pickable(&found(world, &picked)).first().copied());
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

    /// The pane of the viewport named `name`, as tools name them ([`viewport::name`]).
    fn pane_named(&self, name: &str) -> Option<usize> {
        let model = self.model.as_ref()?;
        (0..model.panes.len()).find(|&pane| viewport::name(pane) == name)
    }
}

/// The entities that the core says are selected, which `world` holds.
fn model_selection(shared: &Shared, world: &World) -> Vec<Entity> {
    shared
        .borrow()
        .selection
        .iter()
        .filter_map(|identity| world.find(identity))
        .collect()
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
            // The mouse's raw motion turns the camera of the viewport navigated, and the keys fly
            // it, while the cursor is held.
            Event::RawMotion { delta } => {
                if let Some(pane) = model.navigated() {
                    pane.navigation.motion(*delta);
                }
            }
            // Escape cancels a handle dragged, what it moved put back.
            Event::Key(key)
                if key.state == slime_platform::ButtonState::Pressed
                    && key.physical == slime_platform::KeyCode::Escape
                    && model.cancel_drag() =>
            {
                return true;
            }
            Event::Key(key) => {
                let down = key.state == slime_platform::ButtonState::Pressed;
                return model
                    .navigated()
                    .is_some_and(|pane| pane.navigation.key(key.physical, down, Instant::now()));
            }
            Event::ModifiersChanged(modifiers) => {
                for pane in &mut model.panes {
                    pane.navigation.set_fast(modifiers.shift);
                }
            }
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
            self.display = Some(model.frame(world, size, scale).0);
            // What the interface did as it laid out, a submenu opened by the pointer's rest.
            again |= model.act(world);
            if let Some(choice) = model.dialog_wanted() {
                model.dialog_opened(window.choose_file(choice), Instant::now());
            }
        }
        // The layout written once it stood still, with a level open or not.
        if let Some(due) = model.layout_waits() {
            ctx.request_frame_at(due);
        }
        // The keys that fly a camera move it at every frame while they are held.
        let now = Instant::now();
        for pane in &mut model.panes {
            again |= pane.navigation.advance(now);
        }
        let held = model.navigated().is_some();
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
        self.prepare_viewports(ctx, world);
        // What the buttons asked where no viewport could answer, without a level or a GPU
        // device, is not answered later, against another image.
        if let Some(model) = &mut self.model {
            for pane in &mut model.panes {
                pane.wanted.clear();
            }
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
        for viewport in &mut self.viewports {
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
        let viewports = &self.viewports;
        if let Err(error) = drawing.bridge.draw(
            gpu,
            &mut drawing.renderer,
            model.ui.fonts(),
            display,
            |key| viewport_image(viewports, key),
            target,
        ) {
            ctx.exit_with_error(error);
            return;
        }
        // The time to the level drawn whole, as a level opened is measured: every viewport built
        // so far waits for no mesh (`docs/adr/0189-first-measures-of-time.md`).
        if !self.drawn_whole
            && self.viewports.iter().any(Viewport::shown)
            && !self.viewports.iter().any(Viewport::loading)
        {
            self.drawn_whole = true;
            tracing::info!(
                "the level is drawn whole, {:.1} ms after the start",
                slime_app::since_start().as_secs_f64() * 1e3
            );
        }
    }

    fn views(&self) -> Vec<ShownView> {
        let Some(model) = &self.model else {
            return Vec::new();
        };
        (0..self.viewports.len())
            .filter_map(|pane| {
                let rect = model.viewport_rect(pane)?;
                Some(ShownView {
                    name: viewport::name(pane),
                    x: rect.x.round() as u32,
                    y: rect.y.round() as u32,
                    width: rect.width.round() as u32,
                    height: rect.height.round() as u32,
                })
            })
            .collect()
    }

    fn reports(&self) -> Vec<(String, Reflected)> {
        self.viewports
            .iter()
            .enumerate()
            .map(|(pane, viewport)| (viewport::name(pane), Reflected::new(&viewport.report())))
            .collect()
    }

    fn view_settings(&self, name: &str) -> Option<Reflected> {
        let pane = self.pane_named(name)?;
        let model = self.model.as_ref()?;
        Some(Reflected::new(&model.panes[pane].settings))
    }

    fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
        let pane = self
            .pane_named(name)
            .ok_or_else(|| format!("the editor has no view `{name}`"))?;
        let model = self
            .model
            .as_mut()
            .ok_or("the editor's window is not open yet")?;
        model.panes[pane].settings = settings.get().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn pick(&mut self, view: &str, (x, y): (u32, u32), answer: PickAnswer) -> Result<(), String> {
        let pane = self
            .pane_named(view)
            .ok_or_else(|| format!("the editor has no view `{view}`"))?;
        let viewport = self
            .viewports
            .get_mut(pane)
            .ok_or("the editor's viewport waits for a level and a GPU device")?;
        let area = slime_render::Viewport {
            x,
            y,
            width: 1,
            height: 1,
        };
        self.picks
            .push((viewport.pick(area), pane, Asker::Tool(answer)));
        Ok(())
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every GPU object goes; the viewports are made again once the device is. The pickings of
        // the lost device go with it, a tool's told that no answer came.
        self.picks.clear();
        self.viewports.clear();
        self.level = None;
        self.drawing = None;
    }

    fn gpu_restored(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.drawing = Some(Drawing::new(gpu, format)?);
        }
        Ok(())
    }
}

/// The image of the viewport whose key in the display list is `key`, if one has it.
fn viewport_image(viewports: &[Viewport], key: ImageKey) -> Option<CanvasImage<'_>> {
    viewports
        .iter()
        .enumerate()
        .find(|&(pane, _)| viewport::image_key(pane) == key)
        .and_then(|(_, viewport)| viewport.image())
        .map(CanvasImage::Target)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use slime_app::Simulation;
    use slime_platform::{ButtonState, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey};
    use slime_reflect::Reflect;
    use slime_world::{Command, PersistentId};
    use uwui::display::Primitive;
    use uwui::text::{TextBounds, TextStyle};

    use super::*;
    use crate::operation::Changes;
    use crate::program::Lighting;

    const SIZE: [f32; 2] = [1280.0, 800.0];

    /// Where the system has a font for Greek, which the editor's look does not ship: fontconfig's
    /// on Linux, which the dev shell loads, and DirectWrite's on Windows. Without one, the test says
    /// so and checks nothing.
    #[test]
    fn the_window_falls_back_on_the_system_s_fonts_for_what_its_own_lack() {
        let greek = "Ελληνικά";
        let style = TextStyle {
            fonts: vec![look::TEXT_FAMILY.to_owned()],
            locale: Some("en".to_owned()),
            ..TextStyle::default()
        };
        let drawn_whole = |fonts: &mut Fonts| {
            let layout = fonts.lay_out(greek, &style, 1.0, TextBounds::default());
            !layout.runs().is_empty()
                && layout
                    .runs()
                    .iter()
                    .all(|run| run.glyphs.iter().all(|glyph| glyph.id != 0))
        };
        if !drawn_whole(&mut Fonts::with_system_fonts()) {
            eprintln!("the system gives no font for Greek here: nothing to fall back on");
            return;
        }
        let mut model = Model::new(Shared::default());
        assert!(drawn_whole(model.ui.fonts_mut()));
    }

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
            Self::playing(name, None, None)
        }

        /// The yard, its program playing the game that `play` starts, built as `build` says.
        fn playing(
            name: &str,
            play: Option<fn(&crate::Played<'_>) -> std::process::Command>,
            build: Option<crate::Build>,
        ) -> Self {
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
                    "[entities]\nwell = { components = { Name = \"well\", Transform = {} } }\n",
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
                play,
                build,
                layout: None,
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
        ui.set_shortcuts(&[
            Shortcuts::from_text("editor.keys", SHORTCUTS),
            Shortcuts::from_text("editor-blender.keys", BLENDER_SHORTCUTS),
        ])
        .expect("valid shortcuts");
        let scopes = ui.shortcuts();
        assert!(!scopes.is_empty());
        for (scope, chords) in scopes {
            for (chord, command) in chords {
                // What the buttons do over the viewport: a gesture, or what selects.
                let known = if scope == VIEWPORT_SCOPE {
                    Bound::named(&command).is_some()
                } else {
                    operation::operation(&command).is_some()
                };
                assert!(known, "{scope}, {chord}: `{command}` is no operation");
            }
        }
    }

    /// The left button over the viewport: a click, a drag that draws a marquee, and Alt and a click,
    /// each asking what is drawn there, and what the viewport found selected in the core
    /// (`docs/adr/0159-picking-and-the-selection.md`).
    #[test]
    fn the_left_button_over_the_viewport_asks_what_is_drawn_there_and_selects_it() {
        let mut yard = Yard::new("pick");
        let rect = yard.model.viewport_rect(0).expect("laid out");
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
        assert_eq!(std::mem::take(&mut yard.model.panes[0].wanted), [wanted]);
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
        let (display, _) = yard.model.frame(yard.host.world(), SIZE, 1.0);
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
            std::mem::take(&mut yard.model.panes[0].wanted),
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
            std::mem::take(&mut yard.model.panes[0].wanted),
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

    /// Blender's way, chosen from the View menu: the middle button orbits, Alt and a click go
    /// through what lies behind, and the choice is kept for the next start
    /// (`docs/adr/0160-two-viewports-and-blenders-way.md`).
    #[test]
    fn blenders_way_moves_the_buttons_and_is_kept_for_the_next_start() {
        let mut yard = Yard::new("blender");
        let keys = yard.directory.join("state").join("keys");
        yard.shared.borrow_mut().keys = Some(keys.clone());
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
        let (mut shift, mut alt) = (Modifiers::default(), Modifiers::default());
        shift.shift = true;
        alt.alt = true;
        let bound = |model: &Model, button, modifiers| {
            model
                .ui
                .button_command(VIEWPORT_SCOPE, button, modifiers)
                .map(str::to_owned)
        };
        let none = Modifiers::default();
        assert_eq!(
            bound(&yard.model, MouseButton::Middle, none).as_deref(),
            Some("pan")
        );
        run(&mut yard, "view.keys_blender");
        assert_eq!(
            bound(&yard.model, MouseButton::Middle, none).as_deref(),
            Some("orbit")
        );
        assert_eq!(
            bound(&yard.model, MouseButton::Middle, shift).as_deref(),
            Some("pan")
        );
        assert_eq!(
            bound(&yard.model, MouseButton::Left, shift).as_deref(),
            Some("select")
        );
        assert_eq!(fs::read_to_string(&keys).expect("kept").trim(), "blender");
        // Alt and the left button go through and do not orbit: held, they hold no cursor, and
        // dragged, they turn nothing and ask for nothing.
        let rect = yard.model.viewport_rect(0).expect("laid out");
        let camera = yard.model.panes[0].navigation.camera();
        yard.model.ui.pointer_moved(rect.x + 40.0, rect.y + 30.0);
        yard.model.ui.modifiers(alt);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.frame();
        assert!(!yard.model.panes[0].navigation.holds_cursor());
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.frame();
        assert_eq!(
            std::mem::take(&mut yard.model.panes[0].wanted),
            [Wanted::Through { at: [40.0, 30.0] }]
        );
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.model.ui.pointer_moved(rect.x + 140.0, rect.y + 90.0);
        yard.frame();
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.model.ui.modifiers(none);
        yard.frame();
        assert_eq!(yard.model.panes[0].navigation.camera(), camera);
        assert!(yard.model.panes[0].wanted.is_empty());
        yard.model
            .ui
            .pointer_button(MouseButton::Middle, ButtonState::Pressed);
        yard.frame();
        assert!(yard.model.panes[0].navigation.holds_cursor());
        yard.model
            .ui
            .pointer_button(MouseButton::Middle, ButtonState::Released);
        yard.frame();
        // A window opened again, as at the next start, takes the choice.
        let again = Model::new(yard.shared.clone());
        assert_eq!(
            bound(&again, MouseButton::Middle, none).as_deref(),
            Some("orbit")
        );
        run(&mut yard, "view.keys_default");
        assert_eq!(
            bound(&yard.model, MouseButton::Middle, none).as_deref(),
            Some("pan")
        );
        assert_eq!(fs::read_to_string(&keys).expect("kept").trim(), "default");
    }

    /// The point of the screen, in pixels, where the camera of the first pane shows `point`.
    fn on_screen(model: &Model, point: DVec3) -> (f32, f32) {
        let rect = model.viewport_rect(0).expect("laid out");
        let camera = model.panes[0].navigation.camera();
        let clip = camera
            .view_projection(rect.width / rect.height)
            .project_point3(point)
            .as_vec3();
        (
            rect.x + (clip.x + 1.0) / 2.0 * rect.width,
            rect.y + (1.0 - clip.y) / 2.0 * rect.height,
        )
    }

    /// A handle dragged over the viewport moves what is selected live, one step at its release,
    /// agents' requests waiting from its press to its release; Escape cancels another drag, the
    /// well put back; a press let go without a move makes no step
    /// (`docs/adr/0161-the-open-step-and-the-handles.md`).
    #[test]
    fn a_handle_dragged_moves_what_is_selected_as_one_step_and_escape_cancels_it() {
        let mut yard = Yard::new("handles");
        let well: PersistentId = "yard/well".parse().expect("an identity");
        yard.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(vec![well.clone()]));
        yard.frame();
        yard.frame();
        let entity = yard.host.world().find(&well).expect("the well");
        let x = |yard: &Yard| {
            yard.host
                .world()
                .get::<slime_world::Transform>(entity)
                .expect("placed")
                .translation
                .x
        };
        let gizmo = yard
            .model
            .gizmo(0, yard.host.world(), 1.0)
            .expect("the well's handles");
        let grab = on_screen(&yard.model, gizmo.pivot + DVec3::X * gizmo.size * 0.6);
        let away = on_screen(&yard.model, gizmo.pivot + DVec3::X * gizmo.size * 1.6);
        let drag = |yard: &mut Yard, end: bool| {
            yard.model.ui.pointer_moved(grab.0, grab.1);
            yard.model
                .ui
                .pointer_button(MouseButton::Left, ButtonState::Pressed);
            yard.frame();
            assert!(yard.session.edits_wait(), "agents wait from the press on");
            yard.model.ui.pointer_moved(away.0, away.1);
            yard.frame();
            yard.frame();
            if end {
                yard.model
                    .ui
                    .pointer_button(MouseButton::Left, ButtonState::Released);
            } else {
                assert!(yard.model.cancel_drag());
            }
            yard.frame();
            yard.frame();
        };
        drag(&mut yard, false);
        assert_eq!(x(&yard), 0.0, "cancelled");
        assert!(yard.history().0.is_empty());
        assert!(!yard.session.edits_wait());
        // Pressed and let go where it was.
        yard.model.ui.pointer_moved(grab.0, grab.1);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.frame();
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.frame();
        yard.frame();
        assert!(yard.history().0.is_empty(), "no step");
        assert!(!yard.session.edits_wait());
        drag(&mut yard, true);
        assert!(
            (x(&yard) - gizmo.size).abs() < 1e-3,
            "moved a handle's length: {}",
            x(&yard)
        );
        assert_eq!(yard.history().0, ["Move"]);
        assert!(!yard.session.edits_wait(), "agents go on once it ends");
        assert!(
            yard.model.panes[0].wanted.is_empty(),
            "nothing picked meanwhile"
        );
    }

    /// The operations that create and arrange, run by their names as the menus and the keys run
    /// them, each a step: a duplicate in place, nudged, the nudge repeated, the well and its
    /// duplicate grouped where they were, the group copied and pasted, through the window, which
    /// pastes what the editor copied without a clipboard of the system, deleted, and an entity
    /// created (`docs/adr/0162-creating-copying-and-arranging.md`).
    #[test]
    fn what_is_selected_is_duplicated_nudged_grouped_copied_pasted_and_deleted_each_a_step() {
        let mut yard = Yard::new("arrange");
        let run = |yard: &mut Yard, name: &str| {
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: name.to_owned(),
                        arguments: Vec::new(),
                    },
                )
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            yard.frame();
            yard.frame();
        };
        let selected = |yard: &Yard| yard.shared.borrow().selection.clone();
        let x = |yard: &Yard, id: &PersistentId| {
            let world = yard.host.world();
            let entity = world.find(id).expect("alive");
            world
                .interpolated_transform(entity, 1.0)
                .expect("placed")
                .translation
                .x
        };
        let well: PersistentId = "yard/well".parse().expect("an identity");
        yard.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(vec![well.clone()]));
        yard.frame();
        run(&mut yard, "edit.duplicate");
        let [copy] = selected(&yard).try_into().expect("the duplicate selected");
        assert_ne!(copy, well);
        assert!(copy.to_string().starts_with("yard/well-"), "{copy}");
        run(&mut yard, "edit.nudge_right");
        run(&mut yard, "edit.repeat");
        assert!((x(&yard, &copy) - 0.5).abs() < 1e-9);
        assert_eq!(x(&yard, &well), 0.0);
        yard.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(vec![well.clone(), copy.clone()]));
        yard.frame();
        run(&mut yard, "edit.group");
        let [group] = selected(&yard).try_into().expect("the group selected");
        let world = yard.host.world();
        let grouped = world.find(&group).expect("the group");
        assert_eq!(world.children(grouped).count(), 2);
        assert!((x(&yard, &group) - 0.25).abs() < 1e-9, "at their middle");
        assert_eq!(x(&yard, &well), 0.0, "where it was");
        assert!((x(&yard, &copy) - 0.5).abs() < 1e-9, "where it was");
        run(&mut yard, "edit.copy");
        assert!(yard.shared.borrow().clipboard.is_some());
        run(&mut yard, "edit.paste");
        let [pasted] = selected(&yard).try_into().expect("the paste selected");
        assert_ne!(pasted, group);
        let world = yard.host.world();
        assert_eq!(
            world.children(world.find(&pasted).expect("pasted")).count(),
            2
        );
        run(&mut yard, "edit.delete");
        assert!(selected(&yard).is_empty());
        assert!(yard.host.world().find(&pasted).is_none());
        run(&mut yard, "edit.create");
        let [created] = selected(&yard).try_into().expect("the entity selected");
        assert!(created.to_string().starts_with("yard/entity-"), "{created}");
        assert_eq!(
            yard.history().0,
            [
                "Duplicate",
                "Nudge right",
                "Nudge right",
                "Group",
                "Paste",
                "Delete",
                "Create an entity"
            ]
        );
    }

    /// Runs the operation `name` as a person does, then two frames, the window's included.
    fn run(yard: &mut Yard, name: &str) {
        run_with(yard, name, &[]);
    }

    /// Where the entity `id` is in the world.
    fn placed(yard: &Yard, id: &PersistentId) -> slime_world::GlobalTransform {
        let world = yard.host.world();
        world
            .interpolated_transform(world.find(id).expect("alive"), 1.0)
            .expect("placed")
    }

    /// A clipboard of the system that a test holds.
    #[derive(Clone, Default)]
    struct Board(std::sync::Arc<std::sync::Mutex<Option<String>>>);

    impl uwui::tree::Clipboard for Board {
        fn text(&mut self) -> Option<String> {
            self.0.lock().expect("not poisoned").clone()
        }

        fn set_text(&mut self, text: &str) {
            *self.0.lock().expect("not poisoned") = Some(text.to_owned());
        }
    }

    /// Under a base turned a quarter about Y and twice as large: a post grouped with the well stays
    /// where it was in the world, the group at their middle; nudged, it moves a quarter of a metre
    /// along the world's X; an entity created goes where it is; a paste repeated pastes again; and
    /// a paste right after a copy, before the window put the copy on the system's clipboard, which
    /// held something else, pastes the copy (`docs/adr/0162-creating-copying-and-arranging.md`).
    #[test]
    fn what_is_grouped_nudged_and_created_under_a_turned_parent_stays_where_it_is_in_the_world() {
        let mut yard = Yard::new("turned");
        let board = Board::default();
        board
            .0
            .lock()
            .expect("not poisoned")
            .replace("a word".to_owned());
        yard.model.ui.set_clipboard(Box::new(board.clone()));
        let id = |text: &str| -> PersistentId { text.parse().expect("an identity") };
        let (base, post, well) = (id("yard/base-k3f9"), id("yard/post-k3f9"), id("yard/well"));
        let transform = |translation: DVec3, quarter: bool, scale: f32| {
            slime_world::Transform {
                translation,
                rotation: if quarter {
                    slime_world::glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                } else {
                    slime_world::glam::Quat::IDENTITY
                },
                scale,
                ..slime_world::Transform::IDENTITY
            }
            .to_value()
        };
        yard.shared.borrow_mut().requests.push_back(Request::Apply {
            label: "A post on a base".to_owned(),
            commands: vec![
                Command::Create {
                    entity: base.clone(),
                    parent: Some(id("yard")),
                    rank: None,
                    components: vec![(
                        "Transform".to_owned(),
                        transform(DVec3::new(1.0, 0.0, 0.0), true, 2.0),
                    )],
                },
                Command::Create {
                    entity: post.clone(),
                    parent: Some(base.clone()),
                    rank: None,
                    components: vec![(
                        "Transform".to_owned(),
                        transform(DVec3::new(1.0, 0.0, 0.0), false, 1.0),
                    )],
                },
            ],
            selection: Some(vec![post.clone(), well.clone()]),
        });
        yard.frame();
        let (post_at, well_at) = (placed(&yard, &post), placed(&yard, &well));
        assert!(post_at.translation.distance(DVec3::new(1.0, 0.0, -2.0)) < 1e-6);
        run(&mut yard, "edit.group");
        let near = |a: DVec3, b: DVec3| a.distance(b) < 1e-5;
        assert!(near(placed(&yard, &post).translation, post_at.translation));
        assert!(
            placed(&yard, &post)
                .rotation
                .abs_diff_eq(post_at.rotation, 1e-5)
        );
        assert!(near(placed(&yard, &well).translation, well_at.translation));
        let [group] = yard
            .shared
            .borrow()
            .selection
            .clone()
            .try_into()
            .expect("the group selected");
        assert!(near(
            placed(&yard, &group).translation,
            (post_at.translation + well_at.translation) / 2.0
        ));
        yard.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(vec![post.clone()]));
        yard.frame();
        run(&mut yard, "edit.nudge_right");
        assert!(near(
            placed(&yard, &post).translation,
            post_at.translation + DVec3::new(0.25, 0.0, 0.0)
        ));
        run(&mut yard, "edit.create");
        let [created] = yard
            .shared
            .borrow()
            .selection
            .clone()
            .try_into()
            .expect("the entity selected");
        assert!(near(
            placed(&yard, &created).translation,
            post_at.translation + DVec3::new(0.25, 0.0, 0.0)
        ));
        // Copied and pasted at once, before a frame; then the paste repeated.
        yard.shared
            .borrow_mut()
            .requests
            .push_back(Request::Select(vec![post.clone()]));
        yard.frame();
        for name in ["edit.copy", "edit.paste"] {
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: name.to_owned(),
                        arguments: Vec::new(),
                    },
                )
                .expect(name);
        }
        yard.frame();
        yard.frame();
        run(&mut yard, "edit.repeat");
        let done = yard.history().0;
        assert_eq!(done[done.len() - 2..], ["Paste", "Paste"], "{done:?}");
        assert!(
            board
                .0
                .lock()
                .expect("not poisoned")
                .as_deref()
                .is_some_and(|text| text.starts_with("# do")),
            "the copy on the system's clipboard"
        );
    }

    /// The history panel shows a row before the first step, then each step, the one that the level
    /// stands at marked; a press on a row goes to it, undoing or doing again the steps between; the
    /// prefabs that the level can place are found in the palette, and one chosen is placed by its
    /// asset ID (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    #[test]
    fn a_row_of_the_history_panel_goes_to_its_step_and_a_prefab_is_chosen_in_the_palette() {
        let mut yard = Yard::new("history rows");
        run(&mut yard, "edit.create");
        run(&mut yard, "edit.create");
        let marked = |yard: &Yard| -> Vec<usize> {
            yard.model
                .history
                .iter()
                .filter(|&&(row, _)| yard.model.ui.state(row).is_some_and(|state| state.selected))
                .map(|&(_, steps)| steps)
                .collect()
        };
        assert_eq!(yard.model.history.len(), 3);
        assert_eq!(marked(&yard), [2]);
        // A press on the row before the first step undoes both.
        let (row, _) = yard.model.history[0];
        let rect = yard.model.ui.rect(row).expect("laid out");
        yard.model
            .ui
            .pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.frame();
        yard.frame();
        let (done, undone) = yard.history();
        assert!(done.is_empty());
        assert_eq!(undone.len(), 2);
        assert_eq!(marked(&yard), [0]);
        // Then to the second step, both done again.
        run_with(&mut yard, "edit.go_to_step", &["2"]);
        assert_eq!(yard.history().0.len(), 2);
        assert!(
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: "edit.go_to_step".to_owned(),
                        arguments: vec!["3".to_owned()],
                    },
                )
                .is_err(),
            "no third step"
        );
        // Without a prefab, the palette does not open, and the user is told why.
        run(&mut yard, "edit.choose_prefab");
        assert!(!yard.model.ui.palette_open());
        assert!(
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.contains("no prefab")),
            "told in a notification"
        );
        // The palette of prefabs, and one chosen.
        let lamp = slime_assets::AssetId::from_u128(7);
        yard.shared.borrow_mut().prefabs = vec![(lamp, "lamp.scene".to_owned())];
        run(&mut yard, "edit.choose_prefab");
        assert!(yard.model.ui.palette_open());
        assert!(
            yard.model.ui.meaning().iter().any(|meaning| {
                meaning.path.starts_with("palette") && meaning.name.contains("lamp.scene")
            }),
            "the prefab found in the palette"
        );
        let chosen = Action {
            element: None,
            kind: ActionKind::Command(format!("{PLACE}{lamp}")),
        };
        assert!(matches!(
            yard.model.request_of(&chosen),
            Some(Request::Run { name, arguments })
                if name == "edit.place" && arguments == [lamp.to_string()]
        ));
    }

    /// Runs the operation `name` with `arguments`, then two frames.
    fn run_with(yard: &mut Yard, name: &str, arguments: &[&str]) {
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Run {
                    name: name.to_owned(),
                    arguments: arguments
                        .iter()
                        .map(|&argument| argument.to_owned())
                        .collect(),
                },
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        yard.frame();
        yard.frame();
    }

    /// The environment variable that makes this test program a stub game, which Play starts.
    const STUB_GAME: &str = "SLIME_EDITOR_STUB_GAME";

    /// The environment variable that makes the stub game say that it resumes its save.
    const STUB_RESUMES: &str = "SLIME_EDITOR_STUB_RESUMES";

    /// The environment variable that names this test program to the program that a test builds,
    /// which starts it as the stub game.
    const STUB_PROGRAM: &str = "SLIME_EDITOR_STUB_PROGRAM";

    /// A game that spawns the yard from the cooked data, as Play hands it over, tells the well's
    /// name in its log, again each time it changes, saves its runs, a file that says so, unless the
    /// cooked data holds a file `nosave`, and runs until it is stopped, or ends after a few frames,
    /// as a crash, when the cooked data holds a file `crash`.
    struct Stub {
        assets: PathBuf,
        frames: u32,
        /// The well's name as last told, none once it is gone.
        named: Option<String>,
    }

    impl Stub {
        /// Tells the well's name, or that it is gone, when it changed since it was last told.
        fn tell(&mut self, world: &World) {
            let name = world
                .find(&"yard/well".parse().expect("an identity"))
                .map(|well| {
                    world
                        .get::<slime_world::Name>(well)
                        .map(|name| name.0.clone())
                        .unwrap_or_default()
                });
            if name != self.named {
                match &name {
                    Some(name) => tracing::info!("the well is named {name}"),
                    None => tracing::info!("the well is gone"),
                }
                self.named = name;
            }
        }
    }

    impl slime_app::Simulation for Stub {
        fn start(
            &mut self,
            _: &mut slime_app::SimulationContext<'_>,
            world: &mut World,
        ) -> Result<(), slime_app::AppError> {
            let assets = slime_assets::CookedAssets::open(&self.assets)?;
            let mut scenes = slime_world::SceneLibrary::of(assets);
            let yard = scenes.resolve(world.registry(), slime_assets::AssetId::from_u128(1))?;
            world.spawn_scene("yard", &yard)?;
            if std::env::var_os(STUB_RESUMES).is_some() {
                tracing::info!("the stub resumes its save");
            }
            self.tell(world);
            Ok(())
        }

        fn save(&mut self, _: &World, file: &std::path::Path) -> Result<(), slime_app::AppError> {
            if self.assets.join("nosave").exists() {
                return Err("the stub saves no run here".into());
            }
            fs::write(file, "saved")?;
            Ok(())
        }

        fn update(&mut self, _: &mut slime_app::SimulationContext<'_>, world: &mut World) {
            self.tell(world);
            self.frames += 1;
            if self.frames == 50 && self.assets.join("crash").exists() {
                std::process::exit(3);
            }
            // Headless frames run back to back: a little rest each keeps the test light.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// The stub game, when Play starts this test program as one; nothing otherwise. With a file
    /// `slow` in the cooked data, it waits half a minute before it starts, as a game that loads.
    #[test]
    fn stub_game() {
        let Some(assets) = std::env::var_os(STUB_GAME) else {
            return;
        };
        if PathBuf::from(&assets).join("slow").exists() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        let mut stub = Stub {
            assets: PathBuf::from(assets),
            frames: 0,
            named: None,
        };
        slime_app::run(&mut stub, &mut (), &AppConfig::default()).expect("the stub game runs");
    }

    /// This test program, as the stub game on the cooked data that Play gives.
    fn stub_command(played: &crate::Played<'_>) -> std::process::Command {
        let mut command =
            std::process::Command::new(std::env::current_exe().expect("the test program"));
        command
            .args([
                "--exact",
                "window::tests::stub_game",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(STUB_GAME, played.assets)
            .stderr(std::process::Stdio::null());
        command
    }

    /// The program that cargo builds for the test of building the game again, which starts this
    /// test program as the stub game, its standard streams its own: `{}` takes a comment, which a
    /// change of its code changes.
    const WRAPPER: &str = "// {}\nfn main() {\n    let stub = std::env::var_os(\"SLIME_EDITOR_STUB_PROGRAM\").expect(\"the stub\");\n    let status = std::process::Command::new(stub)\n        .args([\"--exact\", \"window::tests::stub_game\", \"--nocapture\", \"--test-threads=1\"])\n        .status()\n        .expect(\"the stub runs\");\n    std::process::exit(status.code().unwrap_or(1));\n}\n";

    /// The program that cargo built from [`WRAPPER`], which starts the stub game on the cooked
    /// data that Play gives, resuming its save when Play says so.
    fn wrapper_command(played: &crate::Played<'_>) -> std::process::Command {
        let mut command = std::process::Command::new(played.program("wrapper"));
        command.env(STUB_GAME, played.assets).env(
            STUB_PROGRAM,
            std::env::current_exe().expect("the test program"),
        );
        if played.resume {
            command.env(STUB_RESUMES, "1");
        }
        command
    }

    /// Frames of `yard` until `done` holds, at most for half a minute.
    fn until(yard: &mut Yard, what: &str, done: impl Fn(&Yard) -> bool) {
        let started = std::time::Instant::now();
        while !done(yard) {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(30),
                "{what}: not within half a minute; console: {:?}",
                yard.shared.borrow().console
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
            yard.frame();
        }
    }

    /// A press of the left button in the middle of `element`, as a person's.
    fn click(yard: &mut Yard, element: ElementId) {
        let rect = yard.model.ui.rect(element).expect("laid out");
        yard.model
            .ui
            .pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.frame();
    }

    /// The button of Play's toolbar that runs `name`.
    fn button(yard: &Yard, name: &str) -> ElementId {
        yard.model
            .toolbar
            .iter()
            .find(|(_, each)| *each == name)
            .map(|&(button, _)| button)
            .expect("on the toolbar")
    }

    /// Play starts the game on the level as the editor holds it, a step not written included, the
    /// user's file left as it was; the steps made while it starts, and made, undone and done again
    /// while it runs reach it, but for an undone deletion, which the user is told of, as of a step
    /// that the game refuses, no other level opens meanwhile, and Play does not start during a
    /// gesture (`docs/adr/0165-editing-while-playing.md`); the toolbar's buttons and speeds,
    /// pressed, pause, step, change the speed and stop it, the toolbar showing game time as the
    /// game says it stands; a game that is stopped while it starts ends at once; a game that crashes leaves the
    /// editor running, which tells the user (`docs/adr/0164-play.md`).
    #[test]
    fn play_runs_the_level_as_the_editor_holds_it_and_outlives_its_game() {
        let mut yard = Yard::playing("play", Some(stub_command), None);
        yard.rename("pump");
        let level = yard.directory.join("yard.scene");
        let written = fs::read(&level).expect("the level's file");
        // Not during a gesture, whose cancel would not reach the game.
        let world = yard.host.world_mut();
        yard.session
            .handle(world, Request::Gesture(Vec::new()))
            .expect("opened");
        let refused = yard.session.handle(
            yard.host.world_mut(),
            Request::Run {
                name: "play.start".to_owned(),
                arguments: Vec::new(),
            },
        );
        assert!(
            refused.is_err_and(|error| error.to_string().contains("gesture")),
            "refused"
        );
        yard.session
            .handle(yard.host.world_mut(), Request::CancelGesture)
            .expect("cancelled");
        let pressed = button(&yard, "play.start");
        click(&mut yard, pressed);
        // A step made while the game starts reaches it once it runs, after the level as Play
        // wrote it. The press is answered at the next frame.
        yard.frame();
        assert_eq!(yard.shared.borrow().play, Playing::Starting);
        yard.rename("early");
        let told = |yard: &Yard, line: &str| {
            yard.shared
                .borrow()
                .console
                .iter()
                .filter(|each| each.contains(line))
                .count()
        };
        until(
            &mut yard,
            "the game runs on the steps not written",
            |yard| {
                matches!(yard.shared.borrow().play, Playing::Running { .. })
                    && told(yard, "the well is named pump") == 1
                    && told(yard, "the well is named early") == 1
            },
        );
        // Edits made while it runs reach it, an undo too.
        yard.rename("tap");
        until(&mut yard, "the game sees the rename", |yard| {
            told(yard, "the well is named tap") == 1
        });
        run(&mut yard, "edit.undo");
        until(&mut yard, "the game sees the undo", |yard| {
            told(yard, "the well is named early") == 2
        });
        // Another level, which the game does not play, is not opened meanwhile.
        let other = yard.directory.join("other.scene");
        let refused = yard.session.handle(
            yard.host.world_mut(),
            Request::Run {
                name: "file.open".to_owned(),
                arguments: vec![other.display().to_string()],
            },
        );
        assert!(
            refused.is_err_and(|error| error.to_string().contains("Stop it first")),
            "refused"
        );
        // A deletion reaches it; its undo does not, which the user is told, and a rename of the
        // entity that it lacks then is refused, which the user is told too.
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/well".parse().expect("an identity")]),
            )
            .expect("selected");
        run(&mut yard, "edit.delete");
        until(&mut yard, "the game sees the deletion", |yard| {
            told(yard, "the well is gone") == 1
        });
        let shown = |yard: &Yard, text: &str| {
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.contains(text))
        };
        run(&mut yard, "edit.undo");
        assert!(shown(&yard, "An undone deletion does not reach the game"));
        yard.rename("spout");
        until(&mut yard, "the game refuses the rename", |yard| {
            shown(yard, "The game refused: command 0")
        });
        assert_eq!(told(&yard, "the well is named spout"), 0);
        let pressed = button(&yard, "play.pause");
        click(&mut yard, pressed);
        let pressed = button(&yard, "play.step");
        click(&mut yard, pressed);
        let quarter = yard
            .model
            .ui
            .children(yard.model.speeds.expect("the speeds"))[0];
        click(&mut yard, quarter);
        until(
            &mut yard,
            "the game says it is paused at a quarter",
            |yard| {
                yard.shared.borrow().play
                    == Playing::Running {
                        paused: true,
                        speed: 0.25,
                    }
            },
        );
        let pressed = button(&yard, "play.stop");
        click(&mut yard, pressed);
        until(&mut yard, "the game stops", |yard| {
            yard.shared.borrow().play == Playing::Stopped
        });
        assert_eq!(
            fs::read(&level).expect("the level's file"),
            written,
            "left alone"
        );
        // A game slow to start, stopped before it said where it listens: at once.
        let slow = yard.directory.join("cooked").join("slow");
        fs::write(&slow, "").expect("written");
        run(&mut yard, "play.start");
        run(&mut yard, "play.stop");
        let asked = std::time::Instant::now();
        until(&mut yard, "the slow game stops", |yard| {
            yard.shared.borrow().play == Playing::Stopped
        });
        assert!(asked.elapsed() < std::time::Duration::from_secs(5));
        fs::remove_file(&slow).expect("removed");
        // A game that crashes while it runs: the editor goes on, and says how it ended.
        fs::write(yard.directory.join("cooked").join("crash"), "").expect("written");
        run(&mut yard, "play.start");
        until(&mut yard, "the crash is seen", |yard| {
            yard.shared.borrow().play == Playing::Stopped
        });
        yard.frame();
        assert!(
            yard.model.ui.meaning().iter().any(|meaning| {
                meaning.name.contains("the game ended") && meaning.name.contains('3')
            }),
            "told"
        );
        run(&mut yard, "edit.create");
        assert_eq!(
            yard.history().0.last().map(String::as_str),
            Some("Create an entity")
        );
    }

    /// Play builds the game with cargo before it starts it, and ends when it does not build; a
    /// change of its code while it plays builds it again, and the game starts again on a save of
    /// its run, the toolbar marking it; a change that does not build is told, the toolbar marking
    /// it, and the game plays on, until a change builds; a game that does not save its run starts
    /// a new run on the level as the editor holds it (`docs/adr/0166-building-the-game-again.md`).
    #[test]
    fn play_builds_the_game_and_builds_it_again_as_its_code_changes() {
        let crate_directory =
            std::env::temp_dir().join(format!("slime-editor-wrapper-{}", std::process::id()));
        let _ = fs::remove_dir_all(&crate_directory);
        fs::create_dir_all(crate_directory.join("src")).expect("a directory of the test's own");
        fs::write(
            crate_directory.join("Cargo.toml"),
            "[package]\nname = \"wrapper\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .expect("written");
        let main = crate_directory.join("src").join("main.rs");
        fs::write(&main, "fn main() { not rust }\n").expect("written");
        let build = crate::Build {
            directory: crate_directory.clone(),
            arguments: &["--bin", "wrapper"],
        };
        let mut yard = Yard::playing("build", Some(wrapper_command), Some(build));
        let told = |yard: &Yard, line: &str| {
            yard.shared
                .borrow()
                .console
                .iter()
                .filter(|each| each.contains(line))
                .count()
        };
        let shown = |yard: &Yard, text: &str| {
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.contains(text))
        };
        // A game that does not build does not play.
        run(&mut yard, "play.start");
        until(&mut yard, "Play ends", |yard| {
            yard.shared.borrow().play == Playing::Stopped
        });
        yard.frame();
        assert!(shown(&yard, "the game's code does not build"));
        fs::write(&main, WRAPPER.replace("{}", "first")).expect("written");
        run(&mut yard, "play.start");
        let built = |yard: &Yard| yard.shared.borrow().build == BuildMark::None;
        let running = |yard: &Yard| matches!(yard.shared.borrow().play, Playing::Running { .. });
        // The first build takes a few seconds.
        until(&mut yard, "the game built and running", |yard| {
            running(yard) && built(yard) && told(yard, "the well is named") == 1
        });
        // Its code changed: built again, started again where its run stood.
        fs::write(&main, WRAPPER.replace("{}", "second")).expect("written");
        until(&mut yard, "the game built again and resumed", |yard| {
            running(yard) && told(yard, "the stub resumes its save") == 1
        });
        assert!(shown(&yard, "starts again where its run stood"));
        assert_eq!(yard.shared.borrow().build, BuildMark::BuiltAgain);
        // A change that does not build: told, marked, and the game plays on.
        fs::write(&main, "fn main() { not rust }\n").expect("written");
        until(&mut yard, "the build fails", |yard| {
            yard.shared.borrow().build == BuildMark::Failed
        });
        yard.frame();
        assert!(shown(&yard, "The game's code does not build"));
        assert!(running(&yard), "plays on");
        assert!(told(&yard, "error") > 0, "cargo's errors in the console");
        // Mended: built again.
        fs::write(&main, WRAPPER.replace("{}", "third")).expect("written");
        until(&mut yard, "the game built again once mended", |yard| {
            running(yard) && told(yard, "the stub resumes its save") == 2
        });
        // A game that does not save its run: its new run starts on the level as the editor holds
        // it, a step made while it played included.
        fs::write(yard.directory.join("cooked").join("nosave"), "").expect("written");
        yard.rename("kept");
        until(&mut yard, "the game sees the rename", |yard| {
            told(yard, "the well is named kept") == 1
        });
        fs::write(&main, WRAPPER.replace("{}", "fourth")).expect("written");
        until(&mut yard, "a new run on the level as it stands", |yard| {
            running(yard) && told(yard, "the well is named kept") == 2
        });
        assert!(shown(&yard, "starts a new run on the level as it stands"));
        assert_eq!(told(&yard, "the stub resumes its save"), 2);
        assert!(!built(&yard), "marked as built again");
        run(&mut yard, "play.stop");
        until(&mut yard, "the game stops", |yard| {
            yard.shared.borrow().play == Playing::Stopped
        });
        drop(yard);
        let _ = fs::remove_dir_all(&crate_directory);
    }

    /// The hierarchy shows the level's tree, folds and unfolds, selects with a click, with Ctrl
    /// and with Shift, and with the arrows, shows what is selected elsewhere, searches once the
    /// typing paused, and renames in place (`docs/adr/0167-the-hierarchy.md`).
    #[test]
    fn the_hierarchy_folds_selects_shows_searches_and_renames() {
        use uwui::tree::Role;
        let mut yard = Yard::new("hierarchy tree");
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Step {
                    label: "Grow".to_owned(),
                    commands: vec![
                        "[Create]\nentity = \"yard/lamp-a1b2\"\nparent = \"yard\"\n\n[Create.components]\nName = \"lamp\"\n".to_owned(),
                        "[Create]\nentity = \"yard/post-c3d4\"\nparent = \"yard/lamp-a1b2\"\n\n[Create.components]\nName = \"post\"\n".to_owned(),
                    ],
                    selection: None,
                },
            )
            .expect("made");
        yard.frame();
        yard.frame();
        let items = |yard: &Yard| {
            yard.model
                .ui
                .meaning()
                .into_iter()
                .filter(|meaning| meaning.role == Role::TreeItem)
                .map(|meaning| (meaning.name, meaning.level))
                .collect::<Vec<_>>()
        };
        let found = |yard: &Yard, name: &str, end: &str| {
            let meaning = yard.model.ui.meaning();
            let item = meaning
                .iter()
                .find(|meaning| meaning.role == Role::TreeItem && meaning.name == name)
                .unwrap_or_else(|| panic!("the row of {name}"))
                .path
                .clone();
            meaning
                .iter()
                .find(|meaning| meaning.path.starts_with(&item) && meaning.path.ends_with(end))
                .unwrap_or_else(|| panic!("{end} of {name}"))
                .id
        };
        let selected = |yard: &Yard| {
            let world = yard.host.world();
            let mut names: Vec<String> = yard
                .model
                .selected_entities(world)
                .into_iter()
                .filter_map(|entity| {
                    world
                        .get::<slime_world::Name>(entity)
                        .map(|name| name.0.clone())
                })
                .collect();
            names.sort();
            names
        };
        // The root's children show; the lamp is folded.
        let names: Vec<String> = items(&yard).into_iter().map(|(name, _)| name).collect();
        assert!(
            names.contains(&"well".to_owned()) && names.contains(&"lamp".to_owned()),
            "{names:?}"
        );
        assert!(!names.contains(&"post".to_owned()), "folded");
        let unfold = found(&yard, "lamp", "/fold");
        click(&mut yard, unfold);
        yard.frame();
        assert!(
            items(&yard).contains(&("post".to_owned(), Some(3))),
            "{:?}",
            items(&yard)
        );
        // A click, then Ctrl and a click, then Shift and a click.
        let well = found(&yard, "well", "/name");
        click(&mut yard, well);
        yard.frame();
        assert_eq!(selected(&yard), ["well"]);
        let mut held = Modifiers::default();
        held.control = true;
        yard.model.ui.modifiers(held);
        let lamp = found(&yard, "lamp", "/name");
        click(&mut yard, lamp);
        yard.frame();
        assert_eq!(selected(&yard), ["lamp", "well"]);
        let mut held = Modifiers::default();
        held.shift = true;
        yard.model.ui.modifiers(held);
        let post = found(&yard, "post", "/name");
        click(&mut yard, post);
        yard.model.ui.modifiers(Modifiers::default());
        yard.frame();
        assert_eq!(selected(&yard), ["lamp", "post"]);
        // The arrows: the row above the last selected.
        run(&mut yard, "hierarchy.previous");
        assert_eq!(selected(&yard), ["lamp"]);
        // Folded, then selected elsewhere: unfolded and shown.
        run(&mut yard, "hierarchy.fold");
        assert!(!items(&yard).iter().any(|(name, _)| name == "post"));
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/post-c3d4".parse().expect("an identity")]),
            )
            .expect("selected");
        yard.frame();
        yard.frame();
        assert!(items(&yard).iter().any(|(name, _)| name == "post"), "shown");
        // The search, once the typing paused.
        let meaning = yard.model.ui.meaning();
        let field = meaning
            .iter()
            .find(|meaning| meaning.name == "search the hierarchy")
            .expect("the search")
            .id;
        assert!(yard.model.ui.set_value(field, "po"));
        yard.frame();
        std::thread::sleep(crate::hierarchy::SEARCH_PAUSE + std::time::Duration::from_millis(10));
        yard.frame();
        yard.frame();
        let names: Vec<String> = items(&yard).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["post"]);
        assert!(yard.model.ui.set_value(field, ""));
        yard.frame();
        std::thread::sleep(crate::hierarchy::SEARCH_PAUSE + std::time::Duration::from_millis(10));
        yard.frame();
        yard.frame();
        // Renamed in place: F2's operation, the field given a name.
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/well".parse().expect("an identity")]),
            )
            .expect("selected");
        yard.frame();
        run(&mut yard, "edit.rename");
        yard.frame();
        let meaning = yard.model.ui.meaning();
        let field = meaning
            .iter()
            .find(|meaning| meaning.name == "rename")
            .expect("the field that renames")
            .id;
        assert_eq!(yard.model.ui.focus(), Some(field));
        assert!(yard.model.ui.set_value(field, "pump"));
        yard.frame();
        yard.frame();
        assert_eq!(
            yard.history().0.last().map(String::as_str),
            Some("Rename to pump")
        );
        assert!(items(&yard).iter().any(|(name, _)| name == "pump"));
    }

    /// The hierarchy's rows, holding the focus, take the arrows and let every other shortcut
    /// through; a rename given up by a click elsewhere, or by Escape, ends; a level opened in place
    /// of the other shows its own rows (`docs/adr/0167-the-hierarchy.md`).
    #[test]
    fn the_hierarchy_takes_its_keys_lets_the_others_through_and_follows_a_level_opened() {
        use uwui::tree::Role;
        let mut yard = Yard::new("hierarchy keys");
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Step {
                    label: "Grow".to_owned(),
                    commands: vec![
                        "[Create]\nentity = \"yard/lamp-a1b2\"\nparent = \"yard\"\n\n[Create.components]\nName = \"lamp\"\n".to_owned(),
                    ],
                    selection: None,
                },
            )
            .expect("made");
        yard.frame();
        yard.frame();
        let row = |yard: &Yard, name: &str| {
            let meaning = yard.model.ui.meaning();
            let item = meaning
                .iter()
                .find(|meaning| meaning.role == Role::TreeItem && meaning.name == name)
                .unwrap_or_else(|| panic!("the row of {name}"))
                .path
                .clone();
            meaning
                .iter()
                .find(|meaning| meaning.path.starts_with(&item) && meaning.path.ends_with("/name"))
                .unwrap_or_else(|| panic!("the name of {name}"))
                .id
        };
        let selected = |yard: &Yard| {
            let world = yard.host.world();
            yard.model
                .selected_entities(world)
                .into_iter()
                .filter_map(|entity| {
                    world
                        .get::<slime_world::Name>(entity)
                        .map(|name| name.0.clone())
                })
                .collect::<Vec<_>>()
        };
        let well = row(&yard, "well");
        click(&mut yard, well);
        yard.frame();
        assert_eq!(selected(&yard), ["well"]);
        // The arrow, the row's focus taking it; the focus follows the selection. The key's
        // operation runs at the window's next frame, and the selection that it asks at the next.
        yard.key(Key::Named(NamedKey::ArrowDown), false);
        for _ in 0..3 {
            yard.frame();
        }
        assert_eq!(selected(&yard), ["lamp"]);
        assert_eq!(yard.model.ui.focus(), Some(row(&yard, "lamp")));
        // Another shortcut, the row holding the focus: through to the window.
        yard.key(Key::Character("z".to_owned()), true);
        yard.frame();
        assert_eq!(yard.history().1, ["Grow"], "undone");
        yard.key(Key::Character("z".to_owned()), false);
        yard.frame();
        // F2, then a click elsewhere: the rename given up, the focus left where the click put it.
        let well = row(&yard, "well");
        click(&mut yard, well);
        yard.key(Key::Named(NamedKey::F2), false);
        yard.frame();
        yard.frame();
        let rename = |yard: &Yard| {
            yard.model
                .ui
                .meaning()
                .iter()
                .find(|meaning| meaning.name == "rename")
                .map(|meaning| meaning.id)
        };
        assert!(rename(&yard).is_some(), "renaming");
        let well = row(&yard, "yard");
        click(&mut yard, well);
        yard.frame();
        assert!(rename(&yard).is_none(), "given up");
        let focus = yard.model.ui.focus();
        yard.frame();
        assert_eq!(yard.model.ui.focus(), focus, "the focus stays");
        // F2, then Escape.
        yard.key(Key::Named(NamedKey::F2), false);
        yard.frame();
        yard.frame();
        assert!(rename(&yard).is_some());
        yard.key(Key::Named(NamedKey::Escape), false);
        yard.frame();
        assert!(rename(&yard).is_none(), "given up by Escape");
        assert_eq!(yard.history().0, Vec::<String>::new(), "nothing renamed");
        run(&mut yard, "edit.redo");
        yard.frame();
        assert!(
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name == "lamp")
        );
        // Another level, opened in place of the yard, under the same key: its rows, without the
        // yard's lamp, which select.
        let garden = yard.directory.join("garden.scene");
        run_with(&mut yard, "file.open", &[garden.to_str().expect("a path")]);
        yard.frame();
        yard.frame();
        let names: Vec<String> = yard
            .model
            .ui
            .meaning()
            .into_iter()
            .filter(|meaning| meaning.role == Role::TreeItem)
            .map(|meaning| meaning.name)
            .collect();
        assert!(names.contains(&"well".to_owned()), "{names:?}");
        assert!(!names.contains(&"lamp".to_owned()), "{names:?}");
        let well = row(&yard, "well");
        click(&mut yard, well);
        yard.frame();
        assert_eq!(selected(&yard), ["well"]);
    }

    /// A row of the hierarchy dragged onto another goes before it, into it or after it, as the
    /// line or the box under the pointer shows, where it was in the world; a drag onto its own
    /// child shows nothing and makes nothing; Alt and the arrows move what is selected, the focus
    /// following its row (`docs/adr/0168-rearranging-the-hierarchy.md`).
    #[test]
    fn the_hierarchy_rearranges_by_drags_and_keys() {
        use uwui::tree::{Key as Part, Role};
        let mut yard = Yard::new("hierarchy drags");
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Step {
                    label: "Grow".to_owned(),
                    commands: vec![
                        "[Create]\nentity = \"yard/lamp-a1b2\"\nparent = \"yard\"\n\n[Create.components]\nName = \"lamp\"\nTransform = { translation = { x = 2.0, y = 0.0, z = 0.0 } }\n".to_owned(),
                        "[Create]\nentity = \"yard/post-c3d4\"\nparent = \"yard\"\n\n[Create.components]\nName = \"post\"\nTransform = { translation = { x = 0.0, y = 1.0, z = 0.0 } }\n".to_owned(),
                    ],
                    selection: None,
                },
            )
            .expect("made");
        yard.frame();
        yard.frame();
        let item = |yard: &Yard, name: &str| {
            yard.model
                .ui
                .meaning()
                .iter()
                .find(|meaning| meaning.role == Role::TreeItem && meaning.name == name)
                .unwrap_or_else(|| panic!("the row of {name}"))
                .id
        };
        let name = |yard: &Yard, row: &str| {
            let item = item(yard, row);
            let path = [Part::from("content"), Part::from("name")];
            yard.model.ui.find(item, &path).expect("its name")
        };
        let children = |yard: &Yard, parent: &str| {
            let world = yard.host.world();
            let parent = world
                .find(&parent.parse().expect("an identity"))
                .expect("alive");
            world
                .children(parent)
                .filter_map(|child| world.get::<slime_world::Name>(child))
                .map(|name| name.0.clone())
                .collect::<Vec<_>>()
        };
        // Pressed on a row's name, moved past a few pixels onto `at` of another's height.
        let drag = |yard: &mut Yard, from: ElementId, onto: ElementId, at: f32| {
            let from = yard.model.ui.rect(from).expect("laid out");
            let onto = yard.model.ui.rect(onto).expect("laid out");
            let ui = &mut yard.model.ui;
            ui.pointer_moved(from.x + from.width / 2.0, from.y + from.height / 2.0);
            ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
            ui.pointer_moved(onto.x + onto.width / 2.0, onto.y + onto.height * at);
            yard.frame();
        };
        let release = |yard: &mut Yard| {
            yard.model
                .ui
                .pointer_button(MouseButton::Left, ButtonState::Released);
            yard.frame();
            yard.frame();
        };
        let shows = |yard: &Yard, row: &str, part: &str| {
            yard.model
                .ui
                .find(item(yard, row), &[Part::from(part)])
                .is_some()
        };
        assert_eq!(children(&yard, "yard"), ["well", "lamp", "post"]);
        // Onto the lamp's top quarter: a line above it, then the post before it.
        let (post, lamp) = (name(&yard, "post"), item(&yard, "lamp"));
        drag(&mut yard, post, lamp, 0.1);
        assert!(shows(&yard, "lamp", "before"), "a line above the lamp");
        release(&mut yard);
        assert!(!shows(&yard, "lamp", "before"), "the line gone");
        assert_eq!(children(&yard, "yard"), ["well", "post", "lamp"]);
        assert_eq!(
            yard.history().0.last().map(String::as_str),
            Some("Move in the hierarchy")
        );
        // Onto its middle: into it, unfolded, where it was in the world.
        let (post, lamp) = (name(&yard, "post"), item(&yard, "lamp"));
        drag(&mut yard, post, lamp, 0.5);
        release(&mut yard);
        assert_eq!(children(&yard, "yard/lamp-a1b2"), ["post"]);
        let at = placed(&yard, &"yard/post-c3d4".parse().expect("an identity"));
        assert!(
            (at.translation - DVec3::Y).length() < 1e-6,
            "kept where it was: {at:?}"
        );
        assert!(
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.role == Role::TreeItem && meaning.name == "post"),
            "the lamp unfolded"
        );
        // The lamp onto its own child: nothing shows, nothing moves.
        let steps = yard.history().0.len();
        let (lamp, post) = (name(&yard, "lamp"), item(&yard, "post"));
        drag(&mut yard, lamp, post, 0.5);
        assert!(
            !shows(&yard, "post", "before") && !shows(&yard, "post", "after"),
            "no line"
        );
        release(&mut yard);
        assert_eq!(yard.history().0.len(), steps);
        // Alt and the left arrow, the post's row holding the focus: beside the lamp, after it,
        // the focus on its row; Alt and the up arrow: before it.
        let post = name(&yard, "post");
        click(&mut yard, post);
        let alt = |yard: &mut Yard, arrow: NamedKey| {
            let mut modifiers = Modifiers::default();
            modifiers.alt = true;
            yard.model.ui.modifiers(modifiers);
            yard.model.ui.key(&KeyEvent {
                physical: KeyCode::Unidentified,
                logical: Key::Named(arrow),
                text: None,
                state: ButtonState::Pressed,
                repeat: false,
            });
            yard.model.ui.modifiers(Modifiers::default());
            for _ in 0..3 {
                yard.frame();
            }
        };
        alt(&mut yard, NamedKey::ArrowLeft);
        assert_eq!(children(&yard, "yard"), ["well", "lamp", "post"]);
        assert_eq!(
            yard.model.ui.focus(),
            Some(name(&yard, "post")),
            "the focus follows"
        );
        alt(&mut yard, NamedKey::ArrowUp);
        assert_eq!(children(&yard, "yard"), ["well", "post", "lamp"]);
        alt(&mut yard, NamedKey::ArrowRight);
        assert_eq!(children(&yard, "yard/well"), ["post"], "into the well");
        assert_eq!(
            yard.history().0[steps..],
            [
                "Move out of the parent",
                "Move up among the siblings",
                "Move into the sibling above"
            ]
        );
    }

    /// The eye and the lock of a row, pressed, hide and lock its entity and those below it, which
    /// the viewport's pickings leave out, the row offering to show or unlock them, a step made of
    /// none; another level opened forgets them (`docs/adr/0169-the-eye-and-the-lock.md`).
    #[test]
    fn the_eye_and_the_lock_of_a_row_leave_out_of_the_pickings_what_they_cover() {
        use uwui::tree::Role;
        let mut yard = Yard::new("eyes");
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Step {
                    label: "Grow".to_owned(),
                    commands: vec![
                        "[Create]\nentity = \"yard/lamp-a1b2\"\nparent = \"yard\"\n\n[Create.components]\nName = \"lamp\"\n".to_owned(),
                        "[Create]\nentity = \"yard/post-c3d4\"\nparent = \"yard/lamp-a1b2\"\n\n[Create.components]\nName = \"post\"\n".to_owned(),
                    ],
                    selection: None,
                },
            )
            .expect("made");
        yard.frame();
        yard.frame();
        // The part of the row of `name` whose key ends its path, and the part's name.
        let part = |yard: &Yard, name: &str, end: &str| {
            let meaning = yard.model.ui.meaning();
            let item = meaning
                .iter()
                .find(|meaning| meaning.role == Role::TreeItem && meaning.name == name)
                .unwrap_or_else(|| panic!("the row of {name}"))
                .path
                .clone();
            meaning
                .into_iter()
                .find(|meaning| meaning.path.starts_with(&item) && meaning.path.ends_with(end))
                .map(|meaning| (meaning.id, meaning.name))
                .unwrap_or_else(|| panic!("{end} of {name}"))
        };
        let entity = |yard: &Yard, id: &str| {
            let id: PersistentId = id.parse().expect("an identity");
            yard.host.world().find(&id).expect("there")
        };
        // Those of the level's named entities that may be picked, by their names.
        let pickable = |yard: &Yard| {
            let world = yard.host.world();
            let named: Vec<Entity> = world
                .query::<&slime_world::Name>()
                .map(|(entity, _)| entity)
                .collect();
            let mut names: Vec<String> = yard
                .model
                .pickable(&named)
                .into_iter()
                .filter_map(|entity| world.get::<slime_world::Name>(entity))
                .map(|name| name.0.clone())
                .collect();
            names.sort();
            names
        };
        assert_eq!(pickable(&yard), ["lamp", "post", "well", "yard"]);
        let made = yard.model.marks(0, yard.host.world(), 1.0).eyes;
        let steps = yard.history().0.len();
        // The lamp's eye: the lamp and the post below it hidden.
        let (eye, name) = part(&yard, "lamp", "/eye");
        assert_eq!(name, "hide");
        click(&mut yard, eye);
        yard.frame();
        assert_eq!(pickable(&yard), ["well", "yard"]);
        assert_eq!(part(&yard, "lamp", "/eye").1, "show");
        assert!(yard.model.marks(0, yard.host.world(), 1.0).eyes > made);
        assert_eq!(yard.history().0.len(), steps, "no step");
        assert!(
            yard.shared.borrow().selection.is_empty(),
            "nothing selected"
        );
        // What is created below it is hidden too; deleted, then restored under its identity, it
        // is hidden still.
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Step {
                    label: "Grow a wick".to_owned(),
                    commands: vec![
                        "[Create]\nentity = \"yard/wick-e5f6\"\nparent = \"yard/post-c3d4\"\n\n[Create.components]\nName = \"wick\"\n".to_owned(),
                    ],
                    selection: None,
                },
            )
            .expect("made");
        yard.frame();
        assert_eq!(pickable(&yard), ["well", "yard"]);
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/lamp-a1b2".parse().expect("an identity")]),
            )
            .expect("selected");
        run(&mut yard, "edit.delete");
        assert_eq!(pickable(&yard), ["well", "yard"]);
        run(&mut yard, "edit.undo");
        yard.frame();
        assert_eq!(pickable(&yard), ["well", "yard"]);
        assert_eq!(part(&yard, "lamp", "/eye").1, "show");
        run(&mut yard, "edit.select_none");
        // The well's lock: picked by none of the viewport's ways.
        let (lock, name) = part(&yard, "well", "/lock");
        assert_eq!(name, "lock");
        click(&mut yard, lock);
        yard.frame();
        assert_eq!(pickable(&yard), ["yard"]);
        assert_eq!(part(&yard, "well", "/lock").1, "unlock");
        let click_at = Wanted::Click {
            at: [100.0, 80.0],
            adding: false,
        };
        let well = entity(&yard, "yard/well");
        let lamp = entity(&yard, "yard/lamp-a1b2");
        yard.model
            .picked(yard.host.world(), &click_at, &[well, lamp]);
        yard.frame();
        assert!(yard.shared.borrow().selection.is_empty(), "nothing picked");
        // Shown again.
        let (eye, _) = part(&yard, "lamp", "/eye");
        click(&mut yard, eye);
        yard.frame();
        assert_eq!(pickable(&yard), ["lamp", "post", "wick", "yard"]);
        // Another level, opened under the same key: its well, of the same identity, not locked.
        let garden = yard.directory.join("garden.scene");
        run_with(&mut yard, "file.open", &[garden.to_str().expect("a path")]);
        yard.frame();
        yard.frame();
        let well = entity(&yard, "yard/well");
        assert_eq!(yard.model.pickable(&[well]), [well]);
        assert_eq!(part(&yard, "well", "/lock").1, "lock");
    }

    /// The panels in regions of tabs: a tab chosen shows its panel, a border given a value moves,
    /// and a panel moved by its tab's context menu and by `view.move_panel` shows where it went, a
    /// panel or an edge that is no such refused (`docs/adr/0172-regions-of-tabs.md`).
    #[test]
    fn the_panels_lie_in_regions_of_tabs_that_the_user_rearranges() {
        use uwui::tree::Role;
        let mut yard = Yard::new("regions");
        let found = |yard: &Yard, role: Role, name: &str| {
            yard.model
                .ui
                .meaning()
                .into_iter()
                .find(|meaning| meaning.role == role && meaning.name == name)
                .map(|meaning| meaning.id)
        };
        // The console's tab chosen: its panel, waiting for Play, in place of the history's.
        assert!(yard.model.history_slot.is_some());
        let console = found(&yard, Role::Tab, "Console").expect("its tab");
        click(&mut yard, console);
        yard.frame();
        assert!(yard.model.history_slot.is_none(), "behind its tab");
        assert!(found(&yard, Role::Label, "Play has not run yet.").is_some());
        // The border between the left column and the rest, given a value as an agent gives it.
        let width = |yard: &Yard| {
            let panel = found(yard, Role::TabPanel, "Hierarchy").expect("its panel");
            yard.model.ui.rect(panel).map_or(0.0, |rect| rect.width)
        };
        let before = width(&yard);
        let border = yard
            .model
            .ui
            .meaning()
            .into_iter()
            .filter(|meaning| meaning.role == Role::Splitter)
            .max_by(|a, b| {
                let height = |id| yard.model.ui.rect(id).map_or(0.0, |rect| rect.height);
                height(a.id).total_cmp(&height(b.id))
            })
            .expect("a border")
            .id;
        assert!(yard.model.ui.set_value(border, "0.4"));
        yard.frame();
        yard.frame();
        assert!(width(&yard) > before + 100.0, "{before} {}", width(&yard));
        // The history's tab, its context menu: Move to, the window's right edge.
        let history = found(&yard, Role::Tab, "History").expect("its tab");
        yard.model.ui.act(Action {
            element: Some(history),
            kind: ActionKind::ContextMenu { at: [0.0, 0.0] },
        });
        yard.frame();
        let move_to = found(&yard, Role::MenuItem, "Move to").expect("in its context menu");
        click(&mut yard, move_to);
        // The other stacks, by their panels' titles, then the window's edges.
        let places: Vec<String> = yard
            .model
            .ui
            .meaning()
            .into_iter()
            .filter(|meaning| meaning.role == Role::MenuItem && meaning.path.starts_with("move to"))
            .map(|meaning| meaning.name)
            .collect();
        assert_eq!(
            places,
            [
                "Hierarchy",
                "Components",
                "Viewport",
                "The window's left edge",
                "The window's right edge",
                "The window's top",
                "The window's bottom",
                "A floating panel"
            ]
        );
        let edge = found(&yard, Role::MenuItem, "The window's right edge").expect("a place");
        click(&mut yard, edge);
        yard.frame();
        let stacks = |yard: &Yard| -> Vec<Vec<String>> {
            yard.model
                .dock
                .layout()
                .stacks()
                .into_iter()
                .map(<[String]>::to_vec)
                .collect()
        };
        assert_eq!(stacks(&yard).last(), Some(&vec!["history".to_owned()]));
        assert!(yard.model.history_slot.is_some(), "shown where it went");
        // By its operation, as an agent moves it: into the hierarchy's stack.
        run_with(&mut yard, "view.move_panel", &["components", "hierarchy"]);
        yard.frame();
        assert_eq!(stacks(&yard)[0], ["hierarchy", "components"]);
        assert!(yard.model.slot.is_some(), "the components shown");
        // Into the stack that holds it already: nothing moves, and the user is told.
        run_with(&mut yard, "view.move_panel", &["components", "hierarchy"]);
        yard.frame();
        assert_eq!(stacks(&yard)[0], ["hierarchy", "components"]);
        assert!(
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.contains("lies with `hierarchy` already")),
            "told in a notification"
        );
        // F2 shows the hierarchy, behind the components' tab, where it renames.
        yard.session
            .handle(
                yard.host.world_mut(),
                Request::Select(vec!["yard/well".parse().expect("an identity")]),
            )
            .expect("selected");
        run(&mut yard, "edit.rename");
        yard.frame();
        assert!(yard.model.hierarchy_slot.is_some(), "its tab chosen");
        for arguments in [["map", "left"], ["history", "map"], ["history", "history"]] {
            let refused = yard.session.handle(
                yard.host.world_mut(),
                Request::Run {
                    name: "view.move_panel".to_owned(),
                    arguments: arguments.map(str::to_owned).to_vec(),
                },
            );
            assert!(refused.is_err(), "{arguments:?}");
        }
    }

    /// The layout kept in the user's state directory, written once it stood still, which the next
    /// start shows; a panel floated, closed and opened again where it was, by the operations and by
    /// the menus; a layout that cannot be read set aside, the default shown and the user told; and
    /// *Reset layout* (`docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`).
    #[test]
    fn the_layout_is_kept_and_its_panels_float_close_and_come_back() {
        use uwui::tree::Role;
        let mut yard = Yard::new("layout");
        let file = yard.directory.join("state").join("editor.layout");
        yard.shared.borrow_mut().layout = Some(file.clone());
        // A start that finds no layout kept: the default, nothing told.
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        let (default, _) = panels::default(None);
        assert_eq!(yard.model.dock.layout(), &default.layout);
        assert!(!file.exists(), "nothing written until it changes");
        let found = |yard: &Yard, role: Role, name: &str| {
            yard.model
                .ui
                .meaning()
                .into_iter()
                .find(|meaning| meaning.role == role && meaning.name == name)
                .map(|meaning| meaning.id)
        };
        let path = |yard: &Yard, role: Role, name: &str| {
            found(yard, role, name).and_then(|id| yard.model.ui.path_text(id))
        };
        let kept = |file: &Path| panels::kept(file, &panels::default(None).0).layout;
        // The history floated, as an agent floats it: in a floating panel, its rows built there;
        // written once the layout stood still.
        run_with(&mut yard, "view.move_panel", &["history", "float"]);
        let floating = found(&yard, Role::Pane, "History").expect("a floating panel");
        assert!(yard.model.history_slot.is_some(), "its rows afloat");
        assert!(found(&yard, Role::Tab, "History").is_none());
        let due = yard.model.layout_waits().expect("to be written");
        assert!(!file.exists(), "not while it may change again");
        yard.model.keep_layout_by(due);
        assert_eq!(&kept(&file), yard.model.dock.layout());
        assert_eq!(yard.model.layout_waits(), None, "nothing more to wake for");
        // Its bar's context menu has no floating panel to move to: Move to, Viewport.
        yard.model.ui.act(Action {
            element: Some(floating),
            kind: ActionKind::ContextMenu { at: [400.0, 300.0] },
        });
        yard.frame();
        let move_to = found(&yard, Role::MenuItem, "Move to").expect("in its context menu");
        click(&mut yard, move_to);
        assert!(found(&yard, Role::MenuItem, "A floating panel").is_none());
        let viewport = found(&yard, Role::MenuItem, "Viewport").expect("a stack");
        click(&mut yard, viewport);
        yard.frame();
        assert!(
            found(&yard, Role::Pane, "History").is_none(),
            "docked again"
        );
        assert!(found(&yard, Role::Tab, "History").is_some());
        // The console closed by its tab's context menu: no tab, its place kept.
        let stacks = yard.model.dock.layout().stacks().len();
        let console = found(&yard, Role::Tab, "Console").expect("its tab");
        let was = yard.model.ui.path_text(console);
        yard.model.ui.act(Action {
            element: Some(console),
            kind: ActionKind::ContextMenu { at: [0.0, 0.0] },
        });
        yard.frame();
        let close = found(&yard, Role::MenuItem, "Close").expect("in its context menu");
        click(&mut yard, close);
        yard.frame();
        assert!(found(&yard, Role::Tab, "Console").is_none());
        assert!(!yard.model.dock.layout().is_open("console"));
        assert_eq!(
            yard.model.dock.layout().stacks().len(),
            stacks,
            "its stack kept"
        );
        // View > Panels > Console: it opens where it was, its tab shown.
        let view = yard
            .model
            .bar
            .iter()
            .find(|&&(_, menu)| MENUS[menu].0 == "View")
            .map(|&(item, _)| item)
            .expect("View");
        click(&mut yard, view);
        let panels = found(&yard, Role::MenuItem, "Panels").expect("in View");
        click(&mut yard, panels);
        let item = found(&yard, Role::MenuItem, "Console").expect("in Panels");
        click(&mut yard, item);
        yard.frame();
        assert_eq!(path(&yard, Role::Tab, "Console"), was, "where it was");
        // The viewport floated keeps its surface; the components closed by their operation.
        run_with(&mut yard, "view.move_panel", &["viewport", "float"]);
        assert!(yard.model.panes[0].surface.is_some(), "a surface afloat");
        let components = path(&yard, Role::Tab, "Components");
        run_with(&mut yard, "view.close_panel", &["components"]);
        assert!(yard.model.slot.is_none());
        let arranged = yard.model.dock.layout().clone();
        // The next start shows it as it was left, the window that closed having written it.
        drop(std::mem::replace(
            &mut yard.model,
            Model::new(Shared::default()),
        ));
        assert_eq!(kept(&file), arranged);
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        yard.frame();
        assert_eq!(yard.model.dock.layout(), &arranged);
        assert!(
            yard.model.panes[0].surface.is_some(),
            "the viewport afloat again"
        );
        // Opened again where it was.
        run_with(&mut yard, "view.open_panel", &["components"]);
        assert!(yard.model.slot.is_some());
        assert_eq!(path(&yard, Role::Tab, "Components"), components);
        // Reset layout: the default, written.
        run(&mut yard, "view.reset_layout");
        assert_eq!(yard.model.dock.layout(), &default.layout);
        assert!(found(&yard, Role::Pane, "Viewport").is_none(), "docked");
        yard.model.keep_layout_by(Instant::now() + KEEP_AFTER);
        assert_eq!(kept(&file), default.layout);
        // A file that cannot be read: set aside, the default shown, the user told.
        std::fs::write(&file, "default = [").expect("written");
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        assert_eq!(yard.model.dock.layout(), &default.layout);
        let unreadable = yard
            .directory
            .join("state")
            .join("editor.layout.unreadable");
        assert!(unreadable.is_file());
        assert!(
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.contains("editor.layout.unreadable")),
            "told in a notification"
        );
        // Refused by the core: a panel that the window does not have.
        for (name, arguments) in [
            ("view.close_panel", vec!["map"]),
            ("view.open_panel", vec!["map"]),
            ("view.move_panel", vec!["map", "float"]),
        ] {
            let refused = yard.session.handle(
                yard.host.world_mut(),
                Request::Run {
                    name: name.to_owned(),
                    arguments: arguments.into_iter().map(str::to_owned).collect(),
                },
            );
            assert!(refused.is_err(), "{name}");
        }
    }

    /// A project's default layout, which a game's editor package gives, shows in place of the
    /// editor's and comes back by *Reset layout*; one that cannot be read leaves the layout kept
    /// against it alone; a layout that cannot be written is told once
    /// (`docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`).
    #[test]
    fn a_project_s_default_shows_and_a_layout_unkept_is_told_once() {
        let mut yard = Yard::new("project-layout");
        let file = yard.directory.join("state").join("editor.layout");
        let mut project = panels::default(None).0;
        assert!(
            project
                .layout
                .move_panel("history", &Target::Edge(uwui::dock::Edge::Left))
        );
        project.default = "garden 1".to_owned();
        let text: &'static str = Box::leak(
            slime_serialize::write_as(&project)
                .expect("written")
                .into_boxed_str(),
        );
        yard.shared.borrow_mut().default_layout = Some(text);
        yard.shared.borrow_mut().layout = Some(file.clone());
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        assert_eq!(yard.model.dock.layout(), &project.layout);
        run_with(&mut yard, "view.close_panel", &["history"]);
        run(&mut yard, "view.reset_layout");
        assert_eq!(yard.model.dock.layout(), &project.layout);
        yard.model.keep_layout_by(Instant::now() + KEEP_AFTER);
        let written = std::fs::read_to_string(&file).expect("written");
        assert!(written.contains("default = \"garden 1\""), "{written}");
        // The project's default unreadable: the editor's shows, and the layout kept against the
        // project's is neither set aside nor written over.
        yard.shared.borrow_mut().default_layout = Some("default = [");
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        assert_eq!(yard.model.dock.layout(), &panels::default(None).0.layout);
        run_with(&mut yard, "view.close_panel", &["console"]);
        drop(std::mem::replace(
            &mut yard.model,
            Model::new(Shared::default()),
        ));
        assert_eq!(std::fs::read_to_string(&file).expect("kept"), written);
    }

    /// A layout that cannot be written, its directory taking no new file, is told once, however
    /// often it settles.
    #[cfg(unix)]
    #[test]
    fn a_layout_that_cannot_be_kept_is_told_once() {
        use std::os::unix::fs::PermissionsExt as _;
        let mut yard = Yard::new("layout-unkept");
        let state = yard.directory.join("state");
        fs::create_dir_all(&state).expect("a directory");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o555)).expect("locked");
        // Root, whom permissions do not hold back, has nothing to check here.
        if fs::write(state.join("probe"), "").is_ok() {
            fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).expect("open");
            return;
        }
        yard.shared.borrow_mut().layout = Some(state.join("editor.layout"));
        yard.model = Model::new(yard.shared.clone());
        yard.frame();
        for panel in ["console", "history"] {
            run_with(&mut yard, "view.close_panel", &[panel]);
            yard.model.keep_layout_by(Instant::now() + KEEP_AFTER);
            yard.frame();
        }
        fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).expect("open");
        let told = yard
            .model
            .ui
            .meaning()
            .iter()
            .filter(|meaning| meaning.name.contains("not kept for the next start"))
            .count();
        assert_eq!(told, 1);
    }

    /// A level flies at first at a speed that suits its size, or at the one last used in it, which
    /// the status bar says while the viewport flies and the user's state keeps once it stops.
    #[test]
    fn the_flying_speed_suits_the_level_shows_and_is_kept() {
        let mut yard = Yard::new("flying");
        let speeds = yard.directory.join("state").join("speeds");
        yard.shared.borrow_mut().speeds = Some(speeds.clone());
        // Framed as it opens: a speed of its size.
        yard.model.framed(0, DVec3::ZERO, 400.0, 1.6);
        let first = crate::navigation::level_speed(400.0);
        assert_eq!(yard.model.panes[0].navigation.speed(), first);
        // Flown, the wheel turned: the status bar says it, the state keeps it once it stops.
        let rect = yard.model.viewport_rect(0).expect("laid out");
        let shown = |yard: &Yard| {
            yard.model
                .ui
                .meaning()
                .iter()
                .any(|meaning| meaning.name.starts_with("flying at"))
        };
        yard.model
            .ui
            .pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        yard.model
            .ui
            .pointer_button(MouseButton::Right, ButtonState::Pressed);
        yard.model
            .ui
            .wheel(slime_platform::ScrollDelta::Lines { x: 0.0, y: 2.0 });
        yard.frame();
        yard.frame();
        let flown = yard.model.panes[0].navigation.speed();
        assert!(flown > first, "{flown}");
        assert!(shown(&yard), "the status bar says the speed");
        yard.model
            .ui
            .pointer_button(MouseButton::Right, ButtonState::Released);
        yard.frame();
        yard.frame();
        assert!(!shown(&yard));
        let level = yard.shared.borrow().level.clone().expect("a level");
        let kept = speeds::read(&speeds).expect("kept");
        assert_eq!(speeds::of(&kept, &level), Some(flown));
        // The level opened again flies as it was left, whatever its size.
        yard.model.panes[0].speed_wanted = true;
        yard.model.framed(0, DVec3::ZERO, 3.0, 1.6);
        assert_eq!(yard.model.panes[0].navigation.speed(), flown);
    }

    /// Two viewports side by side, each its own camera and settings, the View menu changing the
    /// one last pressed in (`docs/adr/0160-two-viewports-and-blenders-way.md`).
    #[test]
    fn two_viewports_show_the_level_side_by_side_each_as_it_is_set() {
        let mut yard = Yard::new("split");
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
        let whole = yard.model.viewport_rect(0).expect("laid out");
        run(&mut yard, "view.split");
        yard.frame();
        assert_eq!(yard.model.panes.len(), 2);
        let (first, second) = (
            yard.model.viewport_rect(0).expect("laid out"),
            yard.model.viewport_rect(1).expect("laid out"),
        );
        assert!(
            first.x + first.width <= second.x,
            "side by side: {first:?}, {second:?}"
        );
        assert!(first.width < whole.width / 1.9);
        // The second sees as the first did.
        let camera = |yard: &Yard, pane: usize| yard.model.panes[pane].navigation.camera();
        assert_eq!(camera(&yard, 0), camera(&yard, 1));
        // A press in the second makes it the one that the View menu changes; the wheel moves its
        // camera alone.
        yard.model
            .ui
            .pointer_moved(second.x + 20.0, second.y + 20.0);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        yard.model
            .ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        yard.model
            .ui
            .wheel(slime_platform::ScrollDelta::Lines { x: 0.0, y: 1.0 });
        yard.frame();
        assert_eq!(yard.model.active, 1);
        assert_ne!(camera(&yard, 0), camera(&yard, 1));
        run(&mut yard, "view.shade_normals");
        assert_eq!(yard.model.panes[1].settings.debug, DebugMode::Normals);
        assert_eq!(yard.model.panes[0].settings.debug, DebugMode::Off);
        // Split again while the first flies, a key held: the second takes its camera, not its
        // gesture nor its keys, and the first alone holds the cursor.
        run(&mut yard, "view.split");
        let first = yard.model.viewport_rect(0).expect("laid out");
        yard.model.ui.pointer_moved(first.x + 20.0, first.y + 20.0);
        yard.model
            .ui
            .pointer_button(MouseButton::Right, ButtonState::Pressed);
        yard.frame();
        let now = Instant::now();
        assert!(yard.model.panes[0].navigation.key(KeyCode::KeyW, true, now));
        run(&mut yard, "view.split");
        assert_eq!(yard.model.panes.len(), 2);
        assert!(!yard.model.panes[1].navigation.holds_cursor());
        assert!(
            !yard.model.panes[1]
                .navigation
                .advance(now + Duration::from_millis(50))
        );
        yard.model
            .ui
            .pointer_button(MouseButton::Right, ButtonState::Released);
        yard.frame();
        assert!(yard.model.navigated().is_none(), "no gesture is left");
        // One again: the first stays.
        run(&mut yard, "view.split");
        assert_eq!(yard.model.panes.len(), 1);
        assert_eq!(yard.model.active, 0);
        yard.frame();
        let again = yard.model.viewport_rect(0).expect("laid out");
        assert_eq!(again.width, whole.width);
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
        assert_eq!(yard.model.panes[0].settings.debug, DebugMode::Normals);
        run(&mut yard, "view.shade_light");
        assert_eq!(yard.model.panes[0].settings.debug, DebugMode::Light);
        run(&mut yard, "view.shade_lit");
        assert_eq!(yard.model.panes[0].settings.debug, DebugMode::Off);
        run(&mut yard, "view.wireframe");
        assert!(yard.model.panes[0].wireframe);
        assert!(yard.model.marks(0, yard.host.world(), 1.0).wireframe);
        run(&mut yard, "view.wireframe");
        assert!(!yard.model.panes[0].wireframe);
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
        // The hierarchy's arrows select, as they are for.
        for operation in OPERATIONS.iter().filter(|operation| {
            matches!(operation.changes, Changes::Selection | Changes::Window)
                && !operation.name.starts_with("hierarchy.")
        }) {
            yard.session
                .handle(
                    yard.host.world_mut(),
                    Request::Run {
                        name: operation.name.to_owned(),
                        // The one that takes arguments: a panel moved to an edge.
                        arguments: operation
                            .parameters
                            .iter()
                            .zip(["history", "left"])
                            .map(|(_, argument)| argument.to_owned())
                            .collect(),
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
            selection: None,
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
                uwui::tree::Kind::Label(text) if text == TYPE_THE_PATH)
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
                uwui::tree::Kind::Label(text) if text == NO_DIALOG_SHOWED)
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
