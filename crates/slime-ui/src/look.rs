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
//! images as their builder gives them. Its digits are of one width, so that numbers line up, and its
//! changes move over a short time, a section opening over a longer one. The elements that take the
//! focus show it to the keyboard by the color of a border of one pixel that they always keep,
//! transparent until then for those that show none otherwise, so that the focus moves nothing; a
//! field shows it however it was given; the handle between two panels, by its fill.
//!
//! Its themes stack ([`themes`]): a base theme of the scales that every theme shares, spacings on a
//! grid of 4 pixels, radii, sizes of text and durations; a theme of colors, dark, light or high
//! contrast ([`Colors`]), a palette and the roles that the sheet names, computed from it; and, for a
//! denser layout, a compact theme ([`Density`]). A program stacks a theme of its own after them: one
//! that gives `accent` changes every color that follows the accent. All are documents of the
//! repository, in `crates/slime-ui/look/`, built into the program as text; a program puts a sheet of
//! its own after the look's, whose rules then win, and which names the themes' values
//! (`docs/adr/0141-style-sheets-and-themes.md`, `docs/adr/0149-the-looks-foundations.md`).
//!
//! ```
//! use slime_ui::look::{self, Colors, Density};
//! use slime_ui::text::Fonts;
//! use slime_ui::theme::Theme;
//! use slime_ui::tree::Ui;
//!
//! let mut fonts = Fonts::new();
//! look::register_fonts(&mut fonts);
//! let mut ui = Ui::new(fonts);
//! let mut themes = look::themes(Colors::Dark, Density::Comfortable);
//! // A project's accent, which the hovers, the soft fills and the focus follow.
//! themes.push(Theme::from_text("project.theme", "accent = \"#2BB673\"\n"));
//! ui.set_style(&[look::style_sheet()], &themes)
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

/// The look's themes of colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Colors {
    /// Light text on dark surfaces, the look's own.
    #[default]
    Dark,
    /// Dark text on light surfaces.
    Light,
    /// White text on black surfaces, every surface bordered.
    HighContrast,
}

/// How much the look lays out on a screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    /// The look's own spacing.
    #[default]
    Comfortable,
    /// Smaller text, controls and padding.
    Compact,
}

/// The look's themes in the order that they stack: its base, its theme of `colors`, and its compact
/// theme for [`Density::Compact`], each named in errors by its file.
pub fn themes(colors: Colors, density: Density) -> Vec<Theme> {
    let theme =
        |name: &str, text: &str| Theme::from_text(format!("crates/slime-ui/look/{name}"), text);
    let mut themes = vec![
        theme("base.theme", include_str!("../look/base.theme")),
        match colors {
            Colors::Dark => theme("dark.theme", include_str!("../look/dark.theme")),
            Colors::Light => theme("light.theme", include_str!("../look/light.theme")),
            Colors::HighContrast => theme("contrast.theme", include_str!("../look/contrast.theme")),
        },
    ];
    if density == Density::Compact {
        themes.push(theme(
            "compact.theme",
            include_str!("../look/compact.theme"),
        ));
    }
    themes
}
