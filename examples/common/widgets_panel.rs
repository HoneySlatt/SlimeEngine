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

use slime_ui::action::{ActionKind, FieldValue, Intent};
use slime_ui::display::DisplayList;
use slime_ui::look::{self, Colors, Density};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Align, Dimension, FlexDirection, Sides, Style};
use slime_ui::text::Fonts;
use slime_ui::theme::Theme;
use slime_ui::tree::{Children, Element, ElementId, LayoutDirection, Role, RootSettings, Ui};

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
/// The tabs' titles.
const TABS: [&str; 3] = ["Inspector", "Console", "About"];
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

/// What a check box or a slider gives its value to: a property of a section of the inspector, by
/// its section and its place there, or a level of the console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Property { section: usize, index: usize },
    Level(usize),
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
                    slider("position x", 1.5, -10.0, 10.0, 0.1),
                    slider("position y", 0.0, -10.0, 10.0, 0.1),
                    slider("position z", -4.2, -10.0, 10.0, 0.1),
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
            last: "tab and the arrows move, enter and space press; t, d, a and m change the look"
                .to_owned(),
            revision: 0,
            colors: Colors::Dark,
            density: Density::Comfortable,
            accent: 0,
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
                    };
                    self.tell(format!("{label} {}", if checked { "on" } else { "off" }));
                }
                (ActionKind::Command(command), _) => self.command(&command),
                (ActionKind::Intent(Intent::Cancel), _) if action.element.is_some() => {
                    self.ui.set_focus(None);
                }
                (ActionKind::Intent(Intent::Cancel), _) => stop = true,
                _ => {}
            }
        }
        stop
    }

    /// Does the command of a shortcut: switches the look's colors, density, accent or motion.
    fn command(&mut self, command: &str) {
        let told = match command {
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
            _ => return,
        };
        self.style();
        self.tell(told.to_lowercase());
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
            last: &self.last,
        };
        self.ui.build(self.root, self.revision, |children| {
            built = Some(build(children, &shown));
        });
        if let Some(built) = built {
            self.list = built.list;
            self.targets = built.targets;
        }
        // The list's rows, in its slot, built when it shows others.
        if let Some(list) = self.list {
            self.ui.build(list, 0, |rows| {
                for row in rows.rows() {
                    let name = format!("entity-{row:04}  {}", NAMES[row % NAMES.len()]);
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
    last: &'a str,
}

/// The elements that the program reads, as a build gave them.
#[derive(Default)]
struct Built {
    list: Option<ElementId>,
    targets: Vec<(ElementId, Target)>,
}

/// Builds the interface: the bar of tabs, the panel of the chosen one, and the bar of what the last
/// action did.
fn build(children: &mut Children<'_>, shown: &Shown<'_>) -> Built {
    let mut built = Built::default();
    let mut window = column();
    window.layout.grow = 1.0;
    window.layout.min_size[1] = Dimension::Pixels(0.0);
    let window = Element::container(window).class("window");
    children.add("window", window, |children| {
        let tabs = children.add("tabs", Element::tab_bar(0, Style::default()), |tabs| {
            for title in TABS {
                tabs.add(title, Element::tab(title, Style::default()), |_| {});
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
            let list = Element::list(ENTITIES, 22.0, filling()).named("entities");
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
        let element = Element::slider(slider.value, slider.min, slider.max, slider.step, filling())
            .named(slider.label);
        id = Some(children.add("slider", element, |_| {}));
        let value = Element::label(format!("{:.2}", slider.value), wide(48.0)).class("fixed");
        children.add("value", value, |_| {});
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
