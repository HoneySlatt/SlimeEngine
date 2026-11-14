//! What a uwui interface means, given to screen readers as AccessKit's tree, and what they ask of
//! it given back to the interface.
//!
//! A host makes one [`ScreenReaderTree`] for each window, bound to the target whose part of the
//! interface the window shows, and passes it the three events of its AccessKit adapter: a screen
//! reader asks for the tree ([`ScreenReaderTree::tree_requested`]), asks for an action
//! ([`ScreenReaderTree::act`]), stops reading ([`ScreenReaderTree::deactivated`]). After each frame
//! of its target, [`ScreenReaderTree::update`] gives what changed, which the host gives its
//! adapter. The host keeps the adapter and the window's node, the tree's root, which it makes when
//! an update holds it.
//!
//! The crate reads the interface through its public meaning alone ([`Ui::take_meaning_changes`]):
//! AccessKit's types stay out of `uwui`. It re-exports [`accesskit`], so that a host builds its
//! window's node and hands the updates to its adapter with the crate's own version
//! (`docs/adr/0146-screen-readers.md`,
//! `docs/adr/0262-uwui-accesskits-api-one-tree-per-window-the-hosts-node-made-when-needed.md`).
//!
//! ```
//! use uwui::host::TargetId;
//! use uwui::text::Fonts;
//! use uwui::tree::Ui;
//! use uwui_accesskit::ScreenReaderTree;
//! use uwui_accesskit::accesskit::{Node, Role};
//!
//! let mut ui = Ui::new(Fonts::new());
//! let mut tree = ScreenReaderTree::new(TargetId::MAIN);
//! let window = || {
//!     let mut node = Node::new(Role::Window);
//!     node.set_label("Example");
//!     node
//! };
//! // No screen reader reads the window: no update.
//! assert!(tree.update(Some(&mut ui), false, window).is_none());
//! // One asks for the tree: the next update is whole, the window's node at its root.
//! tree.tree_requested();
//! let update = tree.update(Some(&mut ui), false, window).expect("an update");
//! assert!(update.tree.is_some());
//! assert_eq!(update.nodes[0].1.label(), Some("Example"));
//! // Nothing changed since: an update of nothing but the focus.
//! let update = tree.update(Some(&mut ui), false, window).expect("an update");
//! assert!(update.nodes.is_empty());
//! ```

pub use accesskit;

use accesskit::{
    Action as AccessAction, ActionData, ActionRequest, HasPopup, Live, Node, NodeId,
    Rect as AccessRect, Role as AccessRole, ScrollUnit, TextDirection, TextPosition, TextSelection,
    Toggled, TreeId, TreeInfo, TreeUpdate,
};
use uwui::Rect;
use uwui::action::{Action, ActionKind};
use uwui::host::TargetId;
use uwui::tree::{
    ElementAction, ElementId, FieldText, LayoutDirection, MeaningChanges, MeaningNode,
    MeaningReader, Role, Ui,
};

#[cfg(test)]
mod tests;

/// The window's node, the root of the tree, which no element's number names: theirs start at 1.
const WINDOW: NodeId = NodeId(0);

/// How far a screen reader's scroll of one item moves, in logical pixels: a line of the wheel.
const SCROLL_ITEM: f32 = 40.0;

/// The tree that screen readers read of one window: what one target's part of the interface means,
/// below the window's node, which the host makes.
///
/// It gives updates only while a screen reader reads the window, from
/// [`tree_requested`](Self::tree_requested) to [`deactivated`](Self::deactivated). Meanwhile it
/// holds a reader of the interface's meaning ([`Ui::add_meaning_reader`]), made at its first
/// update, let go when no screen reader reads, so that the interface keeps no changes for it
/// otherwise.
///
/// Its tree is the adapter's own, [`TreeId::ROOT`]: a host that grafts it into a tree of its own,
/// under another id, is not supported.
#[derive(Debug)]
pub struct ScreenReaderTree {
    /// The target that the window shows, whose part of the interface its tree holds.
    target: TargetId,
    /// The window's reader of the interface's meaning, while a screen reader reads.
    reader: Option<MeaningReader>,
    /// Whether the interface refused the window a reader, which is told once.
    refused: bool,
    /// Whether a screen reader reads the window: from the tree it asked for until it no longer
    /// does.
    active: bool,
    /// Whether the next update holds the whole tree.
    whole: bool,
    /// The elements at the top of the target, the window's node's children.
    top: Vec<NodeId>,
    /// Whether the host gave an interface at the last update.
    had_interface: bool,
}

impl ScreenReaderTree {
    /// No screen reader reads yet a window that shows `target`.
    #[must_use]
    pub fn new(target: TargetId) -> Self {
        Self {
            target,
            reader: None,
            refused: false,
            active: false,
            whole: true,
            top: Vec::new(),
            had_interface: false,
        }
    }

    /// A screen reader asked for the tree, AccessKit's adapter's first request: updates are given
    /// from now on, the next whole. The host gives the next update at once, without waiting for a
    /// frame.
    pub fn tree_requested(&mut self) {
        self.active = true;
        self.whole = true;
    }

    /// No screen reader reads the window any more: no update until one asks for the tree again,
    /// and the window's reader of meaning let go in `ui`, the interface, which keeps no changes for
    /// it. Without the interface, the reader is kept, to read again or to let go later.
    pub fn deactivated(&mut self, ui: Option<&mut Ui>) {
        self.active = false;
        if let Some(ui) = ui
            && let Some(reader) = self.reader.take()
        {
            ui.remove_meaning_reader(reader);
        }
    }

    /// What changed in what `ui`, the interface, means for the window since the last update, to
    /// give the adapter after the target's frame; none while no screen reader reads, or while the
    /// interface holds as many readers as it can, which is logged once.
    ///
    /// The update is whole when a screen reader asked for the tree, at the first update of a new
    /// reader of meaning (another interface, whose readers are its own), when the host began or
    /// ceased to give an interface, `ui`, and when the interface makes its changes whole
    /// ([`MeaningChanges::whole`]: a modal root opened or closed, marks past their bound); without
    /// an interface, the tree is the window's node alone.
    ///
    /// The update must reach the adapter: the changes it holds are taken, and the next update
    /// names nodes that only it gave, which AccessKit refuses without it.
    ///
    /// A host that gives another interface leaves the window's reader in the one it left, which
    /// keeps it until that interface goes: an interface holds at most 64 readers
    /// ([`Ui::add_meaning_reader`]).
    ///
    /// `window` makes the window's node, the tree's root: its role, name and bounds, in pixels of
    /// the window's content area. It is called only when the update holds that node: a whole
    /// update, the target's top changed, or `window_changed`, which the host sets when what the
    /// node says changed since the last update, a size or a title. The node's children are set to
    /// the elements at the target's top, replacing any that `window` gave.
    #[must_use = "the update must reach the adapter: the next one names nodes that it gives"]
    pub fn update(
        &mut self,
        ui: Option<&mut Ui>,
        window_changed: bool,
        window: impl FnOnce() -> Node,
    ) -> Option<TreeUpdate> {
        if !self.active {
            return None;
        }
        let whole = std::mem::take(&mut self.whole) || ui.is_some() != self.had_interface;
        self.had_interface = ui.is_some();
        let changes = match ui {
            Some(ui) => self.changes(ui, whole)?,
            // The window alone: its top empty, given when whole.
            None => MeaningChanges {
                whole,
                nodes: Vec::new(),
                top: whole.then(Vec::new),
                focus: None,
            },
        };
        Some(self.translate(changes, window_changed, window))
    }

    /// Does what `request` asks of the element it names in `ui`, through the interface's calls, as
    /// a person's input or an agent's actions do: a press, the focus, a value or a selection set, a
    /// scroll. What names another tree than [`TreeId::ROOT`], an element that the window's target
    /// does not draw, or no element, or asks what the element does not take now, is left out: a
    /// screen reader may ask on a tree older than the window's last, whose element has moved
    /// since to another window
    /// (`docs/adr/0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md`).
    pub fn act(&self, ui: &mut Ui, request: &ActionRequest) {
        tracing::debug!(?request, "a screen reader asks");
        if request.target_tree != TreeId::ROOT {
            return;
        }
        let Some((id, None)) = ui.element_by_number(request.target_node.0) else {
            return;
        };
        if ui
            .root_settings(id)
            .is_none_or(|settings| settings.target != self.target)
        {
            return;
        }
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
            (AccessAction::Expand | AccessAction::Collapse, _)
                if takes(ui, ElementAction::Press) =>
            {
                ui.set_open(id, request.action == AccessAction::Expand);
            }
            // A field clicked takes the focus, as a press of the pointer gives it.
            (AccessAction::Click | AccessAction::Focus, _) => ui.set_focus(Some(id)),
            // The boxes that scroll around it moved so that it shows (`docs/adr/0167-the-hierarchy.md`).
            (AccessAction::ScrollIntoView, _) => ui.scroll_into_view(id),
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

    /// What changed in `ui`'s meaning for the window's reader, whole for `whole`: the whole tree
    /// through a new reader at the first update, or once the interface is another, whose readers
    /// are its own. None where the interface holds as many readers as it can.
    fn changes(&mut self, ui: &mut Ui, whole: bool) -> Option<MeaningChanges> {
        if let Some(changes) = self
            .reader
            .and_then(|reader| ui.take_meaning_changes(reader, whole))
        {
            return Some(changes);
        }
        let Some(reader) = ui.add_meaning_reader(self.target) else {
            if !std::mem::replace(&mut self.refused, true) {
                tracing::warn!(
                    "the interface holds as many readers of meaning as it can: no screen reader \
                     reads the window"
                );
            }
            return None;
        };
        self.reader = Some(reader);
        ui.take_meaning_changes(reader, true)
    }

    /// `changes` as AccessKit's update: the window's node, made by `window`, with them when they
    /// are whole, when the top changed or for `window_changed`.
    fn translate(
        &mut self,
        changes: MeaningChanges,
        window_changed: bool,
        window: impl FnOnce() -> Node,
    ) -> TreeUpdate {
        let mut nodes = Vec::new();
        let top_changed = changes.top.is_some();
        if let Some(top) = &changes.top {
            self.top = top.iter().copied().map(NodeId).collect();
        }
        if changes.whole || top_changed || window_changed {
            let mut node = window();
            node.set_children(self.top.clone());
            nodes.push((WINDOW, node));
        }
        for node in &changes.nodes {
            element_nodes(node, &mut nodes);
        }
        TreeUpdate {
            nodes,
            tree: changes.whole.then(|| TreeInfo {
                root: WINDOW,
                toolkit_name: Some("uwui".to_owned()),
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
    if meaning.expandable {
        element.set_expanded(meaning.state.expanded);
    }
    if let Some(level) = meaning.level {
        element.set_level(level);
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
        element.set_has_popup(match meaning.role {
            Role::ComboBox => HasPopup::Listbox,
            // A color well's picker, a layer of its own fields.
            Role::ColorWell => HasPopup::Dialog,
            _ => HasPopup::Menu,
        });
        element.set_expanded(meaning.state.expanded);
    }
    if matches!(
        meaning.role,
        Role::Tab | Role::ListBoxOption | Role::ListItem | Role::TreeItem
    ) {
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
    // What a screen reader reads may be out of view: it asks for it shown.
    element.add_action(AccessAction::ScrollIntoView);
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
            // Screen readers have no drag: the program's keys do what drops do.
            ElementAction::Drop => {}
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
            ElementAction::SetValue if matches!(meaning.role, Role::ComboBox | Role::ColorWell) => {
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
        // (`uwui::tree::RUN_CHARACTERS`).
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
        Role::Tree => AccessRole::Tree,
        Role::TreeItem => AccessRole::TreeItem,
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
        Role::ColorWell => AccessRole::ColorWell,
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
