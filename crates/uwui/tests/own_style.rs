//! An element's own style written in code (`docs/adr/0231-an-elements-own-style-written-in-code.md`):
//! its states, the mask order against the rules' states, a field set to its default, an array set
//! by element, the styles made shared by the own states that hold, and the place of the call that
//! wrote it.

use slime_color::LinearColor;
use uwui::host::TargetId;
use uwui::style::Dimension;
use uwui::text::Fonts;
use uwui::theme::StyleSheet;
use uwui::tree::{
    Element, ElementId, ElementState, Layer, LayoutDirection, Place, RootSettings, Ui,
};

/// The rules' grey, sRGB's `#808080`.
fn grey() -> LinearColor {
    LinearColor::from_srgb8(128, 128, 128)
}

const RED: LinearColor = LinearColor::rgba(1.0, 0.0, 0.0, 1.0);
const GREEN: LinearColor = LinearColor::rgba(0.0, 1.0, 0.0, 1.0);
const BLUE: LinearColor = LinearColor::rgba(0.0, 0.0, 1.0, 1.0);

/// A sheet whose buttons are grey, red when hovered, and 30 logical pixels tall; and whose buttons
/// of the class `flat`, written after, are transparent.
const SHEET: &str = r##"
button {
    select kind=Button
    paint background="#808080"
    layout { size 80 30; }
}

button-hovered {
    select kind=Button { states Hovered; }
    paint background="#ff0000"
}

flat {
    select kind=Button { classes flat; }
    paint background="#00000000"
}
"##;

/// A root of 400 × 300 logical pixels at scale 1, from left to right.
fn settings() -> RootSettings {
    RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    }
}

/// An interface of one root of 400 × 300 logical pixels under [`SHEET`], holding `element`.
fn holding(element: Element) -> (Ui, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(&[StyleSheet::from_text("buttons.style", SHEET)], &[])
        .expect("a valid sheet");
    let root = ui.add_root("main", settings(), |s| s);
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("subject", element, |_| {}));
    });
    ui.frame(TargetId::MAIN);
    (ui, id.expect("built"))
}

fn background(ui: &Ui, id: ElementId) -> LinearColor {
    ui.style(id).expect("styled").paint.background
}

/// Sets whether `id` is hovered, and makes a frame.
fn hover(ui: &mut Ui, id: ElementId, hovered: bool) {
    let state = ElementState {
        hovered,
        ..ui.state(id).expect("an element")
    };
    ui.set_state(id, state);
    ui.frame(TargetId::MAIN);
}

#[test]
fn a_rules_state_beats_a_base_set_in_code() {
    let (mut ui, button) = holding(Element::button("Save").style(|s| s.background(GREEN)));
    assert_eq!(background(&ui, button), GREEN, "the own base over the rule");
    hover(&mut ui, button, true);
    assert_eq!(
        background(&ui, button),
        RED,
        "the rule's hover over the own base"
    );
}

#[test]
fn a_base_set_in_code_beats_a_stateless_rule_written_after_a_rules_state() {
    let button = Element::button("Save")
        .class("flat")
        .style(|s| s.background(GREEN));
    let (mut ui, button) = holding(button);
    hover(&mut ui, button, true);
    // `[flat]` comes after `[button-hovered]`: the hover does not hold the field, the own base does.
    assert_eq!(background(&ui, button), GREEN);
}

#[test]
fn a_state_set_in_code_beats_a_rules_state() {
    let button = Element::button("Save").style(|s| s.hover(|s| s.background(BLUE)));
    let (mut ui, button) = holding(button);
    assert_eq!(background(&ui, button), grey(), "not hovered: the rule's");
    hover(&mut ui, button, true);
    assert_eq!(background(&ui, button), BLUE, "hovered: the own state's");
}

#[test]
fn a_field_set_in_code_to_its_default_beats_a_rule() {
    let transparent = LinearColor::rgba(0.0, 0.0, 0.0, 0.0);
    let (ui, button) = holding(Element::button("Save").style(|s| s.background(transparent)));
    assert_eq!(background(&ui, button), transparent);
}

#[test]
fn a_width_set_in_code_keeps_the_rules_height() {
    let button = Element::button("Save").style(|s| s.width(Dimension::Pixels(120.0)));
    let (ui, button) = holding(button);
    assert_eq!(
        ui.style(button).expect("styled").layout.size,
        [Dimension::Pixels(120.0), Dimension::Pixels(30.0)]
    );
}

#[test]
fn an_own_hover_shows_where_no_rule_names_the_hover_and_goes() {
    // A box of no rule's that two buttons share: the one hovered, the other not, then each the
    // other way, so that a style made in one state is never served to the other.
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root("main", settings(), |s| s);
    let own = |s: uwui::style::StyleBuilder| s.background(grey()).hover(|s| s.background(BLUE));
    let (mut first, mut second) = (None, None);
    ui.build(root, 0, |children| {
        first = Some(children.add("first", Element::button("A").style(own), |_| {}));
        second = Some(children.add("second", Element::button("B").style(own), |_| {}));
    });
    let (first, second) = (first.expect("built"), second.expect("built"));
    ui.frame(TargetId::MAIN);
    hover(&mut ui, first, true);
    assert_eq!(background(&ui, first), BLUE);
    assert_eq!(background(&ui, second), grey());
    hover(&mut ui, first, false);
    hover(&mut ui, second, true);
    assert_eq!(background(&ui, first), grey());
    assert_eq!(background(&ui, second), BLUE);
}

#[test]
fn each_call_keeps_its_place() {
    let element = Element::button("Save").style(|s| s.background(GREEN));
    let first = line!() - 1;
    let places = element.style.places();
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].file(), file!());
    assert_eq!(places[0].line(), first, "the line of the call of `style`");
    // A second call keeps its own place after the first's.
    let element = element.style(|s| s.background(BLUE));
    let second = line!() - 1;
    let places = element.style.places();
    assert_eq!(places.len(), 2);
    assert_eq!((places[0].line(), places[1].line()), (first, second));
}

#[test]
fn a_controls_size_set_in_a_state_lays_the_element_out_again() {
    let check = Element::check_box("Snap", false).style(|s| s.checked(|s| s.control_size(24.0)));
    let (mut ui, check) = holding(check);
    let before = ui.rect(check).expect("laid out");
    let state = ElementState {
        checked: true,
        ..ui.state(check).expect("an element")
    };
    ui.set_state(check, state);
    ui.frame(TargetId::MAIN);
    let after = ui.rect(check).expect("laid out");
    assert!(
        after.width > before.width,
        "the box of 24 pixels widens the check box: {before:?} then {after:?}"
    );
}

#[test]
fn a_declaration_in_a_state_makes_another_style_than_the_same_in_the_base() {
    // No rule: the two buttons share their rules and their base, and neither is hovered. A style
    // made for the one must not be served to the other.
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root("main", settings(), |s| s);
    let (mut base, mut hovered) = (None, None);
    ui.build(root, 0, |children| {
        let in_base = Element::button("A").style(|s| s.background(BLUE));
        base = Some(children.add("base", in_base, |_| {}));
        let in_hover = Element::button("B").style(|s| s.hover(|s| s.background(BLUE)));
        hovered = Some(children.add("hover", in_hover, |_| {}));
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(background(&ui, base.expect("built")), BLUE);
    assert_eq!(
        background(&ui, hovered.expect("built")),
        LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
        "not hovered: no background"
    );
}

#[test]
fn of_two_own_states_that_hold_the_press_wins_over_the_hover() {
    let button = Element::button("Save").style(|s| {
        s.press(|s| s.background(GREEN))
            .hover(|s| s.background(BLUE))
    });
    let (mut ui, button) = holding(button);
    let state = ElementState {
        hovered: true,
        pressed: true,
        ..ui.state(button).expect("an element")
    };
    ui.set_state(button, state);
    ui.frame(TargetId::MAIN);
    assert_eq!(
        background(&ui, button),
        GREEN,
        "written before, applied after"
    );
}

/// Writes a background, at the place of this function's call of `style`.
fn painted(element: Element) -> Element {
    element.style(|s| s.background(GREEN))
}

#[test]
fn each_place_is_given_once_however_many_calls_wrote_there() {
    let element = painted(Element::button("Save"));
    let element = element.style(|s| s.background(BLUE));
    let between = line!() - 1;
    let places = painted(element).style.places();
    assert_eq!(places.len(), 2, "the helper's place, then this one");
    assert_eq!(places[1].line(), between);
}

#[test]
fn a_roots_and_a_layers_own_style_keep_the_place_of_their_call() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root("main", settings(), |s| s.background(grey()));
    let root_line = line!() - 1;
    let layer = Layer {
        place: Place::Floating {
            over: root,
            origin: [10.0, 10.0],
            size: [100.0, 80.0],
        },
        modal: false,
        dismiss: false,
    };
    let layer = ui.open_layer("layer", layer, |s| s.background(BLUE));
    let layer_line = line!() - 1;
    for (id, line) in [(root, root_line), (layer, layer_line)] {
        let places = ui.element(id).expect("an element").style.places();
        assert_eq!(places.len(), 1);
        assert_eq!((places[0].file(), places[0].line()), (file!(), line));
    }
}

#[test]
fn an_element_whose_style_moved_in_its_code_keeps_the_new_place() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root("main", settings(), |s| s);
    let mut id = None;
    ui.build(root, 0, |children| {
        let button = Element::button("Save").style(|s| s.background(GREEN));
        id = Some(children.add("subject", button, |_| {}));
    });
    ui.frame(TargetId::MAIN);
    // The same style, written on another line, as a reload that moved it would give it.
    let mut moved = 0;
    ui.build(root, 1, |children| {
        let button = Element::button("Save").style(|s| s.background(GREEN));
        moved = line!() - 1;
        children.add("subject", button, |_| {});
    });
    ui.frame(TargetId::MAIN);
    let places = ui.element(id.expect("built")).expect("kept").style.places();
    assert_eq!(
        places.iter().map(|place| place.line()).collect::<Vec<_>>(),
        [moved]
    );
}
