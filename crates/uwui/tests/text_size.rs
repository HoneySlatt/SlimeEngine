//! The user's size of text (`docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md`):
//! it reaches every text, whether a rule or code sized it, but icons; a list's rows grow with it,
//! whole in pixels of the screen, and never overlap; a change of it styles every element again.

use uwui::Rect;
use uwui::host::TargetId;
use uwui::preferences::{Preferences, TextSize};
use uwui::style::Dimension;
use uwui::text::Fonts;
use uwui::theme::StyleSheet;
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

/// A sheet that sizes every text at 13 logical pixels and names Lucide the font of icons; whose
/// labels of the class `icon` are drawn in it, and those of the class `title` at 20.
const SHEET: &str = r##"
everything {
    select
    text { style size=13; }
    icons font=lucide
}

icon {
    select kind=Label { classes icon; }
    text { style { fonts lucide; }; }
}

title {
    select kind=Label { classes title; }
    text { style size=20; }
}
"##;

/// A root of `width` × `height` logical pixels at `scale`.
fn settings(width: f32, height: f32, scale: f32) -> RootSettings {
    RootSettings {
        size: [width, height],
        scale,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    }
}

fn styled_ui() -> Ui {
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(&[StyleSheet::from_text("text.style", SHEET)], &[])
        .expect("a valid sheet");
    ui
}

fn text_size(factor: f32) -> Preferences {
    Preferences {
        text_size: TextSize::factor(factor).expect("within its range"),
        ..Preferences::default()
    }
}

fn size_of(ui: &Ui, id: ElementId) -> f32 {
    ui.style(id).expect("styled").text.style.size
}

#[test]
fn the_size_of_text_reaches_what_rules_and_code_size_but_not_icons() {
    let mut ui = styled_ui();
    let root = ui.add_root("main", settings(400.0, 300.0, 1.0), |s| s);
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        ids.push(children.add("body", Element::label("Body"), |_| {}));
        ids.push(children.add("title", Element::label("Title").class("title"), |_| {}));
        let own = Element::label("Own").style(|s| s.font_size(16.0));
        ids.push(children.add("own", own, |_| {}));
        let icon = Element::label("\u{E06C}").class("icon");
        ids.push(children.add("icon", icon, |_| {}));
    });
    ui.frame(TargetId::MAIN);
    let sizes = |ui: &Ui| ids.iter().map(|&id| size_of(ui, id)).collect::<Vec<_>>();
    assert_eq!(sizes(&ui), [13.0, 20.0, 16.0, 13.0]);

    ui.set_preferences(text_size(2.0));
    ui.frame(TargetId::MAIN);
    assert_eq!(
        sizes(&ui),
        [26.0, 40.0, 32.0, 13.0],
        "every text doubles, the rule's and the code's, but the icon"
    );

    let curve = TextSize::curve(&[(13.0, 26.0), (20.0, 30.0)]).expect("a valid curve");
    ui.set_preferences(Preferences {
        text_size: curve,
        ..Preferences::default()
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(
        sizes(&ui),
        [26.0, 30.0, 26.0 + 4.0 * (3.0 / 7.0), 13.0],
        "larger text grows less along the curve"
    );

    ui.set_preferences(Preferences::default());
    ui.frame(TargetId::MAIN);
    assert_eq!(
        sizes(&ui),
        [13.0, 20.0, 16.0, 13.0],
        "back to the styles' sizes"
    );
}

/// The rows of a list of 100 rows of 20 logical pixels, built, in a root at `scale` under the size
/// of text `factor`, after the frames that build them.
fn rows_at(scale: f32, factor: f32) -> Vec<Rect> {
    let mut ui = styled_ui();
    ui.set_preferences(text_size(factor));
    let root = ui.add_root("main", settings(200.0, 300.0, scale), |s| s);
    let mut list = None;
    ui.build(root, 0, |children| {
        let element =
            Element::list(100, 20.0).style(|s| s.grow(1.0).min_height(Dimension::Pixels(0.0)));
        list = Some(children.add_slot("list", element));
    });
    let list = list.expect("built");
    for revision in 0..3 {
        ui.build(list, revision, |rows| {
            for row in rows.rows() {
                rows.add(row, Element::label("Row"), |_| {});
            }
        });
        ui.frame(TargetId::MAIN);
    }
    ui.children(list)
        .iter()
        .map(|&row| ui.rect(row).expect("laid out"))
        .collect()
}

#[test]
fn a_lists_rows_grow_with_the_text_whole_and_never_overlap() {
    for (scale, factor, height) in [
        (1.0, 1.0, 20.0),
        (1.25, 1.0, 25.0),
        (2.0, 1.0, 40.0),
        (1.0, 2.0, 40.0),
        (1.0, 1.1, 22.0),
        // 20 × 1.25 × 1.1 is 27.5: whole, 28.
        (1.25, 1.1, 28.0),
    ] {
        let rows = rows_at(scale, factor);
        let shown = (300.0 * scale / height).ceil() as usize;
        assert_eq!(
            rows.len(),
            shown,
            "the rows that show at scale {scale}, text {factor}"
        );
        for (number, row) in rows.iter().enumerate() {
            assert_eq!(
                (row.y, row.height),
                (number as f32 * height, height),
                "row {number} at scale {scale}, text {factor}"
            );
        }
    }
}

#[test]
fn a_change_of_the_size_of_text_builds_the_rows_that_now_show() {
    let mut ui = styled_ui();
    let root = ui.add_root("main", settings(200.0, 300.0, 1.0), |s| s);
    let mut list = None;
    ui.build(root, 0, |children| {
        let element = Element::list(100, 20.0).style(|s| s.grow(1.0));
        list = Some(children.add_slot("list", element));
    });
    let list = list.expect("built");
    let build = |ui: &mut Ui| {
        ui.build(list, 0, |rows| {
            for row in rows.rows() {
                rows.add(row, Element::label("Row"), |_| {});
            }
        });
        ui.frame(TargetId::MAIN);
    };
    build(&mut ui);
    build(&mut ui);
    ui.set_preferences(text_size(2.0));
    ui.frame(TargetId::MAIN);
    build(&mut ui);
    assert_eq!(ui.children(list).len(), 8, "300 pixels hold 8 rows of 40");
    ui.set_preferences(text_size(0.5));
    ui.frame(TargetId::MAIN);
    build(&mut ui);
    assert_eq!(ui.children(list).len(), 30, "and 30 rows of 10");
}
