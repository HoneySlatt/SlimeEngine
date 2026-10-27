//! Rich widgets, without a GPU (`docs/adr/0151-rich-widgets.md`): a button that toggles, by the
//! pointer, the keys and an agent, its state the program's only when that changes; a control of
//! segments, chosen by a press and by the arrows, which the program reads; a tab's count, part of its
//! name; a bar of progress, its share its value, and one without a share that moves with the clock.

use std::time::Duration;

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::{Action, ActionKind, FieldValue};
use slime_ui::display::Primitive;
use slime_ui::style::{Dimension, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, LayoutDirection, Role, RootSettings, Ui, Units};

/// An interface whose font is the tests' Noto Sans.
fn ui() -> Ui {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    Ui::new(fonts)
}

/// A style of the tests' font, `width` logical pixels wide where it is given.
fn style(width: Option<f32>) -> Style {
    let mut style = Style::default();
    if let Some(width) = width {
        style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(24.0)];
        style.layout.shrink = 0.0;
    }
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// An interface of 400 by 300 whose root holds what `build` adds, built again with its revision:
/// the interface and its root.
fn interface() -> (Ui, ElementId) {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    (ui, root)
}

/// The values that the queued actions changed and committed, as text.
fn values(ui: &mut Ui) -> Vec<String> {
    ui.take_actions()
        .into_iter()
        .filter_map(|action| {
            let (what, value) = match action.kind {
                ActionKind::Changed(value) => ("changed", value),
                ActionKind::Committed(value) => ("committed", value),
                _ => return None,
            };
            Some(match value {
                FieldValue::Checked(checked) => format!("{what} {checked}"),
                FieldValue::Chosen(place) => format!("{what} {place}"),
                other => format!("{what} {other:?}"),
            })
        })
        .collect()
}

fn key(ui: &mut Ui, named: NamedKey) {
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(named),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.frame();
}

fn click(ui: &mut Ui, id: ElementId) {
    let rect = ui.rect(id).expect("laid out");
    ui.pointer_moved(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
}

#[test]
fn a_button_that_toggles_stays_pressed_and_tells_the_program() {
    let (mut ui, root) = interface();
    let build = |ui: &mut Ui, revision: u64, on: bool| {
        let mut id = None;
        ui.build(root, revision, |children| {
            let toggle = Element::toggle_button("Snap", on, style(Some(80.0)));
            id = Some(children.add("snap", toggle, |_| {}));
        });
        ui.frame();
        id.expect("built")
    };
    let snap = build(&mut ui, 0, false);
    let meaning = ui.meaning_of(snap).expect("drawn");
    assert_eq!(
        (meaning.role, meaning.name.as_str()),
        (Role::ToggleButton, "Snap")
    );
    assert!(!meaning.state.checked);
    click(&mut ui, snap);
    assert_eq!(values(&mut ui), ["changed true", "committed true"]);
    assert!(ui.state(snap).expect("there").checked);
    // Space reaches it once the keys gave it the focus, not the press.
    ui.set_focus(Some(snap));
    key(&mut ui, NamedKey::Space);
    assert_eq!(values(&mut ui), ["changed false", "committed false"]);
    ui.act(Action {
        element: Some(snap),
        kind: ActionKind::Press,
    });
    assert_eq!(values(&mut ui), ["changed true", "committed true"]);
    // The program gives `false` again, as before: the user's state stays; `true`, then `false`,
    // replace it.
    build(&mut ui, 1, false);
    assert!(ui.state(snap).expect("there").checked);
    build(&mut ui, 2, true);
    build(&mut ui, 3, false);
    assert!(!ui.state(snap).expect("there").checked);
}

#[test]
fn a_control_of_segments_is_chosen_by_a_press_and_the_arrows() {
    let (mut ui, root) = interface();
    let mut ids = (None, Vec::new());
    ui.build(root, 0, |children| {
        let control = children.add("mode", Element::segments(0, style(None)), |segments| {
            for title in ["Move", "Turn", "Scale"] {
                ids.1
                    .push(segments.add(title, Element::segment(title, style(None)), |_| {}));
            }
        });
        ids.0 = Some(control);
    });
    ui.frame();
    let (control, segments) = (ids.0.expect("built"), ids.1);
    assert_eq!(
        ui.meaning_of(control).expect("drawn").role,
        Role::RadioGroup
    );
    let first = ui.meaning_of(segments[0]).expect("drawn");
    assert_eq!(
        (first.role, first.state.selected),
        (Role::RadioButton, true)
    );
    click(&mut ui, segments[2]);
    assert_eq!(values(&mut ui), ["changed 2", "committed 2"]);
    assert_eq!(ui.chosen(control), Some(2));
    assert!(ui.state(segments[2]).expect("there").selected);
    // The arrows, from the segment that holds the focus.
    ui.set_focus(Some(segments[2]));
    key(&mut ui, NamedKey::ArrowLeft);
    assert_eq!(values(&mut ui), ["changed 1", "committed 1"]);
    assert_eq!(ui.focus(), Some(segments[1]));
    key(&mut ui, NamedKey::Home);
    assert_eq!(values(&mut ui), ["changed 0", "committed 0"]);
    // A press on the chosen one tells nothing.
    click(&mut ui, segments[0]);
    assert!(values(&mut ui).is_empty());
}

#[test]
fn a_tabs_count_follows_its_title_and_is_part_of_its_name() {
    let (mut ui, root) = interface();
    let mut tabs = Vec::new();
    ui.build(root, 0, |children| {
        children.add("tabs", Element::tab_bar(0, style(None)), |bar| {
            tabs.push(bar.add("log", Element::tab("Log", style(None)), |_| {}));
            let counted = Element::counted_tab("Log", 12, style(None));
            tabs.push(bar.add("counted", counted, |_| {}));
        });
    });
    ui.frame();
    let (plain, counted) = (
        ui.rect(tabs[0]).expect("laid out"),
        ui.rect(tabs[1]).expect("laid out"),
    );
    assert!(counted.width > plain.width + 10.0, "{plain:?} {counted:?}");
    assert_eq!(ui.meaning_of(tabs[1]).expect("drawn").name, "Log (12)");
    assert_eq!(ui.meaning_of(tabs[0]).expect("drawn").name, "Log");
}

#[test]
fn a_bar_of_progress_gives_its_share_and_one_without_moves_with_the_clock() {
    let (mut ui, root) = interface();
    let mut bars = Vec::new();
    ui.build(root, 0, |children| {
        bars.push(children.add(
            "known",
            Element::progress(Some(0.4), style(Some(200.0))),
            |_| {},
        ));
        bars.push(children.add(
            "unknown",
            Element::progress(None, style(Some(200.0))),
            |_| {},
        ));
    });
    let (_, stats) = ui.frame();
    assert!(stats.animating, "the bar without a share moves");
    let meaning = ui.meaning_of(bars[0]).expect("drawn");
    assert_eq!(
        (meaning.role, meaning.value.as_deref()),
        (Role::ProgressIndicator, Some("0.4"))
    );
    assert!(meaning.range.is_some());
    assert_eq!(ui.meaning_of(bars[1]).expect("drawn").value, None);
    // The moving part's place, in the bar without a share, at two times of the clock.
    let unknown = ui.rect(bars[1]).expect("laid out");
    let moving = |ui: &mut Ui| {
        let (list, _) = ui.frame();
        list.items()
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::Box(boxed)
                    if boxed.rect.x >= unknown.x
                        && boxed.rect.y >= unknown.y
                        && boxed.rect.width < unknown.width =>
                {
                    Some(boxed.rect.x)
                }
                _ => None,
            })
            .next()
    };
    ui.advance(Duration::from_millis(500));
    let before = moving(&mut ui).expect("a part that moves");
    ui.advance(Duration::from_millis(300));
    let after = moving(&mut ui).expect("a part that moves");
    assert!(after > before, "{before} then {after}");
}

#[test]
fn a_field_of_search_is_emptied_by_its_mark_once_it_holds_text() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let search = Element::search_field("", style(Some(200.0))).placeholder("Search");
        id = Some(children.add("search", search, |_| {}));
    });
    ui.frame();
    let search = id.expect("built");
    assert_eq!(
        ui.meaning_of(search).expect("drawn").role,
        Role::SearchInput
    );
    let rect = ui.rect(search).expect("laid out");
    // The mark lies at its end, a control's size before its edge's half.
    let mark = (rect.x + rect.width - 8.0, rect.y + rect.height / 2.0);
    let press = |ui: &mut Ui| {
        ui.pointer_moved(mark.0, mark.1);
        ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
        ui.pointer_button(MouseButton::Left, ButtonState::Released);
        ui.frame();
    };
    // Empty, it has no mark: a press there gives it the focus, and empties nothing.
    press(&mut ui);
    assert_eq!(ui.focus(), Some(search));
    assert!(values(&mut ui).is_empty());
    for character in ["t", "o"] {
        ui.key(&KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Character(character.to_owned()),
            text: Some(character.to_owned()),
            state: ButtonState::Pressed,
            repeat: false,
        });
    }
    ui.frame();
    values(&mut ui);
    press(&mut ui);
    assert_eq!(
        values(&mut ui),
        [
            format!("changed {:?}", FieldValue::Text(String::new())),
            format!("committed {:?}", FieldValue::Text(String::new()))
        ]
    );
    assert_eq!(ui.field_text(search), Some(""));
    assert_eq!(ui.focus(), Some(search));
}

#[test]
fn a_fields_prefix_lies_before_its_number_and_drags_it_even_focused() {
    let (mut ui, root) = interface();
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        let plain = Element::number_field(1.0, 0.1, style(Some(120.0)));
        ids.push(children.add("plain", plain, |_| {}));
        let x = Element::number_field(1.0, 0.1, style(Some(120.0))).prefix("X");
        ids.push(children.add("x", x, |_| {}));
    });
    ui.frame();
    let x = ids[1];
    let rect = ui.rect(x).expect("laid out");
    ui.set_focus(Some(x));
    // A drag from its prefix, ten logical pixels to the right: ten steps.
    let at = (rect.x + 3.0, rect.y + rect.height / 2.0);
    ui.pointer_moved(at.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(at.0 + 10.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    let values = values(&mut ui);
    assert_eq!(
        values.last().map(String::as_str),
        Some("committed Number(2.0)")
    );
    assert_eq!(ui.field_text(x), Some("2"));
}

#[test]
fn a_bar_without_a_share_stops_moving_once_hidden_or_given_one() {
    let (mut ui, root) = interface();
    let build = |ui: &mut Ui, revision: u64, open: bool, share: Option<f32>| {
        ui.build(root, revision, |children| {
            let section = Element::section("Cook", open, style(None));
            children.add("cook", section, |section| {
                let bar = Element::progress(share, style(Some(200.0)));
                section.add("bar", bar, |_| {});
            });
        });
        ui.frame();
    };
    build(&mut ui, 0, true, None);
    let (_, stats) = ui.frame();
    assert!(stats.animating);
    // Folded, what the section holds does not show: nothing moves.
    let section = ui.element_by_path("#0/cook").expect("built");
    ui.set_open(section, false);
    ui.frame();
    let (_, stats) = ui.frame();
    assert!(!stats.animating, "hidden");
    ui.set_open(section, true);
    ui.frame();
    let (_, stats) = ui.frame();
    assert!(stats.animating, "shown again");
    build(&mut ui, 1, true, Some(0.5));
    let (_, stats) = ui.frame();
    assert!(!stats.animating, "given a share");
    // Less motion keeps it still.
    build(&mut ui, 2, true, None);
    ui.set_reduced_motion(true);
    ui.frame();
    let (_, stats) = ui.frame();
    assert!(!stats.animating, "less motion");
}

#[test]
fn from_right_to_left_a_fields_prefix_and_marks_stand_at_its_right_and_left() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::RightToLeft,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        let x = Element::number_field(1.0, 0.1, style(Some(120.0))).prefix("X");
        ids.push(children.add("x", x, |_| {}));
        let search = Element::search_field("torch", style(Some(200.0)));
        ids.push(children.add("search", search, |_| {}));
    });
    ui.frame();
    let (x, search) = (ids[0], ids[1]);
    // The prefix at the field's right, which a drag towards the left, its line's end, raises.
    let rect = ui.rect(x).expect("laid out");
    let at = (rect.x + rect.width - 3.0, rect.y + rect.height / 2.0);
    ui.pointer_moved(at.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(at.0 + 10.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert_eq!(ui.field_text(x), Some("2"));
    values(&mut ui);
    // The mark that empties the field of search at its left.
    let rect = ui.rect(search).expect("laid out");
    ui.pointer_moved(rect.x + 8.0, rect.y + rect.height / 2.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert_eq!(ui.field_text(search), Some(""));
}

#[test]
fn a_prefix_given_later_makes_room_and_a_composition_keeps_it_from_dragging() {
    let (mut ui, root) = interface();
    let build = |ui: &mut Ui, revision: u64, prefix: &str| {
        let mut id = None;
        ui.build(root, revision, |children| {
            let field = Element::number_field(1.0, 0.1, style(Some(120.0))).prefix(prefix);
            id = Some(children.add("x", field, |_| {}));
        });
        ui.frame();
        id.expect("built")
    };
    // Where the field draws its number, `1`.
    let number_x = |ui: &mut Ui| {
        let (list, _) = ui.frame();
        list.items()
            .iter()
            .find_map(|item| match &item.primitive {
                Primitive::Text(text) if text.layout.shown() == "1" => Some(text.origin[0]),
                _ => None,
            })
            .expect("its number drawn")
    };
    let field = build(&mut ui, 0, "");
    let without = number_x(&mut ui);
    // The field kept, given a prefix: its number moves past the prefix's room.
    assert_eq!(build(&mut ui, 1, "Width"), field);
    let with = number_x(&mut ui);
    assert!(with > without + 20.0, "{without} then {with}");
    let rect = ui.rect(field).expect("laid out");
    ui.set_focus(Some(field));
    ui.ime(&slime_input::Ime::Enabled);
    ui.ime(&slime_input::Ime::Preedit {
        text: "２".to_owned(),
        cursor: None,
    });
    // A drag from the prefix while it composes moves no number, and cuts nothing.
    let at = (rect.x + 3.0, rect.y + rect.height / 2.0);
    ui.pointer_moved(at.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(at.0 + 30.0, at.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert!(
        !values(&mut ui)
            .iter()
            .any(|value| value.contains("Number(4")),
        "no drag while composing"
    );
}

#[test]
fn a_field_of_numbers_reads_an_expression_and_shows_its_result_once_committed() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(1.0, 0.1, style(Some(160.0)));
        id = Some(children.add("number", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    assert!(ui.set_value(field, "sqrt(2)/2"));
    let committed = values(&mut ui).pop().expect("a commit");
    assert_eq!(
        committed,
        format!("committed {:?}", FieldValue::Number(2f64.sqrt() / 2.0))
    );
    assert_eq!(ui.field_text(field), Some("0.7071067811865476"));
    // A number keeps the user's spelling; what reads as nothing is refused.
    assert!(ui.set_value(field, "1.50"));
    assert_eq!(ui.field_text(field), Some("1.50"));
    assert!(!ui.set_value(field, "1 +"));
    assert!(
        !ui.set_value(field, "10cm"),
        "a field of no units takes none"
    );
}

#[test]
fn a_field_of_lengths_shows_meters_and_converts_what_is_typed() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(1.5, 0.1, style(Some(160.0))).units(Units::Length);
        id = Some(children.add("length", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    assert_eq!(
        ui.meaning_of(field).expect("drawn").value.as_deref(),
        Some("1.5 m")
    );
    assert!(ui.set_value(field, "1m + 20cm"));
    let committed = values(&mut ui).pop().expect("a commit");
    assert!(committed.starts_with("committed Number(1.2"), "{committed}");
    // Its value read back, its unit with it.
    assert!(ui.set_value(field, "2.5 m"));
    assert_eq!(
        values(&mut ui).pop().as_deref(),
        Some("committed Number(2.5)")
    );
    assert!(!ui.set_value(field, "90°"), "an angle is no length");
    // The symbol of its unit is drawn after its number.
    let (list, _) = ui.frame();
    assert!(list.items().iter().any(|item| matches!(
        &item.primitive,
        Primitive::Text(text) if text.layout.shown() == "m"
    )));
}

#[test]
fn a_drag_goes_finer_with_shift_and_coarser_with_control() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(1.0, 0.1, style(Some(160.0)));
        id = Some(children.add("number", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    let rect = ui.rect(field).expect("laid out");
    let drag = |ui: &mut Ui, modifiers: slime_input::Modifiers| {
        let at = (rect.x + 20.0, rect.y + rect.height / 2.0);
        ui.modifiers(modifiers);
        ui.pointer_moved(at.0, at.1);
        ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
        ui.pointer_moved(at.0 + 10.0, at.1);
        ui.pointer_button(MouseButton::Left, ButtonState::Released);
        ui.modifiers(slime_input::Modifiers::default());
        ui.frame();
        ui.set_focus(None);
        values(ui).pop().expect("a commit")
    };
    let mut shift = slime_input::Modifiers::default();
    shift.shift = true;
    assert_eq!(drag(&mut ui, shift), "committed Number(1.1)");
    let mut control = slime_input::Modifiers::default();
    control.control = true;
    assert_eq!(drag(&mut ui, control), "committed Number(11.1)");
}

#[test]
fn a_sections_header_holds_its_box_at_its_end_whose_presses_do_not_fold_it() {
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let mut ui = ui();
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, Style::default());
        let mut ids = (None, None);
        ui.build(root, 0, |children| {
            let mut section_style = style(Some(300.0));
            section_style.layout.size[1] = Dimension::Auto;
            section_style.layout.padding.start = 4.0;
            section_style.layout.padding.end = 4.0;
            let section = Element::section("Light", true, section_style);
            ids.0 = Some(children.add("light", section, |section| {
                section.add("slot", Element::section_header(Style::default()), |slot| {
                    ids.1 = Some(slot.add("reset", Element::button("", style(Some(40.0))), |_| {}));
                });
                section.add("body", Element::label("intensity", style(None)), |_| {});
            }));
        });
        ui.frame();
        let (section, reset) = (ids.0.expect("built"), ids.1.expect("built"));
        let (box_, button) = (
            ui.rect(section).expect("laid out"),
            ui.rect(reset).expect("laid out"),
        );
        // In the header's line, at its end.
        assert!(
            button.y < box_.y + 24.0,
            "{direction:?} {button:?} {box_:?}"
        );
        match direction {
            LayoutDirection::LeftToRight => {
                assert_eq!(button.x + button.width, box_.x + box_.width - 4.0);
            }
            LayoutDirection::RightToLeft => assert_eq!(button.x, box_.x + 4.0),
        }
        // A press on the header box's button reaches the program, and leaves the section open.
        click(&mut ui, reset);
        assert!(ui.state(section).expect("there").expanded);
        assert_eq!(
            ui.take_actions().first().map(|action| &action.kind),
            Some(&ActionKind::Press)
        );
        // Folded, the section keeps its header's box.
        ui.set_open(section, false);
        ui.frame();
        assert!(ui.meaning_of(reset).is_some(), "{direction:?}");
    }
}

#[test]
fn a_tooltip_of_a_title_and_a_text_is_read_as_both() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(0.5, 0.05, style(Some(120.0)))
            .help("Roughness", "How rough the surface is, from 0 to 1");
        id = Some(children.add("roughness", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    assert_eq!(
        ui.meaning_of(field).expect("drawn").description.as_deref(),
        Some("Roughness: How rough the surface is, from 0 to 1")
    );
    let rect = ui.rect(field).expect("laid out");
    ui.pointer_moved(rect.x + 5.0, rect.y + 5.0);
    ui.advance(Duration::from_millis(600));
    ui.frame();
    let title = ui
        .element_by_path("#1/tooltip/title")
        .expect("the tooltip's title");
    assert!(ui.element_by_path("#1/tooltip/text").is_some());
    assert!(
        ui.element(title)
            .is_some_and(|element| element.classes.contains(&"tooltip-title".to_owned()))
    );
}

/// The x of the start and the end of the drawn text `text`, in the display list of a frame.
fn drawn_text(ui: &mut Ui, text: &str) -> Option<(f32, f32)> {
    let (list, _) = ui.frame();
    list.items().iter().find_map(|item| match &item.primitive {
        Primitive::Text(drawn) if drawn.layout.shown() == text => {
            Some((drawn.origin[0], drawn.origin[0] + drawn.layout.width()))
        }
        _ => None,
    })
}

#[test]
fn a_sections_title_keeps_beside_its_mark_and_leaves_the_room_of_its_headers_box() {
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let mut ui = ui();
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, Style::default());
        let build = |ui: &mut Ui, revision: u64, button: Option<f32>| {
            let mut id = None;
            ui.build(root, revision, |children| {
                let mut section_style = style(Some(200.0));
                section_style.layout.size[1] = Dimension::Auto;
                let section = Element::section("Light", true, section_style);
                id = Some(children.add("light", section, |section| {
                    if let Some(width) = button {
                        section.add("box", Element::section_header(Style::default()), |held| {
                            held.add("reset", Element::button("", style(Some(width))), |_| {});
                        });
                    }
                }));
            });
            ui.frame();
            id.expect("built")
        };
        let section = build(&mut ui, 0, None);
        let alone = drawn_text(&mut ui, "Light").expect("its title");
        // A box shown at a later revision: the title stays where it was, beside the mark.
        build(&mut ui, 1, Some(60.0));
        let beside_box = drawn_text(&mut ui, "Light").expect("its title");
        assert_eq!(alone, beside_box, "{direction:?}");
        let held = ui.element_by_path("#0/light/box/reset").expect("built");
        let button = ui.rect(held).expect("laid out");
        let rect = ui.rect(section).expect("laid out");
        match direction {
            LayoutDirection::LeftToRight => {
                assert!(beside_box.0 < rect.x + 30.0 && beside_box.1 < button.x);
            }
            LayoutDirection::RightToLeft => {
                assert!(
                    beside_box.1 > rect.x + rect.width - 30.0 && beside_box.0 > button.x + 60.0
                );
            }
        }
    }
}

#[test]
fn a_modifier_held_in_the_middle_of_a_drag_goes_on_from_where_it_was() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(0.0, 0.1, style(Some(160.0)));
        id = Some(children.add("number", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    let rect = ui.rect(field).expect("laid out");
    let y = rect.y + rect.height / 2.0;
    ui.pointer_moved(rect.x + 20.0, y);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    // Twenty pixels: two whole units.
    ui.pointer_moved(rect.x + 40.0, y);
    assert_eq!(ui.field_text(field), Some("2"));
    // Shift held: the next pixel goes by the step it began with, then ten pixels add ten tenths of
    // a step, from where the drag was.
    let mut shift = slime_input::Modifiers::default();
    shift.shift = true;
    ui.modifiers(shift);
    ui.pointer_moved(rect.x + 41.0, y);
    assert_eq!(ui.field_text(field), Some("2.1"));
    ui.pointer_moved(rect.x + 51.0, y);
    assert_eq!(ui.field_text(field), Some("2.2"));
    // Let go, the whole steps go on from there, not from where the drag began.
    ui.modifiers(slime_input::Modifiers::default());
    ui.pointer_moved(rect.x + 52.0, y);
    ui.pointer_moved(rect.x + 62.0, y);
    assert_eq!(ui.field_text(field), Some("3.21"));
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
}

#[test]
fn what_is_typed_in_a_field_of_lengths_reads_as_it_is_until_committed() {
    let (mut ui, root) = interface();
    let mut id = None;
    ui.build(root, 0, |children| {
        let field = Element::number_field(1.0, 0.1, style(Some(160.0))).units(Units::Length);
        id = Some(children.add("length", field, |_| {}));
    });
    ui.frame();
    let field = id.expect("built");
    ui.set_focus(Some(field));
    // Its text selected whole, then typed over.
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(NamedKey::End),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(NamedKey::Backspace),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    });
    for character in "200cm".chars() {
        ui.key(&KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Character(character.to_string()),
            text: Some(character.to_string()),
            state: ButtonState::Pressed,
            repeat: false,
        });
    }
    ui.frame();
    assert_eq!(
        ui.meaning_of(field).expect("drawn").value.as_deref(),
        Some("200cm")
    );
    key(&mut ui, NamedKey::Enter);
    assert_eq!(ui.field_text(field), Some("2"));
    assert_eq!(
        ui.meaning_of(field).expect("drawn").value.as_deref(),
        Some("2 m")
    );
}
