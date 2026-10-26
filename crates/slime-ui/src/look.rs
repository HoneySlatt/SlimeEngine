//! The editor's look: the fonts and the style made for the interfaces of the engine's editor and
//! of its dev overlay (`docs/adr/0148-widgets.md`), which the `widgets` example draws in today; the
//! overlay moves onto the engine's interface at M6b.8, and the editor gains its window at Stage 6c.
//! It is built only with the feature `editor-look`, so that a game that draws neither carries none
//! of it.
//!
//! Its fonts are Geist, for text, and Geist Mono, for text of fixed width, numbers and code, both
//! variable in their weight, under the SIL Open Font License 1.1, whose file a program that embeds
//! them ships beside it (`docs/adr/0133-font-files-under-the-open-font-license.md`). They hold
//! the Latin script, Vietnamese included, and Cyrillic: the editor falls back on the system's
//! fonts for the other writing systems ([`Fonts::with_system_fonts`]); the overlay, which ships in
//! games, does not.
//!
//! Its style sheet styles labels, buttons, fields and widgets by their kind and their states, boxes
//! by the classes `panel`, `bar` and `window`, labels of the class `muted` in a quieter color, and
//! text in Geist Mono where an element holds the class `fixed`; it leaves boxes of no class and
//! images as their builder gives them. The elements that take the focus show it by the color of a
//! border of one pixel that they always keep, transparent until then for those that show none
//! otherwise, so that the focus moves nothing; the handle between two panels, by its fill. Its one
//! theme, dark, gives the values that the sheet names. Both are documents of the repository,
//! `crates/slime-ui/look/editor.style` and `dark.theme`, built into the program as text: a program
//! puts a sheet of its own after the look's, whose rules then win, and which names the theme's
//! values (`docs/adr/0141-style-sheets-and-themes.md`).
//!
//! ```
//! use slime_ui::look;
//! use slime_ui::text::Fonts;
//! use slime_ui::tree::Ui;
//!
//! let mut fonts = Fonts::new();
//! look::register_fonts(&mut fonts);
//! let mut ui = Ui::new(fonts);
//! ui.set_style(&[look::style_sheet()], &look::theme())
//!     .expect("the look's documents are valid");
//! ```

use crate::text::Fonts;
use crate::theme::{StyleSheet, Theme};

/// The family of the look's text, by which a stack names it.
pub const TEXT_FAMILY: &str = "Geist";
/// The family of the look's text of fixed width.
pub const FIXED_FAMILY: &str = "Geist Mono";

/// Geist 1.7.0 and Geist Mono 1.7.0, their variable fonts of upright faces, as their release gives
/// them (`crates/slime-ui/look/README.md`).
const FONTS: [&[u8]; 2] = [
    include_bytes!("../look/Geist.ttf"),
    include_bytes!("../look/GeistMono.ttf"),
];

/// Registers the look's fonts, Geist and Geist Mono, with `fonts`, which keep them without a copy.
///
/// # Panics
///
/// If the look's files held no font, which its tests rule out.
pub fn register_fonts(fonts: &mut Fonts) {
    for font in FONTS {
        fonts
            .register(font)
            .expect("the look's files hold fonts, which its tests check");
    }
}

/// The look's style sheet, named in errors by its file.
pub fn style_sheet() -> StyleSheet {
    StyleSheet::from_text(
        "crates/slime-ui/look/editor.style",
        include_str!("../look/editor.style"),
    )
}

/// The look's dark theme, the values that its style sheet names, named in errors by its file.
pub fn theme() -> Theme {
    Theme::from_text(
        "crates/slime-ui/look/dark.theme",
        include_str!("../look/dark.theme"),
    )
}
