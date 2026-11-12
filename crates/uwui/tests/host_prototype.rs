//! The prototype of the host's API, without a window, thrown away once M6d.5a's ADR is accepted
//! (`docs/adr/0248-the-hosts-api-cut-in-five-built-where-it-is-read.md`, "Three channels and one
//! callback"): an input method's synchronous question, a paste as the web gives it and copy
//! answered at once, two targets.

use slime_input::{
    ButtonState, CursorShape, Ime, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey,
};
use uwui::Rect;
use uwui::action::{Action, ActionKind};
use uwui::shortcuts::Platform;
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::tree::host_prototype::{
    Answer, HostEvent, Outputs, SurroundingText, TargetId, TextIndex, Units, utf16_len,
};
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

const MAIN: TargetId = TargetId::MAIN;

/// An interface whose fonts are the tests' Noto Sans and Noto Sans Hebrew, following Linux's ways
/// with keys.
fn ui() -> Ui {
    let mut fonts = Fonts::new();
    for file in ["NotoSans.ttf", "NotoSansHebrew.ttf"] {
        let path = format!("{}/tests/fonts/{file}", env!("CARGO_MANIFEST_DIR"));
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
    }
    let mut ui = Ui::new(fonts);
    ui.set_platform(Platform::Linux);
    ui
}

/// A root of 400 by 300 logical pixels at scale 2, so that what the questions give is in pixels
/// of the surface, not logical ones.
fn settings() -> RootSettings {
    RootSettings {
        size: [400.0, 300.0],
        scale: 2.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    }
}

/// Writes `width` by 30 logical pixels, of the tests' fonts.
fn sized(s: StyleBuilder, width: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(30.0))
        .fonts(vec!["Noto Sans".to_owned(), "Noto Sans Hebrew".to_owned()])
}

/// An interface whose root holds a field of one line for each of `texts`, laid out once, the
/// first holding the focus, its caret at its end.
fn fields(texts: &[&str]) -> (Ui, Vec<ElementId>) {
    let mut ui = ui();
    let root = ui.add_root("main", settings(), |s| s);
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        for (index, text) in texts.iter().enumerate() {
            let field = Element::text_field(*text).style(|s| sized(s, 180.0));
            ids.push(children.add(index, field, |_| {}));
        }
    });
    ui.frame();
    ui.set_focus(Some(ids[0]));
    (ui, ids)
}

fn at(utf8: usize, utf16: usize) -> TextIndex {
    TextIndex { utf8, utf16 }
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

/// Presses Ctrl and the key of `letter` through the host, then lets Ctrl go: the answer.
fn control(ui: &mut Ui, letter: &str) -> Answer {
    let mut held = Modifiers::default();
    held.control = true;
    ui.host_event(MAIN, HostEvent::Modifiers(held));
    let answer = ui.host_event(
        MAIN,
        HostEvent::Key(&key(Key::Character(letter.to_owned()), Some(letter))),
    );
    ui.host_event(MAIN, HostEvent::Modifiers(Modifiers::default()));
    answer
}

fn taken() -> Answer {
    Answer {
        taken: true,
        ..Answer::default()
    }
}

/// The one rectangle that `range` of the focused field covers, asked in `units`.
fn one_rect(ui: &mut Ui, range: std::ops::Range<usize>, units: Units) -> Rect {
    let rects = ui
        .range_rects(MAIN, range.clone(), units)
        .expect("a field holds the focus");
    assert_eq!(
        rects.len(),
        1,
        "{range:?} in {units:?}: one line, one direction"
    );
    rects[0]
}

/// Whether `inner` lies within `outer`, up to a pixel.
fn within(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x - 1.0
        && inner.y >= outer.y - 1.0
        && inner.x + inner.width <= outer.x + outer.width + 1.0
        && inner.y + inner.height <= outer.y + outer.height + 1.0
}

#[test]
fn an_input_method_asks_around_the_caret_right_after_its_commit_without_a_frame() {
    // "é" is two bytes and one unit of UTF-16; "😀" four bytes and two units.
    for (committed, bytes, units) in [("é", 2, 1), ("😀", 4, 2)] {
        let (mut ui, ids) = fields(&["ab"]);
        let field = ui.rect(ids[0]).expect("laid out");
        let b = one_rect(&mut ui, 1..2, Units::Utf8);
        assert!(b.width > 0.0 && within(b, field), "{b:?} in {field:?}");
        let answer = ui.host_event(MAIN, HostEvent::Ime(&Ime::Commit(committed.to_owned())));
        assert_eq!(answer, taken());
        // No frame between the commit and the questions.
        let caret = at(2 + bytes, 2 + units);
        assert_eq!(
            ui.surrounding_text(MAIN, 100, 100, Units::Utf16),
            Some(SurroundingText {
                before: format!("ab{committed}"),
                selected: String::new(),
                after: String::new(),
                start: at(0, 0),
                anchor: caret,
                focus: caret,
                composition: None,
            }),
            "{committed}"
        );
        // The last character before the caret, asked in UTF-16 units, then in bytes: the same
        // rectangle, on b's line, to its right.
        let last = one_rect(&mut ui, 2..2 + units, Units::Utf16);
        assert_eq!(last, one_rect(&mut ui, 2..2 + bytes, Units::Utf8));
        assert!(
            last.x >= b.x + b.width - 0.5,
            "{committed}: {last:?} after {b:?}"
        );
        assert!((last.y - b.y).abs() < 0.5 && (last.height - b.height).abs() < 0.5);
        assert!(within(last, field), "{last:?} in {field:?}");
        // A range that cuts a character in two is widened to the whole character.
        if units == 2 {
            assert_eq!(one_rect(&mut ui, 3..4, Units::Utf16), last);
        }
    }
}

#[test]
fn the_text_around_the_caret_is_bounded_in_the_units_asked_without_cutting_a_character() {
    let (mut ui, _) = fields(&["ab"]);
    ui.host_event(MAIN, HostEvent::Ime(&Ime::Commit("😀".to_owned())));
    let before = |ui: &Ui, count: usize, units: Units| {
        let around = ui
            .surrounding_text(MAIN, count, 0, units)
            .expect("a field holds the focus");
        (around.before, around.start)
    };
    assert_eq!(before(&ui, 3, Units::Utf16), ("b😀".to_owned(), at(1, 1)));
    assert_eq!(before(&ui, 2, Units::Utf16), ("😀".to_owned(), at(2, 2)));
    // One unit would cut the emoji: nothing before the caret then.
    assert_eq!(before(&ui, 1, Units::Utf16), (String::new(), at(6, 4)));
    assert_eq!(before(&ui, 5, Units::Utf8), ("b😀".to_owned(), at(1, 1)));
    assert_eq!(before(&ui, 3, Units::Utf8), (String::new(), at(6, 4)));
}

#[test]
fn an_input_method_asks_about_a_composition_in_progress() {
    let (mut ui, _) = fields(&["ab"]);
    let b = one_rect(&mut ui, 1..2, Units::Utf8);
    // "日本" is six bytes and two units; the method's cursor between its two characters.
    let preedit = Ime::Preedit {
        text: "日本".to_owned(),
        cursor: Some((3, 3)),
    };
    assert_eq!(ui.host_event(MAIN, HostEvent::Ime(&preedit)), taken());
    assert_eq!(
        ui.surrounding_text(MAIN, 100, 100, Units::Utf16),
        Some(SurroundingText {
            before: "ab日".to_owned(),
            selected: String::new(),
            after: "本".to_owned(),
            start: at(0, 0),
            anchor: at(5, 3),
            focus: at(5, 3),
            composition: Some(at(2, 2)..at(8, 4)),
        })
    );
    let composed = one_rect(&mut ui, 2..4, Units::Utf16);
    assert!(
        composed.x >= b.x + b.width - 0.5,
        "{composed:?} after {b:?}"
    );
    // The composition committed: what it composed stays, the composition goes.
    ui.host_event(MAIN, HostEvent::Ime(&Ime::Commit("日本".to_owned())));
    let around = ui.surrounding_text(MAIN, 100, 100, Units::Utf8).unwrap();
    assert_eq!(
        (around.before.as_str(), around.composition),
        ("ab日本", None)
    );
    assert_eq!(around.focus, at(8, 4));
}

#[test]
fn the_character_at_a_point_is_the_one_drawn_there() {
    let (mut ui, ids) = fields(&["abc"]);
    let b = one_rect(&mut ui, 1..2, Units::Utf8);
    let middle = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    assert_eq!(ui.character_at(MAIN, middle.0, middle.1), Some(at(1, 1)));
    // Where no character is drawn, none; nor in a field that does not hold the focus.
    let field = ui.rect(ids[0]).unwrap();
    assert_eq!(
        ui.character_at(MAIN, field.x + field.width - 2.0, middle.1),
        None
    );
    ui.set_focus(None);
    assert_eq!(ui.character_at(MAIN, middle.0, middle.1), None);
    assert_eq!(ui.surrounding_text(MAIN, 10, 10, Units::Utf8), None);
}

#[test]
fn a_field_given_the_focus_answers_where_it_will_show_its_text_before_the_next_frame() {
    // Wider than the field: given the focus, its caret at its end, it scrolls to show its end,
    // which the next frame would do; the questions come before that frame.
    let text = "a".repeat(200);
    let (mut ui, ids) = fields(&[&text]);
    let field = ui.rect(ids[0]).unwrap();
    let last = one_rect(&mut ui, 199..200, Units::Utf8);
    assert!(within(last, field), "{last:?} in {field:?}");
    let middle = (last.x + last.width / 2.0, last.y + last.height / 2.0);
    assert_eq!(
        ui.character_at(MAIN, middle.0, middle.1),
        Some(at(199, 199))
    );
    // The next frame shows it where the question placed it.
    ui.frame();
    assert_eq!(one_rect(&mut ui, 199..200, Units::Utf8), last);
}

#[test]
fn copy_and_cut_answer_with_the_text_and_a_paste_event_inserts_its_own_text() {
    let (mut ui, ids) = fields(&["say hello"]);
    let field = ids[0];
    assert!(ui.select_text(field, 4, 9));
    let copied = Answer {
        taken: true,
        clipboard: Some("hello".to_owned()),
        ..Answer::default()
    };
    assert_eq!(ui.host_event(MAIN, HostEvent::Copy), copied);
    assert_eq!(ui.field_text(field), Some("say hello"));
    assert_eq!(ui.host_event(MAIN, HostEvent::Cut), copied);
    assert_eq!(ui.field_text(field), Some("say "));
    // Nothing selected: the field takes the gesture and writes nothing.
    assert_eq!(ui.host_event(MAIN, HostEvent::Copy), taken());
    assert_eq!(ui.host_event(MAIN, HostEvent::Paste("xyz")), taken());
    assert_eq!(ui.field_text(field), Some("say xyz"));
    // A field of one line takes a line break as a space.
    ui.host_event(MAIN, HostEvent::Paste("\nok"));
    assert_eq!(ui.field_text(field), Some("say xyz ok"));
    assert!(!ui.has_clipboard(), "no clipboard object was given");
    // Without a field that holds the focus, the gestures are the program's.
    ui.set_focus(None);
    assert_eq!(ui.host_event(MAIN, HostEvent::Copy), Answer::default());
    assert_eq!(
        ui.host_event(MAIN, HostEvent::Paste("no")),
        Answer::default()
    );
}

#[test]
fn the_keys_of_copy_and_cut_write_the_clipboard_through_the_outputs() {
    let (mut ui, ids) = fields(&["hello"]);
    ui.select_text(ids[0], 0, 5);
    assert_eq!(control(&mut ui, "c"), taken());
    assert_eq!(
        ui.take_outputs(MAIN),
        Outputs {
            write_clipboard: Some("hello".to_owned()),
            ..Outputs::default()
        }
    );
    assert_eq!(
        ui.take_outputs(MAIN),
        Outputs::default(),
        "only what changed"
    );
    assert_eq!(control(&mut ui, "x"), taken());
    assert_eq!(ui.field_text(ids[0]), Some(""));
    assert_eq!(
        ui.take_outputs(MAIN).write_clipboard.as_deref(),
        Some("hello")
    );
}

#[test]
fn the_paste_key_asks_the_host_to_read_the_clipboard_and_takes_one_answer_to_its_number() {
    let (mut ui, ids) = fields(&["", ""]);
    let (first, second) = (ids[0], ids[1]);
    assert_eq!(control(&mut ui, "v"), taken());
    let requests = ui.take_outputs(MAIN).read_clipboard;
    assert_eq!(requests.len(), 1);
    let request = requests[0];
    assert_eq!(ui.field_text(first), Some(""), "nothing before the answer");
    let answer = |request, text| HostEvent::ClipboardRead {
        request,
        text: Some(text),
    };
    assert_eq!(ui.host_event(MAIN, answer(request, "xyz")), taken());
    assert_eq!(ui.field_text(first), Some("xyz"));
    // A second answer to the same number, and one to a number never given, are ignored.
    assert_eq!(
        ui.host_event(MAIN, answer(request, "xyz")),
        Answer::default()
    );
    assert_eq!(
        ui.host_event(MAIN, answer(request + 99, "xyz")),
        Answer::default()
    );
    assert_eq!(ui.field_text(first), Some("xyz"));
    // Two presses, two requests, two answers: two pastes.
    control(&mut ui, "v");
    control(&mut ui, "v");
    let requests = ui.take_outputs(MAIN).read_clipboard;
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0], requests[1]);
    for &request in &requests {
        ui.host_event(MAIN, answer(request, "!"));
    }
    assert_eq!(ui.field_text(first), Some("xyz!!"));
    // The focus moved to another field before the answer: it is dropped, neither field changes.
    control(&mut ui, "v");
    let late = ui.take_outputs(MAIN).read_clipboard[0];
    ui.set_focus(Some(second));
    assert_eq!(ui.host_event(MAIN, answer(late, "late")), Answer::default());
    assert_eq!(ui.field_text(first), Some("xyz!!"));
    assert_eq!(ui.field_text(second), Some(""));
    // Even when it comes back before the answer: the person moved on.
    control(&mut ui, "v");
    let late = ui.take_outputs(MAIN).read_clipboard[0];
    ui.set_focus(Some(first));
    ui.set_focus(Some(second));
    assert_eq!(ui.host_event(MAIN, answer(late, "late")), Answer::default());
    assert_eq!(ui.field_text(second), Some(""));
    // No text on the clipboard, or the person refused: the request ends, nothing is pasted.
    control(&mut ui, "v");
    let empty = ui.take_outputs(MAIN).read_clipboard[0];
    let none = HostEvent::ClipboardRead {
        request: empty,
        text: None,
    };
    assert_eq!(ui.host_event(MAIN, none), taken());
    assert_eq!(ui.field_text(second), Some(""));
}

#[test]
fn on_the_web_the_keys_of_the_clipboard_are_taken_and_left_to_the_platform_s_events() {
    let (mut ui, ids) = fields(&["hello"]);
    ui.set_clipboard_events(true);
    ui.select_text(ids[0], 0, 5);
    let left = Answer {
        taken: true,
        platform_acts: true,
        clipboard: None,
    };
    // The key of paste is taken from the game, but not prevented: the browser fires `paste`.
    assert_eq!(control(&mut ui, "v"), left);
    assert_eq!(ui.take_outputs(MAIN), Outputs::default(), "no request");
    assert_eq!(control(&mut ui, "c"), left);
    assert_eq!(ui.take_outputs(MAIN), Outputs::default(), "no write");
    // The events that follow carry the clipboard.
    assert_eq!(ui.host_event(MAIN, HostEvent::Paste("xyz")), taken());
    assert_eq!(ui.field_text(ids[0]), Some("xyz"));
    // Other keys are prevented as before.
    let typed = key(Key::Character("!".to_owned()), Some("!"));
    assert_eq!(ui.host_event(MAIN, HostEvent::Key(&typed)), taken());
    let tab = key(Key::Named(NamedKey::Tab), None);
    assert_eq!(ui.host_event(MAIN, HostEvent::Key(&tab)), taken());
}

/// Two roots on two targets, `a` then `b`, each holding a button of 100 by 40 logical pixels at
/// its top left corner; `b`'s holds a field of text below its button too. The interface, the
/// buttons, and `b`'s field.
fn two_targets() -> (Ui, [ElementId; 2], ElementId, [TargetId; 2]) {
    let targets = [TargetId(1), TargetId(2)];
    let mut ui = ui();
    let mut buttons = Vec::new();
    let mut field = None;
    for (name, target) in ["a", "b"].into_iter().zip(targets) {
        let root = ui.add_root(name, settings(), |s| s.direction(FlexDirection::Column));
        ui.set_target(root, target);
        ui.build(root, 0, |children| {
            let button =
                Element::button("").style(|s| sized(s, 100.0).height(Dimension::Pixels(40.0)));
            buttons.push(children.add("button", button, |_| {}));
            if name == "b" {
                let text = Element::text_field("").style(|s| sized(s, 180.0));
                field = Some(children.add("field", text, |_| {}));
            }
        });
    }
    ui.frame();
    (ui, [buttons[0], buttons[1]], field.expect("built"), targets)
}

fn press(element: ElementId) -> Action {
    Action {
        element: Some(element),
        kind: ActionKind::Press,
    }
}

fn click(ui: &mut Ui, target: TargetId, x: f32, y: f32) {
    ui.host_event(target, HostEvent::PointerMoved { x, y });
    for state in [ButtonState::Pressed, ButtonState::Released] {
        let button = HostEvent::PointerButton {
            button: MouseButton::Left,
            state,
        };
        assert_eq!(ui.host_event(target, button), taken());
    }
}

#[test]
fn a_press_on_one_target_reaches_its_own_root_alone() {
    let (mut ui, [a, b], _, [on_a, on_b]) = two_targets();
    // Both buttons lie at the same place, each on its own surface.
    assert_eq!(ui.rect(a), ui.rect(b));
    // `a`'s root lies below `b`'s: without targets, the top root would take the press.
    click(&mut ui, on_a, 50.0, 20.0);
    assert_eq!(ui.take_actions(), [press(a)]);
    assert!(ui.state(a).unwrap().hovered && !ui.state(b).unwrap().hovered);
    ui.host_event(on_a, HostEvent::PointerLeft);
    click(&mut ui, on_b, 50.0, 20.0);
    assert_eq!(ui.take_actions(), [press(b)]);
    assert!(ui.state(b).unwrap().hovered && !ui.state(a).unwrap().hovered);
    // Each target's display list holds its own roots.
    let (whole, _) = ui.frame();
    let lists = [ui.target_list(on_a), ui.target_list(on_b)];
    assert!(lists.iter().all(|list| !list.is_empty()));
    assert_eq!(lists[0].len() + lists[1].len(), whole.len());
}

#[test]
fn the_hover_and_the_cursor_are_each_target_s_own() {
    let (mut ui, [a, b], field, [on_a, on_b]) = two_targets();
    // Over `b`'s field, at the place where `a` holds nothing.
    ui.host_event(on_b, HostEvent::PointerMoved { x: 50.0, y: 110.0 });
    assert!(ui.state(field).unwrap().hovered);
    assert_eq!(ui.take_outputs(on_b).cursor, Some(CursorShape::Text));
    assert_eq!(
        ui.take_outputs(on_a).cursor,
        None,
        "`a` still shows the arrow"
    );
    // The same place on `a`: nothing there, and `b`'s pointer stays where it was.
    ui.host_event(on_a, HostEvent::PointerMoved { x: 50.0, y: 110.0 });
    assert_eq!(ui.take_outputs(on_a).cursor, None);
    assert_eq!(ui.take_outputs(on_b).cursor, None, "only what changed");
    assert!(ui.state(field).unwrap().hovered);
    // Over `a`'s button: hovered on `a`, `b`'s untouched.
    ui.host_event(on_a, HostEvent::PointerMoved { x: 50.0, y: 20.0 });
    assert!(ui.state(a).unwrap().hovered && !ui.state(b).unwrap().hovered);
    assert!(ui.state(field).unwrap().hovered);
    // The pointer leaves `b`: its field is no longer hovered, its arrow comes back.
    ui.host_event(on_b, HostEvent::PointerLeft);
    assert!(!ui.state(field).unwrap().hovered);
    assert!(ui.state(a).unwrap().hovered);
    assert_eq!(ui.take_outputs(on_b).cursor, Some(CursorShape::Default));
}

/// What the questions and the units cost over a field of 4,000 bytes, and the pieces of that cost
/// through uwui's public text API; run by hand, in release:
/// `cargo test -p uwui --release --test host_prototype -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, run by hand in release"]
fn cost_over_a_field_of_4000_bytes() {
    use std::hint::black_box;
    use std::time::Instant;
    use uwui::text::{Affinity, TextBounds, TextPosition, TextStyle};

    // ASCII, two-byte, three-byte and four-byte characters, 4,000 bytes in all, on one line.
    let piece = "word é 日 😀 ";
    let text: String = piece.repeat(4000 / piece.len());
    let text = format!("{text}{}", "a".repeat(4000 - text.len()));
    assert_eq!(text.len(), 4000);
    println!(
        "{} characters, {} UTF-16 units",
        text.chars().count(),
        utf16_len(&text)
    );
    let time = |label: &str, runs: u32, run: &mut dyn FnMut()| {
        run();
        let start = Instant::now();
        for _ in 0..runs {
            run();
        }
        let each = start.elapsed().as_nanos() / u128::from(runs);
        println!("{label}: {each} ns");
    };
    time("utf16_len over 4,000 bytes", 20_000, &mut || {
        black_box(utf16_len(black_box(&text)));
    });
    time(
        "encode_utf16().count() over 4,000 bytes",
        20_000,
        &mut || {
            black_box(black_box(&text).encode_utf16().count());
        },
    );

    let (mut ui, _) = fields(&[&text]);
    time(
        "surrounding_text, 100 units each side, caret at the end",
        20_000,
        &mut || {
            black_box(ui.surrounding_text(MAIN, 100, 100, Units::Utf16));
        },
    );
    let end = utf16_len(&text);
    time("range_rects of the last character", 2_000, &mut || {
        black_box(ui.range_rects(MAIN, end - 1..end, Units::Utf16));
    });
    time("range_rects of the first character", 2_000, &mut || {
        black_box(ui.range_rects(MAIN, 0..1, Units::Utf16));
    });
    time("range_rects of the caret at the end", 2_000, &mut || {
        black_box(ui.range_rects(MAIN, end..end, Units::Utf16));
    });
    let last = ui.range_rects(MAIN, end - 1..end, Units::Utf16).unwrap()[0];
    let (x, y) = (last.x + last.width / 2.0, last.y + last.height / 2.0);
    println!(
        "character_at the last character: {:?}",
        ui.character_at(MAIN, x, y)
    );
    time("character_at the last character", 2_000, &mut || {
        black_box(ui.character_at(MAIN, x, y));
    });

    // The pieces: parley's geometry on the same text laid out once, and its shaping.
    let mut fonts = Fonts::new();
    for file in ["NotoSans.ttf", "NotoSansHebrew.ttf"] {
        let path = format!("{}/tests/fonts/{file}", env!("CARGO_MANIFEST_DIR"));
        fonts.register(std::fs::read(path).unwrap()).unwrap();
    }
    let style = TextStyle {
        fonts: vec!["Noto Sans".to_owned(), "Noto Sans Hebrew".to_owned()],
        ..TextStyle::default()
    };
    let layout = fonts.lay_out(&text, &style, 2.0, TextBounds::default());
    let last_char = text.len() - text.chars().last().unwrap().len_utf8()..text.len();
    time(
        "TextLayout::selection of the last character",
        2_000,
        &mut || {
            black_box(layout.selection(last_char.clone()));
        },
    );
    time(
        "TextLayout::selection of the first character",
        2_000,
        &mut || {
            black_box(layout.selection(0..1));
        },
    );
    let caret = TextPosition {
        index: text.len(),
        affinity: Affinity::Upstream,
    };
    time("TextLayout::caret at the end", 2_000, &mut || {
        black_box(layout.caret(caret));
    });
    time(
        "TextLayout::cluster_at the last character",
        2_000,
        &mut || {
            black_box(layout.cluster_at(layout.width() - 2.0, layout.height() / 2.0));
        },
    );
    time(
        "Fonts::lay_out of the 4,000 bytes (shaping and lines)",
        200,
        &mut || {
            black_box(fonts.lay_out(black_box(&text), &style, 2.0, TextBounds::default()));
        },
    );

    // An edit whose text was never laid out, then the question: each commit types a character
    // not typed before, which the edit's own settling lays out (`Ui::settle_field`); the
    // Backspace after it comes back to a text that the cache holds.
    let backspace = key(Key::Named(NamedKey::Backspace), None);
    let mut next = 0x4E00_u32;
    time(
        "an edit to a new text, Backspace, then range_rects",
        200,
        &mut || {
            next += 1;
            let typed = char::from_u32(next).unwrap().to_string();
            ui.host_event(MAIN, HostEvent::Ime(&Ime::Commit(typed)));
            let end = ui
                .surrounding_text(MAIN, 0, 0, Units::Utf16)
                .unwrap()
                .focus
                .utf16;
            black_box(ui.range_rects(MAIN, end - 1..end, Units::Utf16));
            ui.host_event(MAIN, HostEvent::Key(&backspace));
        },
    );
}
