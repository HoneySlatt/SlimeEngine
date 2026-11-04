//! Docking, without a GPU (`docs/adr/0172-regions-of-tabs.md`): a layout normalized after each
//! edit, a panel moved into another stack or to an edge of the window, a border moved that changes
//! the two shares beside it and no other, the layout written and read back as text; a dock built
//! into a tree, its splitters dragged by the pointer, moved by the keys and given a value by an
//! agent, each read by the dock, which keeps each child at its least size, its tabs chosen, and F6
//! going from one of its stacks to the next.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::{Action, ActionKind};
use slime_ui::dock::{Dock, Edge, LEAST, Layout, Node, Target};
use slime_ui::style::Style;
use slime_ui::text::Fonts;
use slime_ui::tree::{ElementId, LayoutDirection, Role, RootSettings, Ui};

/// The editor's layout, in short: the hierarchy above the components on the left, a quarter of
/// the width, the viewport above the history and the console.
fn editor() -> Layout {
    Layout::new(Node::split(
        true,
        vec![
            (
                0.25,
                Node::split(
                    false,
                    vec![
                        (0.5, Node::tabs(&["hierarchy"])),
                        (0.5, Node::tabs(&["components"])),
                    ],
                ),
            ),
            (
                0.75,
                Node::split(
                    false,
                    vec![
                        (0.75, Node::tabs(&["viewport"])),
                        (0.25, Node::tabs(&["history", "console"])),
                    ],
                ),
            ),
        ],
    ))
}

/// The shares of the split at the root, and of each split within it.
fn shares(node: &Node) -> Vec<f32> {
    match node {
        Node::Tabs { .. } => Vec::new(),
        Node::Split { children, .. } => children.iter().map(|child| child.share).collect(),
    }
}

#[test]
fn a_layout_is_normalized_after_each_edit() {
    // A split of one child is the child; a split within one of its direction is merged into it,
    // its shares scaled; an empty stack goes; the shares sum to 1.
    let layout = Layout::new(Node::split(
        true,
        vec![
            (2.0, Node::tabs(&["a"])),
            (
                2.0,
                Node::split(
                    true,
                    vec![(1.0, Node::tabs(&["b"])), (3.0, Node::tabs(&["c"]))],
                ),
            ),
            (1.0, Node::tabs(&[])),
            (1.0, Node::split(false, vec![(1.0, Node::tabs(&["d"]))])),
        ],
    ));
    assert_eq!(shares(&layout.root), [0.4, 0.1, 0.3, 0.2]);
    let stacks: Vec<Vec<String>> = layout
        .stacks()
        .into_iter()
        .map(<[String]>::to_vec)
        .collect();
    assert_eq!(stacks, [["a"], ["b"], ["c"], ["d"]]);
    assert_eq!(
        Layout::new(Node::tabs(&[])).stacks().len(),
        1,
        "one empty stack"
    );
    // A panel held twice stays where it comes first.
    let twice = Layout::new(Node::split(
        false,
        vec![(0.5, Node::tabs(&["a", "b"])), (0.5, Node::tabs(&["b"]))],
    ));
    assert_eq!(twice.root, Node::tabs(&["a", "b"]));
}

#[test]
fn a_panel_moves_into_another_stack_or_to_an_edge_of_the_window() {
    let mut layout = editor();
    // Into the viewport's stack, shown there; its own stack left with the components alone.
    assert!(layout.move_panel("hierarchy", &Target::With("viewport".to_owned())));
    let stacks: Vec<Vec<String>> = layout
        .stacks()
        .into_iter()
        .map(<[String]>::to_vec)
        .collect();
    assert_eq!(
        stacks,
        [
            vec!["components"],
            vec!["viewport", "hierarchy"],
            vec!["history", "console"]
        ]
    );
    let Node::Split { children, .. } = &layout.root else {
        panic!("a split");
    };
    assert!(matches!(
        &children[1].node,
        Node::Split { children, .. } if matches!(&children[0].node, Node::Tabs { chosen: 1, .. })
    ));
    // Into its own stack, or beside a panel that is not there: nothing.
    assert!(!layout.move_panel("hierarchy", &Target::With("viewport".to_owned())));
    assert!(!layout.move_panel("hierarchy", &Target::With("map".to_owned())));
    assert!(!layout.move_panel("map", &Target::Edge(Edge::Left)));
    // To the bottom edge: a stack of its own, a quarter of the height, the whole width.
    assert!(layout.move_panel("console", &Target::Edge(Edge::Bottom)));
    let Node::Split {
        side_by_side,
        children,
    } = &layout.root
    else {
        panic!("a split");
    };
    assert!(!side_by_side);
    assert_eq!(children.len(), 2);
    assert_eq!(children[1].share, 0.25);
    assert_eq!(children[1].node, Node::tabs(&["console"]));
    // To the left edge, where the split at the root lies side by side: merged into it.
    let mut layout = editor();
    assert!(layout.move_panel("history", &Target::Edge(Edge::Left)));
    let Node::Split {
        side_by_side,
        children,
    } = &layout.root
    else {
        panic!("a split");
    };
    assert!(side_by_side);
    assert_eq!(children.len(), 3);
    assert_eq!(shares(&layout.root), [0.25, 0.1875, 0.5625]);
}

#[test]
fn a_border_moved_changes_the_two_shares_beside_it_and_no_other() {
    let mut layout = Layout::new(Node::split(
        true,
        vec![
            (0.2, Node::tabs(&["a"])),
            (0.3, Node::tabs(&["b"])),
            (0.5, Node::tabs(&["c"])),
        ],
    ));
    assert!(layout.resize(&[], 0, 0.8));
    let after = shares(&layout.root);
    assert!((after[0] - 0.4).abs() < 1e-6 && (after[1] - 0.1).abs() < 1e-6);
    assert_eq!(after[2], 0.5);
    assert!(!layout.resize(&[], 2, 0.5), "no border after the last");
    assert!(!layout.resize(&[0], 0, 0.5), "no split there");
}

#[test]
fn a_layout_is_written_and_read_back_as_text() {
    let layout = editor();
    let text = slime_serialize::write_as(&layout).expect("written");
    let (read, warnings) =
        slime_serialize::read_as::<Layout>(&text, slime_serialize::Mode::Strict).expect("read");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(read, layout, "{text}");
}

/// A dock of `layout` in a root of 800 by 600 logical pixels, built and laid out: the interface,
/// the dock and the slot that it is built under.
fn docked(layout: Layout) -> (Ui, Dock, ElementId) {
    docked_in(LayoutDirection::LeftToRight, layout)
}

/// As [`docked`], in a root laid out in `direction`.
fn docked_in(direction: LayoutDirection, layout: Layout) -> (Ui, Dock, ElementId) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut slot = None;
    ui.build(root, 0, |children| {
        let mut whole = Style::default();
        whole.layout.grow = 1.0;
        slot = Some(children.add_slot("dock", slime_ui::tree::Element::container(whole)));
    });
    let slot = slot.expect("built");
    let mut dock = Dock::new(layout);
    frame(&mut ui, &mut dock, slot);
    (ui, dock, slot)
}

/// A frame of the program: the dock takes its actions, is built again, and the tree laid out.
fn frame(ui: &mut Ui, dock: &mut Dock, slot: ElementId) {
    let mut actions = ui.take_actions();
    dock.act(&mut actions);
    dock.build(ui, slot, &|panel| panel.to_uppercase());
    ui.frame();
}

/// A press of a named key, Shift held or not.
fn named(ui: &mut Ui, key: NamedKey, shift: bool) {
    let mut modifiers = slime_input::Modifiers::default();
    modifiers.shift = shift;
    ui.modifiers(modifiers);
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(key),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.modifiers(slime_input::Modifiers::default());
}

#[test]
fn a_dock_s_borders_and_tabs_change_its_layout() {
    let (mut ui, mut dock, slot) = docked(editor());
    let meaning = ui.meaning();
    let splitters: Vec<_> = meaning
        .iter()
        .filter(|meaning| meaning.role == Role::Splitter)
        .collect();
    assert_eq!(splitters.len(), 3, "a border in each split");
    // The left column, a quarter of what the border leaves: its stacks' panels are slots.
    let hierarchy = dock.slot("hierarchy").expect("shown");
    assert!(dock.slot("console").is_none(), "behind the history's tab");
    let width = |ui: &Ui, id: ElementId| ui.rect(id).map_or(0.0, |rect| rect.width);
    assert!(
        (width(&ui, hierarchy) - 198.0).abs() < 1.0,
        "{}",
        width(&ui, hierarchy)
    );
    // The border between the left column and the rest, dragged by the pointer to 400 pixels.
    let border = splitters
        .iter()
        .find(|meaning| {
            ui.rect(meaning.id)
                .is_some_and(|rect| rect.height > 500.0 && rect.x < 300.0)
        })
        .expect("the upright border")
        .id;
    let rect = ui.rect(border).expect("laid out");
    ui.pointer_moved(rect.x + rect.width / 2.0, 300.0);
    assert_eq!(ui.cursor(), slime_input::CursorShape::ResizeHorizontal);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(400.0, 300.0);
    frame(&mut ui, &mut dock, slot);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    frame(&mut ui, &mut dock, slot);
    assert!(dock.take_changed());
    assert!((shares(&dock.layout().root)[0] - 0.5).abs() < 0.01);
    let hierarchy = dock.slot("hierarchy").expect("shown");
    assert!(
        (width(&ui, hierarchy) - 396.0).abs() < 2.0,
        "{}",
        width(&ui, hierarchy)
    );
    // Dragged past the window's edge: the right part keeps its least size.
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(900.0, 300.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    frame(&mut ui, &mut dock, slot);
    let viewport = dock.slot("viewport").expect("shown");
    assert!(
        width(&ui, viewport) >= LEAST - 1.0,
        "{}",
        width(&ui, viewport)
    );
    // By an agent, then by the keys, Home to the least size of the left part.
    assert!(ui.set_value(border, "0.25"));
    frame(&mut ui, &mut dock, slot);
    assert!((shares(&dock.layout().root)[0] - 0.25).abs() < 0.01);
    ui.set_focus(Some(border));
    named(&mut ui, NamedKey::ArrowRight, false);
    frame(&mut ui, &mut dock, slot);
    assert!((shares(&dock.layout().root)[0] - 0.3).abs() < 0.01);
    named(&mut ui, NamedKey::Home, false);
    frame(&mut ui, &mut dock, slot);
    let hierarchy = dock.slot("hierarchy").expect("shown");
    assert!(
        width(&ui, hierarchy) >= LEAST - 1.0,
        "{}",
        width(&ui, hierarchy)
    );
    // The console's tab chosen: its panel shown, the history's not, the layout changed.
    let console = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Tab && meaning.name == "CONSOLE")
        .expect("its tab")
        .id;
    assert_eq!(dock.panel_of_tab(console), Some("console"));
    dock.take_changed();
    ui.act(Action {
        element: Some(console),
        kind: ActionKind::Press,
    });
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("console").is_some() && dock.slot("history").is_none());
    assert!(dock.take_changed());
    // Moved, the panel shows in its new stack.
    assert!(dock.move_panel("history", &Target::Edge(Edge::Right)));
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("history").is_some());
}

#[test]
fn f6_goes_from_one_stack_of_a_dock_to_the_next() {
    let (mut ui, mut dock, slot) = docked(editor());
    // The first element of each stack that takes the focus: the tab shown.
    let tab = |ui: &Ui, name: &str| {
        ui.meaning()
            .into_iter()
            .find(|meaning| meaning.role == Role::Tab && meaning.name == name)
            .map(|meaning| meaning.id)
    };
    let mut visited = Vec::new();
    for _ in 0..4 {
        named(&mut ui, NamedKey::F6, false);
        frame(&mut ui, &mut dock, slot);
        visited.push(ui.focus());
    }
    assert_eq!(
        visited,
        [
            tab(&ui, "HIERARCHY"),
            tab(&ui, "COMPONENTS"),
            tab(&ui, "VIEWPORT"),
            tab(&ui, "HISTORY")
        ]
    );
    // Past the last, to nothing; Shift+F6 back to the last.
    named(&mut ui, NamedKey::F6, false);
    assert_eq!(ui.focus(), None);
    named(&mut ui, NamedKey::F6, true);
    assert_eq!(ui.focus(), tab(&ui, "HISTORY"));
}

/// The splitter of a split of two panels, `a` then `b`, docked in `direction`: the interface, the
/// dock, its slot and the splitter.
fn halves(direction: LayoutDirection, side_by_side: bool) -> (Ui, Dock, ElementId, ElementId) {
    let layout = Layout::new(Node::split(
        side_by_side,
        vec![(0.5, Node::tabs(&["a"])), (0.5, Node::tabs(&["b"]))],
    ));
    let (ui, dock, slot) = docked_in(direction, layout);
    let splitter = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Splitter)
        .expect("a splitter")
        .id;
    (ui, dock, slot, splitter)
}

/// Drags `splitter` from its middle to `to`, the actions that it gave read before the dock takes
/// them: their kinds, in their order.
fn drag(ui: &mut Ui, splitter: ElementId, to: [f32; 2]) -> Vec<ActionKind> {
    let rect = ui.rect(splitter).expect("laid out");
    ui.pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(to[0], to[1]);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.take_actions()
        .into_iter()
        .filter(|action| action.element == Some(splitter))
        .map(|action| action.kind)
        .collect()
}

#[test]
fn a_splitter_follows_the_pointer_from_right_to_left_and_up_and_down() {
    // From right to left, `a` lies on the right: dragged to 200 pixels from the left, it takes
    // what lies right of the pointer.
    let (mut ui, mut dock, slot, splitter) = halves(LayoutDirection::RightToLeft, true);
    let kinds = drag(&mut ui, splitter, [200.0, 300.0]);
    let (last, changes) = kinds.split_last().expect("actions");
    assert!(matches!(last, ActionKind::Committed(_)), "{kinds:?}");
    assert!(
        !changes.is_empty()
            && changes
                .iter()
                .all(|kind| matches!(kind, ActionKind::Changed(_))),
        "{kinds:?}"
    );
    let mut actions: Vec<Action> = kinds
        .into_iter()
        .map(|kind| Action {
            element: Some(splitter),
            kind,
        })
        .collect();
    dock.act(&mut actions);
    assert!(actions.is_empty(), "taken");
    frame(&mut ui, &mut dock, slot);
    let a = dock.slot("a").and_then(|a| ui.rect(a)).expect("shown");
    assert!((a.width - 596.0).abs() < 1.0 && a.x > 190.0, "{a:?}");
    // One above the other: dragged to 150 pixels from the top.
    let (mut ui, mut dock, slot, splitter) = halves(LayoutDirection::LeftToRight, false);
    let mut actions: Vec<Action> = drag(&mut ui, splitter, [400.0, 150.0])
        .into_iter()
        .map(|kind| Action {
            element: Some(splitter),
            kind,
        })
        .collect();
    dock.act(&mut actions);
    frame(&mut ui, &mut dock, slot);
    let a = dock.slot("a").and_then(|a| ui.rect(a)).expect("shown");
    assert!((a.height - 146.0).abs() < 1.0, "{a:?}");
}

#[test]
fn a_click_on_a_splitter_moves_nothing() {
    let (mut ui, _dock, _slot, splitter) = halves(LayoutDirection::LeftToRight, true);
    let rect = ui.rect(splitter).expect("laid out");
    ui.pointer_moved(rect.x + 1.0, 300.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(
        ui.take_actions()
            .iter()
            .all(|action| action.element != Some(splitter)),
        "no share given"
    );
}

#[test]
fn f6_from_outside_every_stack_goes_to_the_first() {
    let (mut ui, mut dock, slot, splitter) = halves(LayoutDirection::LeftToRight, true);
    ui.set_focus(Some(splitter));
    named(&mut ui, NamedKey::F6, false);
    frame(&mut ui, &mut dock, slot);
    let first = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Tab && meaning.name == "A")
        .map(|meaning| meaning.id);
    assert_eq!(ui.focus(), first);
}

#[test]
fn a_dock_made_anew_builds_its_layout_into_a_slot_that_another_built() {
    let (mut ui, _old, slot) = docked(editor());
    let mut dock = Dock::new(Layout::new(Node::tabs(&["history", "console"])));
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("history").is_some());
    assert!(dock.slot("hierarchy").is_none(), "the other dock's");
    // Shown by the program, its tab chosen.
    assert!(dock.show("console"));
    assert!(!dock.show("map"));
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("console").is_some() && dock.slot("history").is_none());
}
