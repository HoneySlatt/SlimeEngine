//! The interface of the `widgets` example, which its image tests draw too: a panel of the editor's
//! kind in the editor's look (`docs/adr/0148-widgets.md`), built without a GPU.
//!
//! A bar of three tabs chooses what the panel shows. *Inspector*: two panels side by side and the
//! handle between them, a list of 200 entities in the look's font of fixed width on the left, and
//! on the right, in a box that scrolls, sections that fold, of sliders and check boxes, the last
//! folded. *Console*: two panels one above the other, the lines of a log in a box that scrolls, and
//! check boxes that choose which of its levels show. *About*: the look's two fonts. A bar at the
//! bottom shows what the last action did. The program keeps the values of the sliders and the
//! check boxes, as the user changes them, and gives them back at each build: the interface keeps
//! what the user did as long as the program's values do not change. Which tab is chosen, which
//! section folded, where the handles stand and how far the boxes scroll are the interface's alone.
//!
//! The look's themes stack (`docs/adr/0149-the-looks-foundations.md`): T switches its colors,
//! dark, light and high contrast, D its density, A the accent, a theme of one value stacked after
//! the look's, and M the motion, the transitions or none.
//!
//! The rich widgets (`docs/adr/0151-rich-widgets.md`): a bar of tools under the menus, a control of
//! segments that chooses the tool, a button that toggles the snapping, a bar of progress of a cook
//! that goes on, and one without a share, which moves; a field of search over the list of entities, which keeps the rows whose names
//! hold what it holds; the position, a vector of three fields, each with its axis's prefix, which
//! drags it, in meters, which reads expressions and units, `1m + 20cm`; the console's tab, with
//! the count of its warnings and errors; the light's section, whose header holds its type and a
//! button that resets it; help over the material's properties, a title and a text.
//!
//! What opens over the panel (`docs/adr/0150-layers.md`): a bar of menus above the tabs, *File*,
//! with a submenu of recent levels, and *View*, which shows the floating panel of statistics, and
//! has a submenu of the look's colors; a menu of the entities' rows, at the pointer's second button
//! or Shift+F10; a list to choose the quality of shadows from, in the *Rendering* section; tooltips
//! on the tabs and on that list. The program builds the menus and the panel as their items are
//! pressed, and forgets them as the interface closes them.

use slime_ui::action::{ActionKind, FieldValue, Intent};
use slime_ui::display::DisplayList;
use slime_ui::look::{self, Colors, Density};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Align, Dimension, FlexDirection, Sides, Style};
use slime_ui::text::Fonts;
use slime_ui::theme::Theme;
use slime_ui::tree::{
    Along, Children, Element, ElementId, ItemMark, Layer, LayoutDirection, Place, Role,
    RootSettings, Side, Ui, Units,
};

/// The rows of the list of entities.
const ENTITIES: usize = 200;
/// What the entities of the list are, in turn.
const NAMES: [&str; 8] = [
    "torch", "camera", "sun", "rock", "tree", "chest", "door", "path",
];
/// The levels of the log's lines, which the console's check boxes show or hide.
const LEVELS: [&str; 3] = ["info", "warning", "error"];
/// The log's lines, by their levels, in turn.
const LINES: [(usize, &str); 6] = [
    (0, "loaded level yard.scene: 214 entities"),
    (0, "cooked 3 assets in 41 ms"),
    (
        1,
        "texture bark.png has no metadata file; cooked with defaults",
    ),
    (0, "shader canvas.wgsl compiled"),
    (2, "prefab lamp.scene names no mesh `shade`"),
    (0, "view main: 1280 x 800, 2 shadow cascades"),
];
/// The tabs' titles, and their tooltips.
const TABS: [&str; 3] = ["Inspector", "Console", "About"];
const TAB_TIPS: [&str; 3] = [
    "The entities and their properties",
    "What the engine told, by levels",
    "The look's fonts",
];
/// The qualities of shadows that the list to choose from offers.
const QUALITIES: [&str; 4] = ["Low", "Medium", "High", "Ultra"];
/// The levels that *File*'s submenu offers.
const RECENT: [&str; 3] = ["yard.scene", "lamp.scene", "zone.scene"];
/// The tools that the control of segments chooses from.
const TOOLS: [&str; 3] = ["Move", "Turn", "Scale"];
/// The help of the properties that have some, as a program gives it from the documentation of
/// what a field edits: the property's label and its text.
const HELP: [(&str, &str); 3] = [
    (
        "roughness",
        "How rough the surface is: 0 reflects as a mirror, 1 scatters the light",
    ),
    (
        "metalness",
        "Whether the surface is a metal: 0 for none, 1 for a metal",
    ),
    (
        "emission",
        "The light that the surface gives off, in the color of its emissive texture",
    ),
];
/// The light's values as it starts, which its section's reset gives back.
const LIGHT: [f64; 2] = [2.5, 12.0];
/// The axes of the position, their prefixes and their classes.
const AXES: [(&str, &str); 3] = [("X", "axis-x"), ("Y", "axis-y"), ("Z", "axis-z")];
/// The accents that A goes through, by their names: the look's own first.
const ACCENTS: [(&str, &str); 4] = [
    ("blue", "#4C8DFF"),
    ("green", "#2BB673"),
    ("violet", "#9B7BFF"),
    ("amber", "#E8A33D"),
];
/// The keys that change the look, as a table of shortcuts names them.
const KEYS: &str = r#"
[global]
T = "switch-colors"
D = "switch-density"
A = "switch-accent"
M = "switch-motion"
"Ctrl+N" = "new-level"
"Ctrl+O" = "open"
"Ctrl+Q" = "quit"
"#;

/// A slider of the inspector: its label, its value, its bounds and its step.
struct Slider {
    label: &'static str,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
}

/// A section of the inspector: its title, whether it opens folded, its sliders and its check boxes.
struct Section {
    title: &'static str,
    folded: bool,
    sliders: Vec<Slider>,
    checks: Vec<(&'static str, bool)>,
}

/// What a check box, a slider or the list to choose from gives its value to: a property of a
/// section of the inspector, by its section and its place there, a level of the console, or the
/// quality of shadows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Property {
        section: usize,
        index: usize,
    },
    Level(usize),
    Quality,
    /// The tool chosen, the snapping, the field of search, an axis of the position.
    Tool,
    Snap,
    Search,
    Position(usize),
}

/// A menu that the program builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    File,
    View,
    Recent,
    Colors,
    /// The menu of the entity of the list's row.
    Entity(usize),
}

/// What an item of a menu does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    /// The command that a shortcut names too.
    Command(&'static str),
    /// Opens a submenu.
    Submenu(Menu),
    /// Opens a level of the recent ones.
    Recent(&'static str),
    /// Shows or hides the floating panel of statistics.
    Statistics,
    /// Switches the look's colors.
    Colors(Colors),
    /// Renames, duplicates or deletes the entity of a row.
    Entity(&'static str, usize),
}

/// The interface, the values that the program keeps of it, and what its bar shows.
pub struct WidgetsPanel {
    pub ui: Ui,
    root: ElementId,
    direction: LayoutDirection,
    /// The list of entities, while the inspector shows.
    list: Option<ElementId>,
    sections: Vec<Section>,
    /// Which levels of the log show.
    levels: [bool; 3],
    /// The sliders and the check boxes, by their elements, as the last build gave them.
    targets: Vec<(ElementId, Target)>,
    /// What the last action did, and the revision of what the interface shows of the program.
    last: String,
    revision: u64,
    /// The look's colors, density and accent, by its place in [`ACCENTS`].
    colors: Colors,
    density: Density,
    accent: usize,
    /// The quality of shadows, by its place in [`QUALITIES`].
    quality: usize,
    /// The tool chosen, by its place in [`TOOLS`], and whether moves snap.
    tool: usize,
    snap: bool,
    /// What the field of search holds, and the entities whose names hold it, by their numbers.
    query: String,
    matches: Vec<usize>,
    /// The position, by its axes.
    position: [f64; 3],
    /// The items of the bar of menus, by their menus, as the last build gave them.
    bar: Vec<(ElementId, Menu)>,
    /// The layers of the menus open, and their items, by what they do.
    menus: Vec<ElementId>,
    items: Vec<(ElementId, Item)>,
    /// The layer of the floating panel of statistics, while it is open.
    statistics: Option<ElementId>,
    /// The button that resets the light, as the last build gave it.
    reset: Option<ElementId>,
}

impl WidgetsPanel {
    /// The interface, laid out in `direction`. Its text is in the look's fonts alone, which hold
    /// every character that it shows, so that it draws the same on every machine.
    pub fn new(direction: LayoutDirection) -> Self {
        let mut fonts = Fonts::new();
        look::register_fonts(&mut fonts);
        let mut ui = Ui::new(fonts);
        ui.set_shortcuts(&[Shortcuts::from_text("widgets.keys", KEYS)])
            .expect("the example's shortcuts are valid");
        let root = ui.add_root(settings([1.0, 1.0], 1.0, direction), column());
        let slider = |label, value, min, max, step| Slider {
            label,
            value,
            min,
            max,
            step,
        };
        let sections = vec![
            Section {
                title: "Transform",
                folded: false,
                sliders: vec![
                    slider("turn", 90.0, 0.0, 360.0, 15.0),
                    slider("scale", 1.0, 0.1, 4.0, 0.1),
                ],
                checks: Vec::new(),
            },
            Section {
                title: "Rendering",
                folded: false,
                sliders: Vec::new(),
                checks: vec![
                    ("visible", true),
                    ("casts shadows", true),
                    ("receives shadows", false),
                    ("drawn in reflections", true),
                ],
            },
            Section {
                title: "Material",
                folded: false,
                sliders: vec![
                    slider("roughness", 0.6, 0.0, 1.0, 0.05),
                    slider("metalness", 0.0, 0.0, 1.0, 0.05),
                    slider("emission", 0.0, 0.0, 8.0, 0.1),
                ],
                checks: vec![("two-sided", false)],
            },
            Section {
                title: "Light",
                folded: false,
                sliders: vec![
                    slider("intensity", 2.5, 0.0, 10.0, 0.1),
                    slider("range", 12.0, 0.0, 50.0, 1.0),
                ],
                checks: vec![("flickers", false)],
            },
            Section {
                title: "Audio",
                folded: false,
                sliders: vec![slider("volume", 0.8, 0.0, 1.0, 0.05)],
                checks: vec![("loops", true)],
            },
            Section {
                title: "Physics",
                folded: true,
                sliders: vec![slider("mass", 12.0, 0.0, 100.0, 0.5)],
                checks: vec![("kinematic", false)],
            },
        ];
        let mut panel = Self {
            ui,
            root,
            direction,
            list: None,
            sections,
            levels: [true; 3],
            targets: Vec::new(),
            last: "f10 reaches the menus; tab and the arrows move, enter and space press; t, d, a \
                   and m change the look"
                .to_owned(),
            revision: 0,
            colors: Colors::Dark,
            density: Density::Comfortable,
            accent: 0,
            quality: 2,
            tool: 0,
            snap: true,
            query: String::new(),
            matches: (0..ENTITIES).collect(),
            position: [1.5, 0.0, -4.2],
            bar: Vec::new(),
            menus: Vec::new(),
            items: Vec::new(),
            statistics: None,
            reset: None,
        };
        panel.style();
        panel
    }

    /// Shows the look in `colors` and `density`, as T and D switch them.
    pub fn set_look(&mut self, colors: Colors, density: Density) {
        self.colors = colors;
        self.density = density;
        self.style();
    }

    /// Styles the interface with the look's sheet and its themes, the accent's stacked after them.
    fn style(&mut self) {
        let mut themes = look::themes(self.colors, self.density);
        let (name, accent) = ACCENTS[self.accent];
        themes.push(Theme::from_text(
            format!("{name}.theme"),
            format!("accent = \"{accent}\"\n"),
        ));
        self.ui
            .set_style(&[look::style_sheet()], &themes)
            .expect("the look's documents are valid");
    }

    /// Does what the actions that the interface queued ask: each value that a slider or a check
    /// box gives kept, and told in the bar once committed. Gives whether the program is asked to
    /// stop: Escape, nothing focused.
    pub fn handle_actions(&mut self) -> bool {
        let mut stop = false;
        for action in self.ui.take_actions() {
            // What opens over the panel: a menu of the bar or of an entity, what an item of a menu
            // does, and a layer that the interface closed, forgotten.
            if let Some(id) = action.element {
                let bar = self
                    .bar
                    .iter()
                    .find(|&&(item, _)| item == id)
                    .map(|&(_, menu)| menu);
                let item = self
                    .items
                    .iter()
                    .find(|&&(each, _)| each == id)
                    .map(|&(_, item)| item);
                match (&action.kind, bar, item) {
                    (ActionKind::Press, Some(menu), _) => {
                        let place = Place::Beside {
                            element: id,
                            side: Side::Below,
                            along: Along::Start,
                        };
                        self.open_menu(place, menu);
                        continue;
                    }
                    (ActionKind::Press, None, Some(item)) => {
                        stop |= self.run(id, item);
                        continue;
                    }
                    (ActionKind::Dismissed, ..) => {
                        self.menus.retain(|&layer| layer != id);
                        let ui = &self.ui;
                        self.items.retain(|&(each, _)| ui.element(each).is_some());
                        continue;
                    }
                    (ActionKind::Press, ..) if Some(id) == self.reset => {
                        self.reset_light();
                        continue;
                    }
                    (&ActionKind::ContextMenu { at }, ..) => {
                        if let Some(row) = self.entity_row(id) {
                            self.open_menu(
                                Place::At {
                                    over: id,
                                    point: at,
                                },
                                Menu::Entity(row),
                            );
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            let target = self
                .targets
                .iter()
                .find(|(id, _)| Some(*id) == action.element)
                .map(|&(_, target)| target);
            match (action.kind, target) {
                // The value shows beside its slider as the user drags it.
                (
                    ActionKind::Changed(FieldValue::Number(value)),
                    Some(Target::Property { section, index }),
                ) => {
                    self.sections[section].sliders[index].value = value;
                    self.revision += 1;
                }
                (
                    ActionKind::Committed(FieldValue::Number(value)),
                    Some(Target::Property { section, index }),
                ) => {
                    let label = self.sections[section].sliders[index].label;
                    self.tell(format!("{label} {value}"));
                }
                (ActionKind::Committed(FieldValue::Checked(checked)), Some(target)) => {
                    let label = match target {
                        Target::Property { section, index } => {
                            let check = &mut self.sections[section].checks[index];
                            check.1 = checked;
                            check.0
                        }
                        Target::Level(level) => {
                            self.levels[level] = checked;
                            LEVELS[level]
                        }
                        Target::Snap => {
                            self.snap = checked;
                            "snap"
                        }
                        // The others are no check box.
                        Target::Quality | Target::Tool | Target::Search | Target::Position(_) => {
                            continue;
                        }
                    };
                    self.tell(format!("{label} {}", if checked { "on" } else { "off" }));
                }
                (ActionKind::Committed(FieldValue::Chosen(quality)), Some(Target::Quality)) => {
                    self.quality = quality;
                    self.tell(format!("shadows {}", QUALITIES[quality].to_lowercase()));
                }
                (ActionKind::Committed(FieldValue::Chosen(tool)), Some(Target::Tool)) => {
                    self.tool = tool;
                    self.tell(format!("tool {}", TOOLS[tool].to_lowercase()));
                }
                // The list keeps the rows that match as the user types.
                (ActionKind::Changed(FieldValue::Text(query)), Some(Target::Search)) => {
                    self.search(query);
                }
                (ActionKind::Committed(FieldValue::Text(query)), Some(Target::Search)) => {
                    self.search(query);
                    let found = self.matches.len();
                    self.tell(format!("{found} entities found"));
                }
                (ActionKind::Changed(FieldValue::Number(value)), Some(Target::Position(axis))) => {
                    self.position[axis] = value;
                }
                (
                    ActionKind::Committed(FieldValue::Number(value)),
                    Some(Target::Position(axis)),
                ) => {
                    self.position[axis] = value;
                    let name = AXES[axis].0.to_lowercase();
                    self.tell(format!("position {name} {value}"));
                }
                (ActionKind::Command(command), _) => stop |= self.command(&command),
                (ActionKind::Intent(Intent::Cancel), _) if action.element.is_some() => {
                    self.ui.set_focus(None);
                }
                (ActionKind::Intent(Intent::Cancel), _) => stop = true,
                _ => {}
            }
        }
        stop
    }

    /// Does the command of a shortcut, or of an item of a menu: switches the look's colors,
    /// density, accent or motion, makes a level, opens one, or quits. Gives whether the program is
    /// asked to stop.
    fn command(&mut self, command: &str) -> bool {
        let told = match command {
            "new-level" | "open" => {
                self.tell(command.replace('-', " "));
                return false;
            }
            "quit" => return true,
            "switch-colors" => {
                self.colors = match self.colors {
                    Colors::Dark => Colors::Light,
                    Colors::Light => Colors::HighContrast,
                    Colors::HighContrast => Colors::Dark,
                };
                format!("colors {:?}", self.colors)
            }
            "switch-density" => {
                self.density = match self.density {
                    Density::Comfortable => Density::Compact,
                    Density::Compact => Density::Comfortable,
                };
                format!("density {:?}", self.density)
            }
            "switch-accent" => {
                self.accent = (self.accent + 1) % ACCENTS.len();
                format!("accent {}", ACCENTS[self.accent].0)
            }
            "switch-motion" => {
                let reduced = !self.ui.reduced_motion();
                self.ui.set_reduced_motion(reduced);
                format!("motion {}", if reduced { "none" } else { "on" })
            }
            _ => return false,
        };
        self.style();
        self.tell(told.to_lowercase());
        false
    }

    /// Does what the item of a menu `id` does: gives whether the program is asked to stop.
    fn run(&mut self, id: ElementId, item: Item) -> bool {
        match item {
            Item::Command(command) => return self.command(command),
            Item::Submenu(menu) => {
                let place = Place::Beside {
                    element: id,
                    side: Side::End,
                    along: Along::Start,
                };
                self.open_menu(place, menu);
            }
            Item::Recent(level) => self.tell(format!("open {level}")),
            Item::Statistics => self.toggle_statistics(),
            Item::Colors(colors) => {
                self.colors = colors;
                self.style();
                self.tell(format!("colors {colors:?}").to_lowercase());
            }
            Item::Entity(verb, row) => self.tell(format!("{verb} entity-{row:04}")),
        }
        false
    }

    /// The items of `menu`: each its label, its shortcut, its mark and what it does.
    fn entries(&self, menu: Menu) -> Vec<(String, &'static str, ItemMark, Item)> {
        let entry = |label: &str, shortcut, mark, item| (label.to_owned(), shortcut, mark, item);
        match menu {
            Menu::File => vec![
                entry(
                    "New level",
                    "Ctrl+N",
                    ItemMark::None,
                    Item::Command("new-level"),
                ),
                entry("Open…", "Ctrl+O", ItemMark::None, Item::Command("open")),
                entry("Recent", "", ItemMark::Submenu, Item::Submenu(Menu::Recent)),
                entry("Quit", "Ctrl+Q", ItemMark::None, Item::Command("quit")),
            ],
            Menu::Recent => RECENT
                .iter()
                .map(|&level| entry(level, "", ItemMark::None, Item::Recent(level)))
                .collect(),
            Menu::View => vec![
                entry(
                    "Statistics",
                    "",
                    ItemMark::Check(self.statistics.is_some()),
                    Item::Statistics,
                ),
                entry("Colors", "", ItemMark::Submenu, Item::Submenu(Menu::Colors)),
                entry(
                    "Less motion",
                    "M",
                    ItemMark::Check(self.ui.reduced_motion()),
                    Item::Command("switch-motion"),
                ),
            ],
            Menu::Colors => [
                ("Dark", Colors::Dark),
                ("Light", Colors::Light),
                ("High contrast", Colors::HighContrast),
            ]
            .into_iter()
            .map(|(label, colors)| {
                let mark = ItemMark::Check(self.colors == colors);
                entry(label, "", mark, Item::Colors(colors))
            })
            .collect(),
            Menu::Entity(row) => ["rename", "duplicate", "delete"]
                .into_iter()
                .map(|verb| {
                    let label = format!("{}{}", verb[..1].to_uppercase(), &verb[1..]);
                    entry(&label, "", ItemMark::None, Item::Entity(verb, row))
                })
                .collect(),
        }
    }

    /// Opens `menu` at `place`, in a layer that closes as the user leaves it.
    fn open_menu(&mut self, place: Place, menu: Menu) {
        let layer = Layer {
            place,
            modal: false,
            dismiss: true,
        };
        let layer = self.ui.open_layer(layer, Style::default());
        let entries = self.entries(menu);
        let mut items = Vec::new();
        self.ui.build(layer, 0, |children| {
            children.add("menu", Element::menu(Style::default()), |menu| {
                for (label, shortcut, mark, item) in entries {
                    // A line parts the command that ends the program from the others.
                    if item == Item::Command("quit") {
                        let line = Element::container(Style::default()).class("separator");
                        menu.add("separator", line, |_| {});
                    }
                    let element =
                        Element::menu_item(label.as_str(), shortcut, mark, Style::default());
                    items.push((menu.add(label.as_str(), element, |_| {}), item));
                }
            });
        });
        self.menus.push(layer);
        self.items.extend(items);
    }

    /// Opens the floating panel of statistics over the window's bottom end, or closes it.
    fn toggle_statistics(&mut self) {
        if let Some(layer) = self.statistics.take() {
            self.ui.remove(layer);
            self.tell("statistics hidden".to_owned());
            return;
        }
        let place = Place::Floating {
            over: self.root,
            origin: [640.0, 330.0],
            size: [260.0, 140.0],
        };
        let layer = Layer {
            place,
            modal: false,
            dismiss: false,
        };
        let layer = self.ui.open_layer(layer, Style::default());
        self.ui.build(layer, 0, |children| {
            let mut style = column();
            style.layout.gap = [4.0, 0.0];
            let panel = Element::floating_panel("Statistics", true, style);
            children.add("statistics", panel, |panel| {
                let lines = [
                    ("entities", "entities        200"),
                    ("frame", "frame       16.6 ms"),
                    ("draws", "draw calls      412"),
                ];
                for (key, line) in lines {
                    panel.add(
                        key,
                        Element::label(line, Style::default()).class("fixed"),
                        |_| {},
                    );
                }
            });
        });
        self.statistics = Some(layer);
        self.tell("statistics shown".to_owned());
    }

    /// The entity of the row of the list that holds `id`, by its number.
    fn entity_row(&self, id: ElementId) -> Option<usize> {
        let path = self.ui.path_text(id)?;
        let (_, row) = path.split_once("/list/#")?;
        let row: usize = row.split('/').next()?.parse().ok()?;
        self.matches.get(row).copied()
    }

    /// Gives the light its first values back.
    fn reset_light(&mut self) {
        if let Some(light) = self
            .sections
            .iter_mut()
            .find(|section| section.title == "Light")
        {
            for (slider, value) in light.sliders.iter_mut().zip(LIGHT) {
                slider.value = value;
            }
        }
        self.tell("light reset".to_owned());
    }

    /// Keeps the entities whose names hold `query`, whatever its case.
    fn search(&mut self, query: String) {
        let lower = query.to_lowercase();
        self.matches = (0..ENTITIES)
            .filter(|&entity| entity_name(entity).contains(&lower))
            .collect();
        self.query = query;
        self.revision += 1;
    }

    /// Shows `what` in the bar, as the last thing done.
    fn tell(&mut self, what: String) {
        tracing::info!("{what}");
        self.last = what;
        self.revision += 1;
    }

    /// Builds what changed, lays the interface out over a surface of `size` logical pixels at
    /// `scale`, and gives its display list.
    pub fn frame(&mut self, size: [f32; 2], scale: f32) -> DisplayList {
        self.ui
            .set_root(self.root, settings(size, scale, self.direction));
        // The builder runs when what the program shows changes, and when a tab is chosen: what it
        // gives then replaces what it gave before.
        let mut built = None;
        let shown = Shown {
            sections: &self.sections,
            levels: self.levels,
            quality: self.quality,
            tool: self.tool,
            snap: self.snap,
            query: &self.query,
            found: self.matches.len(),
            position: self.position,
            last: &self.last,
        };
        self.ui.build(self.root, self.revision, |children| {
            built = Some(build(children, &shown));
        });
        if let Some(built) = built {
            self.list = built.list;
            self.targets = built.targets;
            self.bar = built.bar;
            self.reset = built.reset;
        }
        // The list's rows, in its slot, built when it shows others.
        if let Some(list) = self.list {
            let matches = &self.matches;
            self.ui.build(list, self.revision, |rows| {
                for row in rows.rows() {
                    let name = entity_name(matches[row]);
                    let mut style = Style::default();
                    style.layout.padding.start = 8.0;
                    style.layout.align_items = Align::Center;
                    rows.add(row, Element::container(style), |line| {
                        let label = Element::label(name, Style::default()).class("fixed");
                        line.add("name", label, |_| {});
                    });
                }
            });
        }
        self.ui.frame().0
    }
}

/// What the interface shows of the program.
struct Shown<'a> {
    sections: &'a [Section],
    levels: [bool; 3],
    quality: usize,
    tool: usize,
    snap: bool,
    query: &'a str,
    found: usize,
    position: [f64; 3],
    last: &'a str,
}

/// The elements that the program reads, as a build gave them.
#[derive(Default)]
struct Built {
    list: Option<ElementId>,
    targets: Vec<(ElementId, Target)>,
    bar: Vec<(ElementId, Menu)>,
    /// The button that resets the light, in its section's header.
    reset: Option<ElementId>,
}

/// Builds the interface: the bar of menus, the bar of tabs, the panel of the chosen one, and the
/// bar of what the last action did.
fn build(children: &mut Children<'_>, shown: &Shown<'_>) -> Built {
    let mut built = Built::default();
    let mut window = column();
    window.layout.grow = 1.0;
    window.layout.min_size[1] = Dimension::Pixels(0.0);
    let window = Element::container(window).class("window");
    children.add("window", window, |children| {
        children.add("menus", Element::menu_bar(Style::default()), |bar| {
            for (title, menu) in [("File", Menu::File), ("View", Menu::View)] {
                let item = Element::menu_item(title, "", ItemMark::Submenu, Style::default());
                built.bar.push((bar.add(title, item, |_| {}), menu));
            }
        });
        tools(children, shown, &mut built);
        let tabs = children.add("tabs", Element::tab_bar(0, Style::default()), |tabs| {
            for (title, tip) in TABS.into_iter().zip(TAB_TIPS) {
                // The console's tab counts the warnings and the errors of its log.
                let tab = match title {
                    "Console" => Element::counted_tab(title, flagged_lines(), Style::default()),
                    _ => Element::tab(title, Style::default()),
                };
                tabs.add(title, tab.tooltip(tip), |_| {});
            }
        });
        let chosen = children.chosen(tabs);
        let mut panel = column();
        panel.layout.grow = 1.0;
        panel.layout.min_size[1] = Dimension::Pixels(0.0);
        panel.layout.padding = Sides::all(8.0);
        let panel = Element::container(panel)
            .role(Role::TabPanel)
            .named(TABS[chosen]);
        children.add(TABS[chosen], panel, |children| match chosen {
            0 => inspector(children, shown, &mut built),
            1 => console(children, shown, &mut built),
            _ => about(children),
        });
        let mut status = Style::default();
        status.layout.padding = Sides::symmetric(10.0, 4.0);
        let status = Element::container(status).class("bar");
        children.add("status", status, |children| {
            let label = Element::label(shown.last, Style::default()).class("muted");
            children.add("last", label, |_| {});
        });
    });
    built
}

/// The inspector: the list of entities and the sections of their properties, side by side.
fn inspector(children: &mut Children<'_>, shown: &Shown<'_>, built: &mut Built) {
    let split = Element::split(true, 0.32, filling()).named("inspector");
    children.add("split", split, |panels| {
        let mut entities = column();
        entities.layout.padding = Sides::all(4.0);
        entities.layout.min_size = [Dimension::Pixels(0.0); 2];
        let entities = Element::container(entities).class("panel");
        panels.add("entities", entities, |children| {
            let mut search = Style::default();
            search.layout.margin.bottom = 4.0;
            let search = Element::search_field(shown.query, search)
                .named("search entities")
                .placeholder("Search entities");
            let id = children.add("search", search, |_| {});
            built.targets.push((id, Target::Search));
            let list = Element::list(shown.found, 22.0, filling()).named("entities");
            built.list = Some(children.add_slot("list", list));
        });
        let mut properties = column();
        properties.layout.padding = Sides::all(8.0);
        properties.layout.gap = [6.0, 0.0];
        properties.layout.min_size = [Dimension::Pixels(0.0); 2];
        let properties = Element::scroll(properties)
            .class("panel")
            .named("properties");
        panels.add("properties", properties, |children| {
            for (section_index, section) in shown.sections.iter().enumerate() {
                let mut style = column();
                style.layout.gap = [4.0, 0.0];
                let element = Element::section(section.title, !section.folded, style);
                children.add(section.title, element, |children| {
                    if section.title == "Transform" {
                        position_row(children, shown.position, &mut built.targets);
                    }
                    // The light's header holds its type and a button that resets it.
                    if section.title == "Light" {
                        let slot = Element::section_header(Style::default());
                        children.add("header", slot, |slot| {
                            let kind =
                                Element::label("point light", Style::default()).class("muted");
                            slot.add("type", kind, |_| {});
                            let reset = Element::button("Reset", Style::default())
                                .tooltip("Gives the light its first values back");
                            built.reset = Some(slot.add("reset", reset, |_| {}));
                        });
                    }
                    for (index, slider) in section.sliders.iter().enumerate() {
                        let target = Target::Property {
                            section: section_index,
                            index,
                        };
                        built.targets.push((slider_row(children, slider), target));
                    }
                    for (index, &(label, checked)) in section.checks.iter().enumerate() {
                        let target = Target::Property {
                            section: section_index,
                            index,
                        };
                        let check = Element::check_box(label, checked, Style::default());
                        built
                            .targets
                            .push((children.add(label, check, |_| {}), target));
                    }
                    if section.title == "Rendering" {
                        built
                            .targets
                            .push((quality_row(children, shown.quality), Target::Quality));
                    }
                });
            }
        });
    });
}

/// A row of a slider: its label, the slider, and its value in the font of fixed width; gives the
/// slider.
fn slider_row(children: &mut Children<'_>, slider: &Slider) -> ElementId {
    let mut row = Style::default();
    row.layout.gap = [0.0, 8.0];
    row.layout.align_items = Align::Center;
    let mut id = None;
    children.add(slider.label, Element::container(row), |children| {
        children.add("label", Element::label(slider.label, wide(96.0)), |_| {});
        let mut element =
            Element::slider(slider.value, slider.min, slider.max, slider.step, filling())
                .named(slider.label);
        // Help over the property, from the documentation of what it edits.
        if let Some(&(_, text)) = HELP.iter().find(|(label, _)| *label == slider.label) {
            element = element.help(slider.label, text);
        }
        id = Some(children.add("slider", element, |_| {}));
        let value = Element::label(format!("{:.2}", slider.value), wide(48.0)).class("fixed");
        children.add("value", value, |_| {});
    });
    id.expect("added above")
}

/// The bar of tools under the menus: the control of segments that chooses the tool, the button
/// that toggles the snapping, and the bar of progress of a cook, at the bar's end.
fn tools(children: &mut Children<'_>, shown: &Shown<'_>, built: &mut Built) {
    let mut bar = Style::default();
    bar.layout.padding = Sides::symmetric(8.0, 4.0);
    bar.layout.gap = [0.0, 8.0];
    bar.layout.align_items = Align::Center;
    children.add("tools", Element::container(bar).class("bar"), |children| {
        let segments = Element::segments(shown.tool, Style::default()).named("tool");
        let id = children.add("tool", segments, |segments| {
            for title in TOOLS {
                segments.add(title, Element::segment(title, Style::default()), |_| {});
            }
        });
        built.targets.push((id, Target::Tool));
        let snap = Element::toggle_button("Snap", shown.snap, Style::default())
            .tooltip("Moves go by whole steps of the grid");
        built
            .targets
            .push((children.add("snap", snap, |_| {}), Target::Snap));
        children.add("space", Element::container(filling()), |_| {});
        let cooking = Element::label("cooking textures", Style::default()).class("muted");
        children.add("cooking", cooking, |_| {});
        let progress = Element::progress(Some(0.65), wide(120.0)).named("cooking textures");
        children.add("progress", progress, |_| {});
        // How far the search of the assets went is not known: its bar moves with the clock.
        let indexing = Element::label("indexing", Style::default()).class("muted");
        children.add("indexing", indexing, |_| {});
        let progress = Element::progress(None, wide(60.0)).named("indexing");
        children.add("searching", progress, |_| {});
    });
}

/// The row of the position: its label and a field for each axis, its prefix the axis's, which
/// drags it; gives the fields to `targets`.
fn position_row(
    children: &mut Children<'_>,
    position: [f64; 3],
    targets: &mut Vec<(ElementId, Target)>,
) {
    let mut row = Style::default();
    row.layout.gap = [0.0, 8.0];
    row.layout.align_items = Align::Center;
    let group = Element::container(row).named("position");
    children.add("position", group, |children| {
        children.add("label", Element::label("position", wide(96.0)), |_| {});
        for (axis, (&value, (prefix, class))) in position.iter().zip(AXES).enumerate() {
            let field = Element::number_field(value, 0.1, filling())
                .prefix(prefix)
                .units(Units::Length)
                .class(class)
                .named(format!("position {}", prefix.to_lowercase()));
            targets.push((children.add(prefix, field, |_| {}), Target::Position(axis)));
        }
    });
}

/// The name of the entity `entity`, as the list shows it.
fn entity_name(entity: usize) -> String {
    format!("entity-{entity:04}  {}", NAMES[entity % NAMES.len()])
}

/// How many of the log's lines are warnings or errors.
fn flagged_lines() -> u64 {
    (0..40)
        .filter(|&number| LINES[number % LINES.len()].0 > 0)
        .count() as u64
}

/// A row of the list to choose the quality of shadows from: its label and the list, with a
/// tooltip; gives the list.
fn quality_row(children: &mut Children<'_>, quality: usize) -> ElementId {
    let mut row = Style::default();
    row.layout.gap = [0.0, 8.0];
    row.layout.align_items = Align::Center;
    let mut id = None;
    children.add("shadow quality", Element::container(row), |children| {
        let label = Element::label("shadow quality", wide(96.0));
        children.add("label", label, |_| {});
        let options = QUALITIES.map(str::to_owned).to_vec();
        let choice = Element::choice(options, quality, Style::default())
            .named("shadow quality")
            .tooltip("How sharp the shadows are, and what they cost");
        id = Some(children.add("choice", choice, |_| {}));
    });
    id.expect("added above")
}

/// The console: the lines of a log above, check boxes that choose which of its levels show below.
fn console(children: &mut Children<'_>, shown: &Shown<'_>, built: &mut Built) {
    let split = Element::split(false, 0.75, filling()).named("console");
    children.add("split", split, |panels| {
        let mut lines = column();
        lines.layout.padding = Sides::all(8.0);
        lines.layout.gap = [2.0, 0.0];
        lines.layout.min_size = [Dimension::Pixels(0.0); 2];
        let lines = Element::scroll(lines).class("panel").named("log");
        panels.add("log", lines, |children| {
            for number in 0..40 {
                let (level, line) = LINES[number % LINES.len()];
                if shown.levels[level] {
                    let text = format!("{:03} {:<7} {line}", number + 1, LEVELS[level]);
                    let label = Element::label(text, Style::default()).class("fixed");
                    children.add(number, label, |_| {});
                }
            }
        });
        let mut filters = Style::default();
        filters.layout.padding = Sides::all(8.0);
        filters.layout.gap = [0.0, 16.0];
        filters.layout.align_items = Align::Start;
        filters.layout.min_size = [Dimension::Pixels(0.0); 2];
        let filters = Element::container(filters).class("panel").named("levels");
        panels.add("levels", filters, |children| {
            for (level, &label) in LEVELS.iter().enumerate() {
                let check = Element::check_box(label, shown.levels[level], Style::default());
                let id = children.add(label, check, |_| {});
                built.targets.push((id, Target::Level(level)));
            }
        });
    });
}

/// What the example is, in the look's two fonts.
fn about(children: &mut Children<'_>) {
    let mut style = column();
    style.layout.grow = 1.0;
    style.layout.padding = Sides::all(16.0);
    style.layout.gap = [10.0, 0.0];
    let panel = Element::container(style).class("panel");
    children.add("about", panel, |children| {
        let lines = [
            ("The widgets of a panel, in the editor's look.", None),
            (
                "Geist draws its text; Geist Mono its numbers and code.",
                None,
            ),
            ("0123456789 {}[]() => entity-0042", Some("fixed")),
            ("Tiếng Việt · Русский", None),
            ("Escape, nothing focused, closes the window.", Some("muted")),
        ];
        for (index, (text, class)) in lines.into_iter().enumerate() {
            let mut label = Element::label(text, Style::default());
            if let Some(class) = class {
                label = label.class(class);
            }
            children.add(index, label, |_| {});
        }
    });
}

/// The settings of the root: `size` logical pixels at `scale`, laid out in `direction`.
fn settings(size: [f32; 2], scale: f32, direction: LayoutDirection) -> RootSettings {
    RootSettings {
        size,
        scale,
        direction,
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

/// A style that takes what its parent leaves, and may be smaller than what it holds.
fn filling() -> Style {
    let mut style = Style::default();
    style.layout.grow = 1.0;
    style.layout.min_size = [Dimension::Pixels(0.0); 2];
    style
}

/// A style `width` logical pixels wide, which does not shrink.
fn wide(width: f32) -> Style {
    let mut style = Style::default();
    style.layout.size[0] = Dimension::Pixels(width);
    style.layout.shrink = 0.0;
    style
}
