//! Check boxes and sliders, without a GPU (`docs/adr/0148-widgets.md`): toggled by the pointer, by
//! the keys and by an agent, a slider dragged, pressed on its track and moved by the keys, in a
//! root of either direction, logarithmic or reversed, each read by the program as its value changed
//! and committed; the program's value, which replaces the user's only when it changes; their
//! meaning and their state.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::{Action, ActionKind, FieldValue};
use slime_ui::display::Primitive;
use slime_ui::style::{Dimension, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementAction, ElementId, LayoutDirection, Role, RootSettings, Ui};

/// An interface whose font is the tests' Noto Sans.
fn ui() -> Ui {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    Ui::new(fonts)
}

/// A style of `width` by 20 logical pixels, of the tests' font.
fn sized(width: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(20.0)];
    style.layout.shrink = 0.0;
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// An interface whose root, of 400 by 300 logical pixels at scale 1 and laid out in `direction`,
/// holds `element` at its top: the interface, its root and the element.
fn interface(direction: LayoutDirection, element: Element) -> (Ui, ElementId, ElementId) {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("control", element, |_| {}));
    });
    ui.frame();
    (ui, root, id.expect("built"))
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
                FieldValue::Number(number) => format!("{what} {number}"),
                FieldValue::Text(text) => format!("{what} {text:?}"),
                FieldValue::Chosen(place) => format!("{what} option {place}"),
                FieldValue::Color(color) => format!("{what} {color:?}"),
            })
        })
        .collect()
}

/// A press and a release of the primary button at `x` and `y`.
fn click(ui: &mut Ui, x: f32, y: f32) {
    ui.pointer_moved(x, y);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
}

#[test]
fn a_check_box_toggles_by_the_pointer_the_keys_and_an_agent() {
    let (mut ui, _, check) = interface(
        LayoutDirection::LeftToRight,
        Element::check_box("Shadows", false, sized(200.0)),
    );
    // Its meaning: a check box, named by its label, which takes a press, the focus.
    let meaning = ui.meaning_of(check).expect("it means something");
    assert_eq!(meaning.role, Role::CheckBox);
    assert_eq!(meaning.name, "Shadows");
    assert_eq!(
        meaning.actions,
        [ElementAction::Press, ElementAction::Focus]
    );
    assert!(!meaning.state.checked);
    click(&mut ui, 5.0, 10.0);
    assert_eq!(values(&mut ui), ["changed true", "committed true"]);
    assert!(ui.state(check).expect("a check box").checked);
    assert_eq!(ui.focus(), Some(check));
    // The focus that a press gave leaves Space to the program: nothing is toggled.
    assert!(!ui.key(&named(NamedKey::Space)));
    assert!(values(&mut ui).is_empty());
    // Space while the keys gave it the focus, as Enter: toggled back.
    ui.set_focus(Some(check));
    ui.key(&named(NamedKey::Space));
    assert_eq!(values(&mut ui), ["changed false", "committed false"]);
    // An agent's press.
    ui.act(Action {
        element: Some(check),
        kind: ActionKind::Press,
    });
    assert_eq!(values(&mut ui), ["changed true", "committed true"]);
    // A press let go away from it toggles nothing.
    ui.pointer_moved(5.0, 10.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(300.0, 200.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(values(&mut ui).is_empty());
}

#[test]
fn a_check_box_keeps_what_the_user_toggled_until_the_program_changes_it() {
    let (mut ui, root, check) = interface(
        LayoutDirection::LeftToRight,
        Element::check_box("Shadows", false, sized(200.0)),
    );
    let build = |ui: &mut Ui, revision: u64, checked: bool| {
        ui.build(root, revision, |children| {
            children.add(
                "control",
                Element::check_box("Shadows", checked, sized(200.0)),
                |_| {},
            );
        });
        ui.frame();
    };
    click(&mut ui, 5.0, 10.0);
    ui.take_actions();
    // Built again with the program's state of before: the user's toggle stays.
    build(&mut ui, 1, false);
    assert!(ui.state(check).expect("a check box").checked);
    // The program's state changes: it replaces the user's, and tells no action.
    build(&mut ui, 2, true);
    build(&mut ui, 3, false);
    assert!(!ui.state(check).expect("a check box").checked);
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_slider_is_dragged_pressed_on_its_track_and_moved_by_the_keys() {
    let (mut ui, _, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(0.5, 0.0, 1.0, 0.1, sized(216.0)),
    );
    let meaning = ui.meaning_of(slider).expect("it means something");
    assert_eq!(meaning.role, Role::Slider);
    assert_eq!(meaning.value.as_deref(), Some("0.5"));
    assert_eq!(
        meaning.actions,
        [ElementAction::Focus, ElementAction::SetValue]
    );
    let range = meaning.range.expect("a slider's range");
    assert_eq!((range.min, range.max, range.step), (0.0, 1.0, 0.1));
    // The track runs from the thumb's middle at one end to its middle at the other: 8 to 208.
    ui.pointer_moved(8.0, 10.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    assert_eq!(ui.slider_value(slider), Some(0.0));
    ui.pointer_moved(108.0, 10.0);
    ui.pointer_moved(400.0, 10.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(
        values(&mut ui),
        ["changed 0", "changed 0.5", "changed 1", "committed 1"]
    );
    // The focus that the press gave leaves the arrows to the program.
    assert_eq!(ui.focus(), Some(slider));
    assert!(!ui.key(&named(NamedKey::ArrowLeft)));
    assert!(values(&mut ui).is_empty());
    // Keys while the keys gave it the focus: a step, ten, its bounds, each committed.
    ui.set_focus(Some(slider));
    ui.key(&named(NamedKey::ArrowLeft));
    ui.key(&named(NamedKey::Home));
    ui.key(&named(NamedKey::PageUp));
    let steps = values(&mut ui);
    assert_eq!(steps[0], "changed 0.9");
    assert_eq!(steps[2], "changed 0");
    assert_eq!(steps.last().map(String::as_str), Some("committed 1"));
    assert_eq!(ui.slider_value(slider), Some(1.0));
    // A value set, as an agent or a screen reader sets it, within its bounds and on its steps,
    // which give tenths, not what their sums add up to in binary.
    assert!(ui.set_value(slider, "0.42"));
    assert_eq!(ui.slider_value(slider), Some(0.4));
    ui.key(&named(NamedKey::ArrowRight));
    ui.key(&named(NamedKey::ArrowRight));
    assert_eq!(ui.slider_value(slider), Some(0.6));
    assert!(!ui.set_value(slider, "high"));
    assert!(ui.set_value(slider, "7"));
    assert_eq!(ui.slider_value(slider), Some(1.0));
}

#[test]
fn a_slider_from_right_to_left_starts_at_its_right() {
    let (mut ui, _, slider) = interface(
        LayoutDirection::RightToLeft,
        Element::slider(0.0, 0.0, 10.0, 1.0, sized(216.0)),
    );
    // In a root of 400 pixels laid out from right to left, the slider's box runs from 184 to 400.
    ui.pointer_moved(392.0, 10.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.slider_value(slider), Some(0.0));
    click(&mut ui, 192.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(10.0));
    // The right arrow moves it towards its least value, the end of a line being at the left.
    ui.set_focus(Some(slider));
    ui.key(&named(NamedKey::ArrowRight));
    assert_eq!(ui.slider_value(slider), Some(9.0));
}

#[test]
fn a_logarithmic_slider_multiplies_its_value_by_as_much_along_its_track() {
    // From 0.1 to 10: the track's middle is 1, its quarter 0.316.
    let (mut ui, _, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(1.0, 0.1, 10.0, 0.0, sized(216.0)).logarithmic(),
    );
    // The thumb at 1 stands in the middle of the track, from 8 to 208.
    let middle = |ui: &mut Ui| {
        let (list, _) = ui.frame();
        let thumb = list
            .items()
            .iter()
            .rev()
            .find_map(|item| match &item.primitive {
                Primitive::Box(each) => Some(each.rect),
                _ => None,
            })
            .expect("a thumb");
        thumb.x + thumb.width / 2.0
    };
    assert!((middle(&mut ui) - 108.0).abs() < 0.01);
    click(&mut ui, 58.0, 10.0);
    let value = ui.slider_value(slider).expect("a slider");
    assert!((value - 10f64.powf(-0.5)).abs() < 1e-3, "{value}");
    // A slider of no step moves by a hundredth of its track: a factor of 100 ^ 0.01.
    ui.set_focus(Some(slider));
    ui.key(&named(NamedKey::Home));
    ui.key(&named(NamedKey::ArrowRight));
    let value = ui.slider_value(slider).expect("a slider");
    assert!((value - 0.1 * 100f64.powf(0.01)).abs() < 1e-9, "{value}");
    // A least value not above 0 maps the track linearly.
    let (mut ui, _, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(0.0, 0.0, 10.0, 0.0, sized(216.0)).logarithmic(),
    );
    click(&mut ui, 108.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(5.0));
}

#[test]
fn a_reversed_slider_runs_from_its_most_value_to_its_least() {
    // An exposure, from 18 at its left to 0 at its right.
    let (mut ui, _, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(9.0, 0.0, 18.0, 1.0, sized(216.0)).reversed(),
    );
    click(&mut ui, 8.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(18.0));
    click(&mut ui, 208.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(0.0));
    // The right arrow moves it towards its least value, at the right; up towards its most.
    ui.set_focus(Some(slider));
    ui.key(&named(NamedKey::ArrowUp));
    ui.key(&named(NamedKey::ArrowUp));
    ui.key(&named(NamedKey::ArrowRight));
    assert_eq!(ui.slider_value(slider), Some(1.0));
    // Its meaning keeps its least and most values.
    let range = ui.meaning_of(slider).and_then(|meaning| meaning.range);
    let range = range.expect("a slider's range");
    assert_eq!((range.min, range.max), (0.0, 18.0));
    // From right to left, reversed, its least value lies at the left again.
    let (mut ui, _, slider) = interface(
        LayoutDirection::RightToLeft,
        Element::slider(0.0, 0.0, 10.0, 1.0, sized(216.0)).reversed(),
    );
    click(&mut ui, 192.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(0.0));
    click(&mut ui, 392.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(10.0));
}

#[test]
fn a_slider_keeps_its_value_until_the_program_changes_it() {
    let (mut ui, root, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(2.0, 0.0, 10.0, 1.0, sized(216.0)),
    );
    let build = |ui: &mut Ui, revision: u64, value: f64| {
        ui.build(root, revision, |children| {
            children.add(
                "control",
                Element::slider(value, 0.0, 10.0, 1.0, sized(216.0)),
                |_| {},
            );
        });
        ui.frame();
    };
    ui.set_focus(Some(slider));
    ui.key(&named(NamedKey::End));
    build(&mut ui, 1, 2.0);
    assert_eq!(ui.slider_value(slider), Some(10.0));
    build(&mut ui, 2, 4.4);
    assert_eq!(ui.slider_value(slider), Some(4.0), "on its steps");
}

/// An interface whose root holds a section of `open`, titled "Physics", holding a button "Apply"
/// 40 logical pixels tall: the interface, its root, the section and the button.
fn section(open: bool) -> (Ui, ElementId, ElementId, ElementId) {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let (mut folding, mut button) = (None, None);
    let mut style = sized(200.0);
    style.layout.size[1] = Dimension::Auto;
    style.layout.direction = slime_ui::style::FlexDirection::Column;
    // As tall as it holds, not stretched to its root's height.
    style.layout.align_self = Some(slime_ui::style::Align::Start);
    ui.build(root, 0, |children| {
        folding = Some(children.add(
            "physics",
            Element::section("Physics", open, style),
            |children| {
                let mut tall = sized(200.0);
                tall.layout.size[1] = Dimension::Pixels(40.0);
                button = Some(children.add("apply", Element::button("Apply", tall), |_| {}));
            },
        ));
    });
    ui.frame();
    (ui, root, folding.expect("built"), button.expect("built"))
}

#[test]
fn a_section_folds_what_it_holds_and_opens_by_its_header() {
    let (mut ui, _, folding, button) = section(false);
    let meaning = ui.meaning_of(folding).expect("it means something");
    assert_eq!(meaning.role, Role::Details);
    assert_eq!(meaning.name, "Physics");
    assert!(!meaning.state.expanded);
    // Folded: what it holds is not drawn, reached nor focused, and its box is its header alone.
    assert_eq!(ui.meaning_of(button), None);
    let folded_height = ui.rect(folding).expect("laid out").height;
    assert!(folded_height < 40.0, "{folded_height}");
    // A press on its header opens it.
    click(&mut ui, 5.0, 5.0);
    ui.frame();
    assert!(ui.state(folding).expect("a section").expanded);
    assert!(ui.meaning_of(button).is_some());
    let open_height = ui.rect(folding).expect("laid out").height;
    assert!(open_height >= folded_height + 40.0, "{open_height}");
    // A press on what it holds reaches that, not the header.
    click(&mut ui, 5.0, open_height - 5.0);
    let kinds: Vec<ActionKind> = ui
        .take_actions()
        .into_iter()
        .map(|action| action.kind)
        .collect();
    assert_eq!(kinds, [ActionKind::Press]);
    assert!(ui.state(folding).expect("a section").expanded);
    // Enter while it holds the focus folds it; a section tells the program nothing.
    ui.set_focus(Some(folding));
    ui.key(&named(NamedKey::Enter));
    ui.frame();
    assert!(!ui.state(folding).expect("a section").expanded);
    assert_eq!(ui.meaning_of(button), None);
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_section_opened_by_the_program_and_its_button_reached_by_tab() {
    let (mut ui, _, folding, button) = section(true);
    // Tab: the section's header, then what it holds.
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(folding));
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(button));
    // Folded by the program: the focus leaves what it no longer shows.
    ui.set_open(folding, false);
    ui.frame();
    assert_eq!(ui.focus(), None);
}

/// Builds, under `root`, a bar of three tabs, the program giving `chosen`, then the panel of the
/// chosen one, a text that names it; gives the bar.
fn build_tabs(ui: &mut Ui, root: ElementId, revision: u64, chosen: usize) -> ElementId {
    let mut bar = None;
    ui.build(root, revision, |children| {
        let id = children.add("tabs", Element::tab_bar(chosen, Style::default()), |tabs| {
            for (place, title) in ["General", "Physics", "Audio"].into_iter().enumerate() {
                tabs.add(place, Element::tab(title, sized(80.0)), |_| {});
            }
        });
        let shown = ["General", "Physics", "Audio"][children.chosen(id)];
        let panel = Element::container(Style::default()).role(Role::TabPanel);
        children.add("panel", panel, |panel| {
            panel.add("text", Element::label(shown, sized(200.0)), |_| {});
        });
        bar = Some(id);
    });
    ui.frame();
    bar.expect("built")
}

/// The text of the panel that the tabs show.
fn shown(ui: &Ui) -> String {
    let text = ui
        .element_by_path("main/panel/text")
        .expect("the panel's text");
    ui.meaning_of(text).expect("a text").name
}

#[test]
fn a_tab_chosen_by_the_pointer_or_the_keys_shows_its_panel() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let bar = build_tabs(&mut ui, root, 0, 0);
    let tabs = ui.children(bar).to_vec();
    assert_eq!(ui.meaning_of(bar).expect("a bar").role, Role::TabList);
    let first = ui.meaning_of(tabs[0]).expect("a tab");
    assert_eq!((first.role, first.name.as_str()), (Role::Tab, "General"));
    assert!(first.state.selected);
    assert_eq!(shown(&ui), "General");
    // A press on the second tab chooses it; the builder runs again, the revision unchanged.
    let second = ui.rect(tabs[1]).expect("laid out");
    click(&mut ui, second.x + 5.0, second.y + 5.0);
    assert_eq!(ui.chosen(bar), Some(1));
    build_tabs(&mut ui, root, 0, 0);
    assert_eq!(shown(&ui), "Physics");
    assert!(ui.state(tabs[1]).expect("a tab").selected);
    assert!(!ui.state(tabs[0]).expect("a tab").selected);
    // The arrows while a tab holds the focus that the keys gave choose its neighbours, which take
    // the focus; End the last.
    assert_eq!(ui.focus(), Some(tabs[1]));
    ui.set_focus(Some(tabs[1]));
    ui.key(&named(NamedKey::ArrowRight));
    assert_eq!((ui.chosen(bar), ui.focus()), (Some(2), Some(tabs[2])));
    ui.key(&named(NamedKey::Home));
    assert_eq!(ui.chosen(bar), Some(0));
    // The program's choice replaces the user's when it changes.
    build_tabs(&mut ui, root, 1, 0);
    assert_eq!(ui.chosen(bar), Some(0));
    build_tabs(&mut ui, root, 2, 2);
    assert_eq!(shown(&ui), "Audio");
    // An agent's press chooses a tab as a person's does; a choice tells the program nothing.
    ui.take_actions();
    ui.act(Action {
        element: Some(tabs[1]),
        kind: ActionKind::Press,
    });
    assert_eq!(ui.chosen(bar), Some(1));
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_split_shares_its_length_and_its_handle_is_dragged_and_moved_by_the_keys() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut whole = Style::default();
    whole.layout.size = [Dimension::Pixels(400.0), Dimension::Pixels(300.0)];
    ui.build(root, 0, |children| {
        children.add("split", Element::split(true, 0.25, whole), |panels| {
            panels.add("left", Element::container(Style::default()), |_| {});
            panels.add("right", Element::container(Style::default()), |_| {});
        });
    });
    ui.frame();
    let split = ui.element_by_path("main/split").expect("built");
    let left = ui.element_by_path("main/split/left").expect("built");
    let right = ui.element_by_path("main/split/right").expect("built");
    // The first panel takes a quarter, the handle the next 8 pixels, the second the rest.
    assert_eq!(ui.rect(left).map(|rect| rect.width), Some(100.0));
    assert_eq!(
        ui.rect(right).map(|rect| (rect.x, rect.width)),
        Some((108.0, 292.0))
    );
    let meaning = ui.meaning_of(split).expect("it means something");
    assert_eq!(meaning.role, Role::Splitter);
    assert_eq!(meaning.value.as_deref(), Some("0.25"));
    // The handle dragged to the middle.
    ui.pointer_moved(104.0, 100.0);
    assert_eq!(ui.cursor(), slime_input::CursorShape::ResizeHorizontal);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(204.0, 100.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    assert_eq!(ui.split_share(split), Some(0.5));
    assert_eq!(ui.rect(left).map(|rect| rect.width), Some(200.0));
    // A press beside the handle, on a panel, leaves it alone.
    click(&mut ui, 300.0, 100.0);
    assert_eq!(ui.split_share(split), Some(0.5));
    // The arrows while it holds the focus, a twentieth at a time; End to its end.
    ui.set_focus(Some(split));
    ui.key(&named(NamedKey::ArrowLeft));
    assert!((ui.split_share(split).expect("a split") - 0.45).abs() < 1e-6);
    ui.key(&named(NamedKey::End));
    assert_eq!(ui.split_share(split), Some(1.0));
}

#[test]
fn a_box_whose_content_does_not_fit_shows_a_scroll_bar_that_scrolls_it() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut boxed = Style::default();
    boxed.layout.size = [Dimension::Pixels(100.0), Dimension::Pixels(40.0)];
    boxed.layout.direction = slime_ui::style::FlexDirection::Column;
    let mut scroll = None;
    ui.build(root, 0, |children| {
        scroll = Some(children.add("scroll", Element::scroll(boxed), |children| {
            for name in ["a", "b", "c"] {
                let mut tall = sized(80.0);
                tall.layout.size[1] = Dimension::Pixels(30.0);
                children.add(name, Element::button(name, tall), |_| {});
            }
        }));
    });
    let (list, _) = ui.frame();
    let scroll = scroll.expect("built");
    // 90 pixels of content in 40: a bar along its end, 8 pixels thick, drawn over its content.
    let before = list.items().len();
    assert!(before > 3, "the buttons, then the bar's track and thumb");
    // The thumb, at the bar's top, dragged halfway down its room: the content scrolls halfway.
    ui.pointer_moved(96.0, 2.0);
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    let thumb = 40.0 * 40.0 / 90.0;
    ui.pointer_moved(96.0, 2.0 + (40.0 - thumb) / 2.0);
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Released));
    let [_, y] = ui.scroll(scroll).expect("a box that scrolls");
    assert!((y - 25.0).abs() < 0.5, "{y}");
    // A press on its track below the thumb pages down, to the end.
    ui.frame();
    ui.pointer_moved(96.0, 39.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.scroll(scroll), Some([0.0, 50.0]));
    // Nothing reached the buttons below the bar.
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_third_child_of_two_panels_is_neither_drawn_nor_reached() {
    let mut ui = ui();
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut whole = Style::default();
    whole.layout.size = [Dimension::Pixels(400.0), Dimension::Pixels(300.0)];
    ui.build(root, 0, |children| {
        children.add("split", Element::split(true, 0.5, whole), |panels| {
            panels.add("left", Element::container(Style::default()), |_| {});
            panels.add("right", Element::container(Style::default()), |_| {});
            panels.add("third", Element::button("third", sized(50.0)), |_| {});
        });
    });
    ui.frame();
    let third = ui.element_by_path("main/split/third").expect("built");
    assert_eq!(ui.meaning_of(third), None);
    ui.key(&named(NamedKey::Tab));
    assert_ne!(ui.focus(), Some(third));
}

#[test]
fn a_check_box_s_label_and_a_section_s_title_lie_beside_their_box_from_either_side() {
    // Latin, and Hebrew, which the font lacks but which is laid out from right to left all the
    // same: in a root of either direction, the glyphs lie against the box, or the mark.
    let elements = [
        Element::check_box("shadows", true, sized(300.0)),
        Element::check_box("צללים", true, sized(300.0)),
        Element::section("physics", true, sized(300.0)),
        Element::section("פיזיקה", true, sized(300.0)),
    ];
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        for element in elements.clone() {
            let (mut ui, _, _) = interface(direction, element);
            let (list, _) = ui.frame();
            let mut boxed = None;
            let mut text = None;
            for item in list.items() {
                match &item.primitive {
                    Primitive::Box(primitive) if primitive.rect.width == 16.0 => {
                        boxed.get_or_insert(primitive.rect);
                    }
                    Primitive::Text(primitive) => text = Some(primitive.clone()),
                    _ => {}
                }
            }
            let (boxed, text) = (boxed.expect("its box"), text.expect("its label"));
            // Where its glyphs start and end, the first at the line's start.
            let first = text
                .layout
                .runs()
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.x))
                .fold(f32::INFINITY, f32::min);
            let start = text.origin[0] + first;
            let end = start + text.layout.width();
            // Half the box's side between them.
            let gap = match direction {
                LayoutDirection::LeftToRight => start - (boxed.x + boxed.width),
                LayoutDirection::RightToLeft => boxed.x - end,
            };
            assert!((gap - 8.0).abs() < 0.5, "{direction:?}: {gap}");
        }
    }
}

#[test]
fn a_slider_whose_bounds_are_no_numbers_keeps_its_value_and_draws() {
    // No least bound, so no steps from it; no greatest bound.
    let element = Element::slider(0.55, f64::NAN, f64::NAN, 0.1, sized(216.0));
    let (mut ui, _, slider) = interface(LayoutDirection::LeftToRight, element);
    assert_eq!(ui.slider_value(slider), Some(0.55));
    assert!(ui.set_value(slider, "7.25"));
    assert_eq!(ui.slider_value(slider), Some(7.25));
    let (list, _) = ui.frame();
    for item in list.items() {
        if let Primitive::Box(primitive) = &item.primitive {
            assert!(primitive.rect.x.is_finite(), "{primitive:?}");
        }
    }
}

#[test]
fn a_share_reads_as_its_own_type_writes_it() {
    // 0.32 has no exact value in binary: widened, it would read 0.3199999928474426.
    let (mut ui, _, split) = interface(
        LayoutDirection::LeftToRight,
        Element::split(true, 0.32, sized(300.0)),
    );
    let meaning = ui.meaning_of(split).expect("it means something");
    assert_eq!(meaning.value.as_deref(), Some("0.32"));
    assert!(ui.set_value(split, "0.7"));
    let meaning = ui.meaning_of(split).expect("it means something");
    assert_eq!(meaning.value.as_deref(), Some("0.7"));
}

#[test]
fn a_logarithmic_slider_reaches_its_bounds_exactly() {
    let (mut ui, _, slider) = interface(
        LayoutDirection::LeftToRight,
        Element::slider(1.0, 0.7, 3.0, 0.0, sized(216.0)).logarithmic(),
    );
    click(&mut ui, 208.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(3.0));
    click(&mut ui, 8.0, 10.0);
    assert_eq!(ui.slider_value(slider), Some(0.7));
}
