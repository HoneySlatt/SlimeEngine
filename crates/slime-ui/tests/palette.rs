//! What the editor tells and finds, without a GPU (`docs/adr/0151-rich-widgets.md`): the chords of
//! commands and the shortcuts by scope; the palette of commands, which finds commands by what is
//! typed and gives the chosen one as a shortcut's command, its field's text kept from the program;
//! the sheet of shortcuts; notifications, stacked at the window's bottom end, gone after their
//! kind's time unless the pointer rests on them, their buttons giving commands.

use std::time::Duration;

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::ActionKind;
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::Style;
use slime_ui::text::Fonts;
use slime_ui::theme::StyleSheet;
use slime_ui::tree::{
    ElementId, LayoutDirection, Notification, NotificationKind, PaletteCommand, Role, RootSettings,
    Ui,
};

const KEYS: &str = r#"
[global]
"Ctrl+S" = "save"
"Ctrl+Shift+P" = "palette"
F1 = "shortcuts"

[viewport]
G = "grid"
"#;

/// An interface of 800 by 600 whose font is the tests' Noto Sans, its shortcuts [`KEYS`].
fn interface() -> (Ui, ElementId) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    ui.set_shortcuts(&[Shortcuts::from_text("test.keys", KEYS)])
        .expect("valid shortcuts");
    // Every element in the tests' font, what the interface builds for itself too.
    let sheet = StyleSheet::from_text(
        "test.style",
        "[everything]\nselect = {}\nstyle.text.style.fonts = [\"Noto Sans\"]\n",
    );
    ui.set_style(&[sheet], &[]).expect("a valid sheet");
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut style = Style::default();
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    let root = ui.add_root(settings, style);
    ui.frame();
    (ui, root)
}

fn commands() -> Vec<PaletteCommand> {
    [
        ("Save the level", "save"),
        ("Show the statistics", "statistics"),
        ("Snap to the grid", "grid"),
        ("Quit", "quit"),
    ]
    .into_iter()
    .map(|(name, command)| PaletteCommand {
        name: name.to_owned(),
        command: command.to_owned(),
    })
    .collect()
}

fn key(ui: &mut Ui, logical: Key, text: Option<&str>) {
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical,
        text: text.map(str::to_owned),
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.frame();
}

fn type_text(ui: &mut Ui, text: &str) {
    for character in text.chars() {
        key(
            ui,
            Key::Character(character.to_string()),
            Some(&character.to_string()),
        );
    }
}

/// The names of the palette's rows, as they show.
fn rows(ui: &Ui) -> Vec<String> {
    let Some(list) = ui.element_by_path("#1/palette/commands") else {
        return Vec::new();
    };
    ui.children(list)
        .iter()
        .filter_map(|&row| ui.meaning_of(row))
        .map(|meaning| meaning.name)
        .collect()
}

fn actions(ui: &mut Ui) -> Vec<ActionKind> {
    ui.take_actions()
        .into_iter()
        .map(|action| action.kind)
        .collect()
}

#[test]
fn a_commands_chord_and_the_shortcuts_by_scope() {
    let (ui, _) = interface();
    assert_eq!(ui.command_chord("save").as_deref(), Some("Ctrl+S"));
    assert_eq!(ui.command_chord("palette").as_deref(), Some("Ctrl+Shift+P"));
    assert_eq!(ui.command_chord("grid").as_deref(), Some("G"));
    assert_eq!(ui.command_chord("quit"), None);
    let scopes = ui.shortcuts();
    assert_eq!(scopes[0].0, "global");
    assert_eq!(scopes[0].1[0], ("Ctrl+S".to_owned(), "save".to_owned()));
    assert_eq!(
        scopes[1],
        (
            "viewport".to_owned(),
            vec![("G".to_owned(), "grid".to_owned())]
        )
    );
}

#[test]
fn the_palette_finds_commands_by_what_is_typed_and_gives_the_chosen_one() {
    let (mut ui, _) = interface();
    ui.open_palette("Type a command", commands());
    ui.frame();
    assert!(ui.palette_open());
    let field = ui.element_by_path("#1/palette/search").expect("its field");
    assert_eq!(ui.focus(), Some(field));
    assert_eq!(rows(&ui).len(), 4, "every command, with nothing typed");
    // Each row shows its command's chord.
    let first = ui.element_by_path("#1/palette/commands/#0").expect("a row");
    assert!(matches!(
        ui.element(first).map(|element| &element.kind),
        Some(slime_ui::tree::Kind::MenuItem { shortcut, .. }) if shortcut == "Ctrl+S"
    ));
    type_text(&mut ui, "stat");
    assert_eq!(rows(&ui), ["Show the statistics"]);
    // What the field holds is the palette's: the program reads none of it.
    assert!(actions(&mut ui).is_empty());
    key(&mut ui, Key::Named(NamedKey::Enter), None);
    assert_eq!(
        actions(&mut ui),
        [ActionKind::Command("statistics".to_owned())]
    );
    assert!(!ui.palette_open());
}

#[test]
fn the_arrows_choose_a_press_gives_and_escape_closes_the_palette() {
    let (mut ui, _) = interface();
    ui.open_palette("Type a command", commands());
    ui.frame();
    key(&mut ui, Key::Named(NamedKey::ArrowDown), None);
    key(&mut ui, Key::Named(NamedKey::ArrowDown), None);
    let chosen = ui.element_by_path("#1/palette/commands/#2").expect("a row");
    assert!(ui.state(chosen).expect("there").selected);
    key(&mut ui, Key::Named(NamedKey::ArrowUp), None);
    key(&mut ui, Key::Named(NamedKey::ArrowUp), None);
    key(&mut ui, Key::Named(NamedKey::ArrowUp), None);
    let last = ui.element_by_path("#1/palette/commands/#3").expect("a row");
    assert!(ui.state(last).expect("there").selected, "around");
    // A press on a row gives its command.
    let rect = ui.rect(chosen).expect("laid out");
    ui.pointer_moved(rect.x + 10.0, rect.y + rect.height / 2.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert_eq!(actions(&mut ui), [ActionKind::Command("grid".to_owned())]);
    // Escape closes it, giving nothing.
    ui.open_palette("Type a command", commands());
    ui.frame();
    key(&mut ui, Key::Named(NamedKey::Escape), None);
    assert!(!ui.palette_open());
    assert!(actions(&mut ui).is_empty());
}

#[test]
fn the_sheet_lists_the_shortcuts_by_scope_and_escape_closes_it() {
    let (mut ui, _) = interface();
    ui.open_shortcut_sheet("Keyboard shortcuts", &commands());
    ui.frame();
    let sheet = ui.element_by_path("#1/sheet").expect("the sheet");
    let meaning = ui.meaning_of(sheet).expect("drawn");
    assert_eq!(
        (meaning.role, meaning.name.as_str()),
        (Role::Dialog, "Keyboard shortcuts")
    );
    let save = ui
        .element_by_path("#1/sheet/global Ctrl+S/name")
        .expect("the row of save");
    assert_eq!(ui.meaning_of(save).expect("drawn").name, "Save the level");
    // A command that no name is given for shows as it is.
    assert!(ui.element_by_path("#1/sheet/global F1/name").is_some());
    key(&mut ui, Key::Named(NamedKey::Escape), None);
    assert!(ui.element_by_path("#1/sheet").is_none());
}

#[test]
fn notifications_stack_at_the_bottom_end_and_go_after_their_kinds_time() {
    let (mut ui, _) = interface();
    let saved = ui.notify(Notification {
        kind: NotificationKind::Success,
        text: "Level saved".to_owned(),
        ..Notification::default()
    });
    let failed = ui.notify(Notification {
        kind: NotificationKind::Error,
        text: "A texture is missing".to_owned(),
        ..Notification::default()
    });
    let (_, stats) = ui.frame();
    assert!(stats.animating, "a notification waits for its time");
    let stack = ui.element_by_path("#1/notifications").expect("the stack");
    let rect = ui.rect(stack).expect("laid out");
    assert_eq!(rect.x + rect.width, 800.0 - 16.0);
    assert_eq!(rect.y + rect.height, 600.0 - 16.0);
    let card = ui
        .element_by_path(&format!("#1/notifications/#{}", 0))
        .expect("the first");
    let meaning = ui.meaning_of(card).expect("drawn");
    assert_eq!(
        (meaning.role, meaning.name.as_str()),
        (Role::Status, "Level saved")
    );
    // Five seconds on: the success goes, the error stays.
    ui.advance(Duration::from_secs(6));
    ui.frame();
    assert_eq!(ui.notifications(), [failed]);
    assert!(!ui.close_notification(saved));
    assert!(ui.close_notification(failed));
    ui.frame();
    assert!(ui.element_by_path("#1/notifications").is_none());
}

#[test]
fn the_pointer_resting_keeps_a_notification_and_its_buttons_give_commands() {
    let (mut ui, _) = interface();
    let cooking = ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "3 textures to cook".to_owned(),
        buttons: vec![("Cook".to_owned(), "cook".to_owned())],
        ..Notification::default()
    });
    ui.frame();
    let card = ui.element_by_path("#1/notifications/#0").expect("it");
    let rect = ui.rect(card).expect("laid out");
    ui.pointer_moved(rect.x + 5.0, rect.y + 5.0);
    for _ in 0..4 {
        ui.advance(Duration::from_secs(2));
        ui.frame();
    }
    assert_eq!(
        ui.notifications(),
        [cooking],
        "kept while the pointer rests on it"
    );
    let button = ui
        .element_by_path("#1/notifications/#0/buttons/#0")
        .expect("its button");
    let rect = ui.rect(button).expect("laid out");
    ui.pointer_moved(rect.x + 5.0, rect.y + 5.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert_eq!(actions(&mut ui), [ActionKind::Command("cook".to_owned())]);
    assert!(ui.notifications().is_empty());
    // One with a bar of progress stays, and more than five keep the newest.
    let bar = ui.notify(Notification {
        text: "Cooking".to_owned(),
        progress: Some(Some(0.3)),
        ..Notification::default()
    });
    ui.advance(Duration::from_secs(30));
    ui.frame();
    assert_eq!(ui.notifications(), [bar]);
    for number in 0..5 {
        ui.notify(Notification {
            text: format!("note {number}"),
            ..Notification::default()
        });
    }
    ui.frame();
    assert_eq!(ui.notifications().len(), 5);
    assert!(!ui.notifications().contains(&bar));
}
