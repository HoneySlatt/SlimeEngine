//! Menus, without a GPU (`docs/adr/0150-layers.md`): the arrows that move among a menu's items,
//! around, past those disabled; the arrows that open a submenu and close it, from either direction;
//! a press on an item that closes the menus that led to it, telling the program, and gives the
//! focus back; the pointer that gives an item the focus; the columns of an item; their meaning.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::{Action, ActionKind};
use slime_ui::style::{FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, ElementState, ItemMark, Layer, LayoutDirection, Place, Role,
    RootSettings, Side, Ui,
};

/// A column of the tests' font.
fn column() -> Style {
    let mut style = Style::default();
    style.layout.direction = FlexDirection::Column;
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// A menu open below a button, by the keyboard: the interface, the button, the menu's layer and
/// its items, `new` with a shortcut, `save` disabled, `grid` checked and `recent`, which opens a
/// submenu.
struct Menu {
    ui: Ui,
    button: ElementId,
    layer: ElementId,
    items: [ElementId; 4],
}

impl Menu {
    fn open(direction: LayoutDirection) -> Self {
        let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
        let mut fonts = Fonts::new();
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, column());
        let mut button = None;
        ui.build(root, 0, |children| {
            button = Some(children.add("file", Element::button("File", column()), |_| {}));
        });
        ui.frame();
        let button = button.expect("built");
        ui.set_focus(Some(button));
        let layer = open_menu(
            &mut ui,
            Place::Beside {
                element: button,
                side: Side::Below,
                along: Along::Start,
            },
            &[
                ("new", "Ctrl+N", ItemMark::None),
                ("save", "", ItemMark::None),
                ("grid", "", ItemMark::Check(true)),
                ("recent", "", ItemMark::Submenu),
            ],
        );
        let items = items(&ui, layer);
        ui.set_state(
            items[1],
            ElementState {
                disabled: true,
                ..ElementState::default()
            },
        );
        ui.frame();
        let items = [items[0], items[1], items[2], items[3]];
        Self {
            ui,
            button,
            layer,
            items,
        }
    }

    /// Presses `key`, and lays out.
    fn key(&mut self, key: NamedKey) {
        self.ui.key(&KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Named(key),
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
        });
        self.ui.frame();
    }

    /// Opens the submenu of `recent`, as the program does when it reads its press: its layer and
    /// its items, `one` and `two`.
    fn open_submenu(&mut self) -> (ElementId, Vec<ElementId>) {
        let recent = self.items[3];
        let layer = open_menu(
            &mut self.ui,
            Place::Beside {
                element: recent,
                side: Side::End,
                along: Along::Start,
            },
            &[("one", "", ItemMark::None), ("two", "", ItemMark::None)],
        );
        (layer, items(&self.ui, layer))
    }

    /// The kinds of the actions that the program has not taken yet, with their elements, taken.
    fn actions(&mut self) -> Vec<(Option<ElementId>, ActionKind)> {
        self.ui
            .take_actions()
            .into_iter()
            .map(|action| (action.element, action.kind))
            .collect()
    }
}

/// Opens a layer at `place` that closes as the user leaves it, holding a menu of `items`, each a
/// label, a shortcut and a mark: the layer.
fn open_menu(ui: &mut Ui, place: Place, items: &[(&str, &str, ItemMark)]) -> ElementId {
    let layer = ui.open_layer(
        Layer {
            place,
            modal: false,
            dismiss: true,
        },
        column(),
    );
    ui.build(layer, 0, |children| {
        children.add("menu", Element::menu(column()), |menu| {
            for &(label, shortcut, mark) in items {
                menu.add(
                    label,
                    Element::menu_item(label, shortcut, mark, column()),
                    |_| {},
                );
            }
        });
    });
    ui.frame();
    layer
}

/// The items of the menu that the layer `layer` holds.
fn items(ui: &Ui, layer: ElementId) -> Vec<ElementId> {
    ui.children(ui.children(layer)[0]).to_vec()
}

/// The middle of `id`'s box.
fn middle(ui: &Ui, id: ElementId) -> (f32, f32) {
    let rect = ui.rect(id).expect("laid out");
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

#[test]
fn the_arrows_move_among_a_menus_items_around_past_those_disabled() {
    let mut menu = Menu::open(LayoutDirection::LeftToRight);
    let [new, _, grid, recent] = menu.items;
    // Opened by the keyboard: its first item holds the focus.
    assert_eq!(menu.ui.focus(), Some(new));
    menu.key(NamedKey::ArrowDown);
    assert_eq!(menu.ui.focus(), Some(grid), "past the disabled item");
    menu.key(NamedKey::ArrowDown);
    menu.key(NamedKey::ArrowDown);
    assert_eq!(menu.ui.focus(), Some(new), "around");
    menu.key(NamedKey::ArrowUp);
    assert_eq!(menu.ui.focus(), Some(recent));
    menu.key(NamedKey::Home);
    assert_eq!(menu.ui.focus(), Some(new));
    menu.key(NamedKey::End);
    assert_eq!(menu.ui.focus(), Some(recent));
    assert!(menu.actions().is_empty());
}

#[test]
fn the_arrows_open_a_submenu_and_close_it_the_focus_back_on_its_item() {
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let (forward, backward) = match direction {
            LayoutDirection::LeftToRight => (NamedKey::ArrowRight, NamedKey::ArrowLeft),
            LayoutDirection::RightToLeft => (NamedKey::ArrowLeft, NamedKey::ArrowRight),
        };
        let mut menu = Menu::open(direction);
        let recent = menu.items[3];
        menu.key(NamedKey::End);
        // The arrow towards the end presses the item, whose submenu the program opens.
        menu.key(forward);
        assert_eq!(menu.actions(), [(Some(recent), ActionKind::Press)]);
        let (submenu, items) = menu.open_submenu();
        assert_eq!(menu.ui.focus(), Some(items[0]), "{direction:?}");
        assert!(menu.ui.state(recent).expect("there").expanded);
        // Towards the end again, on the item whose submenu is open: nothing more opens.
        menu.ui.set_focus(Some(recent));
        menu.key(forward);
        assert!(menu.actions().is_empty());
        assert_eq!(menu.ui.focus(), Some(items[0]));
        menu.key(backward);
        assert!(menu.ui.rect(submenu).is_none());
        assert_eq!(menu.actions(), [(Some(submenu), ActionKind::Dismissed)]);
        assert_eq!(menu.ui.focus(), Some(recent));
        assert!(!menu.ui.state(recent).expect("there").expanded);
        // Towards the start in the first menu: it stays.
        menu.key(backward);
        assert!(menu.ui.rect(menu.layer).is_some());
    }
}

#[test]
fn a_press_on_an_item_closes_the_menus_that_led_to_it_and_gives_the_focus_back() {
    let mut menu = Menu::open(LayoutDirection::LeftToRight);
    assert!(menu.ui.state(menu.button).expect("there").expanded);
    menu.key(NamedKey::End);
    menu.key(NamedKey::Enter);
    menu.actions();
    let (submenu, items) = menu.open_submenu();
    menu.key(NamedKey::ArrowDown);
    menu.key(NamedKey::Enter);
    assert_eq!(
        menu.actions(),
        [
            (Some(items[1]), ActionKind::Press),
            (Some(submenu), ActionKind::Dismissed),
            (Some(menu.layer), ActionKind::Dismissed),
        ]
    );
    assert!(menu.ui.rect(menu.layer).is_none() && menu.ui.rect(submenu).is_none());
    assert_eq!(menu.ui.focus(), Some(menu.button));
    assert!(!menu.ui.state(menu.button).expect("there").expanded);
}

#[test]
fn the_pointer_over_an_item_gives_it_a_focus_that_does_not_show_and_its_press_closes() {
    let mut menu = Menu::open(LayoutDirection::LeftToRight);
    let [_, save, grid, recent] = menu.items;
    let (x, y) = middle(&menu.ui, grid);
    menu.ui.pointer_moved(x, y);
    assert_eq!(menu.ui.focus(), Some(grid));
    assert!(!menu.ui.state(grid).expect("there").focus_visible);
    // A disabled item takes nothing.
    let (x, y) = middle(&menu.ui, save);
    menu.ui.pointer_moved(x, y);
    assert_eq!(menu.ui.focus(), Some(grid));
    // An item that opens a submenu stays open as it is pressed, once its submenu is open too.
    let (x, y) = middle(&menu.ui, recent);
    for _ in 0..2 {
        menu.ui.pointer_moved(x, y);
        menu.ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        menu.ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        menu.ui.frame();
        if menu.ui.take_actions().len() == 1 {
            menu.open_submenu();
        }
    }
    assert!(menu.ui.rect(menu.layer).is_some());
    assert!(menu.ui.take_actions().is_empty());
    // Opened by the pointer, the submenu leaves the focus where it was.
    assert_eq!(menu.ui.focus(), Some(recent));
    let (x, y) = middle(&menu.ui, grid);
    menu.ui.pointer_moved(x, y);
    menu.ui
        .pointer_button(MouseButton::Left, ButtonState::Pressed);
    menu.ui
        .pointer_button(MouseButton::Left, ButtonState::Released);
    menu.ui.frame();
    let kinds: Vec<ActionKind> = menu.actions().into_iter().map(|(_, kind)| kind).collect();
    // The press in the menu closes the submenu first, then the item closes the menu.
    assert_eq!(
        kinds,
        [
            ActionKind::Dismissed,
            ActionKind::Press,
            ActionKind::Dismissed
        ]
    );
    assert!(menu.ui.rect(menu.layer).is_none());
}

#[test]
fn an_agents_press_on_an_item_closes_its_menu_too() {
    let mut menu = Menu::open(LayoutDirection::LeftToRight);
    let new = menu.items[0];
    menu.ui.act(Action {
        element: Some(new),
        kind: ActionKind::Press,
    });
    assert_eq!(
        menu.actions(),
        [
            (Some(new), ActionKind::Press),
            (Some(menu.layer), ActionKind::Dismissed)
        ]
    );
}

#[test]
fn an_items_labels_line_up_and_its_shortcut_widens_its_menu() {
    let menu = Menu::open(LayoutDirection::LeftToRight);
    let widths: Vec<f32> = menu
        .items
        .iter()
        .map(|&item| menu.ui.rect(item).expect("laid out").width)
        .collect();
    assert!(widths.iter().all(|&width| width == widths[0]));
    // Without the shortcut, the menu is narrower.
    let mut other = Menu::open(LayoutDirection::LeftToRight);
    let narrow = open_menu(
        &mut other.ui,
        Place::At {
            over: other.button,
            point: [0.0, 100.0],
        },
        &[
            ("new", "", ItemMark::None),
            ("save", "", ItemMark::None),
            ("grid", "", ItemMark::Check(true)),
            ("recent", "", ItemMark::Submenu),
        ],
    );
    let wide = menu.ui.rect(menu.layer).expect("placed").width;
    let narrow = other.ui.rect(narrow).expect("placed").width;
    assert!(wide > narrow + 30.0, "{wide} against {narrow}");
}

#[test]
fn menus_and_their_items_mean_what_they_are() {
    let mut menu = Menu::open(LayoutDirection::LeftToRight);
    let [new, _, grid, recent] = menu.items;
    let menu_meaning = menu
        .ui
        .meaning_of(menu.ui.children(menu.layer)[0])
        .expect("a menu");
    assert_eq!(menu_meaning.role, Role::Menu);
    let meaning = menu.ui.meaning_of(new).expect("an item");
    assert_eq!(
        (meaning.role, meaning.name.as_str()),
        (Role::MenuItem, "new")
    );
    let meaning = menu.ui.meaning_of(grid).expect("an item");
    assert_eq!(meaning.role, Role::MenuItemCheckBox);
    assert!(meaning.state.checked);
    menu.ui.set_focus(Some(recent));
    menu.key(NamedKey::Enter);
    menu.open_submenu();
    assert!(menu.ui.meaning_of(recent).expect("an item").state.expanded);
}
