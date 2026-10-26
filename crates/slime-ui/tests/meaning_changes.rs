//! What changed in what the tree means, without a GPU (`docs/adr/0146-screen-readers.md`): the
//! first changes whole, then only the elements that changed, which, applied one after the other,
//! give the whole tree at each step; a field's text by runs, as screen readers read it, in text
//! of both directions, of several lines, long or empty, with its selection; and widgets used.

use std::collections::{HashMap, HashSet};

use slime_input::{ButtonState, MouseButton};
use slime_ui::action::{Action, ActionKind};
use slime_ui::style::{Dimension, Display, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Element, ElementId, FieldText, LayoutDirection, MeaningChanges, MeaningNode, Role,
    RootSettings, Ui,
};

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

/// Settings of a root of 400 by 300 logical pixels at scale 1.
fn settings(modal: bool) -> RootSettings {
    RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal,
    }
}

/// A style of `width` by `height` logical pixels, which does not shrink, of the tests' fonts.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style.text.style.fonts = vec!["Noto Sans".to_owned(), "Noto Sans Hebrew".to_owned()];
    style
}

/// What a screen reader's adapter holds of the tree, from the changes it was given.
#[derive(Debug, Default, PartialEq)]
struct Held {
    nodes: HashMap<u64, MeaningNode>,
    top: Vec<u64>,
}

impl Held {
    /// Applies `changes`: the elements given replace those of their numbers, and those that no
    /// element's children nor the top names any more leave, with what they held.
    fn apply(&mut self, changes: MeaningChanges) {
        if changes.whole {
            self.nodes.clear();
        }
        for node in changes.nodes {
            self.nodes.insert(node.number, node);
        }
        if let Some(top) = changes.top {
            self.top = top;
        }
        let mut reached = HashSet::new();
        let mut stack = self.top.clone();
        while let Some(number) = stack.pop() {
            let node = self
                .nodes
                .get(&number)
                .unwrap_or_else(|| panic!("the element {number} was never given"));
            assert!(reached.insert(number), "{number} is held twice");
            stack.extend(&node.children);
        }
        self.nodes.retain(|number, _| reached.contains(number));
    }
}

/// The whole tree, as `ui` means it now.
fn whole(ui: &mut Ui) -> Held {
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(true));
    held
}

/// Takes the changes of `ui`, applies them to `held`, and checks that it holds the whole tree;
/// gives the numbers of the elements that changed, sorted, and whether the top changed.
fn step(ui: &mut Ui, held: &mut Held) -> (Vec<u64>, bool) {
    let changes = ui.take_meaning_changes(false);
    assert!(!changes.whole);
    let mut numbers: Vec<u64> = changes.nodes.iter().map(|node| node.number).collect();
    numbers.sort_unstable();
    let top = changes.top.is_some();
    held.apply(changes);
    assert_eq!(*held, whole(ui), "the changes applied give the whole tree");
    (numbers, top)
}

/// The number of `id`, as the whole tree gives it.
fn number(ui: &mut Ui, id: ElementId) -> u64 {
    ui.take_meaning_changes(true)
        .nodes
        .into_iter()
        .find(|node| node.meaning.id == id)
        .expect("an element that means something")
        .number
}

/// A page, a box that only lays others out, holding a named bar of two buttons, a text whose words
/// `revision` changes, a box that only lays out a named panel and, from revision 2, a second, a box
/// that scrolls two buttons, and a field.
fn build(ui: &mut Ui, root: ElementId, revision: u64) {
    let mut column = sized(400.0, 300.0);
    column.layout.direction = FlexDirection::Column;
    ui.build(root, revision, |children| {
        children.add("page", Element::container(column), |children| {
            let bar = Element::container(sized(400.0, 40.0)).named("tools");
            children.add("bar", bar, |children| {
                children.add("open", Element::button("open", sized(100.0, 40.0)), |_| {});
                children.add("save", Element::button("save", sized(100.0, 40.0)), |_| {});
            });
            let words = format!("revision {revision}");
            children.add("status", Element::label(words, sized(400.0, 20.0)), |_| {});
            children.add(
                "layout",
                Element::container(sized(400.0, 40.0)),
                |children| {
                    let panel = Element::container(sized(100.0, 40.0)).named("panel");
                    children.add("panel", panel, |_| {});
                    if revision >= 2 {
                        let second = Element::container(sized(100.0, 40.0)).named("second");
                        children.add("second", second, |_| {});
                    }
                },
            );
            let mut scroll = sized(150.0, 40.0);
            scroll.layout.direction = FlexDirection::Column;
            children.add("scroll", Element::scroll(scroll), |children| {
                for name in ["first", "next"] {
                    children.add(name, Element::button(name, sized(150.0, 30.0)), |_| {});
                }
            });
            children.add("field", Element::text_field("", sized(200.0, 30.0)), |_| {});
        });
    });
}

#[test]
fn the_changes_hold_what_changed_and_applied_give_the_whole_tree() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    build(&mut ui, root, 1);
    ui.frame();
    let mut held = Held::default();
    let first = ui.take_meaning_changes(false);
    assert!(first.whole, "the first changes are whole");
    assert!(first.top.is_some());
    held.apply(first);
    assert_eq!(held, whole(&mut ui));
    // The top: the elements that mean something below the boxes that only lay others out.
    let roles: Vec<Role> = held
        .top
        .iter()
        .map(|number| held.nodes[number].meaning.role)
        .collect();
    assert_eq!(
        roles,
        [
            Role::Group,
            Role::Label,
            Role::Group,
            Role::ScrollView,
            Role::TextInput
        ]
    );
    // Nothing changed: nothing given, after the first changes and after whole ones asked for.
    assert_eq!(step(&mut ui, &mut held), (Vec::new(), false));
    assert!(ui.take_meaning_changes(true).whole);
    assert_eq!(step(&mut ui, &mut held), (Vec::new(), false));

    // A text's words: that text alone.
    let status = ui.element_by_path("#0/page/status").expect("the text");
    let status_number = number(&mut ui, status);
    build(&mut ui, root, 3);
    ui.frame();
    let (changed, top) = step(&mut ui, &mut held);
    // The second panel is new, below a box that means nothing: the top changed with it.
    assert!(top);
    let second = ui.element_by_path("#0/page/layout/second").expect("built");
    let second_number = number(&mut ui, second);
    assert_eq!(changed, sorted(vec![status_number, second_number]));

    // The pointer over a button: its state.
    let open = ui.element_by_path("#0/page/bar/open").expect("the button");
    let open_number = number(&mut ui, open);
    ui.pointer_moved(10.0, 10.0);
    ui.frame();
    assert_eq!(step(&mut ui, &mut held), (vec![open_number], false));

    // A scroll: what moved below the box that scrolls.
    let scroll = ui.element_by_path("#0/page/scroll").expect("the box");
    let moved: Vec<u64> = ["first", "next"]
        .map(|name| {
            let id = ui
                .element_by_path(&format!("#0/page/scroll/{name}"))
                .expect("a button");
            number(&mut ui, id)
        })
        .to_vec();
    ui.scroll_to(scroll, [0.0, 10.0]);
    ui.frame();
    assert_eq!(step(&mut ui, &mut held), (sorted(moved), false));

    // A field typed in: the field, its value and its runs.
    let field = ui.element_by_path("#0/page/field").expect("the field");
    let field_number = number(&mut ui, field);
    ui.pointer_moved(10.0, 200.0);
    ui.set_focus(Some(field));
    ui.frame();
    step(&mut ui, &mut held);
    let typed = slime_input::KeyEvent {
        physical: slime_input::KeyCode::KeyA,
        logical: slime_input::Key::Character("a".to_owned()),
        text: Some("a".to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    };
    assert!(ui.key(&typed));
    ui.frame();
    let (changed, _) = step(&mut ui, &mut held);
    assert!(changed.contains(&field_number), "{changed:?}");
    let node = &held.nodes[&field_number];
    assert_eq!(node.meaning.value.as_deref(), Some("a"));
    let text = node.text.as_ref().expect("a field's text");
    assert_eq!(text.runs[0].text, "a");
    assert_eq!((text.anchor, text.focus), ((0, 1), (0, 1)));
}

#[test]
fn a_modal_root_makes_the_changes_whole_as_it_opens_and_closes() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    build(&mut ui, root, 1);
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    let dialog = ui.add_root(settings(true), Style::default());
    ui.build(dialog, 0, |children| {
        let box_of = Element::container(sized(200.0, 100.0)).role(Role::Dialog);
        children.add("dialog", box_of, |children| {
            children.add("ok", Element::button("ok", sized(50.0, 30.0)), |_| {});
        });
    });
    ui.frame();
    let changes = ui.take_meaning_changes(false);
    assert!(changes.whole, "what every element takes changed");
    held.apply(changes);
    assert_eq!(held, whole(&mut ui));
    // The buttons below take nothing; the dialog is at the top, last.
    let open = ui.element_by_path("#0/page/bar/open").expect("the button");
    let open_number = number(&mut ui, open);
    assert!(held.nodes[&open_number].meaning.actions.is_empty());
    let last = *held.top.last().expect("a top");
    assert_eq!(held.nodes[&last].meaning.role, Role::Dialog);
    ui.remove(dialog);
    ui.frame();
    let changes = ui.take_meaning_changes(false);
    assert!(changes.whole);
    held.apply(changes);
    assert_eq!(held, whole(&mut ui));
    assert!(!held.nodes[&open_number].meaning.actions.is_empty());
}

#[test]
fn a_box_hidden_leaves_the_tree_and_shown_again_comes_back_whole() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    let build = |ui: &mut Ui, revision: u64, display: Display| {
        ui.build(root, revision, |children| {
            let mut style = sized(300.0, 100.0);
            style.layout.display = display;
            children.add("box", Element::container(style), |children| {
                let group = Element::container(sized(200.0, 50.0)).named("group");
                children.add("group", group, |children| {
                    children.add("yes", Element::button("yes", sized(50.0, 30.0)), |_| {});
                });
            });
        });
    };
    build(&mut ui, 0, Display::Flex);
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    assert_eq!(held.nodes.len(), 2);
    build(&mut ui, 1, Display::None);
    ui.frame();
    let (_, top) = step(&mut ui, &mut held);
    assert!(top);
    assert!(held.nodes.is_empty(), "hidden, with what it holds");
    build(&mut ui, 2, Display::Flex);
    ui.frame();
    step(&mut ui, &mut held);
    assert_eq!(held.nodes.len(), 2, "shown again, with what it holds");
}

#[test]
fn numbers_name_one_element_or_run_each_and_find_it_again() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    build(&mut ui, root, 1);
    ui.frame();
    let changes = ui.take_meaning_changes(true);
    let mut numbers = HashSet::new();
    for node in &changes.nodes {
        assert!(node.number > 0);
        assert!(numbers.insert(node.number));
        assert_eq!(
            ui.element_by_number(node.number),
            Some((node.meaning.id, None))
        );
        for (place, run) in node
            .text
            .iter()
            .flat_map(|text| text.runs.iter().enumerate())
        {
            assert!(numbers.insert(run.number), "a run's number is its own");
            assert_eq!(
                ui.element_by_number(run.number),
                Some((node.meaning.id, Some(place)))
            );
        }
    }
    // An element removed and another made in its place: a new number.
    let status = ui.element_by_path("#0/page/status").expect("the text");
    let before = number(&mut ui, status);
    ui.remove(status);
    ui.build(root, 9, |children| {
        children.add("again", Element::label("again", sized(100.0, 20.0)), |_| {});
    });
    ui.frame();
    let again = ui.element_by_path("#0/again").expect("built");
    assert_ne!(number(&mut ui, again), before);
    assert_eq!(ui.element_by_number(before), None);
}

#[test]
fn changes_that_no_one_takes_are_kept_within_bounds_and_then_whole() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    build(&mut ui, root, 1);
    ui.frame();
    ui.take_meaning_changes(false);
    // The pointer over a button and away again, many times, its state changing each time.
    for _ in 0..2_000 {
        ui.pointer_moved(10.0, 10.0);
        ui.pointer_moved(390.0, 290.0);
    }
    ui.frame();
    let changes = ui.take_meaning_changes(false);
    assert!(
        !changes.whole,
        "the same element marked again is marked once"
    );
    // Many elements made and removed, never taken: the next changes are whole.
    for revision in 0..1_500 {
        ui.build(root, 100 + revision, |children| {
            children.add(revision, Element::label("row", sized(10.0, 10.0)), |_| {});
        });
    }
    assert!(ui.take_meaning_changes(false).whole);
}

/// The text of a field holding `text`, in a box of `width` logical pixels, of several lines for
/// `multiline`, its selection from `anchor` to `focus`.
fn runs(text: &str, width: f32, multiline: bool, selection: Option<(usize, usize)>) -> FieldText {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    let mut field = None;
    let element = if multiline {
        Element::text_area(text, sized(width, 200.0))
    } else {
        Element::text_field(text, sized(width, 30.0))
    };
    ui.build(root, 0, |children| {
        field = Some(children.add("field", element, |_| {}));
    });
    ui.frame();
    let field = field.expect("built");
    if let Some((anchor, focus)) = selection {
        ui.set_focus(Some(field));
        assert!(ui.select_text(field, anchor, focus));
    }
    ui.frame();
    ui.field_runs(field).expect("a field")
}

/// The texts of `runs`, end to end.
fn joined(text: &FieldText) -> String {
    text.runs.iter().map(|run| run.text.as_str()).collect()
}

#[test]
fn a_field_s_runs_hold_its_text_by_characters_and_words() {
    let text = runs("hello world", 300.0, false, Some((1, 3)));
    assert_eq!(text.runs.len(), 1);
    let run = &text.runs[0];
    assert_eq!(run.text, "hello world");
    assert!(!run.right_to_left);
    assert_eq!(run.characters.len(), 11);
    assert_eq!(
        run.characters
            .iter()
            .map(|character| character.length)
            .sum::<usize>(),
        11
    );
    // From the run's left, each after the one before.
    assert_eq!(run.characters[0].position, 0.0);
    for pair in run.characters.windows(2) {
        assert!(
            (pair[1].position - (pair[0].position + pair[0].width)).abs() < 0.01,
            "{pair:?}"
        );
    }
    // A word holds the space after it.
    assert_eq!(run.word_starts, [0, 6]);
    assert_eq!((text.anchor, text.focus), ((0, 1), (0, 3)));
    // A letter and its mark are one character, as the selection moves.
    let text = runs("e\u{301}t\u{e9}", 300.0, false, None);
    assert_eq!(
        text.runs[0]
            .characters
            .iter()
            .map(|character| character.length)
            .collect::<Vec<_>>(),
        [3, 1, 2]
    );
}

#[test]
fn a_field_s_runs_follow_the_directions_of_its_text() {
    let hebrew = "שלום עולם";
    let text = runs(hebrew, 300.0, false, None);
    assert_eq!(joined(&text), hebrew);
    assert!(text.runs.iter().all(|run| run.right_to_left));
    // From the run's right, its first character's at 0.
    assert_eq!(text.runs[0].characters[0].position, 0.0);
    // Both directions in one line: runs of each, in the order of the text.
    let mixed = "abc שלום def";
    let text = runs(mixed, 300.0, false, None);
    assert_eq!(joined(&text), mixed);
    assert!(text.runs.iter().any(|run| run.right_to_left));
    assert!(text.runs.iter().any(|run| !run.right_to_left));
    let mut at = 0;
    for run in &text.runs {
        assert_eq!(run.start, at, "the runs follow the text");
        at += run.text.len();
    }
}

#[test]
fn a_field_s_runs_break_with_its_lines_long_runs_and_empty_lines() {
    // A line break ends its line's run; the empty line after the last is an empty run, where the
    // caret at the text's end lies.
    let text = runs("one\ntwo\n", 300.0, true, Some((8, 8)));
    let texts: Vec<&str> = text.runs.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(texts, ["one\n", "two\n", ""]);
    assert_eq!(text.focus, (2, 0));
    assert!(text.runs[1].bounds.y > text.runs[0].bounds.y);
    // The caret before a line break lies on it.
    let text = runs("one\ntwo", 300.0, true, Some((3, 3)));
    assert_eq!(text.focus, (0, 3));
    // A long line, cut every 255 characters.
    let long = "word ".repeat(120);
    let text = runs(&long, 300.0, false, None);
    let lengths: Vec<usize> = text.runs.iter().map(|run| run.characters.len()).collect();
    assert_eq!(lengths, [255, 255, 90]);
    assert_eq!(joined(&text), long);
    // A letter under 200 marks, one cluster of 401 bytes: characters of at most 255 bytes.
    let heaped = format!("a{}", "\u{301}".repeat(200));
    let text = runs(&heaped, 300.0, false, None);
    let lengths: Vec<usize> = text.runs[0]
        .characters
        .iter()
        .map(|character| character.length)
        .collect();
    assert_eq!(lengths, [255, 146]);
    assert_eq!(text.runs[0].characters[1].width, 0.0);
    // An empty field: one empty run, the caret at its start.
    let text = runs("", 300.0, false, None);
    assert_eq!(text.runs.len(), 1);
    assert!(text.runs[0].text.is_empty());
    assert_eq!((text.anchor, text.focus), ((0, 0), (0, 0)));
}

/// `numbers`, sorted.
fn sorted(mut numbers: Vec<u64>) -> Vec<u64> {
    numbers.sort_unstable();
    numbers
}

#[test]
fn a_press_on_a_button_changes_its_state_alone() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    build(&mut ui, root, 1);
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    let open = ui.element_by_path("#0/page/bar/open").expect("the button");
    let open_number = number(&mut ui, open);
    ui.pointer_moved(10.0, 10.0);
    ui.frame();
    step(&mut ui, &mut held);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.frame();
    // Pressed, and given the focus, which every change gives.
    assert_eq!(step(&mut ui, &mut held), (vec![open_number], false));
    assert_eq!(ui.take_meaning_changes(false).focus, Some(open_number));
}

#[test]
fn a_box_named_comes_between_its_children_and_the_element_above_them() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    let build = |ui: &mut Ui, revision: u64, name: Option<&str>| {
        ui.build(root, revision, |children| {
            let mut group = Element::container(sized(200.0, 50.0));
            group.name = name.map(str::to_owned);
            children.add("group", group, |children| {
                children.add("yes", Element::button("yes", sized(50.0, 30.0)), |_| {});
                children.add("no", Element::button("no", sized(50.0, 30.0)), |_| {});
            });
        });
    };
    build(&mut ui, 0, None);
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    assert_eq!(held.top.len(), 2, "the buttons, below a box of no meaning");
    // Named, the box is a group that holds them.
    build(&mut ui, 1, Some("answers"));
    ui.frame();
    let (changed, top) = step(&mut ui, &mut held);
    assert!(top);
    assert_eq!(changed.len(), 1, "the group alone");
    assert_eq!(held.top.len(), 1);
    assert_eq!(held.nodes[&held.top[0]].children.len(), 2);
    // Its name changed: the group alone, the top as it was.
    build(&mut ui, 2, Some("choices"));
    ui.frame();
    assert!(!step(&mut ui, &mut held).1);
    // Its name taken away: the buttons at the top again.
    build(&mut ui, 3, None);
    ui.frame();
    step(&mut ui, &mut held);
    assert_eq!(held.top.len(), 2);
}

#[test]
fn rows_built_by_a_scroll_another_kind_under_a_key_and_a_disabled_button_keep_the_tree_whole() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    let build = |ui: &mut Ui, revision: u64, kind_of_second: bool| {
        ui.build(root, revision, |children| {
            children.add(
                "list",
                Element::list(1_000, 20.0, sized(200.0, 60.0)),
                |rows| {
                    for row in rows.rows() {
                        rows.add(
                            row,
                            Element::label(format!("row {row}"), sized(200.0, 20.0)),
                            |_| {},
                        );
                    }
                },
            );
            let second = if kind_of_second {
                Element::button("second", sized(80.0, 30.0))
            } else {
                Element::label("second", sized(80.0, 30.0))
            };
            children.add("second", second, |_| {});
        });
    };
    build(&mut ui, 0, true);
    ui.frame();
    build(&mut ui, 0, true);
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    // Other rows shown by a scroll: the list's builder runs again, rows come and go.
    let list = ui.element_by_path("#0/list").expect("the list");
    ui.scroll_to(list, [0.0, 400.0]);
    build(&mut ui, 0, true);
    ui.frame();
    step(&mut ui, &mut held);
    let list_number = number(&mut ui, list);
    let rows: Vec<String> = held.nodes[&list_number]
        .children
        .iter()
        .map(|row| held.nodes[row].meaning.name.clone())
        .collect();
    assert_eq!(rows.first().map(String::as_str), Some("row 20"));
    // Another kind under the same key: another element, of another number.
    let button = ui.element_by_path("#0/second").expect("the button");
    let before = number(&mut ui, button);
    build(&mut ui, 1, false);
    ui.frame();
    step(&mut ui, &mut held);
    assert!(!held.nodes.contains_key(&before));
    let label = ui.element_by_path("#0/second").expect("the text");
    assert_eq!(
        held.nodes[&number(&mut ui, label)].meaning.role,
        Role::Label
    );
    // A button disabled takes nothing.
    build(&mut ui, 2, true);
    ui.frame();
    step(&mut ui, &mut held);
    let button = ui.element_by_path("#0/second").expect("the button");
    let mut state = ui.state(button).expect("a button");
    state.disabled = true;
    ui.set_state(button, state);
    ui.frame();
    step(&mut ui, &mut held);
    assert!(
        held.nodes[&number(&mut ui, button)]
            .meaning
            .actions
            .is_empty()
    );
}

#[test]
fn a_root_resized_or_removed_below_others_changes_what_it_shows_and_their_paths() {
    let mut ui = ui();
    let first = ui.add_root(settings(false), Style::default());
    let second = ui.add_root(settings(false), Style::default());
    for root in [first, second] {
        ui.build(root, 0, |children| {
            children.add("wide", Element::button("wide", sized(350.0, 30.0)), |_| {});
        });
    }
    ui.frame();
    let mut held = Held::default();
    held.apply(ui.take_meaning_changes(false));
    // The first root narrowed: what shows of its button.
    ui.set_root(
        first,
        RootSettings {
            size: [200.0, 300.0],
            ..settings(false)
        },
    );
    ui.frame();
    step(&mut ui, &mut held);
    let wide = ui.element_by_path("#0/wide").expect("the button");
    let bounds = held.nodes[&number(&mut ui, wide)].meaning.bounds;
    assert_eq!(bounds.map(|bounds| bounds.width), Some(200.0));
    // The first root removed: the second's elements are named from #0 on.
    ui.remove(first);
    ui.frame();
    step(&mut ui, &mut held);
    let paths: Vec<&str> = held
        .nodes
        .values()
        .map(|node| node.meaning.path.as_str())
        .collect();
    assert_eq!(paths, ["#0/wide"]);
}

#[test]
fn widgets_used_change_what_they_mean_and_keep_the_tree_whole() {
    let mut ui = ui();
    let root = ui.add_root(settings(false), Style::default());
    let mut column = sized(400.0, 300.0);
    column.layout.direction = FlexDirection::Column;
    let build = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            children.add("page", Element::container(column.clone()), |children| {
                let check = Element::check_box("shadows", false, sized(200.0, 24.0));
                children.add("check", check, |_| {});
                let slider = Element::slider(0.5, 0.0, 1.0, 0.1, sized(200.0, 24.0));
                children.add("slider", slider, |_| {});
                let section = Element::section("physics", false, sized(200.0, 60.0));
                children.add("section", section, |children| {
                    children.add("mass", Element::label("mass", sized(100.0, 20.0)), |_| {});
                });
                let bar = children.add("tabs", Element::tab_bar(0, sized(200.0, 24.0)), |tabs| {
                    for title in ["scene", "light"] {
                        tabs.add(title, Element::tab(title, sized(80.0, 24.0)), |_| {});
                    }
                });
                let shown = ["scene", "light"][children.chosen(bar)];
                let panel = Element::container(sized(200.0, 40.0))
                    .role(Role::TabPanel)
                    .named(shown);
                children.add(shown, panel, |_| {});
            });
        });
        ui.frame();
    };
    build(&mut ui);
    let mut held = whole(&mut ui);
    let path = |ui: &Ui, path: &str| ui.element_by_path(path).expect("built");
    let (check, slider) = (path(&ui, "#0/page/check"), path(&ui, "#0/page/slider"));
    let (section, light) = (
        path(&ui, "#0/page/section"),
        path(&ui, "#0/page/tabs/light"),
    );
    let numbers = [check, slider, section].map(|id| number(&mut ui, id));

    // Each widget used gives its element changed: checked, moved, opened.
    for (id, element_number) in [check, slider, section].into_iter().zip(numbers) {
        if id == slider {
            assert!(ui.set_value(slider, "0.8"));
        } else {
            ui.act(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
        }
        ui.frame();
        let (changed, _) = step(&mut ui, &mut held);
        assert!(changed.contains(&element_number), "{changed:?}");
    }
    let given = |held: &Held, number: u64| held.nodes[&number].meaning.clone();
    assert!(given(&held, numbers[0]).state.checked);
    assert_eq!(given(&held, numbers[1]).value.as_deref(), Some("0.8"));
    // Opened, the section gives what it holds, which meant nothing folded.
    let section_node = &held.nodes[&numbers[2]];
    assert!(section_node.meaning.state.expanded);
    assert_eq!(section_node.children.len(), 1);

    // A tab chosen runs the builder again: the tabs' states and the new panel change.
    ui.act(Action {
        element: Some(light),
        kind: ActionKind::Press,
    });
    build(&mut ui);
    step(&mut ui, &mut held);
    let panels: Vec<String> = held
        .nodes
        .values()
        .filter(|node| node.meaning.role == Role::TabPanel)
        .map(|node| node.meaning.name.clone())
        .collect();
    assert_eq!(panels, ["light"]);
    let light_number = number(&mut ui, light);
    assert!(held.nodes[&light_number].meaning.state.selected);
}
