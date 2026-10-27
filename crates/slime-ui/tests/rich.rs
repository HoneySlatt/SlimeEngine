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
use slime_ui::tree::{Element, ElementId, LayoutDirection, Role, RootSettings, Ui};

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
