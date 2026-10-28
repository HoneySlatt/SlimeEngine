//! Lists to choose from, without a GPU (`docs/adr/0150-layers.md`): the list of options that one
//! opens below it, as wide as it at least, its chosen option marked and given the focus by the
//! keyboard; an option chosen by the keys or the pointer, which the program reads; the list closed
//! unchanged by Escape, a press outside it, or a press on what opened it; the keys that choose
//! while it is closed; the program's choice, which replaces the user's only when it changes; an
//! agent's choice by an option's text or place; options that are disabled, passed over, and their
//! help; their meaning.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey};
use slime_ui::action::{Action, ActionKind, FieldValue};
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    ChoiceOption, Element, ElementAction, ElementId, LayoutDirection, Role, RootSettings, Tooltip,
    Ui,
};

const OPTIONS: [&str; 3] = ["Low", "Medium", "High"];

/// A list to choose from of [`OPTIONS`], the one at `chosen` chosen, 160 logical pixels wide, at
/// the top of a root of 400 by 300.
struct Choice {
    ui: Ui,
    root: ElementId,
    choice: ElementId,
    builds: u64,
}

impl Choice {
    fn new(chosen: usize) -> Self {
        let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
        let mut fonts = Fonts::new();
        fonts
            .register(std::fs::read(path).expect("the test font"))
            .expect("a font");
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        column.text.style.fonts = vec!["Noto Sans".to_owned()];
        let root = ui.add_root("main", settings, column);
        let mut choice = Self {
            ui,
            root,
            choice: root,
            builds: 0,
        };
        choice.build(chosen);
        choice
    }

    /// Builds the list to choose from again, the program giving `chosen`.
    fn build(&mut self, chosen: usize) {
        let mut style = Style::default();
        style.layout.size[0] = Dimension::Pixels(160.0);
        style.layout.align_self = Some(slime_ui::style::Align::Start);
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        let options = OPTIONS.map(str::to_owned).to_vec();
        let mut id = None;
        self.builds += 1;
        self.ui.build(self.root, self.builds, |children| {
            id = Some(children.add("quality", Element::choice(options, chosen, style), |_| {}));
        });
        self.ui.frame();
        self.choice = id.expect("built");
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

    fn click(&mut self, x: f32, y: f32) {
        self.ui.pointer_moved(x, y);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Pressed);
        self.ui
            .pointer_button(MouseButton::Left, ButtonState::Released);
        self.ui.frame();
    }

    /// The list open below the list to choose from, and its options; none while it is closed.
    fn list(&self) -> Option<(ElementId, Vec<ElementId>)> {
        let list = self.ui.element_by_path("choices")?;
        let menu = self.ui.element_by_path("choices/options")?;
        Some((list, self.ui.children(menu).to_vec()))
    }

    fn actions(&mut self) -> Vec<ActionKind> {
        self.ui
            .take_actions()
            .into_iter()
            .map(|action| action.kind)
            .collect()
    }
}

/// The middle of `id`'s box.
fn middle(ui: &Ui, id: ElementId) -> (f32, f32) {
    let rect = ui.rect(id).expect("laid out");
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

fn chosen(place: usize) -> [ActionKind; 2] {
    [
        ActionKind::Changed(FieldValue::Chosen(place)),
        ActionKind::Committed(FieldValue::Chosen(place)),
    ]
}

#[test]
fn the_keyboard_opens_the_list_on_the_chosen_option_and_chooses_another() {
    let mut choice = Choice::new(1);
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::Enter);
    let (list, options) = choice.list().expect("open");
    assert_eq!(options.len(), 3);
    assert_eq!(choice.ui.focus(), Some(options[1]));
    let states: Vec<bool> = options
        .iter()
        .map(|&option| choice.ui.state(option).expect("there").selected)
        .collect();
    assert_eq!(states, [false, true, false]);
    assert!(choice.ui.state(choice.choice).expect("there").expanded);
    // Below it, as wide as it at least.
    let (rect, below) = (
        choice.ui.rect(choice.choice).expect("laid out"),
        choice.ui.rect(list).expect("placed"),
    );
    assert!(below.y >= rect.y + rect.height && below.width >= rect.width);
    choice.key(NamedKey::ArrowDown);
    choice.key(NamedKey::Enter);
    assert_eq!(choice.actions(), chosen(2));
    assert!(choice.list().is_none());
    assert_eq!(choice.ui.chosen(choice.choice), Some(2));
    assert_eq!(choice.ui.focus(), Some(choice.choice));
    assert!(!choice.ui.state(choice.choice).expect("there").expanded);
}

#[test]
fn the_pointer_chooses_and_a_press_outside_or_on_the_list_to_choose_from_closes_unchanged() {
    let mut choice = Choice::new(0);
    let (x, y) = middle(&choice.ui, choice.choice);
    choice.click(x, y);
    let (_, options) = choice.list().expect("open");
    // Opened by the pointer: the focus stays on what opened it.
    assert_eq!(choice.ui.focus(), Some(choice.choice));
    choice.click(x, y);
    assert!(choice.list().is_none(), "a press on it closes it");
    choice.click(x, y);
    choice.click(390.0, 290.0);
    assert!(choice.list().is_none(), "a press outside closes it");
    assert!(choice.actions().is_empty());
    choice.click(x, y);
    let (_, options_again) = choice.list().expect("open");
    assert_eq!(options.len(), options_again.len());
    let (ox, oy) = middle(&choice.ui, options_again[2]);
    choice.click(ox, oy);
    assert_eq!(choice.actions(), chosen(2));
    assert!(choice.list().is_none());
}

#[test]
fn escape_closes_the_list_unchanged_telling_the_program_nothing() {
    let mut choice = Choice::new(0);
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::Space);
    choice.key(NamedKey::ArrowDown);
    choice.key(NamedKey::Escape);
    assert!(choice.list().is_none());
    assert!(choice.actions().is_empty());
    assert_eq!(choice.ui.chosen(choice.choice), Some(0));
    assert_eq!(choice.ui.focus(), Some(choice.choice));
}

#[test]
fn closed_the_arrows_choose_at_once_and_alt_down_or_f4_open() {
    let mut choice = Choice::new(0);
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::ArrowDown);
    assert_eq!(choice.actions(), chosen(1));
    choice.key(NamedKey::End);
    assert_eq!(choice.actions(), chosen(2));
    // At the last, down chooses nothing more.
    choice.key(NamedKey::ArrowDown);
    assert!(choice.actions().is_empty());
    choice.key(NamedKey::Home);
    assert_eq!(choice.actions(), chosen(0));
    choice.key(NamedKey::ArrowUp);
    assert!(choice.actions().is_empty());
    choice.key(NamedKey::F4);
    assert!(choice.list().is_some());
    choice.key(NamedKey::Escape);
    let mut alt = Modifiers::default();
    alt.alt = true;
    choice.ui.modifiers(alt);
    choice.key(NamedKey::ArrowDown);
    assert!(choice.list().is_some());
    assert!(choice.actions().is_empty());
}

#[test]
fn the_programs_choice_replaces_the_users_only_when_it_changes() {
    let mut choice = Choice::new(0);
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::ArrowDown);
    choice.ui.take_actions();
    // The program gives 0 again, as before: the user's choice stays.
    choice.build(0);
    assert_eq!(choice.ui.chosen(choice.choice), Some(1));
    choice.build(2);
    assert_eq!(choice.ui.chosen(choice.choice), Some(2));
    // A place past the options is the last.
    choice.build(9);
    assert_eq!(choice.ui.chosen(choice.choice), Some(2));
}

#[test]
fn an_agent_chooses_by_an_options_text_or_its_place() {
    let mut choice = Choice::new(0);
    assert!(choice.ui.set_value(choice.choice, "High"));
    assert_eq!(choice.actions(), chosen(2));
    assert!(choice.ui.set_value(choice.choice, "1"));
    assert_eq!(choice.actions(), chosen(1));
    assert!(!choice.ui.set_value(choice.choice, "Ultra"));
    assert!(!choice.ui.set_value(choice.choice, "3"));
    assert!(choice.actions().is_empty());
    // A press opens its list, as a person's.
    choice.ui.act(Action {
        element: Some(choice.choice),
        kind: ActionKind::Press,
    });
    choice.ui.frame();
    let (_, options) = choice.list().expect("open");
    assert_eq!(choice.ui.focus(), Some(options[1]));
}

#[test]
fn a_list_to_choose_from_means_a_combo_box_and_its_options_a_list() {
    let mut choice = Choice::new(1);
    let meaning = choice.ui.meaning_of(choice.choice).expect("drawn");
    assert_eq!(meaning.role, Role::ComboBox);
    assert_eq!(meaning.value.as_deref(), Some("Medium"));
    assert!(meaning.popup && !meaning.state.expanded);
    assert!(meaning.actions.contains(&ElementAction::SetValue));
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::Enter);
    assert!(
        choice
            .ui
            .meaning_of(choice.choice)
            .expect("drawn")
            .state
            .expanded
    );
    let (list, options) = choice.list().expect("open");
    let menu = choice.ui.children(list)[0];
    assert_eq!(
        choice.ui.meaning_of(menu).expect("drawn").role,
        Role::ListBox
    );
    let option = choice.ui.meaning_of(options[1]).expect("drawn");
    assert_eq!(
        (option.role, option.name.as_str(), option.state.selected),
        (Role::ListBoxOption, "Medium", true)
    );
}

#[test]
fn a_choice_kept_by_the_user_stays_among_fewer_options() {
    let mut choice = Choice::new(0);
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::End);
    choice.ui.take_actions();
    assert_eq!(choice.ui.chosen(choice.choice), Some(2));
    // The program gives one option, chosen as before: the user's choice is the last there is.
    let mut style = Style::default();
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    let mut id = None;
    choice.ui.build(choice.root, 99, |children| {
        id = Some(children.add(
            "quality",
            Element::choice(vec!["Low".to_owned()], 0, style),
            |_| {},
        ));
    });
    choice.ui.frame();
    assert_eq!(choice.ui.chosen(choice.choice), Some(0));
    let meaning = choice.ui.meaning_of(choice.choice).expect("drawn");
    assert_eq!(meaning.value.as_deref(), Some("Low"));
}

#[test]
fn a_list_to_choose_from_in_a_modal_layer_opens_and_chooses() {
    let mut choice = Choice::new(0);
    let dialog = choice.ui.open_layer(
        "layer",
        slime_ui::tree::Layer {
            place: slime_ui::tree::Place::Floating {
                over: choice.root,
                origin: [100.0, 100.0],
                size: [200.0, 120.0],
            },
            modal: true,
            dismiss: true,
        },
        Style::default(),
    );
    let mut style = Style::default();
    style.layout.size[0] = Dimension::Pixels(160.0);
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    let mut inner = None;
    choice.ui.build(dialog, 0, |children| {
        let options = OPTIONS.map(str::to_owned).to_vec();
        inner = Some(children.add("quality", Element::choice(options, 0, style), |_| {}));
    });
    choice.ui.frame();
    let inner = inner.expect("built");
    let (x, y) = middle(&choice.ui, inner);
    choice.click(x, y);
    let menu = choice
        .ui
        .element_by_path("choices/options")
        .expect("its list, over the dialog");
    let options = choice.ui.children(menu).to_vec();
    // The keys move into the list, and the pointer chooses there.
    choice.ui.set_focus(Some(options[1]));
    assert_eq!(choice.ui.focus(), Some(options[1]));
    let (ox, oy) = middle(&choice.ui, options[2]);
    choice.click(ox, oy);
    assert_eq!(choice.actions(), chosen(2));
    assert_eq!(choice.ui.chosen(inner), Some(2));
    assert!(choice.ui.rect(dialog).is_some(), "the dialog stays");
}

#[test]
fn options_that_are_disabled_are_passed_over_and_options_show_their_help() {
    let mut choice = Choice::new(0);
    let mut style = Style::default();
    style.layout.size[0] = Dimension::Pixels(160.0);
    style.layout.align_self = Some(slime_ui::style::Align::Start);
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    let options = [
        ChoiceOption::from("Low"),
        ChoiceOption {
            label: "Medium".to_owned(),
            disabled: true,
            help: Some(Tooltip {
                title: "Medium".to_owned(),
                text: "needs a second light".to_owned(),
            }),
        },
        ChoiceOption::from("High"),
    ];
    choice.ui.build(choice.root, 99, |children| {
        let element = Element::choice(options.clone(), 0, style);
        children.add("quality", element, |_| {});
    });
    choice.ui.frame();
    // Closed, the arrows pass over it, both ways.
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::ArrowDown);
    assert_eq!(choice.actions(), chosen(2));
    choice.key(NamedKey::ArrowUp);
    assert_eq!(choice.actions(), chosen(0));
    // Neither an agent nor a press chooses it.
    assert!(!choice.ui.set_value(choice.choice, "Medium"));
    assert!(!choice.ui.set_value(choice.choice, "1"));
    choice.key(NamedKey::Enter);
    let (_, items) = choice.list().expect("open");
    let state = choice.ui.state(items[1]).expect("there");
    assert!(state.disabled);
    let meaning = choice.ui.meaning_of(items[1]).expect("drawn");
    assert_eq!(
        meaning.description.as_deref(),
        Some("Medium: needs a second light")
    );
    let (x, y) = middle(&choice.ui, items[1]);
    choice.click(x, y);
    assert!(choice.actions().is_empty());
    assert_eq!(choice.ui.chosen(choice.choice), Some(0));
    // The keys in the open list pass over it too.
    choice.key(NamedKey::Escape);
    assert!(choice.list().is_none());
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::Enter);
    let (_, items) = choice.list().expect("open");
    assert_eq!(choice.ui.focus(), Some(items[0]));
    choice.key(NamedKey::ArrowDown);
    assert_eq!(choice.ui.focus(), Some(items[2]));
    // The program may give it chosen: the list opens on the nearest option that is not.
    choice.key(NamedKey::Escape);
    choice.ui.build(choice.root, 100, |children| {
        let mut style = Style::default();
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        children.add("quality", Element::choice(options, 1, style), |_| {});
    });
    choice.ui.frame();
    assert_eq!(choice.ui.chosen(choice.choice), Some(1));
    choice.ui.set_focus(Some(choice.choice));
    choice.key(NamedKey::Enter);
    let (_, items) = choice.list().expect("open");
    assert_eq!(choice.ui.focus(), Some(items[2]));
}
