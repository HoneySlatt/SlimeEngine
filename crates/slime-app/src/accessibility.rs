//! What a presentation's interface means, given to screen readers through AccessKit's adapter of
//! the window, and what they ask of it given back to the interface
//! (`docs/adr/0146-screen-readers.md`).
//!
//! The interface gives what changed in its meaning in the engine's own types
//! ([`Ui::take_meaning_changes`]); this module alone translates them into AccessKit's tree, whose
//! types stay out of `slime-ui`.

use accesskit::{
    Action as AccessAction, ActionData, ActionRequest, HasPopup, Live, Node, NodeId,
    Rect as AccessRect, Role as AccessRole, ScrollUnit, TextDirection, TextPosition, TextSelection,
    Toggled, TreeId, TreeInfo, TreeUpdate,
};
use slime_platform::{Accessibility, PhysicalSize, Window};
use slime_ui::Rect;
use slime_ui::action::{Action, ActionKind};
use slime_ui::tree::{
    ElementAction, ElementId, FieldText, LayoutDirection, MeaningChanges, MeaningNode, Role, Ui,
};

/// The node of the window, the root of the tree, which no element's number names: theirs start at
/// 1.
const WINDOW: NodeId = NodeId(0);

/// How far a screen reader's scroll of one item moves, in logical pixels: a line of the wheel.
const SCROLL_ITEM: f32 = 40.0;

/// What screen readers read of a window: whether one does, and what the window's node holds.
pub(crate) struct ScreenReaders {
    /// The window's title, its node's name.
    title: String,
    /// Whether a screen reader reads the window: from the tree it asked for until it no longer
    /// does.
    active: bool,
    /// Whether the next update holds the whole tree.
    whole: bool,
    /// What the window's node last held: its size and the elements at the top of the interface.
    size: PhysicalSize,
    top: Vec<NodeId>,
    /// Whether the presentation gave an interface at the last update.
    had_interface: bool,
}

impl ScreenReaders {
    /// No screen reader reads a window of `title` yet.
    pub(crate) fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            active: false,
            whole: true,
            size: PhysicalSize {
                width: 0,
                height: 0,
            },
            top: Vec::new(),
            had_interface: false,
        }
    }

    /// A screen reader asked `asked` of `window`, whose presentation gives `ui`: the whole tree at
    /// once, an action given to the interface, or no update from then on.
    pub(crate) fn asked(&mut self, window: &Window, ui: Option<&mut Ui>, asked: &Accessibility) {
        match asked {
            Accessibility::TreeRequested => {
                self.active = true;
                self.whole = true;
                self.update(window, ui);
            }
            Accessibility::Deactivated => self.active = false,
            Accessibility::Action(request) => {
                if let Some(ui) = ui {
                    act(ui, request);
                }
            }
        }
    }

    /// Gives `window` what changed in what `ui`, the presentation's interface, means, while a
    /// screen reader reads it: after a frame.
    pub(crate) fn update(&mut self, window: &Window, ui: Option<&mut Ui>) {
        if let Some(update) = self.next_update(ui, window.inner_size()) {
            window.update_accessibility(update);
        }
    }

    /// The update of what changed in what `ui` means, for a window's content area of `size`: none
    /// while no screen reader reads the window; the whole tree when one asked for it, or when the
    /// presentation began or ceased to give an interface.
    fn next_update(&mut self, ui: Option<&mut Ui>, size: PhysicalSize) -> Option<TreeUpdate> {
        if !self.active {
            return None;
        }
        let whole = std::mem::take(&mut self.whole) || ui.is_some() != self.had_interface;
        self.had_interface = ui.is_some();
        let changes = match ui {
            Some(ui) => ui.take_meaning_changes(whole),
            None => MeaningChanges {
                whole,
                nodes: Vec::new(),
                top: Some(Vec::new()),
                focus: None,
            },
        };
        Some(self.translate(changes, size))
    }

    /// `changes` as AccessKit's update, the window's content area of `size`: the window's node
    /// with them when the top or the size changed.
    fn translate(&mut self, changes: MeaningChanges, size: PhysicalSize) -> TreeUpdate {
        let mut nodes = Vec::new();
        let top_changed = changes.top.is_some();
        if let Some(top) = &changes.top {
            self.top = top.iter().copied().map(NodeId).collect();
        }
        if changes.whole || top_changed || size != self.size {
            self.size = size;
            let mut window = Node::new(AccessRole::Window);
            window.set_label(self.title.clone());
            window.set_bounds(AccessRect {
                x0: 0.0,
                y0: 0.0,
                x1: f64::from(size.width),
                y1: f64::from(size.height),
            });
            window.set_children(self.top.clone());
            nodes.push((WINDOW, window));
        }
        for node in &changes.nodes {
            element_nodes(node, &mut nodes);
        }
        TreeUpdate {
            nodes,
            tree: changes.whole.then(|| TreeInfo {
                root: WINDOW,
                toolkit_name: Some("SlimeEngine".to_owned()),
                toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            tree_id: TreeId::ROOT,
            focus: changes.focus.map_or(WINDOW, NodeId),
        }
    }
}

/// AccessKit's node of `node`, then those of the runs of its text, into `nodes`.
fn element_nodes(node: &MeaningNode, nodes: &mut Vec<(NodeId, Node)>) {
    let meaning = &node.meaning;
    let role = role(meaning.role);
    let mut element = Node::new(role);
    // A screen reader reads a text's words as its value, and the name of others.
    if meaning.role == Role::Label {
        element.set_value(meaning.name.clone());
    } else if !meaning.name.is_empty() {
        element.set_label(meaning.name.clone());
    }
    if let Some(value) = &meaning.value {
        element.set_value(value.clone());
        if matches!(
            meaning.role,
            Role::SpinButton | Role::Slider | Role::Splitter | Role::ProgressIndicator
        ) && let Some(number) = number_of(value)
        {
            element.set_numeric_value(number);
        }
    }
    if let Some(range) = meaning.range {
        element.set_min_numeric_value(range.min);
        element.set_max_numeric_value(range.max);
        if range.step > 0.0 {
            element.set_numeric_value_step(range.step);
        }
    }
    if matches!(meaning.role, Role::Details | Role::Pane) {
        element.set_expanded(meaning.state.expanded);
    }
    if let Some(description) = &meaning.description {
        element.set_description(description.as_str());
    }
    if let Some(placeholder) = &meaning.placeholder {
        element.set_placeholder(placeholder.as_str());
    }
    // A notification is read as it comes: a warning or an error at once.
    match meaning.role {
        Role::Status => element.set_live(Live::Polite),
        Role::Alert => element.set_live(Live::Assertive),
        _ => {}
    }
    if meaning.popup {
        element.set_has_popup(if meaning.role == Role::ComboBox {
            HasPopup::Listbox
        } else {
            HasPopup::Menu
        });
        element.set_expanded(meaning.state.expanded);
    }
    if matches!(meaning.role, Role::Tab | Role::ListBoxOption) {
        element.set_selected(meaning.state.selected);
    }
    if meaning.role == Role::RadioButton {
        element.set_toggled(if meaning.state.selected {
            Toggled::True
        } else {
            Toggled::False
        });
    }
    if matches!(
        meaning.role,
        Role::CheckBox | Role::MenuItemCheckBox | Role::ToggleButton
    ) {
        element.set_toggled(if meaning.state.checked {
            Toggled::True
        } else {
            Toggled::False
        });
    }
    if let Some(bounds) = meaning.bounds {
        element.set_bounds(rect(bounds));
    }
    if meaning.state.disabled {
        element.set_disabled();
    }
    for action in &meaning.actions {
        match action {
            ElementAction::Press if matches!(meaning.role, Role::Details | Role::Pane) => {
                element.add_action(AccessAction::Click);
                element.add_action(if meaning.state.expanded {
                    AccessAction::Collapse
                } else {
                    AccessAction::Expand
                });
            }
            ElementAction::Press => element.add_action(AccessAction::Click),
            ElementAction::Focus => element.add_action(AccessAction::Focus),
            ElementAction::Scroll => {
                for scroll in [
                    AccessAction::ScrollUp,
                    AccessAction::ScrollDown,
                    AccessAction::ScrollLeft,
                    AccessAction::ScrollRight,
                ] {
                    element.add_action(scroll);
                }
            }
            ElementAction::SetValue if matches!(meaning.role, Role::Slider | Role::Splitter) => {
                element.add_action(AccessAction::SetValue);
                element.add_action(AccessAction::Increment);
                element.add_action(AccessAction::Decrement);
            }
            ElementAction::SetValue if meaning.role == Role::ComboBox => {
                element.add_action(AccessAction::SetValue);
            }
            ElementAction::SetValue => {
                element.add_action(AccessAction::SetValue);
                element.add_action(AccessAction::SetTextSelection);
            }
        }
    }
    let mut children: Vec<NodeId> = Vec::new();
    if let Some(text) = &node.text {
        children.extend(text.runs.iter().map(|run| NodeId(run.number)));
        let place = |(run, character): (usize, usize)| {
            Some(TextPosition {
                node: NodeId(text.runs.get(run)?.number),
                character_index: character,
            })
        };
        if let (Some(anchor), Some(focus)) = (place(text.anchor), place(text.focus)) {
            element.set_text_selection(TextSelection { anchor, focus });
        }
    }
    children.extend(node.children.iter().copied().map(NodeId));
    element.set_children(children);
    nodes.push((NodeId(node.number), element));
    if let Some(text) = &node.text {
        run_nodes(text, nodes);
    }
}

/// AccessKit's nodes of the runs of `text`, into `nodes`.
fn run_nodes(text: &FieldText, nodes: &mut Vec<(NodeId, Node)>) {
    for run in &text.runs {
        let mut node = Node::new(AccessRole::TextRun);
        node.set_value(run.text.clone());
        node.set_bounds(rect(run.bounds));
        node.set_text_direction(if run.right_to_left {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        });
        // A character's length, and a word's start among a run's characters, fit in a byte
        // (`slime_ui::tree::RUN_CHARACTERS`).
        node.set_character_lengths(
            run.characters
                .iter()
                .map(|character| character.length as u8)
                .collect::<Vec<u8>>(),
        );
        node.set_character_positions(
            run.characters
                .iter()
                .map(|character| character.position)
                .collect::<Vec<f32>>(),
        );
        node.set_character_widths(
            run.characters
                .iter()
                .map(|character| character.width)
                .collect::<Vec<f32>>(),
        );
        node.set_word_starts(
            run.word_starts
                .iter()
                .map(|&start| start as u8)
                .collect::<Vec<u8>>(),
        );
        nodes.push((NodeId(run.number), node));
    }
}

/// The number that a value gives: the whole of it, or the number before the symbol of its unit,
/// `1.5 m`; none where it reads as neither.
fn number_of(value: &str) -> Option<f64> {
    let value = value.trim();
    value
        .parse::<f64>()
        .ok()
        .or_else(|| {
            let (number, unit) = value.rsplit_once(' ')?;
            let unit = !unit.is_empty() && !unit.chars().any(|c| c.is_ascii_digit());
            unit.then(|| number.trim().parse::<f64>().ok()).flatten()
        })
        .filter(|number| number.is_finite())
}

/// AccessKit's role of `role`, of the same name, a button for a button that toggles.
fn role(role: Role) -> AccessRole {
    match role {
        Role::GenericContainer => AccessRole::GenericContainer,
        Role::Group => AccessRole::Group,
        Role::Label => AccessRole::Label,
        Role::Image => AccessRole::Image,
        Role::Button => AccessRole::Button,
        Role::ScrollView => AccessRole::ScrollView,
        Role::List => AccessRole::List,
        Role::ListItem => AccessRole::ListItem,
        Role::Dialog => AccessRole::Dialog,
        Role::Toolbar => AccessRole::Toolbar,
        Role::TextInput => AccessRole::TextInput,
        Role::SearchInput => AccessRole::SearchInput,
        Role::MultilineTextInput => AccessRole::MultilineTextInput,
        Role::SpinButton => AccessRole::SpinButton,
        Role::CheckBox => AccessRole::CheckBox,
        Role::Slider => AccessRole::Slider,
        Role::Details => AccessRole::Details,
        Role::TabList => AccessRole::TabList,
        Role::Tab => AccessRole::Tab,
        Role::TabPanel => AccessRole::TabPanel,
        Role::Splitter => AccessRole::Splitter,
        Role::Menu => AccessRole::Menu,
        Role::MenuBar => AccessRole::MenuBar,
        Role::MenuItem => AccessRole::MenuItem,
        Role::MenuItemCheckBox => AccessRole::MenuItemCheckBox,
        Role::ComboBox => AccessRole::ComboBox,
        Role::ListBox => AccessRole::ListBox,
        Role::Pane => AccessRole::Pane,
        // ARIA's toggle button: a button that AccessKit reads as toggled.
        Role::ToggleButton => AccessRole::Button,
        Role::RadioGroup => AccessRole::RadioGroup,
        Role::RadioButton => AccessRole::RadioButton,
        Role::ProgressIndicator => AccessRole::ProgressIndicator,
        Role::Status => AccessRole::Status,
        Role::Alert => AccessRole::Alert,
        Role::ListBoxOption => AccessRole::ListBoxOption,
    }
}

/// `rect`, in pixels of the surface, as AccessKit holds it: in the window's content area, without
/// transforms.
fn rect(rect: Rect) -> AccessRect {
    AccessRect {
        x0: f64::from(rect.x),
        y0: f64::from(rect.y),
        x1: f64::from(rect.x + rect.width),
        y1: f64::from(rect.y + rect.height),
    }
}

/// Does what `request` asks of the element it names in `ui`, through the interface's calls, as a
/// person's input or an agent's actions do: a press, the focus, a value or a selection set, a
/// scroll. What names no element, or asks what the element does not take now, is left out.
fn act(ui: &mut Ui, request: &ActionRequest) {
    tracing::debug!(?request, "a screen reader asks");
    let Some((id, None)) = ui.element_by_number(request.target_node.0) else {
        return;
    };
    let takes = |ui: &Ui, wanted: ElementAction| {
        ui.meaning_of(id)
            .is_some_and(|meaning| meaning.actions.contains(&wanted))
    };
    match (request.action, &request.data) {
        (AccessAction::Click, _) if takes(ui, ElementAction::Press) => ui.act(Action {
            element: Some(id),
            kind: ActionKind::Press,
        }),
        // A section opened or folded.
        (AccessAction::Expand | AccessAction::Collapse, _) if takes(ui, ElementAction::Press) => {
            ui.set_open(id, request.action == AccessAction::Expand);
        }
        // A field clicked takes the focus, as a press of the pointer gives it.
        (AccessAction::Click | AccessAction::Focus, _) => ui.set_focus(Some(id)),
        (AccessAction::SetValue, Some(ActionData::Value(text))) => {
            ui.set_value(id, text);
        }
        (AccessAction::SetValue, Some(ActionData::NumericValue(number))) => {
            ui.set_value(id, &number.to_string());
        }
        // A slider moved by a step.
        (AccessAction::Increment | AccessAction::Decrement, _)
            if takes(ui, ElementAction::SetValue) =>
        {
            let Some(meaning) = ui.meaning_of(id) else {
                return;
            };
            let (Some(value), Some(range)) = (
                meaning.value.and_then(|value| value.parse::<f64>().ok()),
                meaning.range,
            ) else {
                return;
            };
            let step = if range.step > 0.0 {
                range.step
            } else {
                (range.max - range.min) / 100.0
            };
            let towards = if request.action == AccessAction::Increment {
                step
            } else {
                -step
            };
            ui.set_value(id, &(value + towards).to_string());
        }
        (AccessAction::SetTextSelection, Some(ActionData::SetTextSelection(selection)))
            if takes(ui, ElementAction::SetValue) =>
        {
            select(ui, id, selection);
        }
        (
            AccessAction::ScrollUp
            | AccessAction::ScrollDown
            | AccessAction::ScrollLeft
            | AccessAction::ScrollRight,
            data,
        ) if takes(ui, ElementAction::Scroll) => {
            let page = matches!(data, Some(ActionData::ScrollUnit(ScrollUnit::Page)));
            scroll_by(ui, id, request.action, page);
        }
        _ => {}
    }
}

/// Selects in the field `id` what `selection` names, places among its runs.
fn select(ui: &mut Ui, id: ElementId, selection: &TextSelection) {
    let Some(text) = ui.field_runs(id) else {
        return;
    };
    // The byte of the field's text at a place of a run.
    let byte = |position: &TextPosition| {
        let run = text.runs.iter().find(|run| run.number == position.node.0)?;
        let within: usize = run
            .characters
            .iter()
            .take(position.character_index)
            .map(|character| character.length)
            .sum();
        Some(run.start + within)
    };
    if let (Some(anchor), Some(focus)) = (byte(&selection.anchor), byte(&selection.focus)) {
        ui.select_text(id, anchor, focus);
    }
}

/// Scrolls the content of `id` as `action` asks: by an item, or by what shows of it for `page`.
/// Left and right are on screen: in a root laid out from right to left, whose content starts at its
/// right, the offset from the start grows to the left.
fn scroll_by(ui: &mut Ui, id: ElementId, action: AccessAction, page: bool) {
    let Some(settings) = ui.root_settings(id) else {
        return;
    };
    let scale = settings.scale;
    let shown = ui
        .meaning_of(id)
        .and_then(|meaning| meaning.bounds)
        .unwrap_or_default();
    let [at_x, at_y] = ui.scroll(id).unwrap_or_default();
    let step = |extent: f32| {
        if page { extent / scale } else { SCROLL_ITEM }
    };
    let towards_start = match settings.direction {
        LayoutDirection::LeftToRight => AccessAction::ScrollLeft,
        LayoutDirection::RightToLeft => AccessAction::ScrollRight,
    };
    let offset = match action {
        AccessAction::ScrollUp => [at_x, at_y - step(shown.height)],
        AccessAction::ScrollDown => [at_x, at_y + step(shown.height)],
        action if action == towards_start => [at_x - step(shown.width), at_y],
        _ => [at_x + step(shown.width), at_y],
    };
    ui.scroll_to(id, offset);
}

#[cfg(test)]
mod tests {
    use accesskit_consumer::Tree;
    use slime_ui::action::FieldValue;
    use slime_ui::style::{Dimension, FlexDirection, Style};
    use slime_ui::text::Fonts;
    use slime_ui::tree::{Element, ItemMark, RootSettings};

    use super::*;

    /// An interface of no element, whose texts are laid out in the font of slime-ui's tests, and
    /// its root, of 400 by 300 logical pixels at scale 1.
    fn empty() -> (Ui, ElementId) {
        let path = format!(
            "{}/../slime-ui/tests/fonts/NotoSans.ttf",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut fonts = Fonts::new();
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, Style::default());
        (ui, root)
    }

    /// An interface of a named bar of two buttons, a text and a field, the first button holding
    /// the focus: the interface and its root.
    fn interface() -> (Ui, ElementId) {
        let (mut ui, root) = empty();
        build(&mut ui, root, "ready");
        ui.frame();
        let open = ui.element_by_path("#0/page/bar/open").expect("built");
        ui.set_focus(Some(open));
        ui.frame();
        (ui, root)
    }

    /// Builds the interface's elements, its text saying `status`.
    fn build(ui: &mut Ui, root: ElementId, status: &str) {
        let sized = |width: f32, height: f32| {
            let mut style = Style::default();
            style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
            style.layout.shrink = 0.0;
            style.text.style.fonts = vec!["Noto Sans".to_owned()];
            style
        };
        let mut column = sized(400.0, 300.0);
        column.layout.direction = FlexDirection::Column;
        ui.build(root, status.len() as u64, |children| {
            children.add("page", Element::container(column), |children| {
                let bar = Element::container(sized(400.0, 40.0)).named("tools");
                children.add("bar", bar, |children| {
                    children.add("open", Element::button("Open", sized(100.0, 40.0)), |_| {});
                    children.add("save", Element::button("Save", sized(100.0, 40.0)), |_| {});
                });
                children.add("status", Element::label(status, sized(400.0, 20.0)), |_| {});
                children.add(
                    "name",
                    Element::text_field("Ada", sized(200.0, 30.0)).named("Name"),
                    |_| {},
                );
            });
        });
    }

    /// The window's size in the tests.
    const SIZE: PhysicalSize = PhysicalSize {
        width: 400,
        height: 300,
    };

    #[test]
    fn the_whole_tree_is_the_window_holding_what_the_interface_means() {
        let (mut ui, _) = interface();
        let mut readers = ScreenReaders::new("Example");
        let update = readers.translate(ui.take_meaning_changes(true), SIZE);
        let tree = Tree::new(update, true);
        let state = tree.state();
        let window = state.root();
        assert_eq!(window.role(), AccessRole::Window);
        assert_eq!(window.label().as_deref(), Some("Example"));
        let top: Vec<AccessRole> = window.children().map(|node| node.role()).collect();
        assert_eq!(
            top,
            [AccessRole::Group, AccessRole::Label, AccessRole::TextInput]
        );
        let bar = window.children().next().expect("the bar");
        assert_eq!(bar.label().as_deref(), Some("tools"));
        let buttons: Vec<Option<String>> = bar.children().map(|node| node.label()).collect();
        assert_eq!(buttons, [Some("Open".to_owned()), Some("Save".to_owned())]);
        // The text, read from its value; the focus, on the first button.
        let status = window.children().nth(1).expect("the text");
        assert_eq!(status.value().as_deref(), Some("ready"));
        let focus = state.focus().expect("a focus");
        assert_eq!(focus.label().as_deref(), Some("Open"));
        assert!(focus.is_clickable(&accesskit_consumer::common_filter));
        // The field: its name, its value, its text by runs, its caret at its end.
        let field = window.children().nth(2).expect("the field");
        assert_eq!(field.label().as_deref(), Some("Name"));
        assert_eq!(field.value().as_deref(), Some("Ada"));
        assert!(field.supports_text_ranges());
        let selection = field.text_selection().expect("a caret");
        assert!(selection.is_degenerate());
        assert_eq!(field.document_range().text(), "Ada");
    }

    #[test]
    fn an_update_holds_what_changed_alone_and_keeps_the_tree_whole() {
        let (mut ui, root) = interface();
        let mut readers = ScreenReaders::new("Example");
        let mut tree = Tree::new(readers.translate(ui.take_meaning_changes(true), SIZE), true);
        // Nothing changed: an update of nothing but the focus.
        let update = readers.translate(ui.take_meaning_changes(false), SIZE);
        assert!(update.nodes.is_empty());
        // The text's words changed: that node alone.
        build(&mut ui, root, "saved at last");
        ui.frame();
        let update = readers.translate(ui.take_meaning_changes(false), SIZE);
        assert_eq!(update.nodes.len(), 1);
        assert_eq!(update.nodes[0].1.value(), Some("saved at last"));
        let mut changes = Changes::default();
        tree.update_and_process_changes(update, &mut changes);
        assert_eq!(changes.updated, 1);
        assert_eq!(
            tree.state()
                .root()
                .children()
                .nth(1)
                .and_then(|node| node.value())
                .as_deref(),
            Some("saved at last")
        );
        // The bar's name taken away: its buttons move to the window, as AccessKit takes it.
        let bar = ui.element_by_path("#0/page/bar").expect("built");
        let mut element = ui.element(bar).expect("the bar").clone();
        element.name = None;
        ui.build(root, 99, |children| {
            children.add("page", Element::container(Style::default()), |children| {
                children.add("bar", element, |children| {
                    for (key, label) in [("open", "Open"), ("save", "Save")] {
                        children.add(key, Element::button(label, Style::default()), |_| {});
                    }
                });
            });
        });
        ui.frame();
        tree.update_and_process_changes(
            readers.translate(ui.take_meaning_changes(false), SIZE),
            &mut changes,
        );
        let top: Vec<Option<String>> = tree
            .state()
            .root()
            .children()
            .map(|node| node.label())
            .collect();
        assert_eq!(top, [Some("Open".to_owned()), Some("Save".to_owned())]);
        build(&mut ui, root, "saved at last");
        ui.frame();
        tree.update_and_process_changes(
            readers.translate(ui.take_meaning_changes(false), SIZE),
            &mut changes,
        );
        assert_eq!(tree.state().root().children().count(), 3);
        // The window resized: its node, with the elements it holds.
        let larger = PhysicalSize {
            width: 800,
            height: 600,
        };
        let update = readers.translate(ui.take_meaning_changes(false), larger);
        assert_eq!(update.nodes.len(), 1);
        tree.update_and_process_changes(update, &mut changes);
        assert_eq!(tree.state().root().children().count(), 3);
    }

    #[test]
    fn what_a_screen_reader_asks_reaches_the_interface_as_a_person_s_input() {
        let (mut ui, _) = interface();
        let save = ui.element_by_path("#0/page/bar/save").expect("built");
        let name = ui.element_by_path("#0/page/name").expect("built");
        let changes = ui.take_meaning_changes(true);
        let number = |id: ElementId| {
            changes
                .nodes
                .iter()
                .find(|node| node.meaning.id == id)
                .map(|node| NodeId(node.number))
                .expect("an element")
        };
        let request = |target: NodeId, action, data| ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: target,
            data,
        };
        // A click presses the button, which the program reads.
        act(&mut ui, &request(number(save), AccessAction::Click, None));
        let kinds: Vec<ActionKind> = ui
            .take_actions()
            .into_iter()
            .map(|action| action.kind)
            .collect();
        assert_eq!(kinds, [ActionKind::Press]);
        // The focus, then a value set: changed and committed.
        act(&mut ui, &request(number(name), AccessAction::Focus, None));
        assert_eq!(ui.focus(), Some(name));
        let value = Some(ActionData::Value("Grace".into()));
        act(
            &mut ui,
            &request(number(name), AccessAction::SetValue, value),
        );
        assert_eq!(ui.field_text(name), Some("Grace"));
        assert_eq!(ui.take_actions().len(), 2);
        // A selection from the second character to the end, by the run's places.
        ui.frame();
        let text = ui.field_runs(name).expect("a field");
        let run = NodeId(text.runs[0].number);
        let selection = TextSelection {
            anchor: TextPosition {
                node: run,
                character_index: 1,
            },
            focus: TextPosition {
                node: run,
                character_index: 5,
            },
        };
        let data = Some(ActionData::SetTextSelection(selection));
        act(
            &mut ui,
            &request(number(name), AccessAction::SetTextSelection, data),
        );
        let text = ui.field_runs(name).expect("a field");
        assert_eq!((text.anchor, text.focus), ((0, 1), (0, 5)));
        // The window's node, and a number that names nothing: left out.
        act(&mut ui, &request(WINDOW, AccessAction::Click, None));
        act(&mut ui, &request(NodeId(9_999), AccessAction::Click, None));
        assert!(ui.take_actions().is_empty());
    }

    #[test]
    fn the_whole_tree_follows_an_interface_that_comes_and_goes() {
        let (mut ui, _) = interface();
        let mut readers = ScreenReaders::new("Example");
        assert!(
            readers.next_update(Some(&mut ui), SIZE).is_none(),
            "no screen reader reads the window"
        );
        readers.active = true;
        // No interface: the window alone.
        let update = readers.next_update(None, SIZE).expect("an update");
        let mut tree = Tree::new(update, true);
        assert_eq!(tree.state().root().children().count(), 0);
        let mut changes = Changes::default();
        // An interface given: the whole tree, then what changed alone.
        let update = readers.next_update(Some(&mut ui), SIZE).expect("an update");
        assert!(update.tree.is_some());
        tree.update_and_process_changes(update, &mut changes);
        assert_eq!(tree.state().root().children().count(), 3);
        let update = readers.next_update(Some(&mut ui), SIZE).expect("an update");
        assert!(update.nodes.is_empty());
        // Gone again: the window alone, whole.
        let update = readers.next_update(None, SIZE).expect("an update");
        assert!(update.tree.is_some());
        tree.update_and_process_changes(update, &mut changes);
        assert_eq!(tree.state().root().children().count(), 0);
    }

    #[test]
    fn a_modal_root_gives_a_whole_update_that_the_tree_takes() {
        let (mut ui, _) = interface();
        let mut readers = ScreenReaders::new("Example");
        let mut tree = Tree::new(readers.translate(ui.take_meaning_changes(true), SIZE), true);
        let settings = RootSettings {
            size: [200.0, 100.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [100.0, 100.0],
            modal: true,
        };
        let dialog = ui.add_root(settings, Style::default());
        ui.build(dialog, 0, |children| {
            let dialog = Element::container(Style::default()).role(Role::Dialog);
            children.add("dialog", dialog, |children| {
                children.add("ok", Element::button("OK", Style::default()), |_| {});
            });
        });
        ui.frame();
        let update = readers.translate(ui.take_meaning_changes(false), SIZE);
        assert!(update.tree.is_some(), "whole");
        let mut changes = Changes::default();
        tree.update_and_process_changes(update, &mut changes);
        let state = tree.state();
        let top: Vec<AccessRole> = state.root().children().map(|node| node.role()).collect();
        assert_eq!(top.last(), Some(&AccessRole::Dialog));
        // The buttons below take nothing; the dialog's button holds the focus.
        let bar = state.root().children().next().expect("the bar");
        assert!(
            bar.children()
                .all(|button| !button.is_clickable(&accesskit_consumer::common_filter))
        );
        assert_eq!(
            state.focus().and_then(|node| node.label()).as_deref(),
            Some("OK")
        );
    }

    #[test]
    fn a_click_on_a_disabled_button_does_nothing() {
        let (mut ui, _) = interface();
        let save = ui.element_by_path("#0/page/bar/save").expect("built");
        let mut state = ui.state(save).expect("a button");
        state.disabled = true;
        ui.set_state(save, state);
        ui.frame();
        let number = ui
            .take_meaning_changes(true)
            .nodes
            .into_iter()
            .find(|node| node.meaning.id == save)
            .map(|node| NodeId(node.number))
            .expect("the button");
        let focus = ui.focus();
        for action in [AccessAction::Click, AccessAction::Focus] {
            let request = ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: number,
                data: None,
            };
            act(&mut ui, &request);
        }
        assert!(ui.take_actions().is_empty());
        assert_eq!(ui.focus(), focus);
    }

    #[test]
    fn a_selection_spans_lines_and_scrolls_follow_the_direction_of_their_root() {
        let path = format!(
            "{}/../slime-ui/tests/fonts/NotoSans.ttf",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut fonts = Fonts::new();
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::RightToLeft,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, Style::default());
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(200.0), Dimension::Pixels(100.0)];
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        let mut wide = Style::default();
        wide.layout.size = [Dimension::Pixels(600.0), Dimension::Pixels(200.0)];
        wide.layout.shrink = 0.0;
        let (mut area, mut scroll) = (None, None);
        ui.build(root, 0, |children| {
            area = Some(children.add("area", Element::text_area("one\ntwo", style), |_| {}));
            let mut boxed = Style::default();
            boxed.layout.size = [Dimension::Pixels(100.0), Dimension::Pixels(50.0)];
            scroll = Some(children.add("scroll", Element::scroll(boxed), |children| {
                children.add("content", Element::container(wide), |_| {});
            }));
        });
        ui.frame();
        let (area, scroll) = (area.expect("built"), scroll.expect("built"));
        let number = |ui: &mut Ui, id: ElementId| {
            ui.take_meaning_changes(true)
                .nodes
                .into_iter()
                .find(|node| node.meaning.id == id)
                .map(|node| NodeId(node.number))
                .expect("an element")
        };
        // From the second character of the first line to the first of the second.
        let text = ui.field_runs(area).expect("a field");
        assert_eq!(text.runs.len(), 2);
        let selection = TextSelection {
            anchor: TextPosition {
                node: NodeId(text.runs[0].number),
                character_index: 1,
            },
            focus: TextPosition {
                node: NodeId(text.runs[1].number),
                character_index: 1,
            },
        };
        let target = number(&mut ui, area);
        let request = |target, action, data| ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: target,
            data,
        };
        act(
            &mut ui,
            &request(
                target,
                AccessAction::SetTextSelection,
                Some(ActionData::SetTextSelection(selection)),
            ),
        );
        let text = ui.field_runs(area).expect("a field");
        assert_eq!((text.anchor, text.focus), ((0, 1), (1, 1)));
        // In a root laid out from right to left, the content starts at the right: a scroll to the
        // left moves away from the start.
        let target = number(&mut ui, scroll);
        let item = Some(ActionData::ScrollUnit(ScrollUnit::Item));
        act(
            &mut ui,
            &request(target, AccessAction::ScrollLeft, item.clone()),
        );
        assert_eq!(ui.scroll(scroll), Some([SCROLL_ITEM, 0.0]));
        act(&mut ui, &request(target, AccessAction::ScrollRight, item));
        assert_eq!(ui.scroll(scroll), Some([0.0, 0.0]));
    }

    /// What the tree said changed in an update.
    #[derive(Default)]
    struct Changes {
        updated: usize,
    }

    impl accesskit_consumer::TreeChangeHandler for Changes {
        fn node_added(&mut self, _node: &accesskit_consumer::NodeRef<'_>) {}
        fn node_updated(
            &mut self,
            _old: &accesskit_consumer::NodeRef<'_>,
            _new: &accesskit_consumer::NodeRef<'_>,
        ) {
            self.updated += 1;
        }
        fn focus_moved(
            &mut self,
            _old: Option<&accesskit_consumer::NodeRef<'_>>,
            _new: Option<&accesskit_consumer::NodeRef<'_>>,
        ) {
        }
        fn node_removed(&mut self, _node: &accesskit_consumer::NodeRef<'_>) {}
    }

    #[test]
    fn widgets_are_given_with_their_states_and_values_and_take_what_a_screen_reader_asks() {
        let (mut ui, root) = empty();
        let sized = |width: f32| {
            let mut style = Style::default();
            style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(24.0)];
            style.layout.shrink = 0.0;
            style.text.style.fonts = vec!["Noto Sans".to_owned()];
            style
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        ui.build(root, 0, |children| {
            children.add("page", Element::container(column), |children| {
                let check = Element::check_box("Shadows", true, sized(200.0));
                children.add("check", check, |_| {});
                let slider = Element::slider(0.5, 0.0, 1.0, 0.1, sized(200.0)).named("Volume");
                children.add("slider", slider, |_| {});
                let section = Element::section("Physics", false, sized(200.0));
                children.add("section", section, |children| {
                    children.add("mass", Element::label("12 kg", sized(100.0)), |_| {});
                });
                let bar = children.add("tabs", Element::tab_bar(1, sized(200.0)), |tabs| {
                    for title in ["Scene", "Light"] {
                        tabs.add(title, Element::tab(title, sized(80.0)), |_| {});
                    }
                });
                assert_eq!(children.chosen(bar), 1);
                let split = Element::split(true, 0.25, sized(200.0)).named("Panels");
                children.add("split", split, |panels| {
                    for key in ["left", "right"] {
                        panels.add(key, Element::container(Style::default()), |_| {});
                    }
                });
            });
        });
        ui.frame();
        let mut readers = ScreenReaders::new("Example");
        let changes = ui.take_meaning_changes(true);
        let numbers: Vec<(ElementId, NodeId)> = changes
            .nodes
            .iter()
            .map(|node| (node.meaning.id, NodeId(node.number)))
            .collect();
        let tree = Tree::new(readers.translate(changes, SIZE), true);
        let state = tree.state();
        let nodes: Vec<_> = state.root().children().collect();
        let roles: Vec<AccessRole> = nodes.iter().map(|node| node.role()).collect();
        assert_eq!(
            roles,
            [
                AccessRole::CheckBox,
                AccessRole::Slider,
                AccessRole::Details,
                AccessRole::TabList,
                AccessRole::Splitter,
            ]
        );
        assert_eq!(nodes[0].toggled(), Some(Toggled::True));
        let slider = &nodes[1];
        assert_eq!(
            (
                slider.numeric_value(),
                slider.min_numeric_value(),
                slider.max_numeric_value(),
                slider.numeric_value_step()
            ),
            (Some(0.5), Some(0.0), Some(1.0), Some(0.1))
        );
        let filter = accesskit_consumer::common_filter;
        assert!(slider.supports_action(AccessAction::Increment, &filter));
        // Folded, a section holds nothing that a screen reader reads.
        assert_eq!(nodes[2].data().is_expanded(), Some(false));
        assert_eq!(nodes[2].children().count(), 0);
        assert!(nodes[2].supports_action(AccessAction::Expand, &filter));
        let tabs: Vec<Option<bool>> = nodes[3].children().map(|tab| tab.is_selected()).collect();
        assert_eq!(tabs, [Some(false), Some(true)]);
        assert_eq!(nodes[4].numeric_value(), Some(0.25));

        // What a screen reader asks: the slider a step up, the check box clicked, the section
        // opened, the first tab clicked.
        let [slider, check, section, scene, bar] = [
            "#0/page/slider",
            "#0/page/check",
            "#0/page/section",
            "#0/page/tabs/Scene",
            "#0/page/tabs",
        ]
        .map(|path| ui.element_by_path(path).expect("built"));
        let number = |id: ElementId| {
            numbers
                .iter()
                .find(|(element, _)| *element == id)
                .map(|&(_, number)| number)
                .expect("an element")
        };
        let request = |id: ElementId, action| ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: number(id),
            data: None,
        };
        act(&mut ui, &request(slider, AccessAction::Increment));
        assert_eq!(ui.slider_value(slider), Some(0.6));
        act(&mut ui, &request(check, AccessAction::Click));
        assert!(!ui.state(check).expect("there").checked);
        act(&mut ui, &request(section, AccessAction::Expand));
        assert!(ui.state(section).expect("there").expanded);
        act(&mut ui, &request(scene, AccessAction::Click));
        assert_eq!(ui.chosen(bar), Some(0));
        // The program reads the slider's change and the check box's, each committed.
        let kinds: Vec<ActionKind> = ui
            .take_actions()
            .into_iter()
            .map(|action| action.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                ActionKind::Changed(FieldValue::Number(0.6)),
                ActionKind::Committed(FieldValue::Number(0.6)),
                ActionKind::Changed(FieldValue::Checked(false)),
                ActionKind::Committed(FieldValue::Checked(false)),
            ]
        );
    }

    #[test]
    fn layers_are_given_with_their_roles_popups_and_descriptions_and_take_a_choice() {
        let (mut ui, root) = empty();
        let sized = |width: f32| {
            let mut style = Style::default();
            style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(24.0)];
            style.layout.shrink = 0.0;
            style.text.style.fonts = vec!["Noto Sans".to_owned()];
            style
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        ui.build(root, 0, |children| {
            children.add("page", Element::container(column.clone()), |children| {
                children.add("menu", Element::menu(column), |menu| {
                    let grid = Element::menu_item("Grid", "G", ItemMark::Check(true), sized(160.0));
                    menu.add("grid", grid, |_| {});
                    let recent = Element::menu_item("Recent", "", ItemMark::Submenu, sized(160.0));
                    menu.add("recent", recent, |_| {});
                });
                let options = vec!["Low".to_owned(), "High".to_owned()];
                let choice = Element::choice(options, 1, sized(120.0))
                    .named("Quality")
                    .tooltip("How sharp the shadows are");
                children.add("choice", choice, |_| {});
                let pane = Element::floating_panel("Statistics", true, sized(200.0));
                children.add("pane", pane, |_| {});
            });
        });
        ui.frame();
        let mut readers = ScreenReaders::new("Example");
        let changes = ui.take_meaning_changes(true);
        let numbers: Vec<(ElementId, NodeId)> = changes
            .nodes
            .iter()
            .map(|node| (node.meaning.id, NodeId(node.number)))
            .collect();
        let tree = Tree::new(readers.translate(changes, SIZE), true);
        let state = tree.state();
        let nodes: Vec<_> = state.root().children().collect();
        let roles: Vec<AccessRole> = nodes.iter().map(|node| node.role()).collect();
        assert_eq!(
            roles,
            [AccessRole::Menu, AccessRole::ComboBox, AccessRole::Pane]
        );
        let items: Vec<_> = nodes[0].children().collect();
        assert_eq!(items[0].role(), AccessRole::MenuItemCheckBox);
        assert_eq!(items[0].toggled(), Some(Toggled::True));
        assert_eq!(items[1].role(), AccessRole::MenuItem);
        assert_eq!(items[1].data().has_popup(), Some(HasPopup::Menu));
        assert_eq!(items[1].data().is_expanded(), Some(false));
        let choice = &nodes[1];
        assert_eq!(choice.data().has_popup(), Some(HasPopup::Listbox));
        assert_eq!(
            choice.data().description(),
            Some("How sharp the shadows are")
        );
        assert_eq!(choice.value().as_deref(), Some("High"));
        assert_eq!(nodes[2].data().is_expanded(), Some(true));

        // What a screen reader asks: the list given an option by its text.
        let element = ui.element_by_path("#0/page/choice").expect("built");
        let number = numbers
            .iter()
            .find(|&&(id, _)| id == element)
            .map(|&(_, number)| number)
            .expect("an element");
        act(
            &mut ui,
            &ActionRequest {
                action: AccessAction::SetValue,
                target_tree: TreeId::ROOT,
                target_node: number,
                data: Some(ActionData::Value("Low".into())),
            },
        );
        assert_eq!(ui.chosen(element), Some(0));
    }

    #[test]
    fn rich_widgets_are_given_with_their_roles_states_and_values() {
        let (mut ui, root) = empty();
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(120.0), Dimension::Pixels(24.0)];
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        ui.build(root, 0, |children| {
            children.add("page", Element::container(column), |children| {
                let snap = Element::toggle_button("Snap", true, style.clone());
                children.add("snap", snap, |_| {});
                children.add("tool", Element::segments(1, style.clone()), |segments| {
                    for title in ["Move", "Turn"] {
                        segments.add(title, Element::segment(title, style.clone()), |_| {});
                    }
                });
                let progress = Element::progress(Some(0.25), style.clone()).named("Cooking");
                children.add("progress", progress, |_| {});
                let search = Element::search_field("", style.clone()).placeholder("Search");
                children.add("search", search, |_| {});
            });
        });
        ui.frame();
        let mut readers = ScreenReaders::new("Example");
        let tree = Tree::new(readers.translate(ui.take_meaning_changes(true), SIZE), true);
        let state = tree.state();
        let nodes: Vec<_> = state.root().children().collect();
        let roles: Vec<AccessRole> = nodes.iter().map(|node| node.role()).collect();
        assert_eq!(
            roles,
            [
                AccessRole::Button,
                AccessRole::RadioGroup,
                AccessRole::ProgressIndicator,
                AccessRole::SearchInput
            ]
        );
        assert_eq!(nodes[0].toggled(), Some(Toggled::True));
        let segments: Vec<Option<Toggled>> = nodes[1]
            .children()
            .map(|segment| segment.toggled())
            .collect();
        assert_eq!(segments, [Some(Toggled::False), Some(Toggled::True)]);
        assert_eq!(nodes[2].numeric_value(), Some(0.25));
        assert_eq!(nodes[3].data().placeholder(), Some("Search"));
    }

    #[test]
    fn a_value_with_the_symbol_of_its_unit_gives_its_number() {
        assert_eq!(number_of("1.5 m"), Some(1.5));
        assert_eq!(number_of(" 2 "), Some(2.0));
        assert_eq!(number_of("90 °"), Some(90.0));
        assert_eq!(number_of("1 + 2"), None);
        assert_eq!(number_of("10cm"), None);
    }

    #[test]
    fn notifications_are_read_as_they_come_a_warning_at_once() {
        let (mut ui, _) = empty();
        ui.notify(slime_ui::tree::Notification {
            text: "Saved".to_owned(),
            ..Default::default()
        });
        ui.notify(slime_ui::tree::Notification {
            kind: slime_ui::tree::NotificationKind::Warning,
            text: "A texture is missing".to_owned(),
            ..Default::default()
        });
        ui.frame();
        let mut readers = ScreenReaders::new("Example");
        let tree = Tree::new(readers.translate(ui.take_meaning_changes(true), SIZE), true);
        let state = tree.state();
        let nodes: Vec<_> = state.root().children().collect();
        let read: Vec<(AccessRole, Option<Live>)> = nodes
            .iter()
            .map(|node| (node.role(), node.data().live()))
            .collect();
        assert_eq!(
            read,
            [
                (AccessRole::Status, Some(Live::Polite)),
                (AccessRole::Alert, Some(Live::Assertive))
            ]
        );
    }
}
