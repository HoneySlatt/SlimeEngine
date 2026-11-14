//! Tables of shortcuts in KDL (`uwui::shortcuts`,
//! `docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`): the primary modifier
//! resolved on each platform, a shortcut written as each platform writes it, chords compared as
//! they resolve, and each refusal at its line.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, Modifiers};
use uwui::DocumentErrors;
use uwui::action::ActionKind;
use uwui::host::Person;
use uwui::shortcuts::{Platform, Shortcuts};
use uwui::text::Fonts;
use uwui::tree::{LayoutDirection, RootSettings, Ui};

/// An interface following `platform`'s ways, its shortcuts `table`.
fn interface(platform: Platform, table: &str) -> Ui {
    let mut ui = Ui::new(Fonts::new());
    ui.set_platform(platform);
    ui.set_shortcuts(&[Shortcuts::from_text("test.keys.kdl", table)])
        .expect("a table of shortcuts");
    ui
}

/// The commands that a press of the key that types `letter`, `modifiers` held, gives.
fn commands(ui: &mut Ui, modifiers: Modifiers, letter: &str) -> Vec<String> {
    ui.modifiers(modifiers);
    ui.key(&KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Character(letter.to_owned()),
        text: Some(letter.to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    });
    ui.modifiers(Modifiers::default());
    ui.take_actions()
        .into_iter()
        .filter_map(|action| match action.kind {
            ActionKind::Command(name) => Some(name),
            _ => None,
        })
        .collect()
}

/// The modifiers held: Control, and ⌘, the key with the system's logo.
fn held(control: bool, command: bool) -> Modifiers {
    let mut modifiers = Modifiers::default();
    modifiers.control = control;
    modifiers.super_key = command;
    modifiers
}

#[test]
fn primary_is_ctrl_on_linux_and_windows_and_command_on_macos_and_ctrl_is_control_everywhere() {
    let table = "scope global {\n    Primary+Z undo\n    Ctrl+K kill\n}\n";
    for platform in [Platform::Linux, Platform::Windows] {
        let mut ui = interface(platform, table);
        assert_eq!(commands(&mut ui, held(true, false), "z"), ["undo"]);
        assert!(commands(&mut ui, held(false, true), "z").is_empty());
        assert_eq!(commands(&mut ui, held(true, false), "k"), ["kill"]);
    }
    let mut ui = interface(Platform::Mac, table);
    assert_eq!(commands(&mut ui, held(false, true), "z"), ["undo"]);
    assert!(commands(&mut ui, held(true, false), "z").is_empty());
    assert_eq!(commands(&mut ui, held(true, false), "k"), ["kill"]);
    // The platform changed after the table was read: the chords resolve on the new one.
    ui.set_platform(Platform::Linux);
    assert_eq!(commands(&mut ui, held(true, false), "z"), ["undo"]);
    assert_eq!(Platform::default(), Platform::BUILT);
}

#[test]
fn a_shortcut_is_written_as_each_platform_writes_it() {
    let table = "scope global {\n    Primary+Shift+Z redo\n    Ctrl+Alt+Shift+Super+K all\n    \
                 Alt+Tab switch\n    Primary+ArrowUp top\n    Primary+Backspace erase\n    \
                 F2 rename\n    Super+Q quit\n}\n";
    let shown = |platform| {
        let ui = interface(platform, table);
        ["redo", "switch", "top", "erase", "rename", "quit"]
            .map(|command| ui.command_chord(command).expect("a chord"))
    };
    // Microsoft's order, the Windows key, Ctrl, Alt, Shift
    // (https://learn.microsoft.com/en-us/windows/win32/uxguide/inter-keyboard), the table's keys;
    // on Linux, Super last, as GTK writes it.
    let linux = [
        "Ctrl+Shift+Z",
        "Alt+Tab",
        "Ctrl+ArrowUp",
        "Ctrl+Backspace",
        "F2",
        "Super+Q",
    ];
    assert_eq!(shown(Platform::Linux), linux);
    let mut windows = linux;
    windows[5] = "Win+Q";
    assert_eq!(shown(Platform::Windows), windows);
    // Apple's symbols and order, Control, Option, Shift, Command, without a joint
    // (https://developer.apple.com/design/human-interface-guidelines/keyboards,
    // https://support.apple.com/en-us/102650).
    assert_eq!(shown(Platform::Mac), ["⇧⌘Z", "⌥⇥", "⌘▲", "⌘⌫", "F2", "⌘Q"]);
    let all = |platform| interface(platform, table).command_chord("all");
    // Every modifier, in each platform's order.
    assert_eq!(all(Platform::Mac).as_deref(), Some("⌃⌥⇧⌘K"));
    assert_eq!(
        all(Platform::Windows).as_deref(),
        Some("Win+Ctrl+Alt+Shift+K")
    );
    assert_eq!(
        all(Platform::Linux).as_deref(),
        Some("Ctrl+Alt+Shift+Super+K")
    );
    // A chord outside the tables, as a hint names one, is written the same way.
    assert_eq!(
        uwui::shortcuts::shown("Primary+Shift+P", Platform::Mac).as_deref(),
        Some("⇧⌘P")
    );
    assert_eq!(
        uwui::shortcuts::shown("Primary+Shift+P", Platform::Windows).as_deref(),
        Some("Ctrl+Shift+P")
    );
    assert_eq!(uwui::shortcuts::shown("Hyper+P", Platform::Linux), None);
    // The sheet of shortcuts lists them as the platform writes them too.
    let ui = interface(Platform::Mac, table);
    assert_eq!(
        ui.shortcuts()[0].1[0],
        ("⇧⌘Z".to_owned(), "redo".to_owned())
    );
}

#[test]
fn a_later_table_gives_a_chord_again_as_the_platform_resolves_it() {
    let earlier = Shortcuts::from_text("earlier", "scope global { Primary+S save; }\n");
    let later = Shortcuts::from_text("later", "scope global { Ctrl+S spell; }\n");
    for (platform, save, spell) in [
        (Platform::Linux, None, Some("Ctrl+S")),
        (Platform::Mac, Some("⌘S"), Some("⌃S")),
    ] {
        let mut ui = Ui::new(Fonts::new());
        ui.set_platform(platform);
        ui.set_shortcuts(&[earlier.clone(), later.clone()])
            .expect("two tables");
        assert_eq!(ui.command_chord("save").as_deref(), save, "{platform:?}");
        assert_eq!(ui.command_chord("spell").as_deref(), spell, "{platform:?}");
        assert_eq!(
            ui.shortcuts()[0].1.len(),
            if save.is_some() { 2 } else { 1 }
        );
    }
}

#[test]
fn a_table_is_refused_at_its_line_for_each_thing_wrong_on_any_platform() {
    for (table, line, problem) in [
        // Two chords of a scope that are the same keys on one platform.
        (
            "scope global {\n    Primary+Z undo\n    Ctrl+Z other\n}\n",
            3,
            "`Ctrl+Z` is the same keys on Linux as the chord at line 2 of the scope `global`",
        ),
        (
            "scope global {\n    Primary+Q quit\n    Super+Q other\n}\n",
            3,
            "on macOS",
        ),
        (
            "scope global {\n    Z undo\n    z other\n}\n",
            3,
            "the same keys",
        ),
        // A chord that ends with a modifier, or joins none by a `+`.
        (
            "scope a { Primary+Shift undo; }\n",
            1,
            "ends with a modifier, `Shift`",
        ),
        ("scope a { Alt undo; }\n", 1, "ends with a modifier"),
        ("scope a { +Z undo; }\n", 1, "joins no modifier by a `+`"),
        (
            "scope a { Ctrl++Z undo; }\n",
            1,
            "joins no modifier by a `+`",
        ),
        ("scope a { ++ undo; }\n", 1, "joins no modifier by a `+`"),
        // A modifier that a platform makes twice the same key.
        (
            "scope global { Primary+Ctrl+Z undo; }\n",
            1,
            "which it is on Linux and Windows",
        ),
        (
            "scope global { Primary+Super+Z undo; }\n",
            1,
            "which it is on macOS",
        ),
        (
            "scope global { Primary+Primary+Z undo; }\n",
            1,
            "names `Primary` twice",
        ),
        // A scope given twice, or not a scope.
        (
            "scope a { Z one; }\nscope a { X two; }\n",
            2,
            "the scope `a` is given twice in the table, first at line 1",
        ),
        ("global { Z undo; }\n", 1, "`global` is not a scope"),
        ("(t)scope global { Z undo; }\n", 1, "a scope takes no type"),
        ("scope (t)a { Z undo; }\n", 1, "a scope is named by a name"),
        ("scope \"\" { Z undo; }\n", 1, "a scope's name is not empty"),
        ("scope a { Z \"\"; }\n", 1, "not by an empty text"),
        ("scope { Z undo; }\n", 1, "a scope is named by a name"),
        ("scope 3 { Z undo; }\n", 1, "not `3`"),
        ("scope a b { Z undo; }\n", 1, "takes its name alone"),
        ("scope a x=1 { Z undo; }\n", 1, "takes its name alone"),
        ("scope \"$a\" { Z undo; }\n", 1, "never starts with `$`"),
        // A chord and its command.
        (
            "scope a {\n    Z\n}\n",
            2,
            "names a command by its name, not nothing",
        ),
        ("scope a { Z #null; }\n", 1, "not `#null`"),
        ("scope a { Z 3; }\n", 1, "not `3`"),
        ("scope a { Z (t)undo; }\n", 1, "names a command by its name"),
        ("scope a { Z x=undo; }\n", 1, "not a property, `x`"),
        (
            "scope a { Z undo redo; }\n",
            1,
            "names one command, not \"redo\"",
        ),
        ("scope a { Z undo x=1; }\n", 1, "names one command"),
        ("scope a { Z undo { X; }; }\n", 1, "takes no child"),
        ("scope a { (t)Z undo; }\n", 1, "takes no type"),
        ("scope a { Z \"$undo\"; }\n", 1, "never starts with `$`"),
        ("scope a { \"$\" undo; }\n", 1, "never starts with `$`"),
        ("scope a { \"$Z\" undo; }\n", 1, "never starts with `$`"),
        // A key that KDL does not take bare: the reader of KDL ends the name before it.
        ("scope a { Primary+; undo; }\n", 1, "is written in quotes"),
        (
            "scope a {\n    Hyper+Z undo\n}\n",
            2,
            "`Hyper` is not a modifier",
        ),
        // Not KDL.
        ("scope a {\n", 2, "line 2"),
    ] {
        let error = Ui::new(Fonts::new())
            .set_shortcuts(&[Shortcuts::from_text("wrong.keys.kdl", table)])
            .expect_err(table)
            .to_string();
        assert!(
            error.starts_with(&format!("wrong.keys.kdl: line {line},")) && error.contains(problem),
            "{table}: {error}"
        );
    }
    // Every error is told, each at its line.
    let error = Ui::new(Fonts::new())
        .set_shortcuts(&[Shortcuts::from_text(
            "wrong.keys.kdl",
            "scope a {\n    Foo one\n    Z two\n    Bar three\n}\nscope a\n",
        )])
        .expect_err("three errors");
    let lines: Vec<String> = error.errors().iter().map(ToString::to_string).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (error, line) in lines.iter().zip([2, 4, 6]) {
        assert!(
            error.starts_with(&format!("wrong.keys.kdl: line {line},")),
            "{error}"
        );
    }
}

#[test]
fn chords_that_kdl_takes_only_in_quotes_read_as_their_keys() {
    let table = "scope global {\n    \"1\" first\n    \"Primary+=\" zoom\n    Primary++ bigger\n    \
                 \".\" frame\n    - smaller\n}\n";
    let mut ui = interface(Platform::Linux, table);
    assert_eq!(commands(&mut ui, Modifiers::default(), "1"), ["first"]);
    assert_eq!(commands(&mut ui, held(true, false), "="), ["zoom"]);
    assert_eq!(commands(&mut ui, held(true, false), "+"), ["bigger"]);
    assert_eq!(commands(&mut ui, Modifiers::default(), "."), ["frame"]);
    assert_eq!(commands(&mut ui, Modifiers::default(), "-"), ["smaller"]);
}

#[test]
fn the_errors_of_a_table_are_gathered_up_to_their_cap_in_time_linear_in_the_table() {
    // Each chord the same keys as the first, on a line of its own: every error past the first
    // sixteen left out, and each placed by the lines counted once.
    let mut table = String::from("scope a {\n");
    for _ in 0..100_000 {
        table.push_str("    Z one\n");
    }
    table.push_str("}\n");
    let error = Ui::new(Fonts::new())
        .set_shortcuts(&[Shortcuts::from_text("wrong.keys.kdl", table)])
        .expect_err("chords given twice");
    assert_eq!(error.errors().len(), DocumentErrors::MOST);
    assert!(error.more());
    assert!(
        error.errors()[0]
            .to_string()
            .starts_with("wrong.keys.kdl: line 3,"),
        "{error}"
    );
}

#[test]
fn the_sheet_of_shortcuts_lists_two_chords_that_show_alike() {
    // On macOS, Tab shows as `⇥`, as the character `⇥` does: two rows that show alike.
    let mut ui = interface(
        Platform::Mac,
        "scope global {\n    Tab next\n    \"⇥\" mark\n}\n",
    );
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    ui.add_root("main", settings, |s| s);
    ui.open_shortcut_sheet("Shortcuts", &[]);
    ui.frame();
    let chords = &ui.shortcuts()[0].1;
    assert_eq!(chords.len(), 2);
    assert_eq!(chords[0].0, chords[1].0);
}
