//! The host's questions and the deletion around the caret
//! (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`, "Questions" and "Rules"): an
//! input method asks around the caret right after its commit, with no frame between, in the unit
//! that it counts in; a range's box and the character at a point in the target's physical pixels;
//! whether a point is the interface's.

use std::hint::black_box;
use std::time::Instant;

use slime_input::{ButtonState, Ime, Key, KeyCode, KeyEvent, NamedKey};
use uwui::Rect;
use uwui::host::{Answer, Event, Input, RangeBox, TargetId, TextAround, TextQuery, TextUnit};
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::EventTime;
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

const MAIN: TargetId = TargetId::MAIN;

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

/// Writes `width` by 30 logical pixels, of the tests' fonts.
fn sized(s: StyleBuilder, width: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(30.0))
        .fonts(vec!["Noto Sans".to_owned(), "Noto Sans Hebrew".to_owned()])
}

/// An interface whose root, of 400 by 300 logical pixels at scale 2, so that the answers are in
/// physical pixels, not logical ones, holds a field of one line, 180 pixels wide, for each of
/// `texts`, laid out once, the first holding the focus, its caret at its end.
fn fields(texts: &[&str]) -> (Ui, Vec<ElementId>) {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 2.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
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

/// `input`, given on the main target at the interface's start.
fn give(ui: &mut Ui, input: Input<'_>) -> Answer {
    ui.event(Event::new(
        MAIN,
        EventTime::simulated(uwui::time::Instant::ZERO),
        input,
    ))
}

/// An input method's commit of `text`.
fn commit(ui: &mut Ui, text: &str) -> Answer {
    give(ui, Input::InputMethod(&Ime::Commit(text.to_owned())))
}

/// Asks up to `before` and `after` units around the caret, in `unit`, the composition included.
fn around(ui: &mut Ui, before: usize, after: usize, unit: TextUnit) -> Option<TextAround> {
    let query = TextQuery {
        before,
        after,
        unit,
        composition: true,
    };
    ui.text_around(MAIN, query)
}

/// The box of `range` of the focused field, asked in `unit`, which must lie on one line.
fn one_box(ui: &mut Ui, range: std::ops::Range<usize>, unit: TextUnit) -> Rect {
    let RangeBox { rect, end } = ui
        .range_box(MAIN, range.clone(), unit)
        .expect("a field holds the focus");
    assert_eq!(end, range.end, "{range:?} in {unit:?}: on one line");
    rect
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
    // "é" is two bytes, one unit of UTF-16 and one character; "😀" four bytes, two units, one
    // character.
    for (committed, bytes, units) in [("é", 2, 1), ("😀", 4, 2)] {
        let (mut ui, ids) = fields(&["ab"]);
        let field = ui.rect(ids[0]).expect("laid out");
        let b = one_box(&mut ui, 1..2, TextUnit::Utf8);
        assert!(b.width > 0.0 && within(b, field), "{b:?} in {field:?}");
        assert_eq!(commit(&mut ui, committed), Answer::Taken);
        // No frame between the commit and the questions.
        for (unit, caret) in [
            (TextUnit::Utf8, 2 + bytes),
            (TextUnit::Utf16, 2 + units),
            (TextUnit::CodePoint, 3),
        ] {
            assert_eq!(
                around(&mut ui, 100, 100, unit),
                Some(TextAround {
                    text: format!("ab{committed}"),
                    start: 0,
                    anchor: caret,
                    focus: caret,
                    composition: None,
                }),
                "{committed} in {unit:?}"
            );
        }
        // The last character before the caret, in UTF-16 units, then in bytes: the same box, on
        // b's line, to its right.
        let last = one_box(&mut ui, 2..2 + units, TextUnit::Utf16);
        assert_eq!(last, one_box(&mut ui, 2..2 + bytes, TextUnit::Utf8));
        assert!(
            last.x >= b.x + b.width - 0.5,
            "{committed}: {last:?} after {b:?}"
        );
        assert!((last.y - b.y).abs() < 0.5 && (last.height - b.height).abs() < 0.5);
        assert!(within(last, field), "{last:?} in {field:?}");
        // A range that cuts a character in two is widened to the whole character.
        if units == 2 {
            assert_eq!(
                ui.range_box(MAIN, 3..4, TextUnit::Utf16)
                    .map(|answer| answer.rect),
                Some(last)
            );
        }
    }
}

#[test]
fn the_text_around_the_caret_is_bounded_in_the_unit_asked_without_cutting_a_character() {
    let (mut ui, _) = fields(&["ab"]);
    let _ = commit(&mut ui, "😀");
    let before = |ui: &mut Ui, count: usize, unit: TextUnit| {
        let around = around(ui, count, 0, unit).expect("a field holds the focus");
        (around.text, around.start)
    };
    assert_eq!(before(&mut ui, 3, TextUnit::Utf16), ("b😀".to_owned(), 1));
    assert_eq!(before(&mut ui, 2, TextUnit::Utf16), ("😀".to_owned(), 2));
    // One unit would cut the emoji: nothing before the caret then.
    assert_eq!(before(&mut ui, 1, TextUnit::Utf16), (String::new(), 4));
    assert_eq!(before(&mut ui, 5, TextUnit::Utf8), ("b😀".to_owned(), 1));
    assert_eq!(before(&mut ui, 3, TextUnit::Utf8), (String::new(), 6));
    assert_eq!(
        before(&mut ui, 1, TextUnit::CodePoint),
        ("😀".to_owned(), 2)
    );
}

#[test]
fn the_selection_is_given_whole_with_its_anchor_and_its_focus() {
    let (mut ui, ids) = fields(&["one two three"]);
    // From the end of "two" back to its start: the anchor after it, the focus before it.
    assert!(ui.select_text(ids[0], 7, 4));
    assert_eq!(
        around(&mut ui, 1, 1, TextUnit::Utf8),
        Some(TextAround {
            text: " two ".to_owned(),
            start: 3,
            anchor: 4,
            focus: 1,
            composition: None,
        })
    );
}

#[test]
fn an_input_method_asks_about_a_composition_in_progress_with_it_or_without_it() {
    let (mut ui, _) = fields(&["ab"]);
    let b = one_box(&mut ui, 1..2, TextUnit::Utf8);
    // "日本" is six bytes and two units; the method's cursor between its two characters.
    let preedit = Ime::Preedit {
        text: "日本".to_owned(),
        cursor: Some((3, 3)),
    };
    assert_eq!(give(&mut ui, Input::InputMethod(&preedit)), Answer::Taken);
    assert_eq!(
        around(&mut ui, 100, 100, TextUnit::Utf16),
        Some(TextAround {
            text: "ab日本".to_owned(),
            start: 0,
            anchor: 3,
            focus: 3,
            composition: Some(2..4),
        })
    );
    // Without it, as Wayland asks: the text committed, the caret where the composition began.
    let without = TextQuery {
        before: 100,
        after: 100,
        unit: TextUnit::Utf8,
        composition: false,
    };
    assert_eq!(
        ui.text_around(MAIN, without),
        Some(TextAround {
            text: "ab".to_owned(),
            start: 0,
            anchor: 2,
            focus: 2,
            composition: None,
        })
    );
    let composed = one_box(&mut ui, 2..4, TextUnit::Utf16);
    assert!(
        composed.x >= b.x + b.width - 0.5,
        "{composed:?} after {b:?}"
    );
    // The composition committed: what it composed stays, the composition goes.
    let _ = commit(&mut ui, "日本");
    let answer = around(&mut ui, 100, 100, TextUnit::Utf8).expect("a field holds the focus");
    assert_eq!((answer.text.as_str(), answer.composition), ("ab日本", None));
    assert_eq!(answer.focus, 8);
}

#[test]
fn a_range_over_two_lines_gives_its_first_lines_box_and_where_it_ends() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
    let mut area = None;
    ui.build(root, 0, |children| {
        let element = Element::text_area("first\nsecond")
            .style(|s| sized(s, 200.0).height(Dimension::Pixels(100.0)));
        area = Some(children.add("area", element, |_| {}));
    });
    ui.frame();
    ui.set_focus(area);
    // "rst\nsec": its first line's part ends after the line break, the sixth byte.
    let first = ui
        .range_box(MAIN, 2..9, TextUnit::Utf8)
        .expect("a field holds the focus");
    assert_eq!(first.end, 6);
    let second = ui
        .range_box(MAIN, first.end..9, TextUnit::Utf8)
        .expect("a field holds the focus");
    assert_eq!(second.end, 9);
    assert!(
        second.rect.y >= first.rect.y + first.rect.height - 0.5,
        "{second:?} below {first:?}"
    );
    // Empty: the caret's box there.
    let caret = ui
        .range_box(MAIN, 2..2, TextUnit::Utf8)
        .expect("a field holds the focus");
    assert_eq!((caret.end, caret.rect.width), (2, 1.0));
}

#[test]
fn a_range_of_two_directions_is_boxed_whole_on_its_line() {
    // Latin, then Hebrew: the range crosses from one direction to the other.
    let (mut ui, _) = fields(&["ab שלום"]);
    let whole = ui
        .range_box(MAIN, 0..12, TextUnit::Utf8)
        .expect("a field holds the focus");
    let latin = one_box(&mut ui, 0..2, TextUnit::Utf8);
    let hebrew = one_box(&mut ui, 3..12, TextUnit::Utf8);
    assert_eq!(whole.end, 12);
    assert!(
        within(latin, whole.rect) && within(hebrew, whole.rect),
        "{whole:?}"
    );
}

#[test]
fn the_character_at_a_point_is_the_one_drawn_there() {
    let (mut ui, ids) = fields(&["aé😀c"]);
    let emoji = one_box(&mut ui, 3..7, TextUnit::Utf8);
    let middle = (emoji.x + emoji.width / 2.0, emoji.y + emoji.height / 2.0);
    for (unit, start) in [
        (TextUnit::Utf8, 3),
        (TextUnit::Utf16, 2),
        (TextUnit::CodePoint, 2),
    ] {
        assert_eq!(ui.character_at(MAIN, middle.0, middle.1, unit), Some(start));
    }
    // Where no character is drawn, none; nor in a field that does not hold the focus.
    let field = ui.rect(ids[0]).expect("laid out");
    assert_eq!(
        ui.character_at(MAIN, field.x + field.width - 2.0, middle.1, TextUnit::Utf8),
        None
    );
    ui.set_focus(None);
    assert_eq!(
        ui.character_at(MAIN, middle.0, middle.1, TextUnit::Utf8),
        None
    );
    assert_eq!(around(&mut ui, 10, 10, TextUnit::Utf8), None);
    assert_eq!(ui.range_box(MAIN, 0..1, TextUnit::Utf8), None);
}

#[test]
fn a_field_given_the_focus_answers_where_it_will_show_its_text_before_the_next_frame() {
    // Wider than the field: given the focus, its caret at its end, it scrolls to show its end,
    // which the next frame would do; the questions come before that frame.
    let text = "a".repeat(200);
    let (mut ui, ids) = fields(&[&text]);
    let field = ui.rect(ids[0]).expect("laid out");
    let last = one_box(&mut ui, 199..200, TextUnit::Utf8);
    assert!(within(last, field), "{last:?} in {field:?}");
    let middle = (last.x + last.width / 2.0, last.y + last.height / 2.0);
    assert_eq!(
        ui.character_at(MAIN, middle.0, middle.1, TextUnit::Utf8),
        Some(199)
    );
    // The next frame shows it where the question placed it.
    ui.frame();
    assert_eq!(one_box(&mut ui, 199..200, TextUnit::Utf8), last);
}

#[test]
fn questions_about_another_target_have_no_answer() {
    let (mut ui, _) = fields(&["ab"]);
    let other = TargetId(1);
    let query = TextQuery {
        before: 10,
        after: 10,
        unit: TextUnit::Utf8,
        composition: true,
    };
    assert_eq!(ui.text_around(other, query), None);
    assert_eq!(ui.range_box(other, 0..1, TextUnit::Utf8), None);
    assert_eq!(ui.character_at(other, 10.0, 10.0, TextUnit::Utf8), None);
    assert!(!ui.covers(other, 10.0, 10.0));
}

#[test]
fn a_point_is_the_interfaces_where_a_press_would_be_taken() {
    let (ui, ids) = fields(&["ab"]);
    let field = ui.rect(ids[0]).expect("laid out");
    assert!(ui.covers(MAIN, field.x + 4.0, field.y + 4.0));
    // The root lays its field out and draws nothing of its own beside it.
    assert!(!ui.covers(MAIN, 700.0, 500.0));
}

#[test]
fn the_input_method_deletes_around_the_caret_in_the_unit_it_counts() {
    let (mut ui, ids) = fields(&["a😀bc"]);
    // The caret after "a😀b": two units of UTF-16 before it, the emoji's, deleted whole.
    let left = Key::Named(NamedKey::ArrowLeft);
    let arrow = KeyEvent {
        physical: KeyCode::ArrowLeft,
        logical: left,
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    };
    let _ = give(&mut ui, Input::Key(&arrow));
    let delete = |before, after, unit| Input::DeleteAround {
        before,
        after,
        unit,
    };
    assert_eq!(give(&mut ui, delete(1, 0, TextUnit::Utf8)), Answer::Taken);
    assert_eq!(ui.field_text(ids[0]), Some("a😀c"));
    // One unit of the emoji's two would cut it: nothing is deleted.
    let _ = give(&mut ui, delete(1, 0, TextUnit::Utf16));
    assert_eq!(ui.field_text(ids[0]), Some("a😀c"));
    let _ = give(&mut ui, delete(2, 0, TextUnit::Utf16));
    assert_eq!(ui.field_text(ids[0]), Some("ac"));
    // Past both ends, it stops there; the caret stays between what it kept.
    let _ = give(&mut ui, delete(9, 0, TextUnit::CodePoint));
    assert_eq!(ui.field_text(ids[0]), Some("c"));
    assert_eq!(
        around(&mut ui, 9, 9, TextUnit::Utf8).map(|around| around.focus),
        Some(0)
    );
    let _ = give(&mut ui, delete(0, 9, TextUnit::CodePoint));
    assert_eq!(ui.field_text(ids[0]), Some(""));
    // Without a field that holds the focus, not taken.
    ui.set_focus(None);
    assert_eq!(
        give(&mut ui, delete(1, 1, TextUnit::Utf8)),
        Answer::NotTaken
    );
}

#[test]
fn a_deletion_around_keeps_the_selection_and_the_composition() {
    let (mut ui, ids) = fields(&["one two three"]);
    ui.select_text(ids[0], 4, 7);
    let delete = |before, after| Input::DeleteAround {
        before,
        after,
        unit: TextUnit::Utf8,
    };
    let _ = give(&mut ui, delete(1, 1));
    assert_eq!(ui.field_text(ids[0]), Some("onetwothree"));
    let answer = around(&mut ui, 0, 0, TextUnit::Utf8).expect("a field holds the focus");
    assert_eq!(
        (answer.text.as_str(), answer.anchor, answer.focus),
        ("two", 0, 3)
    );
    // While an input method composes, counted from the composition's ends, which stays.
    let (mut ui, ids) = fields(&["ab"]);
    let preedit = Ime::Preedit {
        text: "日".to_owned(),
        cursor: Some((3, 3)),
    };
    let _ = give(&mut ui, Input::InputMethod(&preedit));
    let _ = give(&mut ui, delete(1, 0));
    assert_eq!(ui.field_text(ids[0]), Some("a日"));
    let answer = around(&mut ui, 9, 9, TextUnit::Utf8).expect("a field holds the focus");
    assert_eq!(answer.composition, Some(1..4));
    // One edit, which an undo gives back.
    let _ = commit(&mut ui, "日");
    let undo = KeyEvent {
        physical: KeyCode::KeyZ,
        logical: Key::Character("z".to_owned()),
        text: Some("z".to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    };
    let mut held = slime_input::Modifiers::default();
    held.control = true;
    let _ = give(&mut ui, Input::Modifiers(held));
    let _ = give(&mut ui, Input::Key(&undo));
    assert_eq!(ui.field_text(ids[0]), Some("ab"));
}

/// What the questions cost over a field of one line of 4,000 bytes, against M6d.5a's prototype
/// (`docs/measurements.md`, M6d.5c2): `cargo test -p uwui --release --test questions --
/// --ignored --nocapture`. Not a benchmark: it prints the time of each, which a run at rest
/// reads.
#[test]
#[ignore = "a measure, run by hand in release"]
fn cost_over_a_field_of_4000_bytes() {
    // ASCII, two-byte, three-byte and four-byte characters, as the prototype measured them; Latin
    // and Hebrew, a text of two directions in many short runs; and a few of those followed by one
    // long run of Latin, where the caret stands.
    let fill = |head: String| format!("{head}{}", "a".repeat(4000 - head.len()));
    let texts = [
        (
            "one direction",
            fill("word é 日 😀 ".repeat(4000 / "word é 日 😀 ".len())),
        ),
        (
            "two directions",
            fill("word שלום ".repeat(4000 / "word שלום ".len())),
        ),
        (
            "two directions, then one long run",
            fill("Héllo wörld שלום 😀 ".repeat(20)),
        ),
    ];
    for (name, text) in texts {
        assert_eq!(text.len(), 4000);
        println!("{name}:");
        cost_over(&text);
    }
}

/// What the questions cost over a field of one line holding `text`, printed.
fn cost_over(text: &str) {
    let time = |label: &str, runs: u32, run: &mut dyn FnMut()| {
        run();
        let start = Instant::now();
        for _ in 0..runs {
            run();
        }
        let each = start.elapsed().as_nanos() / u128::from(runs);
        println!("{label}: {each} ns");
    };
    let (mut ui, _) = fields(&[text]);
    time(
        "text_around, 100 units each side, caret at the end",
        20_000,
        &mut || {
            black_box(around(&mut ui, 100, 100, TextUnit::Utf16));
        },
    );
    let end = text.encode_utf16().count();
    time("range_box of the last character", 2_000, &mut || {
        black_box(ui.range_box(MAIN, end - 1..end, TextUnit::Utf16));
    });
    time("range_box of the first character", 2_000, &mut || {
        black_box(ui.range_box(MAIN, 0..1, TextUnit::Utf16));
    });
    time("range_box of the caret at the end", 2_000, &mut || {
        black_box(ui.range_box(MAIN, end..end, TextUnit::Utf16));
    });
    let last = ui
        .range_box(MAIN, end - 1..end, TextUnit::Utf16)
        .expect("a field holds the focus")
        .rect;
    let (x, y) = (last.x + last.width / 2.0, last.y + last.height / 2.0);
    time("character_at the last character", 2_000, &mut || {
        black_box(ui.character_at(MAIN, x, y, TextUnit::Utf16));
    });
    // An edit whose text was never laid out, then the question: each commit types a character
    // not typed before; the Backspace after it comes back to a text that the cache holds.
    let backspace = KeyEvent {
        physical: KeyCode::Backspace,
        logical: Key::Named(NamedKey::Backspace),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    };
    let mut next = 0x4E00_u32;
    time(
        "an edit to a new text, Backspace, then range_box",
        200,
        &mut || {
            next += 1;
            let typed = char::from_u32(next).expect("a character").to_string();
            let _ = commit(&mut ui, &typed);
            let end = around(&mut ui, 0, 0, TextUnit::Utf16)
                .expect("a field")
                .focus;
            black_box(ui.range_box(MAIN, end - 1..end, TextUnit::Utf16));
            let _ = give(&mut ui, Input::Key(&backspace));
        },
    );
}

#[test]
fn the_text_around_holds_the_whole_composition_however_small_the_bounds() {
    for cursor in [None, Some((0, 0)), Some((3, 3))] {
        let (mut ui, _) = fields(&["ab"]);
        let preedit = Ime::Preedit {
            text: "日本".to_owned(),
            cursor,
        };
        let _ = give(&mut ui, Input::InputMethod(&preedit));
        for (before, after) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let answer = around(&mut ui, before, after, TextUnit::Utf16)
                .unwrap_or_else(|| panic!("{cursor:?}: a field holds the focus"));
            let composition = answer.composition.clone().expect("it composes");
            let units: Vec<u16> = answer.text.encode_utf16().collect();
            assert_eq!(
                String::from_utf16(&units[composition]).ok().as_deref(),
                Some("日本"),
                "{cursor:?}, {before} before and {after} after: {answer:?}"
            );
            assert!(answer.focus <= units.len() && answer.anchor <= units.len());
        }
    }
}

#[test]
fn a_press_in_a_field_while_an_input_method_composes_keeps_what_it_composed() {
    let (mut ui, ids) = fields(&["ab"]);
    let field = ui.rect(ids[0]).expect("laid out");
    let preedit = Ime::Preedit {
        text: "日本".to_owned(),
        cursor: Some((6, 6)),
    };
    let _ = give(&mut ui, Input::InputMethod(&preedit));
    let _ = ui.outputs(MAIN);
    // A press at the field's start, then a drag over its text: the composition is kept as typed,
    // and the field's input starts again, which the host is told.
    let at = |x: f32| Input::PointerMoved {
        x,
        y: field.y + field.height / 2.0,
    };
    let button = |state| Input::PointerButton {
        button: slime_input::MouseButton::Left,
        state,
    };
    let _ = give(&mut ui, at(field.x + 1.0));
    let _ = give(&mut ui, button(ButtonState::Pressed));
    let _ = give(&mut ui, at(field.x + field.width - 1.0));
    let _ = give(&mut ui, button(ButtonState::Released));
    assert_eq!(ui.field_text(ids[0]), Some("ab日本"));
    assert_eq!(
        ui.outputs(MAIN).input_method,
        Some(uwui::host::InputMethod::On)
    );
    let answer = around(&mut ui, 9, 9, TextUnit::Utf8).expect("a field holds the focus");
    assert_eq!(answer.composition, None);
    // What edits after it reads the selection that the drag made.
    let _ = give(
        &mut ui,
        Input::DeleteAround {
            before: 1,
            after: 1,
            unit: TextUnit::Utf8,
        },
    );
    let _ = give(&mut ui, Input::Cut);
    assert_eq!(ui.field_text(ids[0]), Some(""));
    // The program's selection, while a new composition is shown, ends it as well.
    let _ = give(&mut ui, Input::InputMethod(&preedit));
    assert!(ui.select_text(ids[0], 0, 0));
    assert_eq!(ui.field_text(ids[0]), Some("日本"));
    assert_eq!(
        around(&mut ui, 9, 9, TextUnit::Utf8).map(|answer| answer.composition),
        Some(None)
    );
}

#[test]
fn a_boxes_ends_follow_its_direction() {
    // Right to left: the word's first letter at its right end; left to right, at its left.
    for (text, first, rtl) in [("שלום", 0..2, true), ("word", 0..1, false)] {
        let (mut ui, _) = fields(&[text]);
        let word = one_box(&mut ui, 0..text.len(), TextUnit::Utf8);
        let letter = one_box(&mut ui, first.clone(), TextUnit::Utf8);
        assert!(
            word.width > letter.width && letter.width > 0.0,
            "{text}: {word:?}, {letter:?}"
        );
        let (word_end, letter_end) = if rtl {
            (word.x + word.width, letter.x + letter.width)
        } else {
            (word.x, letter.x)
        };
        assert!(
            (word_end - letter_end).abs() < 0.5,
            "{text}: {word:?}, {letter:?}"
        );
        assert!(within(letter, word), "{text}: {letter:?} in {word:?}");
    }
}

#[test]
fn a_point_is_the_interfaces_under_a_button_that_draws_nothing_and_anywhere_under_a_modal_root() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
    ui.build(root, 0, |children| {
        // A button of no label, no fill and no border: it draws nothing, yet takes a press.
        let bare = Element::button("").style(|s| {
            s.width(Dimension::Pixels(100.0))
                .height(Dimension::Pixels(40.0))
        });
        children.add("bare", bare, |_| {});
    });
    ui.frame();
    assert!(ui.covers(MAIN, 50.0, 20.0));
    assert!(!ui.covers(MAIN, 300.0, 200.0));
    let modal = RootSettings {
        size: [100.0, 100.0],
        origin: [150.0; 2],
        modal: true,
        ..settings
    };
    ui.add_root("dialog", modal, |s| s);
    ui.frame();
    assert!(
        ui.covers(MAIN, 300.0, 20.0),
        "any point while a modal root is open"
    );
}
