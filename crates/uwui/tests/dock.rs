//! Docking, without a GPU (`docs/adr/0172-regions-of-tabs.md`,
//! `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`): a layout normalized after
//! each edit, and whole after random ones, a panel moved into another stack, to an edge of the
//! window or afloat, closed and shown again where it was, a border moved that changes the two
//! shares beside it and no other, passing over what is closed, a layout fitted to a program's
//! panels, mended as it is read, written and read back as text; a dock built into a tree, building
//! no closed panel, its splitters dragged by the pointer, moved by the keys and given a value by an
//! agent, each read by the dock, which keeps each child at its least size and settles as the drag
//! ends, its tabs chosen, F6 going from one of its stacks to the next, and its floating panels
//! placed where their stacks stood or in the middle of the window, kept where the pointer lets them
//! go, closed and opened again.

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use uwui::action::{Action, ActionKind};
use uwui::dock::{self, Dock, Edge, LEAST, Layout, Node, Read, ReadError, Reading, Target};
use uwui::host::{Person, TargetId};
use uwui::text::Fonts;
use uwui::tree::{ElementId, LayoutDirection, Role, RootSettings, Ui};

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

/// `layout` written as a document and read back strictly, without warnings.
fn written_and_read(layout: &Layout, default: Option<&str>) -> (String, Read) {
    let text = dock::write(layout, default).expect("written");
    let read = dock::read("written.layout.kdl", &text, Reading::Strict)
        .unwrap_or_else(|error| panic!("{text}\n{error}"));
    assert!(read.warnings.is_none(), "{text}");
    (text, read)
}

#[test]
fn a_layout_is_written_and_read_back_as_text() {
    let layout = editor();
    let (text, read) = written_and_read(&layout, Some("editor 1"));
    assert_eq!(read.layout, layout, "{text}");
    assert_eq!(read.default.as_deref(), Some("editor 1"));
    assert_eq!(
        text,
        "version 1
default \"editor 1\"
Split side_by_side=#true {
    children share=0.25 {
        Split side_by_side=#false {
            children share=0.5 { Tabs chosen=0 { panels hierarchy } }
            children share=0.5 { Tabs chosen=0 { panels components } }
        }
    }
    children share=0.75 {
        Split side_by_side=#false {
            children share=0.75 { Tabs chosen=0 { panels viewport } }
            children share=0.25 { Tabs chosen=0 { panels history console } }
        }
    }
}
"
    );
    // Without a default, none is written nor read.
    let (text, read) = written_and_read(&layout, None);
    assert!(!text.contains("default"));
    assert_eq!(read.default, None);
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
    let root = ui.add_root("main", settings, |s| s);
    let mut slot = None;
    ui.build(root, 0, |children| {
        let whole = uwui::tree::Element::container().style(|s| s.grow(1.0));
        slot = Some(children.add_slot("dock", whole));
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
    assert_eq!(
        ui.outputs(TargetId::MAIN).cursor,
        Some(slime_input::CursorShape::ResizeHorizontal)
    );
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
    assert_eq!(dock.panel_of(console), Some("console"));
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

/// The keys of each stack's panels, in the order of the tree.
fn stacks(layout: &Layout) -> Vec<Vec<String>> {
    layout
        .stacks()
        .into_iter()
        .map(<[String]>::to_vec)
        .collect()
}

/// The panel that the stack holding `panel` shows.
fn shown_with(layout: &Layout, panel: &str) -> Option<String> {
    fn find(node: &Node, panel: &str) -> Option<String> {
        match node {
            Node::Tabs { panels, chosen } => panels
                .iter()
                .any(|each| each == panel)
                .then(|| panels[*chosen as usize].clone()),
            Node::Split { children, .. } => {
                children.iter().find_map(|child| find(&child.node, panel))
            }
        }
    }
    find(&layout.root, panel)
}

#[test]
fn a_closed_panel_keeps_its_place_and_comes_back_there() {
    let mut layout = editor();
    let before = layout.clone();
    // The history closed: still in its stack, the console shown there; its shares unchanged.
    assert!(layout.close("history"));
    assert!(!layout.close("history"), "closed already");
    assert!(!layout.close("map"));
    assert_eq!(stacks(&layout), stacks(&before));
    assert!(!layout.is_open("history") && layout.holds("history"));
    assert_eq!(shown_with(&layout, "console").as_deref(), Some("console"));
    // The console closed too: the stack shows nothing, and keeps its share.
    assert!(layout.close("console"));
    assert_eq!(shares(&layout.root), shares(&before.root));
    // Other panels moved meanwhile, the history comes back to its stack, chosen.
    assert!(layout.move_panel("components", &Target::With("viewport".to_owned())));
    assert!(layout.show("history"));
    assert!(layout.is_open("history"));
    assert_eq!(shown_with(&layout, "history").as_deref(), Some("history"));
    assert_eq!(stacks(&layout)[2], ["history", "console"]);
    assert!(!layout.show("history"), "shown already");
    assert!(!layout.show("map"));
    // Moved while closed, it opens where it goes.
    assert!(layout.move_panel("console", &Target::Edge(Edge::Top)));
    assert!(layout.is_open("console"));
}

#[test]
fn a_panel_floats_and_docks_again() {
    let mut layout = editor();
    assert!(layout.move_panel("history", &Target::Float));
    assert!(layout.floats("history") && layout.is_open("history"));
    assert_eq!(
        layout.floating[0].size,
        [0.0, 0.0],
        "yet to be placed by the dock"
    );
    assert_eq!(stacks(&layout)[3], ["console"]);
    // Not afloat again, nor into a floating panel's stack, which it has none of.
    assert!(!layout.move_panel("history", &Target::Float));
    assert!(!layout.move_panel("console", &Target::With("history".to_owned())));
    // Closed afloat, opened again afloat, above the others.
    assert!(layout.move_panel("console", &Target::Float));
    assert!(layout.close("history"));
    assert!(layout.show("history"));
    let floating: Vec<&str> = layout.floating.iter().map(|f| f.panel.as_str()).collect();
    assert_eq!(floating, ["console", "history"]);
    // Docked again by a move.
    assert!(layout.move_panel("history", &Target::With("viewport".to_owned())));
    assert!(!layout.floats("history"));
    assert_eq!(shown_with(&layout, "viewport").as_deref(), Some("history"));
}

#[test]
fn a_border_passes_over_a_child_whose_panels_are_closed() {
    let mut layout = Layout::new(Node::split(
        true,
        vec![
            (0.2, Node::tabs(&["a"])),
            (0.3, Node::tabs(&["b"])),
            (0.5, Node::tabs(&["c"])),
        ],
    ));
    assert!(layout.close("b"));
    // The border after `a` lies between `a` and `c`.
    assert!(layout.resize(&[], 0, 0.5));
    let after = shares(&layout.root);
    assert!(
        (after[0] - 0.35).abs() < 1e-6 && (after[2] - 0.35).abs() < 1e-6,
        "{after:?}"
    );
    assert_eq!(after[1], 0.3);
    assert!(
        !layout.resize(&[], 1, 0.5),
        "after a child that does not show"
    );
}

#[test]
fn a_layout_fits_the_program_s_panels() {
    let mut layout = editor();
    layout.closed.push("map".to_owned());
    // An unknown panel goes; a panel that it lacks comes after the one before it in the list,
    // not chosen; another, first in the list, beside the one after it.
    layout.fit(&[
        "outline",
        "hierarchy",
        "components",
        "inspector",
        "viewport",
        "history",
    ]);
    assert_eq!(
        stacks(&layout),
        [
            vec!["hierarchy", "outline"],
            vec!["components", "inspector"],
            vec!["viewport"],
            vec!["history"]
        ]
    );
    assert!(layout.closed.is_empty());
    assert_eq!(
        shown_with(&layout, "inspector").as_deref(),
        Some("components")
    );
    // A layout of no known panel gives them in one stack.
    let mut layout = Layout::new(Node::tabs(&["map"]));
    layout.fit(&["a", "b"]);
    assert_eq!(stacks(&layout), [["a", "b"]]);
}

#[test]
fn a_layout_mends_its_floating_panels_and_its_closed_ones() {
    let mut layout = editor();
    layout.floating = vec![
        uwui::dock::Floating {
            panel: "map".to_owned(),
            origin: [f32::NAN, 40.0],
            size: [10.0, 500.0],
        },
        uwui::dock::Floating {
            panel: "notes".to_owned(),
            origin: [5.0, 5.0],
            size: [f32::INFINITY, 300.0],
        },
        // Docked already: it stays docked.
        uwui::dock::Floating {
            panel: "history".to_owned(),
            origin: [5.0, 5.0],
            size: [200.0, 200.0],
        },
    ];
    layout.closed = vec!["notes".to_owned(), "notes".to_owned(), "nothing".to_owned()];
    layout.normalize();
    assert_eq!(layout.floating.len(), 2);
    assert_eq!(layout.floating[0].origin, [0.0, 40.0]);
    assert_eq!(layout.floating[0].size, [LEAST, 500.0]);
    assert_eq!(layout.floating[1].size, [0.0, 0.0], "to be placed again");
    assert_eq!(layout.closed, ["notes"]);
}

#[test]
fn a_layout_with_floating_and_closed_panels_is_written_and_read_back() {
    let mut layout = editor();
    layout.move_panel("history", &Target::Float);
    layout.floating[0].origin = [100.0, 80.0];
    layout.floating[0].size = [300.0, 200.0];
    layout.close("components");
    let (text, read) = written_and_read(&layout, None);
    assert_eq!(read.layout, layout, "{text}");
    assert!(
        text.ends_with(
            "floating panel=history { origin 100 80; size 300 200 }\nclosed components\n"
        ),
        "{text}"
    );
    // A layout without floating nor closed panels writes neither, and reads with none.
    let docked = editor();
    let (text, read) = written_and_read(&docked, None);
    assert!(
        !text.contains("floating") && !text.contains("closed"),
        "{text}"
    );
    assert_eq!(read.layout, docked, "{text}");
}

/// A generator of numbers that repeats from its seed: xorshift.
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn bits(&mut self) -> u32 {
        self.below(1 << 32) as u32
    }
}

/// Whether `node` is whole: no split of fewer than two children nor within one of its direction,
/// shares from 0 to 1 summing to 1, no empty stack, each chosen panel one that the stack holds,
/// open where it holds one; its panels gathered in `panels`.
fn whole(node: &Node, parent: Option<bool>, closed: &[String], panels: &mut Vec<String>) {
    match node {
        Node::Tabs {
            panels: tabs,
            chosen,
        } => {
            assert!(!tabs.is_empty(), "an empty stack");
            let chosen = &tabs[*chosen as usize];
            assert!(
                !closed.contains(chosen) || tabs.iter().all(|tab| closed.contains(tab)),
                "a closed panel chosen beside an open one"
            );
            panels.extend(tabs.iter().cloned());
        }
        Node::Split {
            side_by_side,
            children,
        } => {
            assert!(children.len() >= 2, "a split of one child");
            assert_ne!(
                Some(*side_by_side),
                parent,
                "a split within one of its direction"
            );
            let sum: f32 = children.iter().map(|child| child.share).sum();
            assert!((sum - 1.0).abs() < 1e-4, "shares summing to {sum}");
            for child in children {
                assert!((0.0..=1.0).contains(&child.share));
                whole(&child.node, Some(*side_by_side), closed, panels);
            }
        }
    }
}

#[test]
fn a_layout_stays_whole_after_random_edits() {
    let keys = [
        "hierarchy",
        "components",
        "viewport",
        "history",
        "console",
        "map",
    ];
    let targets = [
        Target::Edge(Edge::Left),
        Target::Edge(Edge::Right),
        Target::Edge(Edge::Top),
        Target::Edge(Edge::Bottom),
        Target::Float,
    ];
    for seed in 1..=64_u64 {
        let mut numbers = Numbers(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut layout = editor();
        layout.fit(&keys);
        for _ in 0..200 {
            let panel = keys[numbers.below(keys.len())];
            match numbers.below(6) {
                0 => {
                    let other = keys[numbers.below(keys.len())];
                    layout.move_panel(panel, &Target::With(other.to_owned()));
                }
                1 => {
                    layout.move_panel(panel, &targets[numbers.below(targets.len())]);
                }
                2 => {
                    layout.close(panel);
                }
                3 => {
                    layout.show(panel);
                }
                4 => {
                    let mut path = Vec::new();
                    while numbers.below(2) == 0 {
                        path.push(numbers.below(3));
                    }
                    let share = numbers.below(101) as f32 / 100.0;
                    layout.resize(&path, numbers.below(4), share);
                }
                _ => {
                    // Read back as a program reads what it kept.
                    let text = dock::write(&layout, Some("random 1")).expect("written");
                    let read =
                        dock::read("random.layout.kdl", &text, Reading::Lenient).expect("read");
                    assert_eq!(read.layout, layout, "{text}");
                }
            }
            let mut panels = Vec::new();
            whole(&layout.root, None, &layout.closed, &mut panels);
            panels.extend(layout.floating.iter().map(|f| f.panel.clone()));
            panels.sort();
            let mut all: Vec<String> = keys.iter().map(|&key| key.to_owned()).collect();
            all.sort();
            assert_eq!(panels, all, "each panel once, seed {seed}");
            assert!(layout.closed.iter().all(|panel| layout.holds(panel)));
            let mut again = layout.clone();
            again.normalize();
            assert_eq!(again, layout, "normalized, seed {seed}");
        }
    }
}

/// The tab named `name`, if it shows.
fn tab(ui: &Ui, name: &str) -> Option<ElementId> {
    ui.meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Tab && meaning.name == name)
        .map(|meaning| meaning.id)
}

#[test]
fn a_dock_builds_no_closed_panel_and_no_border_beside_what_it_hides() {
    let (mut ui, mut dock, slot) = docked(editor());
    assert!(dock.close("components"));
    assert!(!dock.close("components"));
    assert!(dock.take_changed());
    frame(&mut ui, &mut dock, slot);
    assert!(tab(&ui, "COMPONENTS").is_none());
    assert!(dock.slot("components").is_none());
    // The left column holds the hierarchy alone, the whole height: no border in it.
    let splitters: Vec<String> = ui
        .meaning()
        .into_iter()
        .filter(|meaning| meaning.role == Role::Splitter)
        .map(|meaning| meaning.name)
        .collect();
    assert_eq!(
        splitters,
        [
            "between HIERARCHY and VIEWPORT",
            "between VIEWPORT and HISTORY"
        ]
    );
    let hierarchy = dock
        .slot("hierarchy")
        .and_then(|slot| ui.rect(slot))
        .expect("shown");
    assert!(hierarchy.height > 500.0, "{hierarchy:?}");
    // The history closed: the console shown in its stack, its tab alone.
    assert!(dock.close("history"));
    frame(&mut ui, &mut dock, slot);
    assert!(tab(&ui, "HISTORY").is_none() && dock.slot("console").is_some());
    // Both shown again where they were.
    assert!(dock.show("components") && dock.show("history"));
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("components").is_some() && dock.slot("history").is_some());
    assert_eq!(dock.layout(), &editor());
}

#[test]
fn a_dock_floats_a_panel_where_its_stack_stood_and_keeps_where_it_is_let_go() {
    let (mut ui, mut dock, slot) = docked(editor());
    let history = dock
        .slot("history")
        .and_then(|slot| ui.rect(slot))
        .expect("shown");
    assert!(dock.move_panel("history", &Target::Float));
    dock.take_changed();
    frame(&mut ui, &mut dock, slot);
    // Placed where its stack stood, the whole stack, its bar of tabs included.
    let floating = &dock.layout().floating[0];
    assert!(
        (floating.origin[0] - history.x).abs() < 1.0 && floating.origin[1] <= history.y,
        "{floating:?} {history:?}"
    );
    assert!((floating.size[0] - history.width).abs() < 1.0);
    let content = dock.slot("history").expect("its slot afloat");
    let panel = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Pane && meaning.name == "HISTORY")
        .expect("a floating panel")
        .id;
    assert_eq!(dock.panel_of(panel), Some("history"));
    assert!(
        ui.path_text(content)
            .expect("there")
            .starts_with("floating history/panel/content")
    );
    // Its bar dragged by the pointer: the dock keeps where it was let go.
    let bar = ui.rect(panel).expect("laid out");
    ui.pointer_moved(bar.x + bar.width / 2.0, bar.y + 8.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(bar.x + bar.width / 2.0 - 100.0, bar.y - 50.0);
    frame(&mut ui, &mut dock, slot);
    assert!(!dock.take_changed(), "not while the pointer holds it");
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    frame(&mut ui, &mut dock, slot);
    assert!(dock.take_changed());
    let moved = dock.layout().floating[0].origin;
    assert!(
        (moved[0] - (floating_origin(&ui, panel)[0])).abs() < 1.0,
        "{moved:?}"
    );
    // Closed, its layer goes; shown again, it opens where it was let go.
    assert!(dock.close("history"));
    frame(&mut ui, &mut dock, slot);
    assert!(dock.slot("history").is_none());
    assert!(ui.path_text(content).is_none(), "its layer gone");
    assert!(dock.show("history"));
    frame(&mut ui, &mut dock, slot);
    let panel = dock
        .slot("history")
        .and_then(|slot| ui.rect(slot))
        .expect("afloat");
    assert!((panel.x - moved[0]).abs() < 1.0, "{panel:?} {moved:?}");
    // Docked again, its layer goes.
    assert!(dock.move_panel("history", &Target::Edge(Edge::Bottom)));
    frame(&mut ui, &mut dock, slot);
    assert!(
        !ui.meaning()
            .iter()
            .any(|meaning| meaning.role == Role::Pane && meaning.name == "HISTORY"),
        "no floating panel"
    );
    assert!(dock.slot("history").is_some());
}

/// Where the floating panel `panel`'s layer stands, in logical pixels.
fn floating_origin(ui: &Ui, panel: ElementId) -> [f32; 2] {
    let rect = ui.rect(panel).expect("laid out");
    [rect.x, rect.y]
}

#[test]
fn a_panel_floated_from_no_stack_shown_stands_in_the_middle_of_the_window() {
    let mut dock = Dock::new(editor());
    // Floated before the dock was ever built: no stack stood anywhere.
    assert!(dock.move_panel("console", &Target::Float));
    let (mut ui, _, slot) = docked(Layout::new(Node::tabs(&["other"])));
    frame(&mut ui, &mut dock, slot);
    let floating = &dock.layout().floating[0];
    assert_eq!(floating.size, [800.0 / 3.0, 300.0]);
    assert_eq!(floating.origin, [(800.0 - 800.0 / 3.0) / 2.0, 150.0]);
}

#[test]
fn a_layout_set_anew_opens_its_floating_panels_again() {
    let mut layout = editor();
    layout.move_panel("history", &Target::Float);
    layout.floating[0].origin = [40.0, 30.0];
    layout.floating[0].size = [200.0, 150.0];
    let (mut ui, mut dock, slot) = docked(layout.clone());
    let first = dock.slot("history").expect("afloat");
    dock.set_layout(editor());
    assert!(dock.take_changed());
    frame(&mut ui, &mut dock, slot);
    assert!(ui.path_text(first).is_none(), "its layer gone");
    assert_eq!(dock.layout(), &editor());
    dock.set_layout(layout);
    frame(&mut ui, &mut dock, slot);
    let afloat = dock
        .slot("history")
        .and_then(|slot| ui.rect(slot))
        .expect("afloat");
    assert!((afloat.x - 40.0).abs() < 1.0, "{afloat:?}");
}

#[test]
fn a_border_s_drag_settles_as_it_is_let_go() {
    let (mut ui, mut dock, slot, splitter) = halves(LayoutDirection::LeftToRight, true);
    let rect = ui.rect(splitter).expect("laid out");
    ui.pointer_moved(rect.x + rect.width / 2.0, 300.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(300.0, 300.0);
    frame(&mut ui, &mut dock, slot);
    assert!(!dock.take_changed(), "a step of the drag");
    assert!(shares(&dock.layout().root)[0] < 0.4, "but shown");
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    frame(&mut ui, &mut dock, slot);
    assert!(dock.take_changed());
}

#[test]
fn a_splitter_across_a_closed_child_moves_the_two_beside_it() {
    let layout = Layout::new(Node::split(
        true,
        vec![
            (0.2, Node::tabs(&["a"])),
            (0.3, Node::tabs(&["b"])),
            (0.5, Node::tabs(&["c"])),
        ],
    ));
    let (mut ui, mut dock, slot) = docked(layout);
    assert!(dock.close("b"));
    frame(&mut ui, &mut dock, slot);
    let splitters: Vec<_> = ui
        .meaning()
        .into_iter()
        .filter(|meaning| meaning.role == Role::Splitter)
        .collect();
    assert_eq!(splitters.len(), 1);
    assert_eq!(splitters[0].name, "between A and C");
    // `a` and `c` share the width by their shares, 2 to 5.
    let width = |ui: &Ui, dock: &Dock, panel| {
        dock.slot(panel)
            .and_then(|slot| ui.rect(slot))
            .map_or(0.0, |rect| rect.width)
    };
    let (a, c) = (width(&ui, &dock, "a"), width(&ui, &dock, "c"));
    assert!((a / (a + c) - 0.2 / 0.7).abs() < 0.01, "{a} {c}");
    // Given a value as an agent gives it: `a` and `c` change, `b` keeps its share.
    assert!(ui.set_value(splitters[0].id, "0.5"));
    frame(&mut ui, &mut dock, slot);
    let after = shares(&dock.layout().root);
    assert!(
        (after[0] - 0.35).abs() < 1e-4 && (after[2] - 0.35).abs() < 1e-4,
        "{after:?}"
    );
    assert_eq!(after[1], 0.3);
    let (a, c) = (width(&ui, &dock, "a"), width(&ui, &dock, "c"));
    assert!((a - c).abs() < 1.0, "{a} {c}");
}

#[test]
fn a_floating_panel_opens_no_larger_than_the_window_and_an_agent_places_it() {
    let mut layout = editor();
    layout.move_panel("history", &Target::Float);
    layout.floating[0].origin = [10.0, 10.0];
    layout.floating[0].size = [3000.0, 2000.0];
    let (mut ui, mut dock, slot) = docked(layout);
    let floating = dock
        .slot("history")
        .and_then(|content| ui.rect(content))
        .expect("afloat");
    assert!(
        floating.width <= 800.0 && floating.height <= 600.0,
        "{floating:?}"
    );
    // An agent's place of no size, or not finite, is left out; another is kept by the dock.
    let panel = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.role == Role::Pane && meaning.name == "HISTORY")
        .expect("a floating panel")
        .id;
    let root = ui
        .path_text(panel)
        .map(|path| path.split('/').next().map(str::to_owned));
    assert_eq!(root.flatten().as_deref(), Some("floating history"));
    let layer = ui.element_by_path("floating history").expect("its layer");
    for size in [[0.0, 100.0], [f32::NAN, 100.0]] {
        ui.act(Action {
            element: Some(layer),
            kind: ActionKind::Placed {
                origin: [20.0, 20.0],
                size,
            },
        });
    }
    frame(&mut ui, &mut dock, slot);
    assert_eq!(
        dock.layout().floating[0].size,
        [3000.0, 2000.0],
        "nothing taken"
    );
    ui.act(Action {
        element: Some(layer),
        kind: ActionKind::Placed {
            origin: [20.0, 30.0],
            size: [300.0, 200.0],
        },
    });
    frame(&mut ui, &mut dock, slot);
    assert!(dock.take_changed());
    assert_eq!(dock.layout().floating[0].origin, [20.0, 30.0]);
    assert_eq!(dock.layout().floating[0].size, [300.0, 200.0]);
}

/// The lines of the errors that `text` gives, read as `reading` says; none for a text read.
fn error_lines(text: &str, reading: Reading) -> Vec<u32> {
    match dock::read("test.layout.kdl", text, reading) {
        Ok(_) => Vec::new(),
        Err(ReadError::Unreadable(errors)) => errors
            .errors()
            .iter()
            .map(|error| error.error.line)
            .collect(),
        Err(error) => panic!("{text}\n{error}"),
    }
}

/// A stack of one panel, as a layout's document writes it.
const STACK: &str = "Tabs chosen=0 { panels viewport }";

#[test]
fn a_layout_of_a_later_version_is_not_read() {
    // What follows its version is not read, whatever it is.
    let later = "version 2\nfuture { of the format }\n";
    assert!(matches!(
        dock::read("later.layout.kdl", later, Reading::Strict),
        Err(ReadError::Later { version: 2 })
    ));
    let text = format!("version 1\n{STACK}\n");
    assert!(dock::read("now.layout.kdl", &text, Reading::Strict).is_ok());
}

#[test]
fn what_a_layout_does_not_know_is_left_out_leniently_and_refused_strictly() {
    let text = format!(
        "version 1\n\
         default \"editor 1\" later=1\n\
         Split side_by_side=#true weight=2 {{\n\
         \x20   children share=0.5 3 {{ {STACK}; notes }}\n\
         \x20   children share=0.5 {{ Tabs chosen=0 {{ panels history; pinned history }} }}\n\
         \x20   gap 4\n\
         }}\n\
         floating panel=console {{ origin 1 2; size 300 200 7; folded #true }}\n\
         theme dark\n"
    );
    let read = dock::read("later.layout.kdl", &text, Reading::Lenient).expect("read");
    let warnings = read.warnings.expect("warnings");
    let lines: Vec<u32> = warnings
        .errors()
        .iter()
        .map(|warning| warning.error.line)
        .collect();
    assert_eq!(lines, [2, 3, 4, 4, 5, 6, 8, 8, 9], "{warnings}");
    let known = "version 1\n\
                 default \"editor 1\"\n\
                 Split side_by_side=#true {\n\
                 \x20   children share=0.5 { Tabs chosen=0 { panels viewport } }\n\
                 \x20   children share=0.5 { Tabs chosen=0 { panels history } }\n\
                 }\n\
                 floating panel=console { origin 1 2; size 300 200 }\n";
    let want = dock::read("known.layout.kdl", known, Reading::Strict).expect("read");
    assert_eq!(read.layout, want.layout);
    assert_eq!(read.default, want.default);
    assert_eq!(
        error_lines(&text, Reading::Strict),
        [2, 3, 4, 4, 5, 6, 8, 8, 9]
    );
}

#[test]
fn a_layout_s_document_refuses_what_it_cannot_mean() {
    for (text, lines) in [
        ("", vec![1]),
        (&*format!("{STACK}\nversion 1\n"), vec![1, 2]),
        (&*format!("version 1\nversion 1\n{STACK}\n"), vec![2]),
        (&*format!("version 0\n{STACK}\n"), vec![1]),
        (&*format!("version \"1\"\n{STACK}\n"), vec![1]),
        (&*format!("version 1.5\n{STACK}\n"), vec![1]),
        ("version 1\n", vec![2]),
        (&*format!("version 1\n{STACK}\n{STACK}\n"), vec![3]),
        (
            &*format!("version 1\ndefault a\ndefault b\n{STACK}\n"),
            vec![3],
        ),
        (&*format!("version 1\ndefault\n{STACK}\n"), vec![2]),
        (&*format!("version 1\ndefault 1\n{STACK}\n"), vec![2]),
        (
            &*format!("version 1\ndefault \"$editor\"\n{STACK}\n"),
            vec![2],
        ),
        ("version 1\n(stack)Tabs chosen=0 { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=(n)0 { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=0 chosen=1 { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=-1 { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=1.5 { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=\"0\" { panels a }\n", vec![2]),
        ("version 1\nTabs { panels a }\n", vec![2]),
        ("version 1\nTabs chosen=0\n", vec![2]),
        ("version 1\nTabs chosen=0 { panels a; panels b }\n", vec![2]),
        ("version 1\nTabs chosen=0 { panels \"$a\" }\n", vec![2]),
        ("version 1\nTabs chosen=0 { panels 1 }\n", vec![2]),
        (
            "version 1\nSplit { children share=1 { Tabs chosen=0 { panels a } } }\n",
            vec![2],
        ),
        ("version 1\nSplit side_by_side=1\n", vec![2]),
        (
            "version 1\nSplit side_by_side=#true {\n    children { Tabs chosen=0 { panels a } }\n}\n",
            vec![3],
        ),
        (
            "version 1\nSplit side_by_side=#true {\n    children share=a { Tabs chosen=0 { panels a } }\n}\n",
            vec![3],
        ),
        (
            "version 1\nSplit side_by_side=#true {\n    children share=1\n}\n",
            vec![3],
        ),
        (
            "version 1\nSplit side_by_side=#true {\n    children share=1 { Tabs chosen=0 { panels a }; Tabs chosen=0 { panels b } }\n}\n",
            vec![3],
        ),
        (
            &*format!("version 1\n{STACK}\nfloating {{ origin 1 2; size 3 4 }}\n"),
            vec![3],
        ),
        (
            &*format!("version 1\n{STACK}\nfloating panel=a {{ size 3 4 }}\n"),
            vec![3],
        ),
        (
            &*format!("version 1\n{STACK}\nfloating panel=a {{ origin 1; size 3 4 }}\n"),
            vec![3],
        ),
        (
            &*format!("version 1\n{STACK}\nfloating panel=a {{ origin 1 b; size 3 4 }}\n"),
            vec![3],
        ),
        (
            &*format!(
                "version 1\n{STACK}\nfloating panel=a {{ origin 1 2; origin 1 2; size 3 4 }}\n"
            ),
            vec![3],
        ),
        (
            &*format!("version 1\n{STACK}\nclosed a\nclosed b\n"),
            vec![4],
        ),
        (&*format!("version 1\n{STACK}\nclosed #true\n"), vec![3]),
    ] {
        assert_eq!(error_lines(text, Reading::Lenient), lines, "{text}");
    }
    // A value of the wrong kind, at its own line, after a node continued on the next.
    for (text, line) in [
        ("version \\\n    \"1\"\nTabs chosen=0 { panels a }\n", 2),
        (
            "version 1\ndefault \\\n    1\nTabs chosen=0 { panels a }\n",
            3,
        ),
        ("version 1\nTabs \\\n    chosen=\"x\" { panels a }\n", 3),
        (
            "version 1\nTabs chosen=0 {\n    panels a \\\n        #true\n}\n",
            4,
        ),
        ("version 1\nSplit \\\n    side_by_side=1\n", 3),
        (
            "version 1\nSplit side_by_side=#true {\n    children \\\n        share=a { Tabs chosen=0 { panels a } }\n}\n",
            4,
        ),
        (
            "version 1\nTabs chosen=0 { panels a }\nfloating \\\n    panel=1 { origin 1 2; size 3 4 }\n",
            4,
        ),
        (
            "version 1\nTabs chosen=0 { panels a }\nfloating panel=a {\n    origin 1 \\\n        b\n    size 3 4\n}\n",
            5,
        ),
    ] {
        assert_eq!(error_lines(text, Reading::Strict), [line], "{text}");
    }
    // A text that is not KDL, at its line.
    assert_eq!(
        error_lines("version 1\nTabs chosen=0 {\n", Reading::Lenient),
        [3]
    );
}

#[test]
fn a_layout_whose_splits_lie_deeper_than_a_document_is_read_is_not_written() {
    // Splits within each other, of alternate directions, `count` deep.
    let nested = |count: usize| {
        let mut node = Node::tabs(&["inner"]);
        for level in 0..count {
            node = Node::Split {
                side_by_side: level % 2 == 0,
                children: vec![
                    uwui::dock::Share { share: 0.5, node },
                    uwui::dock::Share {
                        share: 0.5,
                        node: Node::tabs(&[&format!("panel {level}")]),
                    },
                ],
            };
        }
        Layout {
            root: node,
            floating: Vec::new(),
            closed: Vec::new(),
        }
    };
    let deepest = nested(31);
    let (_, read) = written_and_read(&deepest, None);
    assert_eq!(read.layout, deepest);
    assert_eq!(
        dock::write(&nested(32), None),
        Err(dock::WriteError::TooDeep)
    );
}

#[test]
fn a_panel_or_a_default_named_with_a_dollar_is_not_written() {
    let mut layout = editor();
    assert_eq!(
        dock::write(&layout, Some("$editor")),
        Err(dock::WriteError::Dollar("$editor".to_owned()))
    );
    layout.closed = vec!["$notes".to_owned()];
    assert_eq!(
        dock::write(&layout, None),
        Err(dock::WriteError::Dollar("$notes".to_owned()))
    );
}

#[test]
fn a_damaged_layout_is_refused_or_read_never_panicking() {
    let mut layout = editor();
    layout.move_panel("history", &Target::Float);
    layout.floating[0].origin = [100.5, 80.25];
    layout.floating[0].size = [300.0, 200.0];
    layout.close("components");
    let text = dock::write(&layout, Some("editor 1")).expect("written");
    damage::damage(text.as_bytes(), |bytes| {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        for reading in [Reading::Lenient, Reading::Strict] {
            if let Ok(read) = dock::read("damaged.layout.kdl", text, reading) {
                let mut layout = read.layout;
                layout.normalize();
            }
        }
    });
}

/// Panels' keys, a program's and hostile ones: a space, a keyword, a number, a sign that KDL takes
/// as syntax, quotes, a backslash, a newline, a character that a document may not hold as itself,
/// another script, and nothing.
const HOSTILE_KEYS: [&str; 14] = [
    "hierarchy",
    "viewport",
    "a panel",
    "true",
    "-1",
    "a#b",
    "a=b",
    "say \"hi\"",
    "back\\slash",
    "two\nlines",
    "\u{7f}",
    "パネル",
    "",
    "null",
];

/// A random float: any pattern of bits, or a value that writing a float gets wrong most often.
fn random_float(numbers: &mut Numbers) -> f32 {
    const SPECIAL: [f32; 10] = [
        0.25,
        0.55,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
        f32::MAX,
        7.038_531e-26,
        -7.038_531e-26,
    ];
    if numbers.below(2) == 0 {
        SPECIAL[numbers.below(SPECIAL.len())]
    } else {
        f32::from_bits(numbers.bits())
    }
}

/// A random key.
fn random_key(numbers: &mut Numbers) -> String {
    HOSTILE_KEYS[numbers.below(HOSTILE_KEYS.len())].to_owned()
}

/// A random part of a layout, at most `depth` splits deep: splits of none to four children, stacks
/// of none to four panels, a stack's chosen panel any place.
fn random_node(numbers: &mut Numbers, depth: usize) -> Node {
    if depth == 0 || numbers.below(3) == 0 {
        let panels = (0..numbers.below(5)).map(|_| random_key(numbers)).collect();
        let chosen = if numbers.below(2) == 0 {
            numbers.below(5) as u32
        } else {
            numbers.bits()
        };
        return Node::Tabs { panels, chosen };
    }
    Node::Split {
        side_by_side: numbers.below(2) == 0,
        children: (0..numbers.below(5))
            .map(|_| dock::Share {
                share: random_float(numbers),
                node: random_node(numbers, depth - 1),
            })
            .collect(),
    }
}

/// A random layout, as a program may hold it before it normalizes it.
fn random_layout(numbers: &mut Numbers) -> Layout {
    Layout {
        root: random_node(numbers, 6),
        floating: (0..numbers.below(4))
            .map(|_| dock::Floating {
                panel: random_key(numbers),
                origin: [random_float(numbers), random_float(numbers)],
                size: [random_float(numbers), random_float(numbers)],
            })
            .collect(),
        closed: (0..numbers.below(4)).map(|_| random_key(numbers)).collect(),
    }
}

/// Random layouts, not normalized, of hostile keys and any float, written and read back strictly
/// to the same layout, bit for bit: `Debug` writes the shortest text that reads back to each finite
/// float, `-0.0` apart from `0.0`, and every value that is not a number as `NaN`. A new seed at each
/// run, printed on failure; `UWUI_DOCK_SEED` replays one.
#[test]
fn random_layouts_are_written_and_read_back_bit_for_bit() {
    let seed = std::env::var("UWUI_DOCK_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |time| time.as_nanos() as u64)
                | 1
        });
    let mut numbers = Numbers(seed);
    for case in 0..1_000 {
        let layout = random_layout(&mut numbers);
        let text = dock::write(&layout, None).expect("written as KDL");
        let context = || format!("case {case}, from the seed UWUI_DOCK_SEED={seed}:\n{text}");
        let read = dock::read("random.layout.kdl", &text, Reading::Strict)
            .unwrap_or_else(|error| panic!("{}\nread: {error}", context()));
        assert!(read.warnings.is_none(), "{}", context());
        assert_eq!(
            format!("{:?}", read.layout),
            format!("{layout:?}"),
            "{}",
            context()
        );
    }
}
