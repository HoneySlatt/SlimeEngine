use accesskit_consumer::Tree;
use slime_color::LinearColor;
use uwui::action::FieldValue;
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::tree::{Element, ItemMark, RootSettings};

use super::*;

impl ScreenReaderTree {
    /// The update of what changed in `ui` for the window's reader, as a frame gives it, the
    /// window's node given again for `window_changed`, then of `size`.
    fn read(&mut self, ui: &mut Ui, whole: bool, window_changed: bool, size: Size) -> TreeUpdate {
        let changes = self.changes(ui, whole).expect("a reader");
        self.translate(changes, window_changed, || window(size))
    }

    /// The update of what changed in `ui`, or of the window alone without `ui`, as a host gives
    /// it after a frame, the window unchanged.
    fn next(&mut self, ui: Option<&mut Ui>) -> Option<TreeUpdate> {
        self.update(ui, false, || window(SIZE))
    }
}

/// A window's content area in the tests, in pixels: its width and its height.
type Size = [f64; 2];

/// The window's size in the tests.
const SIZE: Size = [400.0, 300.0];

/// The node of the tests' window, "Example", of `size`, as a host makes it.
fn window([width, height]: Size) -> Node {
    let mut node = Node::new(AccessRole::Window);
    node.set_label("Example");
    node.set_bounds(AccessRect {
        x0: 0.0,
        y0: 0.0,
        x1: width,
        y1: height,
    });
    node
}

/// Does what `request` asks of `ui` for a window that shows `target`.
fn act(ui: &mut Ui, target: TargetId, request: &ActionRequest) {
    ScreenReaderTree::new(target).act(ui, request);
}

/// The whole of what `ui`'s main target means, through a reader of its own, let go then.
fn whole(ui: &mut Ui) -> MeaningChanges {
    let reader = ui.add_meaning_reader(TargetId::MAIN).expect("a reader");
    let changes = ui.take_meaning_changes(reader, true).expect("its changes");
    ui.remove_meaning_reader(reader);
    changes
}

/// An interface of no element, whose texts are laid out in the font of uwui's tests, and
/// its root, of 400 by 300 logical pixels at scale 1.
fn empty() -> (Ui, ElementId) {
    let path = format!(
        "{}/../uwui/tests/fonts/NotoSans.ttf",
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
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    (ui, root)
}

/// An interface of a named bar of two buttons, a text and a field, the first button holding
/// the focus: the interface and its root.
fn interface() -> (Ui, ElementId) {
    let (mut ui, root) = empty();
    build(&mut ui, root, "ready");
    ui.frame(uwui::host::TargetId::MAIN);
    let open = ui.element_by_path("main/page/bar/open").expect("built");
    ui.set_focus(Some(open));
    ui.frame(uwui::host::TargetId::MAIN);
    (ui, root)
}

/// Builds the interface's elements, its text saying `status`.
fn build(ui: &mut Ui, root: ElementId, status: &str) {
    fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
        s.width(Dimension::Pixels(width))
            .height(Dimension::Pixels(height))
            .shrink(0.0)
            .fonts(vec!["Noto Sans".to_owned()])
    }
    ui.build(root, status.len() as u64, |children| {
        let page =
            Element::container().style(|s| sized(s, 400.0, 300.0).direction(FlexDirection::Column));
        children.add("page", page, |children| {
            let bar = Element::container()
                .style(|s| sized(s, 400.0, 40.0))
                .named("tools");
            children.add("bar", bar, |children| {
                let open = Element::button("Open").style(|s| sized(s, 100.0, 40.0));
                children.add("open", open, |_| {});
                let save = Element::button("Save").style(|s| sized(s, 100.0, 40.0));
                children.add("save", save, |_| {});
            });
            let status = Element::label(status).style(|s| sized(s, 400.0, 20.0));
            children.add("status", status, |_| {});
            children.add(
                "name",
                Element::text_field("Ada")
                    .style(|s| sized(s, 200.0, 30.0))
                    .named("Name"),
                |_| {},
            );
        });
    });
}

#[test]
fn the_whole_tree_is_the_window_holding_what_the_interface_means() {
    let (mut ui, _) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let update = readers.read(&mut ui, true, false, SIZE);
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
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let mut tree = Tree::new(readers.read(&mut ui, true, false, SIZE), true);
    // Nothing changed: an update of nothing but the focus.
    let update = readers.read(&mut ui, false, false, SIZE);
    assert!(update.nodes.is_empty());
    // The text's words changed: that node alone.
    build(&mut ui, root, "saved at last");
    ui.frame(uwui::host::TargetId::MAIN);
    let update = readers.read(&mut ui, false, false, SIZE);
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
    let bar = ui.element_by_path("main/page/bar").expect("built");
    let mut element = ui.element(bar).expect("the bar").clone();
    element.name = None;
    ui.build(root, 99, |children| {
        children.add("page", Element::container(), |children| {
            children.add("bar", element, |children| {
                for (key, label) in [("open", "Open"), ("save", "Save")] {
                    children.add(key, Element::button(label), |_| {});
                }
            });
        });
    });
    ui.frame(uwui::host::TargetId::MAIN);
    tree.update_and_process_changes(readers.read(&mut ui, false, false, SIZE), &mut changes);
    let top: Vec<Option<String>> = tree
        .state()
        .root()
        .children()
        .map(|node| node.label())
        .collect();
    assert_eq!(top, [Some("Open".to_owned()), Some("Save".to_owned())]);
    build(&mut ui, root, "saved at last");
    ui.frame(uwui::host::TargetId::MAIN);
    tree.update_and_process_changes(readers.read(&mut ui, false, false, SIZE), &mut changes);
    assert_eq!(tree.state().root().children().count(), 3);
    // The window resized: its node, with the elements it holds.
    let update = readers.read(&mut ui, false, true, [800.0, 600.0]);
    assert_eq!(update.nodes.len(), 1);
    tree.update_and_process_changes(update, &mut changes);
    assert_eq!(tree.state().root().children().count(), 3);
}

#[test]
fn the_window_s_node_is_the_host_s_made_only_when_an_update_holds_it() {
    let (mut ui, root) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let made = std::cell::Cell::new(0);
    let update = |readers: &mut ScreenReaderTree, ui: &mut Ui, window_changed: bool| {
        readers.update(Some(ui), window_changed, || {
            made.set(made.get() + 1);
            let mut node = window(SIZE);
            // A child of the host's, which the target's top replaces.
            node.set_children(vec![NodeId(9_999)]);
            node
        })
    };
    assert!(
        update(&mut readers, &mut ui, true).is_none(),
        "no screen reader reads"
    );
    assert_eq!(made.get(), 0);
    readers.tree_requested();
    // Whole: the window's node, its children the target's top, and uwui the toolkit.
    let first = update(&mut readers, &mut ui, false).expect("an update");
    assert_eq!(made.get(), 1);
    let toolkit = first
        .tree
        .as_ref()
        .and_then(|tree| tree.toolkit_name.as_deref());
    assert_eq!(toolkit, Some("uwui"));
    let mut tree = Tree::new(first, true);
    assert_eq!(tree.state().root().children().count(), 3);
    // Nothing changed, then a text's words: no node of the window.
    assert!(update(&mut readers, &mut ui, false).is_some());
    build(&mut ui, root, "saved at last");
    ui.frame(TargetId::MAIN);
    let text = update(&mut readers, &mut ui, false).expect("an update");
    assert_eq!(made.get(), 1);
    tree.update_and_process_changes(text, &mut Changes::default());
    // The host says its node changed: made again, its children still the top.
    let resized = update(&mut readers, &mut ui, true).expect("an update");
    assert_eq!(made.get(), 2);
    tree.update_and_process_changes(resized, &mut Changes::default());
    assert_eq!(tree.state().root().children().count(), 3);
    // A root added changes the top: made again.
    let panel = ui.add_root("panel", ui.root_settings(root).expect("a root"), |s| s);
    ui.build(panel, 0, |children| {
        children.add("ok", Element::button("OK"), |_| {});
    });
    ui.frame(TargetId::MAIN);
    tree.update_and_process_changes(
        update(&mut readers, &mut ui, false).expect("an update"),
        &mut Changes::default(),
    );
    assert_eq!(made.get(), 3);
    assert_eq!(tree.state().root().children().count(), 4);
}

#[test]
fn without_an_interface_the_window_s_node_is_made_once() {
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    readers.tree_requested();
    let made = std::cell::Cell::new(0);
    for _ in 0..3 {
        let update = readers.update(None, false, || {
            made.set(made.get() + 1);
            window(SIZE)
        });
        assert!(update.is_some(), "a screen reader reads");
    }
    assert_eq!(
        made.get(),
        1,
        "the window alone, whole at first, unchanged then"
    );
}

#[test]
fn what_a_screen_reader_asks_reaches_the_interface_as_a_person_s_input() {
    let (mut ui, _) = interface();
    let save = ui.element_by_path("main/page/bar/save").expect("built");
    let name = ui.element_by_path("main/page/name").expect("built");
    let changes = whole(&mut ui);
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
    act(
        &mut ui,
        TargetId::MAIN,
        &request(number(save), AccessAction::Click, None),
    );
    let kinds: Vec<ActionKind> = ui
        .take_actions()
        .into_iter()
        .map(|action| action.kind)
        .collect();
    assert_eq!(kinds, [ActionKind::Press]);
    // The focus, then a value set: changed and committed.
    act(
        &mut ui,
        TargetId::MAIN,
        &request(number(name), AccessAction::Focus, None),
    );
    assert_eq!(ui.focus(), Some(name));
    let value = Some(ActionData::Value("Grace".into()));
    act(
        &mut ui,
        TargetId::MAIN,
        &request(number(name), AccessAction::SetValue, value),
    );
    assert_eq!(ui.field_text(name), Some("Grace"));
    assert_eq!(ui.take_actions().len(), 2);
    // A selection from the second character to the end, by the run's places.
    ui.frame(uwui::host::TargetId::MAIN);
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
        TargetId::MAIN,
        &request(number(name), AccessAction::SetTextSelection, data),
    );
    let text = ui.field_runs(name).expect("a field");
    assert_eq!((text.anchor, text.focus), ((0, 1), (0, 5)));
    // The window's node, and a number that names nothing: left out.
    act(
        &mut ui,
        TargetId::MAIN,
        &request(WINDOW, AccessAction::Click, None),
    );
    act(
        &mut ui,
        TargetId::MAIN,
        &request(NodeId(9_999), AccessAction::Click, None),
    );
    assert!(ui.take_actions().is_empty());
}

#[test]
fn the_whole_tree_follows_an_interface_that_comes_and_goes() {
    let (mut ui, _) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    assert!(
        readers.next(Some(&mut ui)).is_none(),
        "no screen reader reads the window"
    );
    readers.active = true;
    // No interface: the window alone.
    let update = readers.next(None).expect("an update");
    let mut tree = Tree::new(update, true);
    assert_eq!(tree.state().root().children().count(), 0);
    let mut changes = Changes::default();
    // An interface given: the whole tree, then what changed alone.
    let update = readers.next(Some(&mut ui)).expect("an update");
    assert!(update.tree.is_some());
    tree.update_and_process_changes(update, &mut changes);
    assert_eq!(tree.state().root().children().count(), 3);
    let update = readers.next(Some(&mut ui)).expect("an update");
    assert!(update.nodes.is_empty());
    // Gone again: the window alone, whole.
    let update = readers.next(None).expect("an update");
    assert!(update.tree.is_some());
    tree.update_and_process_changes(update, &mut changes);
    assert_eq!(tree.state().root().children().count(), 0);
}

#[test]
fn a_modal_root_gives_a_whole_update_that_the_tree_takes() {
    let (mut ui, _) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let mut tree = Tree::new(readers.read(&mut ui, true, false, SIZE), true);
    let settings = RootSettings {
        size: [200.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [100.0, 100.0],
        modal: true,
        target: uwui::host::TargetId::MAIN,
    };
    let dialog = ui.add_root("dialog", settings, |s| s);
    ui.build(dialog, 0, |children| {
        let dialog = Element::container().role(Role::Dialog);
        children.add("dialog", dialog, |children| {
            children.add("ok", Element::button("OK"), |_| {});
        });
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let update = readers.read(&mut ui, false, false, SIZE);
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
    let save = ui.element_by_path("main/page/bar/save").expect("built");
    let mut state = ui.state(save).expect("a button");
    state.disabled = true;
    ui.set_state(save, state);
    ui.frame(uwui::host::TargetId::MAIN);
    let number = whole(&mut ui)
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
        act(&mut ui, TargetId::MAIN, &request);
    }
    assert!(ui.take_actions().is_empty());
    assert_eq!(ui.focus(), focus);
}

#[test]
fn a_selection_spans_lines_and_scrolls_follow_the_direction_of_their_root() {
    let path = format!(
        "{}/../uwui/tests/fonts/NotoSans.ttf",
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
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let (mut area, mut scroll) = (None, None);
    ui.build(root, 0, |children| {
        let text_area = Element::text_area("one\ntwo").style(|s| {
            s.width(Dimension::Pixels(200.0))
                .height(Dimension::Pixels(100.0))
                .fonts(vec!["Noto Sans".to_owned()])
        });
        area = Some(children.add("area", text_area, |_| {}));
        let boxed = Element::scroll().style(|s| {
            s.width(Dimension::Pixels(100.0))
                .height(Dimension::Pixels(50.0))
        });
        scroll = Some(children.add("scroll", boxed, |children| {
            let wide = Element::container().style(|s| {
                s.width(Dimension::Pixels(600.0))
                    .height(Dimension::Pixels(200.0))
                    .shrink(0.0)
            });
            children.add("content", wide, |_| {});
        }));
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let (area, scroll) = (area.expect("built"), scroll.expect("built"));
    let number = |ui: &mut Ui, id: ElementId| {
        whole(ui)
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
        TargetId::MAIN,
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
        TargetId::MAIN,
        &request(target, AccessAction::ScrollLeft, item.clone()),
    );
    assert_eq!(ui.scroll(scroll), Some([SCROLL_ITEM, 0.0]));
    act(
        &mut ui,
        TargetId::MAIN,
        &request(target, AccessAction::ScrollRight, item),
    );
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
    fn sized(s: StyleBuilder, width: f32) -> StyleBuilder {
        s.width(Dimension::Pixels(width))
            .height(Dimension::Pixels(24.0))
            .shrink(0.0)
            .fonts(vec!["Noto Sans".to_owned()])
    }
    ui.build(root, 0, |children| {
        let page = Element::container().style(|s| s.direction(FlexDirection::Column));
        children.add("page", page, |children| {
            let check = Element::check_box("Shadows", true).style(|s| sized(s, 200.0));
            children.add("check", check, |_| {});
            let slider = Element::slider(0.5, 0.0, 1.0, 0.1)
                .style(|s| sized(s, 200.0))
                .named("Volume");
            children.add("slider", slider, |_| {});
            let section = Element::section("Physics", false).style(|s| sized(s, 200.0));
            children.add("section", section, |children| {
                let mass = Element::label("12 kg").style(|s| sized(s, 100.0));
                children.add("mass", mass, |_| {});
            });
            let tab_bar = Element::tab_bar(1).style(|s| sized(s, 200.0));
            let bar = children.add("tabs", tab_bar, |tabs| {
                for title in ["Scene", "Light"] {
                    tabs.add(title, Element::tab(title).style(|s| sized(s, 80.0)), |_| {});
                }
            });
            assert_eq!(children.chosen(bar), 1);
            let split = Element::split(true, 0.25)
                .style(|s| sized(s, 200.0))
                .named("Panels");
            children.add("split", split, |panels| {
                for key in ["left", "right"] {
                    panels.add(key, Element::container(), |_| {});
                }
            });
        });
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let changes = whole(&mut ui);
    let numbers: Vec<(ElementId, NodeId)> = changes
        .nodes
        .iter()
        .map(|node| (node.meaning.id, NodeId(node.number)))
        .collect();
    let tree = Tree::new(readers.translate(changes, false, || window(SIZE)), true);
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
        "main/page/slider",
        "main/page/check",
        "main/page/section",
        "main/page/tabs/Scene",
        "main/page/tabs",
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
    act(
        &mut ui,
        TargetId::MAIN,
        &request(slider, AccessAction::Increment),
    );
    assert_eq!(ui.slider_value(slider), Some(0.6));
    act(
        &mut ui,
        TargetId::MAIN,
        &request(check, AccessAction::Click),
    );
    assert!(!ui.state(check).expect("there").checked);
    act(
        &mut ui,
        TargetId::MAIN,
        &request(section, AccessAction::Expand),
    );
    assert!(ui.state(section).expect("there").expanded);
    act(
        &mut ui,
        TargetId::MAIN,
        &request(scene, AccessAction::Click),
    );
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
    fn sized(s: StyleBuilder, width: f32) -> StyleBuilder {
        s.width(Dimension::Pixels(width))
            .height(Dimension::Pixels(24.0))
            .shrink(0.0)
            .fonts(vec!["Noto Sans".to_owned()])
    }
    fn column(s: StyleBuilder) -> StyleBuilder {
        s.direction(FlexDirection::Column)
    }
    ui.build(root, 0, |children| {
        children.add("page", Element::container().style(column), |children| {
            children.add("menu", Element::menu().style(column), |menu| {
                let grid = Element::menu_item("Grid", "G", ItemMark::Check(true))
                    .style(|s| sized(s, 160.0));
                menu.add("grid", grid, |_| {});
                let recent =
                    Element::menu_item("Recent", "", ItemMark::Submenu).style(|s| sized(s, 160.0));
                menu.add("recent", recent, |_| {});
            });
            let options = vec!["Low".to_owned(), "High".to_owned()];
            let choice = Element::choice(options, 1)
                .style(|s| sized(s, 120.0))
                .named("Quality")
                .tooltip("How sharp the shadows are");
            children.add("choice", choice, |_| {});
            let pane = Element::floating_panel("Statistics", true).style(|s| sized(s, 200.0));
            children.add("pane", pane, |_| {});
            let well = Element::color_well(LinearColor::WHITE)
                .style(|s| sized(s, 60.0))
                .named("Tint");
            children.add("well", well, |_| {});
        });
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let changes = whole(&mut ui);
    let numbers: Vec<(ElementId, NodeId)> = changes
        .nodes
        .iter()
        .map(|node| (node.meaning.id, NodeId(node.number)))
        .collect();
    let tree = Tree::new(readers.translate(changes, false, || window(SIZE)), true);
    let state = tree.state();
    let nodes: Vec<_> = state.root().children().collect();
    let roles: Vec<AccessRole> = nodes.iter().map(|node| node.role()).collect();
    assert_eq!(
        roles,
        [
            AccessRole::Menu,
            AccessRole::ComboBox,
            AccessRole::Pane,
            AccessRole::ColorWell
        ]
    );
    // A color well: its code, and a picker that it opens.
    assert_eq!(nodes[3].value().as_deref(), Some("#ffffff"));
    assert_eq!(nodes[3].data().has_popup(), Some(HasPopup::Dialog));
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
    let element = ui.element_by_path("main/page/choice").expect("built");
    let number = numbers
        .iter()
        .find(|&&(id, _)| id == element)
        .map(|&(_, number)| number)
        .expect("an element");
    act(
        &mut ui,
        TargetId::MAIN,
        &ActionRequest {
            action: AccessAction::SetValue,
            target_tree: TreeId::ROOT,
            target_node: number,
            data: Some(ActionData::Value("Low".into())),
        },
    );
    assert_eq!(ui.chosen(element), Some(0));
    // A color well given a code.
    let element = ui.element_by_path("main/page/well").expect("built");
    let number = numbers
        .iter()
        .find(|&&(id, _)| id == element)
        .map(|&(_, number)| number)
        .expect("an element");
    act(
        &mut ui,
        TargetId::MAIN,
        &ActionRequest {
            action: AccessAction::SetValue,
            target_tree: TreeId::ROOT,
            target_node: number,
            data: Some(ActionData::Value("#000000".into())),
        },
    );
    assert_eq!(ui.color(element), Some(LinearColor::BLACK));
}

/// A tree's items are given with their levels, opened or not where they hold others, and
/// selected or not; a request to show one scrolls it into view
/// (`docs/adr/0167-the-hierarchy.md`).
#[test]
fn a_tree_s_items_are_given_with_their_levels_and_shown_when_asked() {
    let (mut ui, root) = empty();
    let mut tree = None;
    ui.build(root, 0, |children| {
        let element = Element::list(10, 20.0)
            .style(|s| {
                s.width(Dimension::Pixels(200.0))
                    .height(Dimension::Pixels(40.0))
            })
            .role(Role::Tree)
            .named("hierarchy");
        tree = Some(children.add_slot("tree", element));
    });
    let tree = tree.expect("built");
    ui.frame(uwui::host::TargetId::MAIN);
    let mut items = Vec::new();
    ui.build(tree, 0, |rows| {
        for row in rows.rows() {
            let mut item = Element::container()
                .role(Role::TreeItem)
                .named(format!("item {row}"))
                .level(row + 1);
            if row == 0 {
                item = item.expandable();
            }
            items.push(rows.add(row, item, |_| {}));
        }
    });
    let mut state = ui.state(items[0]).expect("built");
    (state.expanded, state.selected) = (true, true);
    ui.set_state(items[0], state);
    ui.frame(uwui::host::TargetId::MAIN);
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let changes = whole(&mut ui);
    let second = changes
        .nodes
        .iter()
        .find(|node| node.meaning.id == items[1])
        .map(|node| NodeId(node.number))
        .expect("the second item");
    let accessible = Tree::new(readers.translate(changes, false, || window(SIZE)), true);
    let state = accessible.state();
    let tree_node = state.root().children().next().expect("the tree");
    assert_eq!(tree_node.role(), AccessRole::Tree);
    let nodes: Vec<_> = tree_node.children().collect();
    assert_eq!(nodes[0].role(), AccessRole::TreeItem);
    assert_eq!(
        (
            nodes[0].level(),
            nodes[0].data().is_expanded(),
            nodes[0].is_selected()
        ),
        (Some(1), Some(true), Some(true))
    );
    assert_eq!(
        (nodes[1].level(), nodes[1].data().is_expanded()),
        (Some(2), None),
        "a leaf neither opens nor closes"
    );
    assert!(
        nodes[1]
            .data()
            .supports_action(AccessAction::ScrollIntoView)
    );
    // A screen reader asks for the second item shown: the list scrolls as little as shows it.
    act(
        &mut ui,
        TargetId::MAIN,
        &ActionRequest {
            action: AccessAction::ScrollIntoView,
            target_tree: TreeId::ROOT,
            target_node: second,
            data: None,
        },
    );
    assert_eq!(ui.scroll(tree), Some([0.0, 0.0]), "it shows already");
}

#[test]
fn rich_widgets_are_given_with_their_roles_states_and_values() {
    let (mut ui, root) = empty();
    fn sized(s: StyleBuilder) -> StyleBuilder {
        s.width(Dimension::Pixels(120.0))
            .height(Dimension::Pixels(24.0))
            .fonts(vec!["Noto Sans".to_owned()])
    }
    ui.build(root, 0, |children| {
        let page = Element::container().style(|s| s.direction(FlexDirection::Column));
        children.add("page", page, |children| {
            let snap = Element::toggle_button("Snap", true).style(sized);
            children.add("snap", snap, |_| {});
            children.add("tool", Element::segments(1).style(sized), |segments| {
                for title in ["Move", "Turn"] {
                    segments.add(title, Element::segment(title).style(sized), |_| {});
                }
            });
            let progress = Element::progress(Some(0.25)).style(sized).named("Cooking");
            children.add("progress", progress, |_| {});
            let search = Element::search_field("").style(sized).placeholder("Search");
            children.add("search", search, |_| {});
        });
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let tree = Tree::new(readers.read(&mut ui, true, false, SIZE), true);
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
    ui.notify(uwui::tree::Notification {
        text: "Saved".to_owned(),
        ..Default::default()
    });
    ui.notify(uwui::tree::Notification {
        kind: uwui::tree::NotificationKind::Warning,
        text: "A texture is missing".to_owned(),
        ..Default::default()
    });
    ui.frame(uwui::host::TargetId::MAIN);
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    let tree = Tree::new(readers.read(&mut ui, true, false, SIZE), true);
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

#[test]
fn a_request_on_another_tree_or_on_another_window_s_element_does_nothing() {
    let (mut ui, _) = interface();
    let other = uwui::host::TargetId(1);
    let root = ui.add_root(
        "other",
        RootSettings {
            target: other,
            ..ui.root_settings(ui.element_by_path("main").expect("built"))
                .expect("a root")
        },
        |s| s,
    );
    ui.build(root, 1, |children| {
        let button = Element::button("Go").style(|s| s.width(Dimension::Pixels(100.0)));
        children.add("go", button, |_| {});
    });
    ui.frame(other);
    let numbers = |ui: &mut Ui, target| {
        let reader = ui.add_meaning_reader(target).expect("a reader");
        let changes = ui.take_meaning_changes(reader, true).expect("its changes");
        ui.remove_meaning_reader(reader);
        changes
            .nodes
            .into_iter()
            .filter(|node| node.meaning.role == Role::Button)
            .map(|node| NodeId(node.number))
            .collect::<Vec<_>>()
    };
    let go = numbers(&mut ui, other)[0];
    let save = numbers(&mut ui, TargetId::MAIN)[1];
    let click = |node, tree| ActionRequest {
        action: AccessAction::Click,
        target_tree: tree,
        target_node: node,
        data: None,
    };
    let (main_window, other_window) = (
        ScreenReaderTree::new(TargetId::MAIN),
        ScreenReaderTree::new(other),
    );
    // The main window's tree takes no request on the other window's button, nor on a tree of
    // another id.
    main_window.act(&mut ui, &click(go, TreeId::ROOT));
    let elsewhere = TreeId(accesskit::Uuid::from_u128(1));
    main_window.act(&mut ui, &click(save, elsewhere));
    assert!(ui.take_actions().is_empty());
    other_window.act(&mut ui, &click(go, TreeId::ROOT));
    main_window.act(&mut ui, &click(save, TreeId::ROOT));
    assert_eq!(ui.take_actions().len(), 2);
}

#[test]
fn the_window_s_reader_is_let_go_when_no_screen_reader_reads() {
    let (mut ui, _) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    readers.active = true;
    assert!(readers.next(Some(&mut ui)).is_some());
    let reader = readers.reader.expect("a reader while one reads");
    readers.deactivated(Some(&mut ui));
    assert_eq!(readers.reader, None);
    assert_eq!(ui.take_meaning_changes(reader, false), None);
    // Told without the interface, the window keeps its reader, which it lets go later.
    readers.active = true;
    assert!(readers.next(Some(&mut ui)).is_some());
    readers.deactivated(None);
    let kept = readers.reader.expect("the reader kept");
    readers.deactivated(Some(&mut ui));
    assert_eq!(ui.take_meaning_changes(kept, false), None);
    // Read again, the whole tree comes through a new reader.
    readers.active = true;
    let update = readers.next(Some(&mut ui)).expect("an update");
    assert!(update.tree.is_some());
}

#[test]
fn updates_of_random_changes_keep_accesskit_s_tree_whole() {
    let (mut ui, root) = interface();
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    readers.active = true;
    let first = readers.next(Some(&mut ui)).expect("the whole tree");
    let mut tree = Tree::new(first, true);
    let mut state: u64 = 3;
    for step in 0..200_u64 {
        // A linear congruential generator: Knuth's MMIX constants.
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        match (state >> 33) % 4 {
            0 => build(&mut ui, root, &"status ".repeat(1 + (step % 3) as usize)),
            1 => {
                let dialog = ui.add_root(
                    "dialog",
                    RootSettings {
                        modal: true,
                        ..ui.root_settings(root).expect("a root")
                    },
                    |s| s,
                );
                ui.frame(TargetId::MAIN);
                ui.remove(dialog);
            }
            2 => {
                let field = ui.element_by_path("main/page/name").expect("built");
                ui.set_value(field, &format!("{step}"));
            }
            _ => {}
        }
        ui.frame(TargetId::MAIN);
        let update = readers.next(Some(&mut ui)).expect("an update");
        // AccessKit panics on an update that names a node that neither it nor the tree
        // holds.
        tree.update_and_process_changes(update, &mut Changes::default());
        let held = Tree::new(readers_whole(&mut ui), true);
        assert_eq!(
            described(&tree),
            described(&held),
            "the updates applied give the whole tree at step {step}"
        );
    }
}

/// The whole tree of `ui`, as a new window's reader gives it.
fn readers_whole(ui: &mut Ui) -> TreeUpdate {
    let mut readers = ScreenReaderTree::new(TargetId::MAIN);
    readers.active = true;
    let update = readers.next(Some(ui)).expect("the whole tree");
    if let Some(reader) = readers.reader {
        ui.remove_meaning_reader(reader);
    }
    update
}

/// Each node of `tree` reached from its root, by its id: its role, name, value and children.
fn described(tree: &Tree) -> Vec<String> {
    let state = tree.state();
    let mut found = Vec::new();
    let mut stack = vec![state.root()];
    while let Some(node) = stack.pop() {
        found.push(format!(
            "{:?} {:?} {:?} {:?} {:?}",
            node.id(),
            node.role(),
            node.label(),
            node.value(),
            node.child_ids().collect::<Vec<_>>()
        ));
        stack.extend(node.children());
    }
    found.sort();
    found
}
