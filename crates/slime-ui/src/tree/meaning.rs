//! What the tree means, as agents and screen readers read it: each element that a person reads or
//! uses, its role, its name, its value, its states, the actions it takes, where it shows and its
//! path (`docs/adr/0143-what-agents-see-of-the-interface.md`); and what changed in it since it was
//! last read, which the elements' marks give (`docs/adr/0146-screen-readers.md`).

use super::{ElementId, ElementState, ItemMark, Key, Kind, Node, Ui};
use crate::Rect;
use crate::layout;
use crate::style::Display;

/// The most characters that a [`TextRun`] holds: screen readers count the characters where its
/// words start in a byte, so a longer piece of one line, of one direction, is cut into several
/// runs.
pub const RUN_CHARACTERS: usize = 255;

/// What an element is to whoever reads the interface without seeing it: an agent, or a screen
/// reader. The roles bear the names of AccessKit's, through which screen readers read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A box that only lays others out: left out of what the tree means, while its children that
    /// mean something are in it.
    GenericContainer,
    /// A box that holds others under a name: a container named by its builder
    /// ([`Element::named`](super::Element::named)), unless it is given another role.
    Group,
    /// A text.
    Label,
    /// An image.
    Image,
    /// A button.
    Button,
    /// A box whose children scroll inside it.
    ScrollView,
    /// A list of rows.
    List,
    /// A row of a list: a container that a list holds, unless it is given another role.
    ListItem,
    /// A dialog, which a modal root holds.
    Dialog,
    /// A bar of buttons.
    Toolbar,
    /// A field of one line of text.
    TextInput,
    /// A field of search.
    SearchInput,
    /// A field of several lines of text.
    MultilineTextInput,
    /// A field of a number.
    SpinButton,
    /// A check box, checked or not ([`ElementState::checked`]).
    CheckBox,
    /// A slider.
    Slider,
    /// A section that folds, open or not ([`ElementState::expanded`]).
    Details,
    /// A bar of tabs.
    TabList,
    /// A tab of a bar, chosen or not ([`ElementState::selected`]).
    Tab,
    /// The panel of the chosen tab, which a builder names so
    /// ([`Element::role`](super::Element::role)).
    TabPanel,
    /// Two panels and the handle between them, its value the first panel's share.
    Splitter,
    /// A menu.
    Menu,
    /// A bar of menus.
    MenuBar,
    /// An item of a menu, open or not where it opens a submenu ([`ElementState::expanded`]).
    MenuItem,
    /// An item of a menu that turns something on or off, checked or not
    /// ([`ElementState::checked`]).
    MenuItemCheckBox,
    /// A list to choose from, open or not ([`ElementState::expanded`]), its value its chosen
    /// option.
    ComboBox,
    /// The list of options that a list to choose from opens.
    ListBox,
    /// A panel that floats over a window, open or folded ([`ElementState::expanded`]).
    Pane,
    /// A button that stays pressed, on or not ([`ElementState::checked`]): AccessKit's button,
    /// toggled.
    ToggleButton,
    /// A control of segments.
    RadioGroup,
    /// A segment of a control of segments, chosen or not ([`ElementState::selected`]).
    RadioButton,
    /// A bar of progress, its value its share where it has one.
    ProgressIndicator,
    /// A notification that tells what went on or went well, which screen readers read politely.
    Status,
    /// A notification of a warning or an error, which screen readers read at once.
    Alert,
    /// An option of that list, chosen or not ([`ElementState::selected`]).
    ListBoxOption,
    /// A color well, open or not ([`ElementState::expanded`]), its value its hexadecimal code.
    ColorWell,
}

impl Role {
    /// Its name, as AccessKit writes it: `Button`.
    pub fn name(self) -> &'static str {
        match self {
            Self::GenericContainer => "GenericContainer",
            Self::Group => "Group",
            Self::Label => "Label",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::ScrollView => "ScrollView",
            Self::List => "List",
            Self::ListItem => "ListItem",
            Self::Dialog => "Dialog",
            Self::Toolbar => "Toolbar",
            Self::TextInput => "TextInput",
            Self::SearchInput => "SearchInput",
            Self::MultilineTextInput => "MultilineTextInput",
            Self::SpinButton => "SpinButton",
            Self::CheckBox => "CheckBox",
            Self::Slider => "Slider",
            Self::Details => "Details",
            Self::TabList => "TabList",
            Self::Tab => "Tab",
            Self::TabPanel => "TabPanel",
            Self::Splitter => "Splitter",
            Self::Menu => "Menu",
            Self::MenuBar => "MenuBar",
            Self::MenuItem => "MenuItem",
            Self::MenuItemCheckBox => "MenuItemCheckBox",
            Self::ComboBox => "ComboBox",
            Self::ListBox => "ListBox",
            Self::Pane => "Pane",
            Self::ToggleButton => "ToggleButton",
            Self::RadioGroup => "RadioGroup",
            Self::RadioButton => "RadioButton",
            Self::ProgressIndicator => "ProgressIndicator",
            Self::Status => "Status",
            Self::Alert => "Alert",
            Self::ListBoxOption => "ListBoxOption",
            Self::ColorWell => "ColorWell",
        }
    }
}

/// The bounds and the step of a slider's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    /// Its least value.
    pub min: f64,
    /// Its most value.
    pub max: f64,
    /// The values it takes are its least and multiples of this from it; 0 for any.
    pub step: f64,
}

/// What an agent can do to an element, as a person would.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementAction {
    /// Press it, as [`ActionKind::Press`](crate::action::ActionKind::Press) does: a check box
    /// toggles, a section opens or folds, a tab is chosen.
    Press,
    /// Give it the focus ([`Ui::set_focus`]).
    Focus,
    /// Scroll its content ([`Ui::scroll_to`]).
    Scroll,
    /// Give a field, a slider or two panels a value, as a person who replaced what it holds and
    /// committed ([`Ui::set_value`]).
    SetValue,
}

impl ElementAction {
    /// Its name: `press`, `focus`, `scroll` or `set_value`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::Focus => "focus",
            Self::Scroll => "scroll",
            Self::SetValue => "set_value",
        }
    }
}

/// What an element means: what an agent reads of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Meaning {
    /// The element.
    pub id: ElementId,
    /// Its path, which names it from one frame to the next ([`Ui::path_text`]).
    pub path: String,
    /// Its role.
    pub role: Role,
    /// Its name: the one its builder gives ([`Element::named`](super::Element::named)), or else a
    /// button's or a check box's label, a section's or a tab's title, or a text's; empty for none.
    pub name: String,
    /// Its value: what a field holds, as the user left it, the text that an input method composes
    /// included, a field of numbers' text whether or not it reads as a number, then the symbol of
    /// its unit where it holds a number, `1.5 m`; a slider's number, two panels' first share and a
    /// bar of progress's share, as Rust writes them; the option that a list to choose from shows;
    /// a color well's hexadecimal code, and the value of a part of its picker; none for another
    /// element.
    pub value: Option<String>,
    /// The bounds and the step of a slider's value, and of two panels' share; none for another
    /// element.
    pub range: Option<ValueRange>,
    /// Its states.
    pub state: ElementState,
    /// Its description: its tooltip's title and text
    /// ([`Tooltip::description`](super::Tooltip::description)); none for none.
    pub description: Option<String>,
    /// The text that a field shows while it holds nothing
    /// ([`Element::placeholder`](super::Element::placeholder)); none for none.
    pub placeholder: Option<String>,
    /// Whether a press opens a layer beside it, which screen readers announce, open or not
    /// ([`ElementState::expanded`]): an item of a menu that opens a submenu, a list to choose
    /// from.
    pub popup: bool,
    /// What can be done to it now: none, for an element that a modal root above its own keeps
    /// from input.
    pub actions: Vec<ElementAction>,
    /// The part of it that shows, in pixels of the surface, as the last [`Ui::frame`] laid it out:
    /// within its clips and its root's edges; none where they hide it whole.
    pub bounds: Option<Rect>,
}

/// What changed in what the tree means since it was last taken ([`Ui::take_meaning_changes`]): the
/// elements whose meaning changed, as a screen reader's adapter sends them.
///
/// The elements form a tree of their own, which leaves out those that mean nothing: an element's
/// children are the elements that mean something nearest below it. An element that the changes do
/// not hold is as it was; one that was in the tree and that no element's children, nor the top,
/// names any more has left it, with what it held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeaningChanges {
    /// Whether they hold the whole tree: every element that is drawn and means something, and the
    /// top. The first changes taken are whole, and so are those after a modal root opened or
    /// closed, which changes what every element takes.
    pub whole: bool,
    /// The elements that changed, or that are new, in no order.
    pub nodes: Vec<MeaningNode>,
    /// The numbers of the elements that mean something below none that does, in the order of the
    /// tree and of the roots, when they changed; always given in whole changes.
    pub top: Option<Vec<u64>>,
    /// The number of the element that holds the focus, drawn; none where none holds it.
    pub focus: Option<u64>,
}

/// An element that means something, as [`MeaningChanges`] gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct MeaningNode {
    /// The number that names it, from 1, given to no other element of the interface, nor to a run
    /// of a field's text, ever.
    pub number: u64,
    /// What it means.
    pub meaning: Meaning,
    /// The numbers of the elements that mean something nearest below it, in the order of the tree:
    /// those below the boxes that only lay others out are its own.
    pub children: Vec<u64>,
    /// A field's text, by runs, and its selection; none for an element that is no field.
    pub text: Option<FieldText>,
}

/// A field's text as a screen reader reads it ([`Ui::field_runs`]): by runs, each a piece of a
/// line of one direction, which a screen reader moves through by characters, words and lines, and
/// the selection within them.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldText {
    /// The runs, in the order of the text, line by line: their texts, end to end, are what the
    /// field holds. An empty line, the field's when its text is empty, or the last after a line
    /// break, is one empty run.
    pub runs: Vec<TextRun>,
    /// Where the selection starts, the side that stays as it grows; collapsed, the caret, with
    /// [`focus`](Self::focus) at the same place: a run's place in [`runs`](Self::runs) and a
    /// character's place in that run, or the run's count of characters at its end. A place at the
    /// end of a line that a line break ends lies on the line break; one where a line breaks within
    /// the text, at the start of the next line.
    pub anchor: (usize, usize),
    /// Where the selection ends, the side that moves as it grows, as [`anchor`](Self::anchor).
    pub focus: (usize, usize),
}

/// A run of a field's text: a piece of one line, of one direction, of up to [`RUN_CHARACTERS`]
/// characters.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// The number that names it, as [`MeaningNode::number`]: the field's runs keep theirs while its
    /// text changes, the first run's for the first run.
    pub number: u64,
    /// Its text, in the order it is read; a line break, when it ends a line, at its end.
    pub text: String,
    /// Where its text starts in what the field holds, in bytes.
    pub start: usize,
    /// Whether it is written from right to left.
    pub right_to_left: bool,
    /// Its box, in pixels of the surface: its line's height, from the left of its first character
    /// to the right of its last, as they lie; where the field scrolls its text, out of the field's
    /// box too.
    pub bounds: Rect,
    /// Its characters, in the order they are read: the clusters that the field's selection moves
    /// by, a letter and its marks, an emoji sequence; a cluster of more than 255 bytes, which only
    /// marks heaped on one letter make, is cut into several, the first advancing, as screen readers
    /// count a character's length in a byte.
    pub characters: Vec<RunCharacter>,
    /// The places in [`characters`](Self::characters) where its words start, as the field moves by
    /// words: a word holds the spaces after it.
    pub word_starts: Vec<usize>,
}

/// A character of a [`TextRun`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunCharacter {
    /// Its length, in bytes.
    pub length: usize,
    /// Where it starts, in pixels from the run's start, its left for a run written from left to
    /// right and its right otherwise, towards its end.
    pub position: f32,
    /// How far it advances, in pixels.
    pub width: f32,
}

impl Ui {
    /// What the tree means, element by element, in the order of the tree and of the roots: each
    /// element that is drawn and means something, a role other than
    /// [`Role::GenericContainer`]. It is made when asked, from the last [`Ui::frame`], at the cost
    /// of a walk of the tree.
    pub fn meaning(&self) -> Vec<Meaning> {
        let mut found = Vec::new();
        for (number, &root) in self.roots.iter().enumerate() {
            let mut stack = vec![(root, Key::Index(number as u64).to_string())];
            while let Some((id, path)) = stack.pop() {
                let Some(node) = self.node(id) else {
                    continue;
                };
                if node.style.layout.display == Display::None {
                    continue;
                }
                if let Some(meaning) = self.meaning_with(id, path.clone()) {
                    found.push(meaning);
                }
                stack.extend(node.children.iter().rev().filter_map(|&child| {
                    Some((child, format!("{path}/{}", self.node(child)?.key)))
                }));
            }
        }
        found
    }

    /// What `id` means; none for an element that is not drawn, or means nothing.
    pub fn meaning_of(&self, id: ElementId) -> Option<Meaning> {
        let path = self.path_text(id)?;
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.node(id)) {
            if node.style.layout.display == Display::None {
                return None;
            }
            at = node.parent;
        }
        self.meaning_with(id, path)
    }

    /// What the element that the pointer would reach at `x` and `y`, in pixels of the surface,
    /// means: that element, or the nearest above it that means something
    /// ([`Ui::element_at`]).
    pub fn meaning_at(&self, x: f32, y: f32) -> Option<Meaning> {
        let mut at = self.element_at(x, y);
        while let Some(id) = at {
            if let Some(meaning) = self.meaning_of(id) {
                return Some(meaning);
            }
            at = self.node(id)?.parent;
        }
        None
    }

    /// The path of `id` as text: the number of its root, in the order the roots stack, then its
    /// keys ([`Ui::path`]), joined by `/`, a name written as it is and a number after `#`:
    /// `#0/bar/open`, `#0/content/list/#57`. A root's number changes when a root below it is
    /// removed, and when a press raises a floating panel's layer; a key's name that holds `/`, or that `#` and digits make, cannot be read back
    /// ([`Ui::element_by_path`]).
    pub fn path_text(&self, id: ElementId) -> Option<String> {
        let root = self.root_of(id)?;
        let number = self.roots.iter().position(|&each| each == root)?;
        let mut text = Key::Index(number as u64).to_string();
        for key in self.path(id)? {
            text.push('/');
            text.push_str(&key.to_string());
        }
        Some(text)
    }

    /// The element that `path` names, written as [`Ui::path_text`] writes it.
    pub fn element_by_path(&self, path: &str) -> Option<ElementId> {
        // A number after `#`, all of it digits.
        let number = |part: &str| {
            part.strip_prefix('#')
                .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|digits| digits.parse::<u64>().ok())
        };
        let mut parts = path.split('/');
        let root = *self
            .roots
            .get(usize::try_from(number(parts.next()?)?).ok()?)?;
        let keys: Vec<Key> = parts
            .map(|part| number(part).map_or_else(|| Key::from(part), Key::Index))
            .collect();
        self.find(root, &keys)
    }

    /// What `id`, drawn, at `path`, means; none for an element of no meaning.
    fn meaning_with(&self, id: ElementId, path: String) -> Option<Meaning> {
        let node = self.node(id)?;
        let element = &node.element;
        let role = self.role_of(node);
        if role == Role::GenericContainer {
            return None;
        }
        let name = match (&element.name, &element.kind) {
            (Some(name), _) => name.clone(),
            (
                None,
                Kind::Button { label }
                | Kind::CheckBox { label, .. }
                | Kind::Section { title: label, .. }
                | Kind::FloatingPanel { title: label, .. }
                | Kind::Tab {
                    title: label,
                    count: None,
                }
                | Kind::MenuItem { label, .. }
                | Kind::ToggleButton { label, .. }
                | Kind::Segment { title: label },
            ) => label.clone(),
            (None, Kind::Label(text)) => text.clone(),
            // A tab's count is part of its name: `Console (3)`.
            (
                None,
                Kind::Tab {
                    title,
                    count: Some(count),
                },
            ) => format!("{title} ({count})"),
            (None, _) => String::new(),
        };
        let mut actions = Vec::new();
        if self.in_scope(id) {
            if self.pressable(id) {
                actions.push(ElementAction::Press);
            }
            if self.focusable(id) {
                actions.push(ElementAction::Focus);
                if node.field.is_some()
                    || matches!(
                        element.kind,
                        Kind::Slider { .. }
                            | Kind::Split { .. }
                            | Kind::Choice { .. }
                            | Kind::ColorWell { .. }
                    )
                {
                    actions.push(ElementAction::SetValue);
                }
            }
            if matches!(element.kind, Kind::Scroll | Kind::List { .. }) {
                actions.push(ElementAction::Scroll);
            }
        }
        let surface = self
            .root_of(id)
            .and_then(|root| self.node(root))
            .map(|root| root.rect);
        let bounds = Some(layout::intersect(
            surface,
            layout::intersect(node.clip, node.rect),
        ))
        .filter(|rect| rect.width > 0.0 && rect.height > 0.0);
        Some(Meaning {
            id,
            path,
            role,
            name,
            value: match node.field.as_ref() {
                // A number with the symbol of its unit, `1.5 m`, which a value given back reads;
                // what the user types meanwhile, `10cm`, as it is.
                Some(field) => Some(match (&element.kind, field.text.trim().parse::<f64>()) {
                    (Kind::NumberField { units, .. }, Ok(_)) => {
                        match super::expression::symbol(*units) {
                            Some(symbol) => format!("{} {symbol}", field.text.trim()),
                            None => field.text.clone(),
                        }
                    }
                    _ => field.text.clone(),
                }),
                // A share as its own type writes it, which a widening would lengthen.
                None => match &element.kind {
                    Kind::Progress { share } => {
                        share.map(|share| share.clamp(0.0, 1.0).to_string())
                    }
                    Kind::Choice { options, .. } => self
                        .chosen(id)
                        .and_then(|chosen| options.get(chosen))
                        .map(|option| option.label.clone()),
                    Kind::ColorWell { .. } => self.color(id).map(super::color::hex),
                    Kind::ColorPart(_) => self.part_value(id),
                    _ => self
                        .slider_value(id)
                        .map(super::field::number_text)
                        .or_else(|| self.split_share(id).map(|share| share.to_string())),
                },
            },
            range: match element.kind {
                Kind::Slider { min, max, step, .. } => Some(ValueRange {
                    min,
                    max: max.max(min),
                    step,
                }),
                Kind::Split { .. } | Kind::Progress { share: Some(_), .. } => Some(ValueRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.0,
                }),
                _ => None,
            },
            state: node.state,
            description: element.tooltip.as_ref().map(super::Tooltip::description),
            placeholder: element.placeholder.clone().filter(|_| node.field.is_some()),
            popup: matches!(
                element.kind,
                Kind::MenuItem {
                    mark: ItemMark::Submenu,
                    ..
                } | Kind::Choice { .. }
                    | Kind::ColorWell { .. }
            ),
            actions,
            bounds,
        })
    }

    /// The role of `node`: its builder's, or what its kind makes it, a container in a list a row.
    fn role_of(&self, node: &Node) -> Role {
        let element = &node.element;
        let in_list = node
            .parent
            .and_then(|parent| self.node(parent))
            .is_some_and(|parent| matches!(parent.element.kind, Kind::List { .. }));
        element.role.unwrap_or(match &element.kind {
            Kind::Container if in_list => Role::ListItem,
            Kind::Container if element.name.is_some() => Role::Group,
            Kind::Container | Kind::SectionHeader => Role::GenericContainer,
            Kind::Label(_) => Role::Label,
            Kind::Image { .. } => Role::Image,
            Kind::Button { .. } => Role::Button,
            Kind::Scroll => Role::ScrollView,
            Kind::List { .. } => Role::List,
            Kind::TextField {
                search: true,
                multiline: false,
                ..
            } => Role::SearchInput,
            Kind::TextField {
                multiline: false, ..
            } => Role::TextInput,
            Kind::TextField { .. } => Role::MultilineTextInput,
            Kind::NumberField { .. } => Role::SpinButton,
            Kind::CheckBox { .. } => Role::CheckBox,
            Kind::Slider { .. } => Role::Slider,
            Kind::Section { .. } => Role::Details,
            Kind::TabBar { .. } => Role::TabList,
            Kind::Tab { .. } => Role::Tab,
            Kind::Split { .. } => Role::Splitter,
            Kind::Menu => Role::Menu,
            Kind::MenuBar => Role::MenuBar,
            Kind::MenuItem {
                mark: ItemMark::Check(_),
                ..
            } => Role::MenuItemCheckBox,
            Kind::MenuItem { .. } => Role::MenuItem,
            Kind::Choice { .. } => Role::ComboBox,
            Kind::FloatingPanel { .. } => Role::Pane,
            Kind::ToggleButton { .. } => Role::ToggleButton,
            Kind::Segments { .. } => Role::RadioGroup,
            Kind::Segment { .. } => Role::RadioButton,
            Kind::Progress { .. } => Role::ProgressIndicator,
            Kind::ColorWell { .. } => Role::ColorWell,
            // The square and the strips are read as sliders, their values in words.
            Kind::ColorPart(_) => Role::Slider,
        })
    }

    /// Whether `id` means something: a role other than [`Role::GenericContainer`].
    pub(crate) fn means(&self, id: ElementId) -> bool {
        self.node(id)
            .is_some_and(|node| self.role_of(node) != Role::GenericContainer)
    }

    /// What changed in what the tree means since it was last taken, or the whole tree for `whole`
    /// or when it must be whole ([`MeaningChanges::whole`]), from the last [`Ui::frame`]; the
    /// elements' marks are cleared. It costs what changed: each element that changed, the boxes
    /// that only lay others out below it, and a field's text laid out, as it draws it. Where the
    /// changes are not taken, the marks are kept up to twice as many as the elements of the tree,
    /// and up to 1 024 for a smaller tree, past which the next changes are whole.
    pub fn take_meaning_changes(&mut self, whole: bool) -> MeaningChanges {
        let whole = std::mem::take(&mut self.meaning_whole) || whole;
        let marked = std::mem::take(&mut self.meant);
        for &id in &marked {
            if let Some(node) = self.node_mut(id) {
                node.changed.meaning = false;
            }
        }
        let top_changed = std::mem::take(&mut self.top_changed);
        let meanings = if whole {
            self.meaning()
        } else {
            marked
                .into_iter()
                .filter_map(|id| self.meaning_of(id))
                .collect()
        };
        let mut nodes = Vec::with_capacity(meanings.len());
        for meaning in meanings {
            let id = meaning.id;
            let Some(number) = self.node(id).map(|node| node.number) else {
                continue;
            };
            let children = self
                .meaning_children(id)
                .into_iter()
                .filter_map(|child| self.node(child).map(|node| node.number))
                .collect();
            let text = self.field_runs(id);
            nodes.push(MeaningNode {
                number,
                meaning,
                children,
                text,
            });
        }
        let top = (whole || top_changed).then(|| {
            self.roots
                .clone()
                .into_iter()
                .filter(|&root| {
                    self.node(root)
                        .is_some_and(|node| node.style.layout.display != Display::None)
                })
                .flat_map(|root| {
                    if self.means(root) {
                        vec![root]
                    } else {
                        self.meaning_children(root)
                    }
                })
                .filter_map(|id| self.node(id).map(|node| node.number))
                .collect()
        });
        let focus = self
            .interaction
            .focus
            .filter(|&focus| self.meaning_of(focus).is_some())
            .and_then(|focus| self.node(focus).map(|node| node.number));
        MeaningChanges {
            whole,
            nodes,
            top,
            focus,
        }
    }

    /// The element of the number `number` ([`MeaningNode::number`]), or the field whose text held
    /// the run of that number, with the run's place among the field's runs: a field keeps the
    /// numbers of its runs as its text shrinks, so the place may be past the runs that it gives now
    /// ([`Ui::field_runs`]). It walks the tree.
    pub fn element_by_number(&self, number: u64) -> Option<(ElementId, Option<usize>)> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            let node = slot.node.as_ref()?;
            let id = ElementId {
                index: index as u32,
                generation: slot.generation,
            };
            if node.number == number {
                return Some((id, None));
            }
            let run = node
                .field
                .as_ref()?
                .runs
                .iter()
                .position(|&run| run == number)?;
            Some((id, Some(run)))
        })
    }

    /// The elements that mean something nearest below `id`, drawn, in the order of the tree.
    fn meaning_children(&self, id: ElementId) -> Vec<ElementId> {
        let mut found = Vec::new();
        let mut stack: Vec<ElementId> = self.children(id).iter().rev().copied().collect();
        while let Some(child) = stack.pop() {
            let Some(node) = self.node(child) else {
                continue;
            };
            if node.style.layout.display == Display::None {
                continue;
            }
            if self.role_of(node) == Role::GenericContainer {
                stack.extend(node.children.iter().rev());
            } else {
                found.push(child);
            }
        }
        found
    }

    /// Marks what `id` means changed: its changes are given when they are next taken.
    pub(crate) fn mark_meaning(&mut self, id: ElementId) {
        if self.meaning_whole {
            return;
        }
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.changed.meaning {
            return;
        }
        node.changed.meaning = true;
        self.meant.push(id);
        // Changes that no one takes are kept within twice the tree; past that, the next are whole.
        if self.meant.len() > 2 * self.len().max(512) {
            for id in std::mem::take(&mut self.meant) {
                if let Some(node) = self.node_mut(id) {
                    node.changed.meaning = false;
                }
            }
            self.meaning_whole = true;
        }
    }

    /// Marks what `id` means changed, and the elements above it up to the first that means
    /// something, whose children are those that mean something nearest below it; the top, when
    /// none above it does.
    pub(crate) fn mark_meaning_up(&mut self, id: ElementId) {
        let mut at = Some(id);
        while let Some(id) = at {
            self.mark_meaning(id);
            if self.means(id) {
                return;
            }
            at = self.node(id).and_then(|node| node.parent);
        }
        self.top_changed = true;
    }

    /// Marks the elements above `id` changed, up to the first that means something, as
    /// [`Ui::mark_meaning_up`] does from `id`'s parent: `id` became or ceased to be one of its
    /// children, or what it holds did.
    pub(crate) fn mark_meaning_above(&mut self, id: ElementId) {
        match self.node(id).and_then(|node| node.parent) {
            Some(parent) => self.mark_meaning_up(parent),
            None => self.top_changed = true,
        }
    }

    /// Marks what `id` and every element below it mean changed: they are shown again, or their
    /// root's box changed what shows of them.
    pub(crate) fn mark_meaning_below(&mut self, id: ElementId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            self.mark_meaning(id);
            stack.extend(self.children(id).iter().copied());
        }
    }
}
