//! Color wells, without a GPU (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the
//! picker that a well opens below it, its square, strips and fields; the color dragged, moved by
//! the keys, typed as channels or a hexadecimal code, and given by an agent, which the program
//! reads as the well's; the program's color, which replaces the user's only when it changes; their
//! meaning.

use slime_color::LinearColor;
use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::{ActionKind, FieldValue};
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, LayoutDirection, Meaning, Role, RootSettings, Ui};

/// A color well of `color`, 60 logical pixels wide, at the top of a root of 600 by 500.
struct Well {
    ui: Ui,
    root: ElementId,
    well: ElementId,
    builds: u64,
}

impl Well {
    fn new(color: LinearColor) -> Self {
        let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
        let mut fonts = Fonts::new();
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [600.0, 500.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        column.text.style.fonts = vec!["Noto Sans".to_owned()];
        let root = ui.add_root(settings, column);
        let mut well = Self {
            ui,
            root,
            well: root,
            builds: 0,
        };
        well.build(color);
        well
    }

    /// Builds the well again, the program giving `color`.
    fn build(&mut self, color: LinearColor) {
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(60.0), Dimension::Pixels(20.0)];
        style.layout.align_self = Some(slime_ui::style::Align::Start);
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        let mut id = None;
        self.builds += 1;
        self.ui.build(self.root, self.builds, |children| {
            id = Some(children.add(
                "tint",
                Element::color_well(color, style).named("tint"),
                |_| {},
            ));
        });
        self.ui.frame();
        self.well = id.expect("built");
    }

    fn find(&self, role: Role, name: &str) -> Meaning {
        self.ui
            .meaning()
            .into_iter()
            .find(|meaning| meaning.role == role && meaning.name == name)
            .unwrap_or_else(|| panic!("a {role:?} named {name:?}"))
    }

    fn key(&mut self, key: NamedKey) {
        self.ui.key(&KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Named(key),
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
        });
        self.ui.frame();
    }

    /// A press at (`x`, `y`), the pointer moved to each of `path` while held, then let go.
    fn drag(&mut self, (x, y): (f32, f32), path: &[(f32, f32)]) {
        self.ui.pointer_moved(x, y);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        for &(x, y) in path {
            self.ui.pointer_moved(x, y);
        }
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        self.ui.frame();
    }

    fn click_well(&mut self) {
        let rect = self.ui.rect(self.well).expect("laid out");
        self.drag((rect.x + 10.0, rect.y + 10.0), &[]);
    }

    /// The colors that the program read, each changed or committed.
    fn colors(&mut self) -> Vec<(&'static str, LinearColor)> {
        self.ui
            .take_actions()
            .into_iter()
            .filter_map(|action| match action.kind {
                ActionKind::Changed(FieldValue::Color(color)) => Some(("changed", color)),
                ActionKind::Committed(FieldValue::Color(color)) => Some(("committed", color)),
                ActionKind::Changed(_) | ActionKind::Committed(_) => {
                    panic!("only the well's color reaches the program: {action:?}")
                }
                _ => None,
            })
            .collect()
    }
}

fn close(a: LinearColor, b: LinearColor) -> bool {
    [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
        .iter()
        .all(|(a, b)| (a - b).abs() < 2e-3)
}

/// The linear color of an sRGB hexadecimal code.
fn srgb(red: u8, green: u8, blue: u8) -> LinearColor {
    let channel = |byte: u8| slime_color::srgb8_to_linear(byte);
    LinearColor::rgb(channel(red), channel(green), channel(blue))
}

#[test]
fn a_well_opens_its_picker_and_means_a_color_well() {
    let mut well = Well::new(srgb(255, 128, 0));
    let meaning = well.ui.meaning_of(well.well).expect("drawn");
    assert_eq!(meaning.role, Role::ColorWell);
    assert_eq!(meaning.value.as_deref(), Some("#ff8000"));
    assert!(meaning.popup && !meaning.state.expanded);
    well.click_well();
    assert!(well.ui.state(well.well).expect("there").expanded);
    // Its square, its strips, its channels and its code, below it.
    let square = well.find(Role::Slider, "saturation and value");
    assert_eq!(square.value.as_deref(), Some("saturation 100%, value 100%"));
    let hue = well.find(Role::Slider, "hue");
    assert_eq!(hue.value.as_deref(), Some("30°"));
    assert_eq!(
        well.find(Role::Slider, "alpha").value.as_deref(),
        Some("100%")
    );
    for name in ["red", "green", "blue", "alpha channel"] {
        well.find(Role::SpinButton, name);
    }
    let code = well.find(Role::TextInput, "hexadecimal code");
    assert_eq!(well.ui.field_text(code.id), Some("#ff8000"));
    let below = well.ui.rect(well.well).expect("laid out");
    assert!(square.bounds.expect("drawn").y >= below.y + below.height);
    // Escape closes it, unchanged.
    well.ui.set_focus(Some(square.id));
    well.key(NamedKey::Escape);
    assert!(!well.ui.state(well.well).expect("there").expanded);
    assert!(well.colors().is_empty());
}

#[test]
fn the_square_and_the_strips_drag_the_color_committed_as_the_pointer_lets_go() {
    let mut well = Well::new(srgb(255, 0, 0));
    well.click_well();
    let square = well
        .find(Role::Slider, "saturation and value")
        .bounds
        .expect("drawn");
    // Half way across and at the top: half saturated red, of full value; then at the bottom: black.
    let top = (square.x + square.width / 2.0, square.y);
    let bottom = (square.x + square.width / 2.0, square.y + square.height);
    well.drag(top, &[bottom]);
    let colors = well.colors();
    assert_eq!(colors.len(), 3, "{colors:?}");
    assert!(close(colors[0].1, srgb(255, 128, 128)), "{colors:?}");
    assert_eq!(colors[1].0, "changed");
    assert_eq!(colors[2], ("committed", LinearColor::rgb(0.0, 0.0, 0.0)));
    // Black has no hue: the strip of hue turns the square's, the color stays black.
    let hue = well.find(Role::Slider, "hue").bounds.expect("drawn");
    well.drag((hue.x + hue.width / 3.0, hue.y + 4.0), &[]);
    assert_eq!(
        well.colors(),
        [("committed", LinearColor::rgb(0.0, 0.0, 0.0))]
    );
    assert_eq!(
        well.find(Role::Slider, "hue").value.as_deref(),
        Some("120°")
    );
    // Value raised again by the keys: the green that the hue kept.
    let square_id = well.find(Role::Slider, "saturation and value").id;
    well.ui.set_focus(Some(square_id));
    let mut shift = slime_input::Modifiers::default();
    shift.shift = true;
    well.ui.modifiers(shift);
    for _ in 0..10 {
        well.key(NamedKey::ArrowUp);
    }
    let colors = well.colors();
    let last = colors.last().expect("committed");
    assert_eq!(last.0, "committed");
    assert!(close(last.1, srgb(128, 255, 128)), "{colors:?}");
    // The strip of alpha, at its start: clear.
    let alpha = well.find(Role::Slider, "alpha").bounds.expect("drawn");
    well.drag((alpha.x, alpha.y + 4.0), &[]);
    let colors = well.colors();
    assert_eq!(colors.last().map(|(_, color)| color.a), Some(0.0));
    assert_eq!(well.ui.color(well.well).map(|color| color.a), Some(0.0));
}

#[test]
fn channels_and_a_code_typed_set_the_color_and_never_reach_the_program_as_fields() {
    let mut well = Well::new(LinearColor::WHITE);
    well.click_well();
    let green = well.find(Role::SpinButton, "green").id;
    assert!(well.ui.set_value(green, "0.25"));
    assert_eq!(
        well.colors(),
        [
            ("changed", LinearColor::rgb(1.0, 0.25, 1.0)),
            ("committed", LinearColor::rgb(1.0, 0.25, 1.0)),
        ]
    );
    // The picker shows it: its code, its channel as typed.
    let code = well.find(Role::TextInput, "hexadecimal code").id;
    well.ui.frame();
    assert_eq!(well.ui.field_text(code), Some("#ff89ff"));
    assert_eq!(well.ui.field_text(green), Some("0.25"));
    assert!(well.ui.set_value(code, "#00ff0080"));
    let colors = well.colors();
    assert!(
        close(colors[1].1, LinearColor::rgba(0.0, 1.0, 0.0, 128.0 / 255.0)),
        "{colors:?}"
    );
    // A code that names no color is given back.
    assert!(well.ui.set_value(code, "teal"));
    assert!(well.colors().is_empty());
    well.ui.frame();
    assert_eq!(well.ui.field_text(code), Some("#00ff0080"));
}

#[test]
fn an_agent_gives_a_code_and_the_program_s_color_replaces_the_user_s_when_it_changes() {
    let mut well = Well::new(LinearColor::BLACK);
    assert!(well.ui.set_value(well.well, "#ffffff"));
    assert_eq!(
        well.colors(),
        [
            ("changed", LinearColor::WHITE),
            ("committed", LinearColor::WHITE),
        ]
    );
    assert!(!well.ui.set_value(well.well, "white"));
    // The program gives black again, as before: the user's white stays; then red.
    well.build(LinearColor::BLACK);
    assert_eq!(well.ui.color(well.well), Some(LinearColor::WHITE));
    well.build(LinearColor::rgb(1.0, 0.0, 0.0));
    assert_eq!(
        well.ui.color(well.well),
        Some(LinearColor::rgb(1.0, 0.0, 0.0))
    );
    let meaning = well.ui.meaning_of(well.well).expect("drawn");
    assert_eq!(meaning.value.as_deref(), Some("#ff0000"));
}
