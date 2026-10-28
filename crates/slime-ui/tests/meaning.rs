//! What the tree means, without a GPU: the role, name, states, actions, bounds and path of each
//! element that means something, the boxes that only lay others out left out; an element found by
//! its path and at a pixel; the part of an element within its root; and a name or a role changed,
//! which draws nothing again.

use slime_ui::Rect;
use slime_ui::style::{Dimension, Display, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Element, ElementAction, ElementId, ElementState, LayoutDirection, Meaning, Role, RootSettings,
    Ui,
};

/// An interface whose fonts are the tests' Noto Sans.
fn ui() -> Ui {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    Ui::new(fonts)
}

/// Settings of a root of `width` by `height` logical pixels at `origin`, at scale 1.
fn settings(width: f32, height: f32, origin: [f32; 2], modal: bool) -> RootSettings {
    RootSettings {
        size: [width, height],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin,
        modal,
    }
}

/// A style of `width` by `height` logical pixels, which does not shrink.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style
}

/// What `meaning` says, short: its path, role, name and actions.
fn summary(meaning: &Meaning) -> (String, Role, String, Vec<ElementAction>) {
    (
        meaning.path.clone(),
        meaning.role,
        meaning.name.clone(),
        meaning.actions.clone(),
    )
}

/// An interface of a bar of two buttons, the second disabled, a text, a box that only lays out a
/// named panel, a box that scrolls three cards, the second cut by it and the third hidden whole, a
/// hidden button, and a list whose rows show: what its root holds, and its root.
fn interface() -> (Ui, ElementId) {
    let mut ui = ui();
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let build = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            children.add("page", Element::container(column.clone()), |children| {
                let bar = Element::container(sized(400.0, 40.0))
                    .role(Role::Toolbar)
                    .named("tools");
                children.add("bar", bar, |children| {
                    children.add("open", Element::button("open", sized(100.0, 40.0)), |_| {});
                    children.add("save", Element::button("save", sized(100.0, 40.0)), |_| {});
                });
                children.add(
                    "status",
                    Element::label("ready", sized(400.0, 20.0)),
                    |_| {},
                );
                children.add(
                    "layout",
                    Element::container(sized(400.0, 40.0)),
                    |children| {
                        let panel = Element::container(sized(100.0, 40.0)).named("panel");
                        children.add("panel", panel, |_| {});
                    },
                );
                let mut scroll = sized(150.0, 40.0);
                scroll.layout.direction = FlexDirection::Column;
                children.add("scroll", Element::scroll(scroll), |children| {
                    children.add(
                        "first",
                        Element::button("first", sized(150.0, 30.0)),
                        |_| {},
                    );
                    children.add(
                        "second",
                        Element::button("second", sized(150.0, 30.0)),
                        |_| {},
                    );
                    children.add(
                        "third",
                        Element::button("third", sized(150.0, 30.0)),
                        |_| {},
                    );
                });
                let mut hidden = sized(100.0, 20.0);
                hidden.layout.display = Display::None;
                children.add("hidden", Element::button("hidden", hidden), |_| {});
                children.add(
                    "list",
                    Element::list(1_000, 20.0, sized(200.0, 60.0)),
                    |rows| {
                        for row in rows.rows() {
                            rows.add(row, Element::container(sized(200.0, 20.0)), |_| {});
                        }
                    },
                );
            });
        });
    };
    // The first frame lays the list out; the next builds the rows that it shows.
    build(&mut ui);
    ui.frame();
    build(&mut ui);
    ui.frame();
    let save = ui
        .element_by_path("main/page/bar/save")
        .expect("the button");
    let mut disabled = ui.state(save).expect("an element");
    disabled.disabled = true;
    ui.set_state(save, disabled);
    ui.frame();
    (ui, root)
}

#[test]
fn each_element_that_means_something_has_its_role_name_actions_and_path() {
    let (ui, _) = interface();
    let meaning = ui.meaning();
    let summaries: Vec<_> = meaning.iter().map(summary).collect();
    let press = vec![ElementAction::Press, ElementAction::Focus];
    let text = str::to_owned;
    assert_eq!(
        summaries[..10],
        [
            (text("main/page/bar"), Role::Toolbar, text("tools"), vec![]),
            (
                text("main/page/bar/open"),
                Role::Button,
                text("open"),
                press.clone()
            ),
            (
                text("main/page/bar/save"),
                Role::Button,
                text("save"),
                vec![]
            ),
            (text("main/page/status"), Role::Label, text("ready"), vec![]),
            (
                text("main/page/layout/panel"),
                Role::Group,
                text("panel"),
                vec![]
            ),
            (
                text("main/page/scroll"),
                Role::ScrollView,
                text(""),
                vec![ElementAction::Scroll]
            ),
            (
                text("main/page/scroll/first"),
                Role::Button,
                text("first"),
                press.clone()
            ),
            (
                text("main/page/scroll/second"),
                Role::Button,
                text("second"),
                press.clone()
            ),
            (
                text("main/page/scroll/third"),
                Role::Button,
                text("third"),
                press
            ),
            (
                text("main/page/list"),
                Role::List,
                text(""),
                vec![ElementAction::Scroll]
            ),
        ],
        "the root, the page and the box that lays out the panel mean nothing; the hidden button is \
         left out"
    );
    let rows: Vec<_> = summaries[10..]
        .iter()
        .map(|row| (row.0.as_str(), row.1))
        .collect();
    assert_eq!(
        rows,
        [
            ("main/page/list/#0", Role::ListItem),
            ("main/page/list/#1", Role::ListItem),
            ("main/page/list/#2", Role::ListItem),
        ]
    );
    let save = &meaning[2];
    assert_eq!(
        save.state,
        ElementState {
            disabled: true,
            ..ElementState::default()
        }
    );
    // Where each shows: the bar at the top; the second card of the box that scrolls, below its
    // first, cut by the box at 40 pixels; the third, below, nowhere.
    assert_eq!(meaning[1].bounds, Some(Rect::new(0.0, 0.0, 100.0, 40.0)));
    assert_eq!(meaning[6].bounds, Some(Rect::new(0.0, 100.0, 150.0, 30.0)));
    assert_eq!(meaning[7].bounds, Some(Rect::new(0.0, 130.0, 150.0, 10.0)));
    assert_eq!(meaning[8].bounds, None);
}

#[test]
fn an_element_is_found_by_its_path_and_at_a_pixel() {
    let (mut ui, _) = interface();
    for meaning in ui.meaning() {
        assert_eq!(ui.element_by_path(&meaning.path), Some(meaning.id));
        assert_eq!(
            ui.path_text(meaning.id).as_deref(),
            Some(meaning.path.as_str())
        );
    }
    // A number is `#` and digits alone; nothing follows the last key.
    for wrong in [
        "",
        "0/page",
        "#1/page",
        "other/page",
        "#+0/page",
        "main/page/",
        "main/page/nothing",
        "main/page/list/#999",
    ] {
        assert_eq!(ui.element_by_path(wrong), None, "{wrong}");
    }
    let open = ui
        .element_by_path("main/page/bar/open")
        .expect("the button");
    assert_eq!(
        ui.meaning_at(50.0, 20.0).map(|meaning| meaning.id),
        Some(open)
    );
    // In the box that lays the panel out, beside the panel: the box means nothing, nor do the page
    // and the root above it.
    assert_eq!(ui.meaning_at(300.0, 80.0), None);
    // Under a modal root, the elements below take no action, and the pointer finds the dialog.
    let modal = ui.add_root(
        "confirm",
        settings(200.0, 100.0, [100.0, 100.0], true),
        Style::default(),
    );
    ui.build(modal, 0, |children| {
        let dialog = Element::container(sized(200.0, 100.0))
            .role(Role::Dialog)
            .named("confirm");
        children.add("dialog", dialog, |children| {
            children.add("ok", Element::button("ok", sized(80.0, 30.0)), |_| {});
            // A name that `#` begins, which no digits follow, is a name.
            children.add("#no", Element::button("no", sized(80.0, 30.0)), |_| {});
        });
    });
    ui.frame();
    let no = ui
        .element_by_path("confirm/dialog/#no")
        .expect("named `#no`");
    assert_eq!(ui.path_text(no).as_deref(), Some("confirm/dialog/#no"));
    let below = ui.meaning_of(open).expect("drawn");
    assert!(below.actions.is_empty(), "{below:?}");
    let dialog = ui.meaning_at(250.0, 180.0).expect("the dialog");
    assert_eq!(
        (dialog.path.as_str(), dialog.role),
        ("confirm/dialog", Role::Dialog)
    );
    let ok = ui.element_by_path("confirm/dialog/ok").expect("the button");
    let ok = ui.meaning_of(ok).expect("drawn");
    assert_eq!(ok.actions, [ElementAction::Press, ElementAction::Focus]);
    assert!(ok.state.focused, "the modal root took the focus");
}

#[test]
fn a_name_or_a_role_changed_draws_nothing_again() {
    let mut ui = ui();
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let build = |ui: &mut Ui, revision: u64, name: &str, role: Role| {
        ui.build(root, revision, |children| {
            let panel = Element::container(sized(100.0, 40.0))
                .named(name)
                .role(role);
            children.add("panel", panel, |_| {});
        });
    };
    build(&mut ui, 0, "before", Role::Group);
    ui.frame();
    build(&mut ui, 1, "after", Role::Group);
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (0, 0));
    build(&mut ui, 2, "after", Role::Toolbar);
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (0, 0));
    let panel = ui.element_by_path("main/panel").expect("the panel");
    assert_eq!(
        ui.meaning_of(panel)
            .map(|meaning| (meaning.name, meaning.role)),
        Some(("after".to_owned(), Role::Toolbar))
    );
}

#[test]
fn an_element_shows_only_within_its_root() {
    let mut ui = ui();
    let root = ui.add_root(
        "main",
        settings(100.0, 30.0, [0.0; 2], false),
        Style::default(),
    );
    ui.build(root, 0, |children| {
        for name in ["inside", "across", "beyond"] {
            children.add(name, Element::button(name, sized(80.0, 30.0)), |_| {});
        }
    });
    ui.frame();
    let bounds: Vec<_> = ui.meaning().iter().map(|meaning| meaning.bounds).collect();
    assert_eq!(
        bounds,
        [
            Some(Rect::new(0.0, 0.0, 80.0, 30.0)),
            Some(Rect::new(80.0, 0.0, 20.0, 30.0)),
            None,
        ]
    );
}
