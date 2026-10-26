//! Fields of text and of numbers, without a GPU, with an input method and a clipboard simulated
//! (`docs/adr/0145-text-edited.md`): text typed, moved through by clusters and words, in text of
//! both directions, selected by the keys and the pointer, undone, copied and pasted, composed;
//! what the program reads of it, changed and committed; a field of numbers typed and dragged, which
//! writes nothing that the user did not change; the program's text, which replaces the user's only
//! when it changes; the keys and presses that a field takes; the pointer's shapes.

use std::sync::{Arc, Mutex};

use slime_input::{
    ButtonState, CursorShape, Ime, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey,
};
use slime_ui::action::{ActionKind, FieldValue};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Dimension, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Clipboard, Element, ElementId, LayoutDirection, RootSettings, Ui};

/// An interface whose fonts are the tests' Noto Sans and Noto Sans Hebrew.
fn ui() -> Ui {
    let mut fonts = Fonts::new();
    for file in ["NotoSans.ttf", "NotoSansHebrew.ttf"] {
        let path = format!("{}/tests/fonts/{file}", env!("CARGO_MANIFEST_DIR"));
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
    }
    Ui::new(fonts)
}

/// A style of 200 by 30 logical pixels, of the tests' fonts.
fn style() -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(200.0), Dimension::Pixels(30.0)];
    style.text.style.fonts = vec!["Noto Sans".to_owned(), "Noto Sans Hebrew".to_owned()];
    style
}

/// An interface whose root, of 400 by 300 logical pixels at scale 1, holds `element` then a
/// button: the interface, its root, the element and the button.
fn interface(element: Element) -> (Ui, ElementId, ElementId, ElementId) {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    let (mut field, mut button) = (None, None);
    ui.build(root, 0, |children| {
        field = Some(children.add("field", element, |_| {}));
        button = Some(children.add("button", Element::button("", style()), |_| {}));
    });
    ui.frame();
    (ui, root, field.expect("built"), button.expect("built"))
}

/// A press of a key that types `text`.
fn typed(text: &str) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Character(text.to_owned()),
        text: Some(text.to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    }
}

/// A press of a named key.
fn named(key: NamedKey) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(key),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    }
}

/// Types `text`, a key for each character.
fn type_text(ui: &mut Ui, text: &str) {
    for character in text.chars() {
        assert!(ui.key(&typed(&character.to_string())), "{character}");
    }
}

/// Presses `key` with `modifiers` held, then lets them go.
fn chord(ui: &mut Ui, modifiers: Modifiers, key: &KeyEvent) -> bool {
    ui.modifiers(modifiers);
    let taken = ui.key(key);
    ui.modifiers(Modifiers::default());
    taken
}

fn control() -> Modifiers {
    let mut modifiers = Modifiers::default();
    modifiers.control = true;
    modifiers
}

fn shift() -> Modifiers {
    let mut modifiers = Modifiers::default();
    modifiers.shift = true;
    modifiers
}

/// The values that the queued actions changed and committed, as text.
fn values(ui: &mut Ui) -> Vec<String> {
    ui.take_actions()
        .into_iter()
        .filter_map(|action| match action.kind {
            ActionKind::Changed(value) => Some(format!("changed {}", shown(&value))),
            ActionKind::Committed(value) => Some(format!("committed {}", shown(&value))),
            _ => None,
        })
        .collect()
}

fn shown(value: &FieldValue) -> String {
    match value {
        FieldValue::Text(text) => format!("{text:?}"),
        FieldValue::Number(number) => format!("{number}"),
    }
}

/// A clipboard that a test reads.
#[derive(Clone, Default)]
struct Board(Arc<Mutex<Option<String>>>);

impl Clipboard for Board {
    fn text(&mut self) -> Option<String> {
        self.0.lock().expect("not poisoned").clone()
    }

    fn set_text(&mut self, text: &str) {
        *self.0.lock().expect("not poisoned") = Some(text.to_owned());
    }
}

#[test]
fn text_typed_is_moved_through_by_clusters_and_words_and_committed_once_changed() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    ui.set_focus(Some(field));
    type_text(&mut ui, "hello world");
    assert_eq!(ui.field_text(field), Some("hello world"));
    assert_eq!(values(&mut ui).len(), 11, "a change for each character");
    // The line's start, a word back from its end, the cluster before the caret, the word before.
    ui.key(&named(NamedKey::Home));
    type_text(&mut ui, "X");
    ui.key(&named(NamedKey::End));
    chord(&mut ui, control(), &named(NamedKey::ArrowLeft));
    type_text(&mut ui, "big ");
    assert_eq!(ui.field_text(field), Some("Xhello big world"));
    ui.key(&named(NamedKey::End));
    ui.key(&named(NamedKey::Backspace));
    assert_eq!(ui.field_text(field), Some("Xhello big worl"));
    chord(&mut ui, control(), &named(NamedKey::Backspace));
    assert_eq!(ui.field_text(field), Some("Xhello big "));
    // Shift extends the selection, which typing replaces.
    ui.key(&named(NamedKey::Home));
    for _ in 0..6 {
        chord(&mut ui, shift(), &named(NamedKey::ArrowRight));
    }
    type_text(&mut ui, "Y");
    assert_eq!(ui.field_text(field), Some("Y big "));
    // Ctrl+A, then the text typed in place of all of it.
    chord(&mut ui, control(), &typed("a"));
    type_text(&mut ui, "new");
    ui.take_actions();
    assert!(ui.key(&named(NamedKey::Enter)));
    assert_eq!(values(&mut ui), [r#"committed "new""#]);
    ui.key(&named(NamedKey::Enter));
    assert!(values(&mut ui).is_empty(), "unchanged since its commit");
}

#[test]
fn typing_is_undone_whole_and_done_again() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    ui.set_focus(Some(field));
    type_text(&mut ui, "abc");
    ui.key(&named(NamedKey::Backspace));
    assert_eq!(ui.field_text(field), Some("ab"));
    chord(&mut ui, control(), &typed("z"));
    assert_eq!(ui.field_text(field), Some("abc"), "the deletion undone");
    chord(&mut ui, control(), &typed("z"));
    assert_eq!(ui.field_text(field), Some(""), "the typing undone whole");
    let mut redo = control();
    redo.shift = true;
    chord(&mut ui, redo, &typed("Z"));
    assert_eq!(ui.field_text(field), Some("abc"));
    chord(&mut ui, control(), &typed("y"));
    assert_eq!(ui.field_text(field), Some("ab"));
}

#[test]
fn text_is_copied_cut_and_pasted_through_the_clipboard() {
    let (mut ui, _, field, _) = interface(Element::text_field("copy me", style()));
    let board = Board::default();
    ui.set_clipboard(Box::new(board.clone()));
    assert!(ui.has_clipboard());
    ui.set_focus(Some(field));
    chord(&mut ui, control(), &typed("a"));
    chord(&mut ui, control(), &typed("c"));
    assert_eq!(board.0.lock().unwrap().as_deref(), Some("copy me"));
    chord(&mut ui, control(), &typed("x"));
    assert_eq!(ui.field_text(field), Some(""));
    chord(&mut ui, control(), &typed("v"));
    chord(&mut ui, control(), &typed("v"));
    assert_eq!(ui.field_text(field), Some("copy mecopy me"));
    // A field of one line takes a line break as a space.
    board.clone().set_text("one\ntwo");
    chord(&mut ui, control(), &typed("a"));
    chord(&mut ui, control(), &typed("v"));
    assert_eq!(ui.field_text(field), Some("one two"));
}

#[test]
fn an_input_method_composes_text_which_one_undo_takes_back() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    let preedit = |text: &str| Ime::Preedit {
        text: text.to_owned(),
        cursor: Some((text.len(), text.len())),
    };
    assert!(!ui.ime(&preedit("に")), "no field holds the focus");
    ui.set_focus(Some(field));
    assert!(
        ui.text_input().is_some(),
        "the program allows the input method"
    );
    ui.ime(&Ime::Enabled);
    ui.ime(&preedit("に"));
    ui.ime(&preedit("にほ"));
    assert_eq!(ui.field_text(field), Some("にほ"));
    assert!(
        values(&mut ui).is_empty(),
        "a composition is not changed text"
    );
    ui.ime(&Ime::Commit("日本".to_owned()));
    assert_eq!(ui.field_text(field), Some("日本"));
    assert_eq!(values(&mut ui), [r#"changed "日本""#]);
    // A composition given up leaves nothing.
    ui.ime(&preedit("ご"));
    ui.ime(&Ime::Disabled);
    assert_eq!(ui.field_text(field), Some("日本"));
    chord(&mut ui, control(), &typed("z"));
    assert_eq!(
        ui.field_text(field),
        Some(""),
        "the text before the composition"
    );
    ui.set_focus(None);
    assert_eq!(ui.text_input(), None);
}

#[test]
fn the_pointer_places_the_caret_and_selects() {
    let (mut ui, _, field, button) = interface(Element::text_field("pointer", style()));
    let press = |ui: &mut Ui, x: f32| {
        ui.pointer_moved(x, 15.0);
        assert!(ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    };
    let release = |ui: &mut Ui| {
        assert!(ui.pointer_button(MouseButton::Left, ButtonState::Released));
    };
    // Pressed after the text's end: the focus, the caret at the end.
    press(&mut ui, 190.0);
    release(&mut ui);
    assert_eq!(ui.focus(), Some(field));
    type_text(&mut ui, "!");
    assert_eq!(ui.field_text(field), Some("pointer!"));
    // Dragged from the start to past the end: all of it selected, which typing replaces.
    press(&mut ui, 0.5);
    ui.pointer_moved(195.0, 15.0);
    release(&mut ui);
    type_text(&mut ui, "Z");
    assert_eq!(ui.field_text(field), Some("Z"));
    // Shift and a press extend the selection from the caret.
    ui.modifiers(shift());
    press(&mut ui, 0.5);
    release(&mut ui);
    ui.modifiers(Modifiers::default());
    type_text(&mut ui, "W");
    assert_eq!(ui.field_text(field), Some("W"));
    // The pointer's shape: the I-beam over the field, the arrow over the button.
    ui.pointer_moved(100.0, 15.0);
    assert_eq!(ui.cursor(), CursorShape::Text);
    assert_eq!(ui.rect(button).map(|rect| rect.x), Some(200.0));
    ui.pointer_moved(300.0, 15.0);
    assert_eq!(ui.cursor(), CursorShape::Default);
}

#[test]
fn a_field_of_numbers_writes_only_what_the_user_changed() {
    let (mut ui, _, field, button) = interface(Element::number_field(1.5, 0.1, style()));
    assert_eq!(ui.field_text(field), Some("1.5"));
    // Focused and left unchanged: nothing.
    ui.set_focus(Some(field));
    ui.set_focus(Some(button));
    assert!(values(&mut ui).is_empty());
    // Typed: a change for each text that reads as a number, then its commit.
    ui.set_focus(Some(field));
    chord(&mut ui, control(), &typed("a"));
    type_text(&mut ui, "2.25");
    ui.key(&named(NamedKey::Enter));
    assert_eq!(
        values(&mut ui),
        [
            "changed 2",
            "changed 2",
            "changed 2.2",
            "changed 2.25",
            "committed 2.25"
        ]
    );
    // Text that reads as no number: none, and the number last committed shown again.
    chord(&mut ui, control(), &typed("a"));
    type_text(&mut ui, "abc");
    ui.key(&named(NamedKey::Enter));
    assert!(values(&mut ui).is_empty());
    assert_eq!(ui.field_text(field), Some("2.25"));
    chord(&mut ui, control(), &typed("a"));
    type_text(&mut ui, "1.5");
    ui.key(&named(NamedKey::Enter));
    ui.take_actions();
    // Dragged while it does not hold the focus: a step for each logical pixel, its value rounded
    // to the step's decimals, committed when let go.
    ui.set_focus(None);
    ui.pointer_moved(100.0, 15.0);
    assert_eq!(ui.cursor(), CursorShape::ResizeHorizontal);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(102.0, 15.0);
    assert_eq!(
        ui.field_text(field),
        Some("1.5"),
        "not yet past the threshold"
    );
    ui.pointer_moved(130.0, 15.0);
    assert_eq!(ui.cursor(), CursorShape::ResizeHorizontal);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.field_text(field), Some("4.5"));
    assert_eq!(values(&mut ui), ["changed 4.5", "committed 4.5"]);
    assert_eq!(ui.focus(), None, "a drag gives no focus");
}

#[test]
fn the_program_s_text_replaces_the_user_s_only_when_it_changes() {
    let (mut ui, root, field, _) = interface(Element::text_field("given", style()));
    let build = |ui: &mut Ui, revision: u64, text: &str| {
        ui.build(root, revision, |children| {
            children.add("field", Element::text_field(text, style()), |_| {});
            children.add("button", Element::button("", style()), |_| {});
        });
        ui.frame();
    };
    ui.set_focus(Some(field));
    type_text(&mut ui, " typed");
    build(&mut ui, 1, "given");
    assert_eq!(ui.field_text(field), Some("given typed"));
    build(&mut ui, 2, "from the program");
    assert_eq!(ui.field_text(field), Some("from the program"));
}

#[test]
fn words_are_moved_through_as_they_show_in_text_from_right_to_left() {
    let (mut ui, _, field, _) = interface(Element::text_field("שלום עולם", style()));
    ui.set_focus(Some(field));
    // The caret at the text's logical end, which shows on the left: the word on its right is the
    // last one's start.
    chord(&mut ui, control(), &named(NamedKey::End));
    chord(&mut ui, control(), &named(NamedKey::ArrowRight));
    type_text(&mut ui, "X");
    assert_eq!(ui.field_text(field), Some("שלום Xעולם"));
}

#[test]
fn a_field_takes_what_it_types_before_the_shortcuts_and_leaves_tab_to_the_focus() {
    let (mut ui, _, field, button) = interface(Element::text_field("", style()));
    let table = Shortcuts::from_text("keys", "[global]\nT = \"theme\"\n");
    ui.set_shortcuts(&[table]).expect("a table");
    ui.set_focus(Some(field));
    assert!(ui.key(&typed("t")));
    assert_eq!(ui.field_text(field), Some("t"));
    assert!(
        !ui.take_actions()
            .iter()
            .any(|action| matches!(action.kind, ActionKind::Command(_))),
        "no shortcut while a field types"
    );
    // Tab moves the focus, which commits the field.
    assert!(ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), Some(button));
    assert_eq!(values(&mut ui), [r#"committed "t""#]);
    // Without a field, the shortcut; a letter with no focus is the game's.
    ui.set_focus(None);
    assert!(ui.key(&typed("t")), "a shortcut's key");
    assert!(!ui.key(&typed("w")), "the game's key");
}

/// A composition of `text`, its caret at its end.
fn preedit(text: &str) -> Ime {
    Ime::Preedit {
        text: text.to_owned(),
        cursor: Some((text.len(), text.len())),
    }
}

#[test]
fn a_composition_leaves_the_text_whole_however_it_starts_and_ends() {
    let (mut ui, _, field, button) = interface(Element::text_field("keep it", style()));
    ui.set_focus(Some(field));
    chord(&mut ui, control(), &typed("a"));
    // The empty composition that input methods send before each commit deletes nothing.
    ui.ime(&preedit(""));
    assert_eq!(ui.field_text(field), Some("keep it"));
    assert!(values(&mut ui).is_empty());
    // A composition over a selection deletes it, a change, and one undo takes the word back.
    ui.ime(&preedit("に"));
    assert_eq!(ui.field_text(field), Some("に"));
    assert_eq!(values(&mut ui), [r#"changed """#]);
    // The keys that reach the field while it composes are the input method's.
    assert!(ui.key(&named(NamedKey::Backspace)));
    assert!(ui.key(&typed("x")));
    assert_eq!(ui.field_text(field), Some("に"));
    ui.ime(&preedit(""));
    ui.ime(&Ime::Commit("日".to_owned()));
    assert_eq!(ui.field_text(field), Some("日"));
    assert_eq!(values(&mut ui), [r#"changed "日""#]);
    chord(&mut ui, control(), &typed("z"));
    assert_eq!(ui.field_text(field), Some("keep it"));
    // The focus leaving while it composes takes the composition away, which commits nothing.
    ui.set_focus(Some(button));
    ui.take_actions();
    ui.set_focus(Some(field));
    ui.key(&named(NamedKey::End));
    ui.ime(&preedit("ご"));
    assert_eq!(ui.field_text(field), Some("keep itご"));
    ui.set_focus(Some(button));
    assert_eq!(ui.field_text(field), Some("keep it"));
    assert!(values(&mut ui).is_empty());
}

#[test]
fn an_edit_that_changes_nothing_tells_nothing() {
    let (mut ui, _, field, _) = interface(Element::text_field("text", style()));
    ui.set_focus(Some(field));
    ui.key(&named(NamedKey::Delete));
    ui.key(&named(NamedKey::Home));
    ui.key(&named(NamedKey::Backspace));
    chord(&mut ui, control(), &typed("z"));
    chord(&mut ui, control(), &typed("y"));
    // Without a clipboard, a cut keeps the selection, which it could not give back.
    chord(&mut ui, control(), &typed("a"));
    chord(&mut ui, control(), &typed("x"));
    assert_eq!(ui.field_text(field), Some("text"));
    assert!(values(&mut ui).is_empty());
    ui.key(&named(NamedKey::Enter));
    assert!(values(&mut ui).is_empty(), "nothing to commit");
}

#[test]
fn the_program_clears_a_field_or_gives_back_a_text_once_another_is_committed() {
    let (mut ui, root, field, button) = interface(Element::text_field("", style()));
    let build = |ui: &mut Ui, revision: u64, text: &str| {
        ui.build(root, revision, |children| {
            children.add("field", Element::text_field(text, style()), |_| {});
            children.add("button", Element::button("", style()), |_| {});
        });
        ui.frame();
    };
    // A message sent: the same empty text, given again, clears the field.
    ui.set_focus(Some(field));
    type_text(&mut ui, "hello");
    ui.key(&named(NamedKey::Enter));
    assert_eq!(
        values(&mut ui).last().map(String::as_str),
        Some(r#"committed "hello""#)
    );
    build(&mut ui, 1, "");
    assert_eq!(ui.field_text(field), Some(""));
    // A text refused: the one the program had, given again, takes its place.
    build(&mut ui, 2, "good");
    type_text(&mut ui, " bad");
    ui.key(&named(NamedKey::Enter));
    build(&mut ui, 3, "good");
    assert_eq!(ui.field_text(field), Some("good"));
    // A text kept: given back, it changes nothing.
    type_text(&mut ui, "!");
    ui.key(&named(NamedKey::Enter));
    build(&mut ui, 4, "good!");
    assert_eq!(ui.field_text(field), Some("good!"));
    // The program's text, replacing what the user changed, leaves nothing to commit.
    ui.take_actions();
    type_text(&mut ui, "?");
    build(&mut ui, 5, "other");
    assert_eq!(ui.field_text(field), Some("other"));
    ui.take_actions();
    ui.set_focus(Some(button));
    assert!(values(&mut ui).is_empty());
}

#[test]
fn a_field_of_several_lines_breaks_lines_moves_between_them_and_commits_with_ctrl_enter() {
    let mut style = style();
    style.layout.size = [Dimension::Pixels(200.0), Dimension::Pixels(80.0)];
    let (mut ui, _, field, _) = interface(Element::text_area("", style));
    ui.set_focus(Some(field));
    type_text(&mut ui, "ab");
    assert!(ui.key(&named(NamedKey::Enter)));
    type_text(&mut ui, "cd");
    assert!(ui.key(&named(NamedKey::ArrowUp)));
    type_text(&mut ui, "X");
    assert_eq!(ui.field_text(field), Some("abX\ncd"));
    assert!(ui.key(&named(NamedKey::ArrowDown)));
    type_text(&mut ui, "Y");
    assert_eq!(ui.field_text(field), Some("abX\ncdY"));
    ui.take_actions();
    assert!(chord(&mut ui, control(), &named(NamedKey::Enter)));
    assert_eq!(values(&mut ui), [r#"committed "abX\ncdY""#]);
}

#[test]
fn a_field_of_one_line_leaves_up_down_and_escape_to_the_focus() {
    let (mut ui, _, field, _) = interface(Element::text_field("one", style()));
    ui.set_focus(Some(field));
    // Taken by the interface, which holds the focus, not typed.
    assert!(ui.key(&named(NamedKey::ArrowUp)));
    assert!(ui.key(&named(NamedKey::Escape)));
    assert_eq!(ui.field_text(field), Some("one"));
    let kinds: Vec<ActionKind> = ui
        .take_actions()
        .into_iter()
        .map(|action| action.kind)
        .collect();
    assert_eq!(
        kinds,
        [ActionKind::Intent(slime_ui::action::Intent::Cancel)]
    );
}

#[test]
fn ctrl_reads_a_letter_of_another_script_by_its_place_and_altgr_types() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    ui.set_focus(Some(field));
    type_text(&mut ui, "ab");
    // Ctrl and the key in Z's place on a Cyrillic layout: an undo.
    let mut cyrillic = typed("я");
    cyrillic.physical = KeyCode::KeyZ;
    chord(&mut ui, control(), &cyrillic);
    assert_eq!(ui.field_text(field), Some(""));
    // Ctrl and Alt together, as AltGr gives them on Windows: the character typed.
    let mut altgr = control();
    altgr.alt = true;
    assert!(chord(&mut ui, altgr, &typed("@")));
    assert_eq!(ui.field_text(field), Some("@"));
}

#[test]
fn a_drag_gives_multiples_of_its_step_whatever_its_sign_and_one_undo() {
    let (mut ui, _, field, _) = interface(Element::number_field(1.0, 0.25, style()));
    let drag = |ui: &mut Ui, to: f32| {
        ui.pointer_moved(100.0, 15.0);
        ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
        for x in 101..=(to as i32) {
            ui.pointer_moved(x as f32, 15.0);
        }
        ui.pointer_button(MouseButton::Left, ButtonState::Released);
    };
    drag(&mut ui, 107.0);
    assert_eq!(ui.field_text(field), Some("2.75"));
    ui.set_focus(Some(field));
    chord(&mut ui, control(), &typed("z"));
    assert_eq!(ui.field_text(field), Some("1"), "the whole drag undone");
    let (mut ui, _, field, _) = interface(Element::number_field(1.0, -0.5, style()));
    drag(&mut ui, 104.0);
    assert_eq!(ui.field_text(field), Some("-1"));
    // A release away from a field pressed without a drag gives it no focus.
    ui.pointer_moved(100.0, 15.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(100.0, 250.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.focus(), None);
}

#[test]
fn a_field_keeps_its_caret_in_view_and_places_a_line_written_from_right_to_left_at_its_end() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    let rect = ui.rect(field).expect("drawn");
    assert!(rect.height > 0.0);
    ui.set_focus(Some(field));
    type_text(&mut ui, &"wide ".repeat(20));
    let caret = ui.text_input().expect("a field takes text");
    assert!(caret.x <= rect.x + rect.width && caret.x > rect.x + rect.width / 2.0);
    ui.key(&named(NamedKey::Home));
    let caret = ui.text_input().expect("a field takes text");
    assert!(caret.x < rect.x + 5.0, "{caret:?}");
    // Hebrew, its start on the right, in a field wider than it.
    let (mut ui, _, field, _) = interface(Element::text_field("שלום", style()));
    ui.set_focus(Some(field));
    ui.key(&named(NamedKey::Home));
    let caret = ui.text_input().expect("a field takes text");
    assert!(caret.x > rect.x + rect.width - 5.0, "{caret:?}");
}

#[test]
fn fields_mean_inputs_that_take_the_focus() {
    let (ui, _, field, _) = interface(Element::number_field(1.0, 1.0, style()));
    let meaning = ui.meaning_of(field).expect("a field means something");
    assert_eq!(meaning.role, slime_ui::tree::Role::SpinButton);
    assert_eq!(meaning.actions, [slime_ui::tree::ElementAction::Focus]);
    let (ui, _, field, _) = interface(Element::text_area("", style()));
    let meaning = ui.meaning_of(field).expect("a field means something");
    assert_eq!(meaning.role, slime_ui::tree::Role::MultilineTextInput);
}

#[test]
fn a_program_that_gives_back_each_change_keeps_what_the_user_is_typing() {
    // A program that mirrors what it reads into the field it builds, as an inspector does.
    let (mut ui, root, field, _) = interface(Element::number_field(1.05, 0.1, style()));
    let mut mirrored = 1.05;
    let build = |ui: &mut Ui, revision: u64, value: f64| {
        ui.build(root, revision, |children| {
            children.add("field", Element::number_field(value, 0.1, style()), |_| {});
            children.add("button", Element::button("", style()), |_| {});
        });
        ui.frame();
    };
    ui.set_focus(Some(field));
    ui.key(&named(NamedKey::End));
    ui.key(&named(NamedKey::Backspace));
    assert_eq!(ui.field_text(field), Some("1.0"));
    for action in ui.take_actions() {
        if let ActionKind::Changed(FieldValue::Number(number)) = action.kind {
            mirrored = number;
        }
    }
    assert_eq!(mirrored, 1.0);
    build(&mut ui, 1, mirrored);
    assert_eq!(ui.field_text(field), Some("1.0"), "the user's own spelling");
    type_text(&mut ui, "7");
    assert_eq!(ui.field_text(field), Some("1.07"));
    // The same while an input method composes: the text given back is the text without it.
    let (mut ui, root, field, _) = interface(Element::text_field("hello", style()));
    let build = |ui: &mut Ui, revision: u64, text: &str| {
        ui.build(root, revision, |children| {
            children.add("field", Element::text_field(text, style()), |_| {});
            children.add("button", Element::button("", style()), |_| {});
        });
        ui.frame();
    };
    ui.set_focus(Some(field));
    chord(&mut ui, control(), &typed("a"));
    ui.ime(&preedit("に"));
    assert_eq!(ui.field_text(field), Some("に"));
    build(&mut ui, 1, "");
    assert_eq!(ui.field_text(field), Some("に"), "the composition kept");
    ui.ime(&Ime::Commit("日".to_owned()));
    assert_eq!(ui.field_text(field), Some("日"));
}

#[test]
fn a_press_that_reaches_no_element_takes_the_focus_away_and_commits() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    ui.set_focus(Some(field));
    type_text(&mut ui, "typed");
    ui.take_actions();
    // Over nothing: the press is the game's, and the field commits.
    ui.pointer_moved(300.0, 250.0);
    assert!(!ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    assert_eq!(ui.focus(), None);
    assert_eq!(values(&mut ui), [r#"committed "typed""#]);
    assert!(!ui.key(&typed("w")), "the keys are the game's again");
    assert_eq!(ui.field_text(field), Some("typed"));
}

#[test]
fn the_caret_stays_within_a_field_whose_text_is_pushed_to_its_end() {
    let mut aligned = style();
    aligned.text.style.alignment = slime_ui::text::Alignment::End;
    let (mut ui, _, field, _) = interface(Element::text_field("", aligned));
    let rect = ui.rect(field).expect("drawn");
    ui.set_focus(Some(field));
    let within = |ui: &Ui| {
        let caret = ui.text_input().expect("a field takes text");
        assert!(
            caret.x >= rect.x && caret.x + caret.width <= rect.x + rect.width,
            "{caret:?} within {rect:?}"
        );
    };
    within(&ui);
    type_text(&mut ui, "end");
    within(&ui);
    type_text(&mut ui, &"wide ".repeat(20));
    within(&ui);
    // Hebrew, which starts on the right, with its caret at its start.
    let (mut ui, _, field, _) = interface(Element::text_field("שלום", style()));
    ui.set_focus(Some(field));
    ui.key(&named(NamedKey::Home));
    within(&ui);
    assert!(ui.field_text(field).is_some());
}

#[test]
fn a_field_takes_the_keys_that_type_through_an_input_method_without_text_of_their_own() {
    let (mut ui, _, field, _) = interface(Element::text_field("", style()));
    ui.set_focus(Some(field));
    // The key with which an input method begins to compose on Windows, before it says so, and a
    // dead key, which the key after it combines with: taken, and typing nothing of their own.
    let mut unnamed = named(NamedKey::F13);
    unnamed.logical = Key::Unidentified;
    assert!(ui.key(&unnamed));
    let mut dead = named(NamedKey::F13);
    dead.logical = Key::Dead(Some('^'));
    assert!(ui.key(&dead));
    assert_eq!(ui.field_text(field), Some(""));
    assert!(values(&mut ui).is_empty());
    // Without a field, they are the game's.
    ui.set_focus(None);
    assert!(!ui.key(&unnamed));
    assert!(!ui.key(&dead));
}

#[test]
fn a_drag_cut_short_by_another_press_commits_what_it_gave() {
    let (mut ui, _, field, _) = interface(Element::number_field(1.0, 0.5, style()));
    ui.pointer_moved(100.0, 15.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(110.0, 15.0);
    assert_eq!(ui.field_text(field), Some("6"));
    ui.take_actions();
    // A press without the release of the one before: the drag commits what it gave.
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    assert_eq!(values(&mut ui), ["committed 6"]);
}
