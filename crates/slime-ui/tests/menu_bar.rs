//! A bar of menus, without a GPU (`docs/adr/0150-layers.md`): F10 and Alt alone that reach it and
//! give the focus back; the arrows that move among its items and between its menus; a command of a
//! menu that gives the focus back to what held it; the pointer that opens and closes a menu, and
//! opens another's as it comes over its item while one is open.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::ActionKind;
use slime_ui::style::{FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, ItemMark, Layer, LayoutDirection, Place, RootSettings, Side, Ui,
};

/// A style of the tests' font, laid out as a column where `column`.
fn style(column: bool) -> Style {
    let mut style = Style::default();
    if column {
        style.layout.direction = FlexDirection::Column;
    }
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// A window whose root, a column, holds a bar of menus, `File`, `Edit` and `View`, then a button.
struct Window {
    ui: Ui,
    items: [ElementId; 3],
    button: ElementId,
}

impl Window {
    fn new() -> Self {
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
        let root = ui.add_root(settings, style(true));
        let (mut items, mut button) = (Vec::new(), None);
        ui.build(root, 0, |children| {
            children.add("bar", Element::menu_bar(style(false)), |bar| {
                for title in ["File", "Edit", "View"] {
                    let item = Element::menu_item(title, "", ItemMark::Submenu, style(false));
                    items.push(bar.add(title, item, |_| {}));
                }
            });
            button = Some(children.add("button", Element::button("Go", style(false)), |_| {}));
        });
        ui.frame();
        Self {
            ui,
            items: [items[0], items[1], items[2]],
            button: button.expect("built"),
        }
    }

    fn key(&mut self, named: NamedKey, state: ButtonState) -> bool {
        let taken = self.ui.key(&KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Named(named),
            text: None,
            state,
            repeat: false,
        });
        self.ui.frame();
        taken
    }

    fn press(&mut self, named: NamedKey) -> bool {
        self.key(named, ButtonState::Pressed)
    }

    /// Opens the menu of `item`, as the program does when it reads its press: its layer and its
    /// items, `one` and `two`.
    fn open_menu(&mut self, item: ElementId) -> (ElementId, Vec<ElementId>) {
        let layer = self.ui.open_layer(
            Layer {
                place: Place::Beside {
                    element: item,
                    side: Side::Below,
                    along: Along::Start,
                },
                modal: false,
                dismiss: true,
            },
            style(true),
        );
        let mut entries = Vec::new();
        self.ui.build(layer, 0, |children| {
            children.add("menu", Element::menu(style(true)), |menu| {
                for label in ["one", "two"] {
                    let entry = Element::menu_item(label, "", ItemMark::None, style(false));
                    entries.push(menu.add(label, entry, |_| {}));
                }
            });
        });
        self.ui.frame();
        (layer, entries)
    }

    fn actions(&mut self) -> Vec<(Option<ElementId>, ActionKind)> {
        self.ui
            .take_actions()
            .into_iter()
            .map(|action| (action.element, action.kind))
            .collect()
    }

    fn middle(&self, id: ElementId) -> (f32, f32) {
        let rect = self.ui.rect(id).expect("laid out");
        (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
    }

    fn click(&mut self, id: ElementId) {
        let (x, y) = self.middle(id);
        self.ui.pointer_moved(x, y);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        self.ui.frame();
    }
}

#[test]
fn f10_reaches_the_bar_the_arrows_move_along_it_and_escape_gives_the_focus_back() {
    let mut window = Window::new();
    let [file, edit, view] = window.items;
    window.ui.set_focus(Some(window.button));
    assert!(window.press(NamedKey::F10));
    assert_eq!(window.ui.focus(), Some(file));
    window.press(NamedKey::ArrowRight);
    assert_eq!(window.ui.focus(), Some(edit));
    window.press(NamedKey::ArrowLeft);
    window.press(NamedKey::ArrowLeft);
    assert_eq!(window.ui.focus(), Some(view), "around");
    window.press(NamedKey::Escape);
    assert_eq!(window.ui.focus(), Some(window.button));
    assert!(window.actions().is_empty());
}

#[test]
fn alt_pressed_and_let_go_alone_reaches_the_bar_and_leaves_it() {
    let mut window = Window::new();
    window.press(NamedKey::Alt);
    assert!(window.key(NamedKey::Alt, ButtonState::Released));
    assert_eq!(window.ui.focus(), Some(window.items[0]));
    window.press(NamedKey::Alt);
    window.key(NamedKey::Alt, ButtonState::Released);
    assert_eq!(window.ui.focus(), None);
    // Alt with another key reaches nothing.
    window.press(NamedKey::Alt);
    window.press(NamedKey::Tab);
    assert!(!window.key(NamedKey::Alt, ButtonState::Released));
}

#[test]
fn the_arrows_move_between_the_bars_menus_and_a_command_gives_the_focus_back() {
    let mut window = Window::new();
    let [file, edit, _] = window.items;
    window.ui.set_focus(Some(window.button));
    window.press(NamedKey::F10);
    window.press(NamedKey::ArrowDown);
    assert_eq!(window.actions(), [(Some(file), ActionKind::Press)]);
    let (file_menu, entries) = window.open_menu(file);
    assert_eq!(window.ui.focus(), Some(entries[0]));
    // Towards the end, on an item that opens no submenu: the next menu.
    window.press(NamedKey::ArrowRight);
    assert_eq!(
        window.actions(),
        [
            (Some(file_menu), ActionKind::Dismissed),
            (Some(edit), ActionKind::Press)
        ]
    );
    let (edit_menu, entries) = window.open_menu(edit);
    assert_eq!(window.ui.focus(), Some(entries[0]));
    window.press(NamedKey::ArrowDown);
    window.press(NamedKey::Enter);
    assert_eq!(
        window.actions(),
        [
            (Some(entries[1]), ActionKind::Press),
            (Some(edit_menu), ActionKind::Dismissed)
        ]
    );
    assert_eq!(window.ui.focus(), Some(window.button));
}

#[test]
fn the_pointer_opens_a_menu_closes_it_and_opens_anothers_as_it_comes_over_its_item() {
    let mut window = Window::new();
    let [file, edit, _] = window.items;
    window.ui.set_focus(Some(window.button));
    window.click(file);
    assert_eq!(window.actions(), [(Some(file), ActionKind::Press)]);
    // The focus stays where it was.
    assert_eq!(window.ui.focus(), Some(window.button));
    let (file_menu, _) = window.open_menu(file);
    assert!(window.ui.state(file).expect("there").expanded);
    let (x, y) = window.middle(edit);
    window.ui.pointer_moved(x, y);
    window.ui.frame();
    assert_eq!(
        window.actions(),
        [
            (Some(file_menu), ActionKind::Dismissed),
            (Some(edit), ActionKind::Press)
        ]
    );
    let (edit_menu, _) = window.open_menu(edit);
    window.click(edit);
    assert_eq!(window.actions(), [(Some(edit_menu), ActionKind::Dismissed)]);
    assert!(window.ui.rect(edit_menu).is_none());
    // Without a menu open, the pointer over an item opens nothing, even resting.
    let (x, y) = window.middle(file);
    window.ui.pointer_moved(x, y);
    window.ui.advance(std::time::Duration::from_secs(1));
    window.ui.frame();
    assert!(window.actions().is_empty());
}

#[test]
fn a_bars_items_lay_out_their_labels_alone() {
    let window = Window::new();
    let file = window.ui.rect(window.items[0]).expect("laid out");
    let button = window.ui.rect(window.button).expect("laid out");
    // `File` is about as wide as the button's `Go`, not four marks wider.
    assert!(file.width < button.width + 20.0, "{file:?} {button:?}");
}

#[test]
fn alt_after_a_press_shows_the_bars_focus_and_alt_in_a_menu_leaves_the_bar() {
    let mut window = Window::new();
    let [file, ..] = window.items;
    // A press of the pointer, then Alt alone: the bar's focus shows, as the keys gave it.
    window.click(window.button);
    window.ui.take_actions();
    window.press(NamedKey::Alt);
    window.key(NamedKey::Alt, ButtonState::Released);
    assert_eq!(window.ui.focus(), Some(file));
    assert!(window.ui.state(file).expect("there").focus_visible);
    window.press(NamedKey::Escape);
    // From a menu of the bar, F10 closes it and gives the focus back to what held it before.
    window.ui.set_focus(Some(window.button));
    window.press(NamedKey::F10);
    window.press(NamedKey::ArrowDown);
    window.actions();
    let (menu, entries) = window.open_menu(file);
    assert_eq!(window.ui.focus(), Some(entries[0]));
    window.press(NamedKey::F10);
    assert!(window.ui.rect(menu).is_none());
    assert_eq!(window.ui.focus(), Some(window.button));
    assert_eq!(window.actions(), [(Some(menu), ActionKind::Dismissed)]);
}
