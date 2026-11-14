//! The clipboard as the host serves it
//! (`docs/adr/0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md`; the cases of
//! `docs/adr/0248-the-hosts-api-cut-in-five-built-where-it-is-read.md`): the keys of copy and cut write through the outputs, the key of paste asks a numbered read whose
//! answer comes as an event; the platform's own copy, cut and paste as events, as the web gives
//! them; a read dropped when the focus moves, a second paste, no answer, a huge text.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, Modifiers, NamedKey};
use uwui::action::ActionKind;
use uwui::host::{Answer, Event, Input, Outputs, ReadId, TargetId};
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::{EventTime, Instant};
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

const MAIN: TargetId = TargetId::MAIN;

/// Writes 180 by 30 logical pixels, of the tests' font.
fn sized(s: StyleBuilder) -> StyleBuilder {
    s.width(Dimension::Pixels(180.0))
        .height(Dimension::Pixels(30.0))
        .fonts(vec!["Noto Sans".to_owned()])
}

/// An interface of the tests' Noto Sans, following Linux's ways with keys, whose root holds a
/// field of one line for each of `texts`, laid out once, the first holding the focus.
fn fields(texts: &[&str]) -> (Ui, Vec<ElementId>) {
    let (ui, _, ids) = rooted_fields(texts);
    (ui, ids)
}

/// [`fields`], and their root.
fn rooted_fields(texts: &[&str]) -> (Ui, ElementId, Vec<ElementId>) {
    let mut fonts = Fonts::new();
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    ui.set_platform(uwui::shortcuts::Platform::Linux);
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        for (index, text) in texts.iter().enumerate() {
            let field = Element::text_field(*text).style(sized);
            ids.push(children.add(index, field, |_| {}));
        }
    });
    ui.frame(TargetId::MAIN);
    ui.set_focus(Some(ids[0]));
    // The input method on for the field, read as a host reads it, so that the outputs then tell
    // the clipboard alone.
    let _ = ui.outputs(MAIN);
    (ui, root, ids)
}

/// `input`, given on the main target at the interface's start.
fn give(ui: &mut Ui, input: Input<'_>) -> Answer {
    ui.event(Event::new(MAIN, EventTime::simulated(Instant::ZERO), input))
}

fn key(logical: Key, text: Option<&str>) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical,
        text: text.map(str::to_owned),
        state: ButtonState::Pressed,
        repeat: false,
    }
}

/// Presses Ctrl and the key of `letter`, then lets Ctrl go: the key's answer.
fn control(ui: &mut Ui, letter: &str) -> Answer {
    let mut held = Modifiers::default();
    held.control = true;
    let _ = give(ui, Input::Modifiers(held));
    let answer = give(
        ui,
        Input::Key(&key(Key::Character(letter.to_owned()), Some(letter))),
    );
    let _ = give(ui, Input::Modifiers(Modifiers::default()));
    answer
}

/// The reads that the outputs ask.
fn reads(ui: &mut Ui) -> Vec<ReadId> {
    ui.outputs(MAIN).clipboard_reads
}

/// The answer to `read`, the clipboard holding `text`.
fn answer<'a>(read: ReadId, text: &'a str) -> Input<'a> {
    Input::ClipboardText {
        read,
        text: Some(text),
    }
}

#[test]
fn the_keys_of_copy_and_cut_write_the_clipboard_through_the_outputs() {
    let (mut ui, ids) = fields(&["hello"]);
    ui.select_text(ids[0], 0, 5);
    assert_eq!(control(&mut ui, "c"), Answer::Taken);
    let outputs = ui.outputs(MAIN);
    assert_eq!(outputs.clipboard_write.as_deref(), Some("hello"));
    assert_eq!(ui.outputs(MAIN), Outputs::default(), "only what changed");
    assert_eq!(control(&mut ui, "x"), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some(""));
    assert_eq!(ui.outputs(MAIN).clipboard_write.as_deref(), Some("hello"));
    // Nothing selected: the key is taken and writes nothing.
    assert_eq!(control(&mut ui, "c"), Answer::Taken);
    assert_eq!(ui.outputs(MAIN).clipboard_write, None);
}

#[test]
fn the_paste_key_asks_the_host_to_read_the_clipboard_and_takes_one_answer_to_its_number() {
    let (mut ui, ids) = fields(&["", ""]);
    let first = ids[0];
    assert_eq!(control(&mut ui, "v"), Answer::Taken);
    let asked = reads(&mut ui);
    assert_eq!(asked.len(), 1);
    assert_eq!(ui.field_text(first), Some(""), "nothing before the answer");
    assert!(reads(&mut ui).is_empty(), "each read told once");
    assert_eq!(give(&mut ui, answer(asked[0], "xyz")), Answer::Taken);
    assert_eq!(ui.field_text(first), Some("xyz"));
    // A second answer to the same read is not taken, and changes nothing.
    assert_eq!(give(&mut ui, answer(asked[0], "xyz")), Answer::NotTaken);
    assert_eq!(ui.field_text(first), Some("xyz"));
    // No text on the clipboard, or the person refused: the read ends, nothing is pasted.
    let _ = control(&mut ui, "v");
    let empty = reads(&mut ui)[0];
    let none = Input::ClipboardText {
        read: empty,
        text: None,
    };
    assert_eq!(give(&mut ui, none), Answer::Taken);
    assert_eq!(ui.field_text(first), Some("xyz"));
}

#[test]
fn a_second_paste_asks_a_second_read_and_each_answer_pastes_as_it_comes() {
    let (mut ui, ids) = fields(&[""]);
    let _ = control(&mut ui, "v");
    let _ = control(&mut ui, "v");
    let asked = reads(&mut ui);
    assert_eq!(asked.len(), 2);
    assert_ne!(asked[0], asked[1]);
    // Answered in the other order: each pastes at the caret when it comes.
    let _ = give(&mut ui, answer(asked[1], "two "));
    let _ = give(&mut ui, answer(asked[0], "one "));
    assert_eq!(ui.field_text(ids[0]), Some("two one "));
}

#[test]
fn a_paste_pending_when_the_focus_moves_is_dropped_even_if_the_focus_comes_back() {
    let (mut ui, ids) = fields(&["", ""]);
    let (first, second) = (ids[0], ids[1]);
    let _ = control(&mut ui, "v");
    let late = reads(&mut ui)[0];
    ui.set_focus(Some(second));
    assert_eq!(give(&mut ui, answer(late, "late")), Answer::NotTaken);
    assert_eq!(ui.field_text(first), Some(""));
    assert_eq!(ui.field_text(second), Some(""));
    // Back before the answer: the person moved on.
    let _ = control(&mut ui, "v");
    let late = reads(&mut ui)[0];
    ui.set_focus(Some(first));
    ui.set_focus(Some(second));
    assert_eq!(give(&mut ui, answer(late, "late")), Answer::NotTaken);
    assert_eq!(ui.field_text(second), Some(""));
    // A read dropped before the host read the outputs is never told.
    let _ = control(&mut ui, "v");
    ui.set_focus(Some(first));
    assert!(reads(&mut ui).is_empty());
}

#[test]
fn a_paste_pending_when_its_field_is_removed_is_dropped() {
    let (mut ui, root, _) = rooted_fields(&["", ""]);
    let _ = control(&mut ui, "v");
    // The program builds its root again without the field that held the focus.
    ui.build(root, 1, |children| {
        children.add(1_usize, Element::text_field("").style(sized), |_| {});
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(ui.focus(), None);
    assert!(reads(&mut ui).is_empty(), "never told the host");
}

#[test]
fn a_host_that_never_answers_keeps_no_more_than_sixteen_reads_waiting() {
    let (mut ui, ids) = fields(&[""]);
    for _ in 0..20 {
        let _ = control(&mut ui, "v");
    }
    let asked = reads(&mut ui);
    assert_eq!(asked.len(), 16, "the oldest four dropped");
    // The field goes on as before, and an answer to a dropped read is not taken.
    let typed = key(Key::Character("a".to_owned()), Some("a"));
    assert_eq!(give(&mut ui, Input::Key(&typed)), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some("a"));
    assert_eq!(give(&mut ui, answer(asked[15], "!")), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some("a!"));
}

#[test]
fn a_huge_text_is_pasted_whole() {
    let (mut ui, ids) = fields(&[""]);
    let huge = "word ".repeat(20_000);
    let _ = control(&mut ui, "v");
    let read = reads(&mut ui)[0];
    assert_eq!(give(&mut ui, answer(read, &huge)), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]).map(str::len), Some(huge.len()));
}

#[test]
fn the_platforms_copy_cut_and_paste_reach_the_field_that_holds_the_focus() {
    let (mut ui, ids) = fields(&["say hello"]);
    let field = ids[0];
    assert!(ui.select_text(field, 4, 9));
    assert_eq!(give(&mut ui, Input::Copy), Answer::Taken);
    assert_eq!(ui.outputs(MAIN).clipboard_write.as_deref(), Some("hello"));
    assert_eq!(ui.field_text(field), Some("say hello"));
    assert_eq!(give(&mut ui, Input::Cut), Answer::Taken);
    assert_eq!(ui.outputs(MAIN).clipboard_write.as_deref(), Some("hello"));
    assert_eq!(ui.field_text(field), Some("say "));
    // Nothing selected: the field takes the gesture and writes nothing.
    assert_eq!(give(&mut ui, Input::Copy), Answer::Taken);
    assert_eq!(ui.outputs(MAIN).clipboard_write, None);
    assert_eq!(give(&mut ui, Input::Paste("xyz")), Answer::Taken);
    assert_eq!(ui.field_text(field), Some("say xyz"));
    // A field of one line takes a line break as a space.
    let _ = give(&mut ui, Input::Paste("\nok"));
    assert_eq!(ui.field_text(field), Some("say xyz ok"));
    // Without a field that holds the focus, the gestures are the program's.
    ui.set_focus(None);
    assert_eq!(give(&mut ui, Input::Copy), Answer::NotTaken);
    assert_eq!(give(&mut ui, Input::Paste("no")), Answer::NotTaken);
}

#[test]
fn on_the_web_the_keys_of_the_clipboard_are_taken_and_left_to_the_platforms_events() {
    let (mut ui, ids) = fields(&["hello"]);
    ui.set_clipboard_events(true);
    ui.select_text(ids[0], 0, 5);
    // The key of paste is taken from the game, but not prevented: the browser fires `paste`.
    assert_eq!(control(&mut ui, "v"), Answer::TakenLeftToPlatform);
    assert!(control(&mut ui, "v").taken());
    assert!(ui.outputs(MAIN).clipboard_reads.is_empty(), "no read");
    assert_eq!(control(&mut ui, "c"), Answer::TakenLeftToPlatform);
    assert_eq!(ui.outputs(MAIN).clipboard_write, None, "no write");
    // The browser's `copy`, then its `paste`, carrying the clipboard: the copy's text read from
    // the outputs inside the handler, the page preventing the event's own copy.
    assert_eq!(give(&mut ui, Input::Copy), Answer::Taken);
    assert_eq!(ui.outputs(MAIN).clipboard_write.as_deref(), Some("hello"));
    assert_eq!(give(&mut ui, Input::Paste("xyz")), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some("xyz"));
    // Other keys are taken as before, and prevented.
    let typed = key(Key::Character("!".to_owned()), Some("!"));
    assert_eq!(give(&mut ui, Input::Key(&typed)), Answer::Taken);
    let tab = key(Key::Named(NamedKey::Tab), None);
    assert_eq!(give(&mut ui, Input::Key(&tab)), Answer::Taken);
}

#[test]
fn a_paste_while_an_input_method_composes_keeps_what_it_composed_and_ends_its_input() {
    let (mut ui, ids) = fields(&["ab"]);
    let preedit = slime_input::Ime::Preedit {
        text: "に".to_owned(),
        cursor: Some((3, 3)),
    };
    let _ = give(&mut ui, Input::InputMethod(&preedit));
    let _ = ui.outputs(MAIN);
    assert_eq!(give(&mut ui, Input::Paste("xyz")), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some("abにxyz"));
    // The host is told that the field's input starts again, which ends what the method composed.
    assert_eq!(
        ui.outputs(MAIN).input_method,
        Some(uwui::host::InputMethod::On)
    );
    // What it composes next starts after the paste.
    let _ = give(&mut ui, Input::InputMethod(&preedit));
    assert_eq!(ui.field_text(ids[0]), Some("abにxyzに"));
}

#[test]
fn the_programs_read_survives_the_focus_and_comes_back_as_an_action() {
    let (mut ui, ids) = fields(&["", ""]);
    ui.write_clipboard("copied");
    let read = ui.read_clipboard();
    ui.set_focus(Some(ids[1]));
    let outputs = ui.outputs(MAIN);
    assert_eq!(outputs.clipboard_write.as_deref(), Some("copied"));
    assert_eq!(outputs.clipboard_reads, [read]);
    ui.take_actions();
    assert_eq!(give(&mut ui, answer(read, "copied")), Answer::Taken);
    assert_eq!(program_reads(&mut ui), [(read, Some("copied".to_owned()))]);
    assert_eq!(ui.field_text(ids[1]), Some(""), "no field pastes it");
}

#[test]
fn a_programs_read_dropped_unanswered_still_reaches_the_program() {
    let (mut ui, _) = fields(&[""]);
    let first = ui.read_clipboard();
    for _ in 0..16 {
        let _ = control(&mut ui, "v");
    }
    // The oldest dropped: the program is told, with no text, at once.
    assert_eq!(program_reads(&mut ui), [(first, None)]);
    assert!(!reads(&mut ui).contains(&first));
}

/// The clipboard's texts that the program's reads gave it, with their reads.
fn program_reads(ui: &mut Ui) -> Vec<(ReadId, Option<String>)> {
    ui.take_actions()
        .into_iter()
        .filter_map(|action| match action.kind {
            ActionKind::ClipboardText { read, text } if action.element.is_none() => {
                Some((read, text))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn another_targets_clipboard_events_are_not_taken() {
    let (mut ui, ids) = fields(&["hello"]);
    let other = Event::new(
        TargetId(1),
        EventTime::simulated(Instant::ZERO),
        Input::Paste("x"),
    );
    assert_eq!(ui.event(other), Answer::NotTaken);
    assert_eq!(ui.field_text(ids[0]), Some("hello"));
}
