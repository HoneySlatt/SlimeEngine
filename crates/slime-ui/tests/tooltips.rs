//! Tooltips, and what the pointer resting does, without a GPU (`docs/adr/0150-layers.md`): a
//! tooltip that shows below its element once the pointer has rested on it for half a second of the
//! interface's clock, and goes as the pointer leaves, presses or a key goes down; that the keys show
//! as they give its element the focus; that the pointer passes through; that screen readers read
//! as a description; an item of a menu that opens its submenu as the pointer rests on it.

use std::time::Duration;

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::ActionKind;
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, ItemMark, Layer, LayoutDirection, Place, RootSettings, Side, Ui,
};

/// A style of the tests' font, `width` by 30 logical pixels where `width` is given.
fn style(width: Option<f32>) -> Style {
    let mut style = Style::default();
    if let Some(width) = width {
        style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(30.0)];
        style.layout.shrink = 0.0;
    }
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// An interface of 400 by 300 whose root, a column, holds two buttons of 100 by 30, the first with
/// a tooltip: the interface, its root and the buttons.
fn interface() -> (Ui, ElementId, [ElementId; 2]) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
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
    let mut column = style(None);
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root("main", settings, column);
    let mut buttons = Vec::new();
    ui.build(root, 0, |children| {
        let save = Element::button("Save", style(Some(100.0))).tooltip("Save the level");
        buttons.push(children.add("save", save, |_| {}));
        buttons.push(children.add("load", Element::button("Load", style(Some(100.0))), |_| {}));
    });
    ui.frame();
    (ui, root, [buttons[0], buttons[1]])
}

/// The clock moved on by `millis`, and a frame.
fn wait(ui: &mut Ui, millis: u64) {
    ui.advance(Duration::from_millis(millis));
    ui.frame();
}

/// The tooltip's layer, the root after the first, if one shows.
fn tooltip(ui: &Ui) -> Option<ElementId> {
    ui.element_by_path("tooltip/tooltip")
}

fn key(ui: &mut Ui, named: NamedKey) -> bool {
    let taken = ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(named),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.frame();
    taken
}

#[test]
fn a_tooltip_shows_below_its_element_once_the_pointer_has_rested_on_it() {
    let (mut ui, _, [save, _]) = interface();
    ui.pointer_moved(50.0, 15.0);
    let (_, stats) = ui.frame();
    assert!(stats.animating, "the clock must move on");
    wait(&mut ui, 400);
    assert!(tooltip(&ui).is_none());
    wait(&mut ui, 200);
    let tip = tooltip(&ui).expect("it shows");
    let (rect, below) = (
        ui.rect(save).expect("laid out"),
        ui.rect(tip).expect("laid out"),
    );
    assert!(below.y >= rect.y + rect.height);
    // In the middle of its element, from left to right.
    let middle = |rect: slime_ui::Rect| rect.x + rect.width / 2.0;
    assert!((middle(rect) - middle(below)).abs() < 1.0);
    let (_, stats) = ui.frame();
    assert!(!stats.animating);
    // The pointer passes through it.
    let (x, y) = (below.x + below.width / 2.0, below.y + below.height / 2.0);
    assert_ne!(
        ui.element_at(x, y).and_then(|id| ui.path_text(id)),
        ui.path_text(tip)
    );
}

#[test]
fn it_goes_as_the_pointer_leaves_presses_or_a_key_goes_down() {
    let (mut ui, _, _) = interface();
    ui.pointer_moved(50.0, 15.0);
    wait(&mut ui, 600);
    assert!(tooltip(&ui).is_some());
    ui.pointer_moved(50.0, 45.0);
    ui.frame();
    assert!(tooltip(&ui).is_none(), "it left");
    // Back, then a press: it goes, and does not show again while the pointer rests.
    ui.pointer_moved(50.0, 15.0);
    wait(&mut ui, 600);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    wait(&mut ui, 600);
    assert!(tooltip(&ui).is_none(), "a press");
    ui.take_actions();
    // A key hides it; Escape does nothing more.
    ui.pointer_moved(50.0, 45.0);
    ui.pointer_moved(50.0, 15.0);
    wait(&mut ui, 600);
    assert!(tooltip(&ui).is_some());
    assert!(key(&mut ui, NamedKey::Escape));
    assert!(tooltip(&ui).is_none(), "a key");
    assert!(ui.take_actions().is_empty());
}

#[test]
fn the_keys_show_the_tooltip_of_the_element_that_they_give_the_focus() {
    let (mut ui, _, [save, load]) = interface();
    ui.set_focus(Some(load));
    key(&mut ui, NamedKey::Tab);
    assert_eq!(ui.focus(), Some(save));
    assert!(tooltip(&ui).is_some(), "at once");
    key(&mut ui, NamedKey::Tab);
    assert!(tooltip(&ui).is_none());
}

#[test]
fn screen_readers_read_it_as_its_elements_description_and_not_twice() {
    let (mut ui, _, [save, load]) = interface();
    let meaning = ui.meaning_of(save).expect("drawn");
    assert_eq!(meaning.description.as_deref(), Some("Save the level"));
    assert_eq!(ui.meaning_of(load).expect("drawn").description, None);
    ui.pointer_moved(50.0, 15.0);
    wait(&mut ui, 600);
    assert!(tooltip(&ui).is_some());
    assert!(
        !ui.meaning()
            .iter()
            .any(|meaning| meaning.name == "Save the level")
    );
}

#[test]
fn an_item_of_a_menu_opens_its_submenu_as_the_pointer_rests_on_it() {
    let (mut ui, root, _) = interface();
    let layer = ui.open_layer(
        "layer",
        Layer {
            place: Place::At {
                over: root,
                point: [200.0, 100.0],
            },
            modal: false,
            dismiss: true,
        },
        style(None),
    );
    let mut items = Vec::new();
    ui.build(layer, 0, |children| {
        let mut column = style(None);
        column.layout.direction = FlexDirection::Column;
        children.add("menu", Element::menu(column), |menu| {
            for (label, mark) in [("new", ItemMark::None), ("recent", ItemMark::Submenu)] {
                let item = Element::menu_item(label, "", mark, style(None));
                items.push(menu.add(label, item, |_| {}));
            }
        });
    });
    ui.frame();
    let middle = |ui: &Ui, id: ElementId| {
        let rect = ui.rect(id).expect("laid out");
        (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
    };
    let (x, y) = middle(&ui, items[1]);
    ui.pointer_moved(x, y);
    wait(&mut ui, 100);
    assert!(ui.take_actions().is_empty());
    wait(&mut ui, 200);
    let actions = ui.take_actions();
    assert_eq!(
        (actions.len(), actions[0].element, &actions[0].kind),
        (1, Some(items[1]), &ActionKind::Press)
    );
    // The program opens it; the pointer resting on the other item closes it.
    let submenu = ui.open_layer(
        "layer",
        Layer {
            place: Place::Beside {
                element: items[1],
                side: Side::End,
                along: Along::Start,
            },
            modal: false,
            dismiss: true,
        },
        style(None),
    );
    ui.build(submenu, 0, |children| {
        children.add("menu", Element::menu(style(None)), |menu| {
            menu.add(
                "one",
                Element::menu_item("one", "", ItemMark::None, style(None)),
                |_| {},
            );
        });
    });
    ui.frame();
    let (x, y) = middle(&ui, items[0]);
    ui.pointer_moved(x, y);
    wait(&mut ui, 300);
    assert!(ui.rect(submenu).is_none());
    let kinds: Vec<ActionKind> = ui.take_actions().into_iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [ActionKind::Dismissed]);
}
