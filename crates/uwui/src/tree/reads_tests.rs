//! The table of what each kind of element reads of its style, checked against what each kind draws
//! (`docs/adr/0229-styles-typed-as-declarations.md`).
//!
//! [`theme::read_by`] and [`theme::reads`] list, for each kind of [`KindName`], the
//! leaves of a [`Style`] that an element of the kind reads beyond its layout, its paint and its
//! transition: `text` for its text's style and color, then the selection's, the control's and the
//! icons' leaves, and `paint.backdrop`, which only a container that is a layer's root draws. A rule
//! that names a kind and sets a leaf outside its list is refused, so a leaf missing from the table
//! would silently stop a rule from working, and a leaf listed that the kind never draws would let
//! a rule do nothing. This test checks the table against the drawing itself.
//!
//! How it checks, for each kind:
//!
//! 1. The kind is shown in one or more *configurations*: a state, a content or a place in which it
//!    draws something different, such as a check box checked and unchecked, a section open and
//!    folded, a menu item with a submenu or a shortcut, a field that holds text, nothing under a
//!    placeholder, or a selection. Each is drawn in a root laid out from left to right and one
//!    from right to left.
//! 2. The element's own style, and no style sheet, sets every leaf the table talks about to a
//!    visible value, the icons in Lucide's font and glyphs, the text in Geist
//!    ([`base_style`]).
//! 3. For each configuration and each leaf, the configuration is drawn with the base style and
//!    with only that leaf changed to another visible value ([`LEAVES`]). The two display lists
//!    are compared primitive by primitive, a text by its glyphs, their places and its color. If
//!    they differ, the kind *reads* the leaf in that configuration.
//! 4. A kind reads a leaf if it does in at least one of its configurations. The leaves that a kind
//!    reads must be exactly those of [`theme::read_by`] (`text` read when the color, the size or
//!    the weight of the text changes the drawing), and [`theme::reads`], given the declaration of
//!    each leaf, must give the same answer.
//!
//! `paint.backdrop` is checked configuration by configuration: a layer's root ([`Ui::open_layer`]),
//! modal or not, draws it, and every other subject draws nothing of it, the plain box of a
//! container included. A layer's root is always a container, so no other kind draws it.
//!
//! The leaves that every kind reads (a layout, a paint and a transition leaf) are not drawn: only
//! [`theme::reads`] is asked about one of each, for every kind.
//!
//! After the frames of each drawing, the subject's computed style is checked to hold the leaves
//! given to it, so that a kind that reads nothing is shown to have been given them.
//!
//! Adding a kind to [`KindName`] makes this file fail to compile, as [`configurations`] matches
//! every kind, and the test fail until the kind is added to [`KindName::ALL`], as it counts the enum's
//! variants through reflection. A failure names the kind, the leaf and the configuration, with the
//! first primitive that differs. The fonts of the look come with the feature `editor-look`, which
//! the test needs.

use std::collections::BTreeSet;

use slime_color::LinearColor;
use slime_reflect::{Kind as TypeKind, Reflect};

use super::{
    Children, Element, ElementId, ElementState, ItemMark, Kind, Layer, LayoutDirection, Place,
    RootSettings, Ui, Units, kind_name,
};
use crate::display::{DisplayList, ImageKey, Primitive};
use crate::look::{self, FIXED_FAMILY, ICON_FAMILY, TEXT_FAMILY};
use crate::style::{Dimension, FlexDirection, Sides, Style, StyleDeclaration};
use crate::text::Fonts;
use crate::theme::{self, KindName};

pub(super) const DIRECTIONS: [LayoutDirection; 2] =
    [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft];

/// A color given as sRGB's 0xRRGGBB, opaque.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The Lucide glyph named `name`, as a text of one character.
fn glyph(name: &str) -> String {
    look::icon(name)
        .unwrap_or_else(|| panic!("Lucide draws `{name}`"))
        .to_string()
}

/// A leaf of a style that the table talks about, and how to change it to another visible value.
struct Leaf {
    /// The leaf as `StyleDeclaration::path` names it.
    path: &'static str,
    /// What the table calls it: `text` for the text's style and color, else its path.
    group: &'static str,
    /// Changes it, from the value that [`base_style`] gives, to another visible one.
    change: fn(&mut Style),
}

/// The leaves that vary by kind. The text's style stands for nine leaves, of which its fonts, its
/// size and its weight are tried beside its color: any of them changing the drawing counts as
/// `text`.
const LEAVES: [Leaf; 17] = [
    Leaf {
        path: "text.color",
        group: "text",
        change: |style| style.text.color = srgb(0xFF4040),
    },
    Leaf {
        path: "text.style.size",
        group: "text",
        change: |style| style.text.style.size = 20.0,
    },
    Leaf {
        path: "text.style.fonts",
        group: "text",
        change: |style| style.text.style.fonts = vec![FIXED_FAMILY.to_owned()],
    },
    Leaf {
        path: "text.style.weight",
        group: "text",
        change: |style| style.text.style.weight = 700.0,
    },
    Leaf {
        path: "text.selection",
        group: "text.selection",
        change: |style| style.text.selection = srgb(0xCC3355),
    },
    Leaf {
        path: "control.size",
        group: "control.size",
        change: |style| style.control.size = 28.0,
    },
    Leaf {
        path: "control.radius",
        group: "control.radius",
        change: |style| style.control.radius = 8.0,
    },
    Leaf {
        path: "control.track",
        group: "control.track",
        change: |style| style.control.track = srgb(0x66FF99),
    },
    Leaf {
        path: "control.fill",
        group: "control.fill",
        change: |style| style.control.fill = srgb(0xFF8800),
    },
    Leaf {
        path: "control.mark",
        group: "control.mark",
        change: |style| style.control.mark = srgb(0x00CCFF),
    },
    Leaf {
        path: "icons.font",
        group: "icons.font",
        change: |style| style.icons.font = FIXED_FAMILY.to_owned(),
    },
    Leaf {
        path: "icons.tick",
        group: "icons.tick",
        change: |style| style.icons.tick = glyph("plus"),
    },
    Leaf {
        path: "icons.down",
        group: "icons.down",
        change: |style| style.icons.down = glyph("arrow-down"),
    },
    Leaf {
        path: "icons.right",
        group: "icons.right",
        change: |style| style.icons.right = glyph("arrow-right"),
    },
    Leaf {
        path: "icons.left",
        group: "icons.left",
        change: |style| style.icons.left = glyph("arrow-left"),
    },
    Leaf {
        path: "icons.search",
        group: "icons.search",
        change: |style| style.icons.search = glyph("star"),
    },
    Leaf {
        path: "icons.clear",
        group: "icons.clear",
        change: |style| style.icons.clear = glyph("trash"),
    },
];

/// One leaf of each part that every kind reads, which no drawing is compared for: only
/// [`theme::reads`] is asked about it. The backdrop and the text's fonts, the other leaves of
/// those parts, are tried elsewhere.
const ALWAYS_READ: [Leaf; 3] = [
    Leaf {
        path: "layout.grow",
        group: "layout",
        change: |style| style.layout.grow = 2.0,
    },
    Leaf {
        path: "paint.opacity",
        group: "paint",
        change: |style| style.paint.opacity = 0.5,
    },
    Leaf {
        path: "transition.duration",
        group: "transition",
        change: |style| style.transition.duration = 0.25,
    },
];

/// The leaf `paint.backdrop`, tried apart: only a layer's root draws it.
const BACKDROP: Leaf = Leaf {
    path: "paint.backdrop",
    group: "paint.backdrop",
    change: |style| style.paint.backdrop = srgb(0x884400),
};

/// The style whose leaves, those of [`LEAVES`] and the backdrop, all hold a visible value. Elements
/// take it as their own style: no style sheet applies.
pub(super) fn base_style() -> Style {
    let mut style = Style::default();
    style.text.style.fonts = vec![TEXT_FAMILY.to_owned()];
    style.text.style.size = 16.0;
    style.text.color = srgb(0xE6E6E6);
    style.text.selection = srgb(0x3355CC);
    style.control.size = 20.0;
    style.control.radius = 3.0;
    style.control.track = srgb(0x4D4D66);
    style.control.fill = srgb(0x2E8B57);
    style.control.mark = srgb(0xF0C040);
    style.icons.font = ICON_FAMILY.to_owned();
    style.icons.tick = glyph("check");
    style.icons.down = glyph("chevron-down");
    style.icons.right = glyph("chevron-right");
    style.icons.left = glyph("chevron-left");
    style.icons.search = glyph("search");
    style.icons.clear = glyph("x");
    style.paint.backdrop = srgb(0x102030);
    style
}

/// `leaves` as the style of a subject: a box that shows, of `width` by `height` logical pixels.
fn sized(leaves: &Style, width: f32, height: f32) -> Style {
    let mut style = shown(leaves);
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style
}

/// `leaves` as the style of a subject, whose box shows, its size left to its kind.
fn shown(leaves: &Style) -> Style {
    let mut style = leaves.clone();
    style.paint.background = srgb(0x202428);
    style
}

/// A field's style: room at both sides for the marks it draws in its padding.
fn field(leaves: &Style, width: f32, height: f32) -> Style {
    let mut style = sized(leaves, width, height);
    style.layout.padding = Sides::symmetric(48.0, 4.0);
    style
}

/// The style of what holds or accompanies a subject, which no leaf changes: Geist, the other
/// leaves at their defaults.
fn plain() -> Style {
    let mut style = Style::default();
    style.text.style.fonts = vec![TEXT_FAMILY.to_owned()];
    style
}

/// [`plain`], of `width` by `height` logical pixels.
fn plain_sized(width: f32, height: f32) -> Style {
    let mut style = plain();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style
}

/// [`plain`], laid out in a column.
fn column() -> Style {
    let mut style = plain();
    style.layout.direction = FlexDirection::Column;
    style
}

/// How a kind is shown: a name that failures give, and what builds the subject under a root, in
/// the state to draw, with the style given, which holds the leaves to try.
pub(super) struct Configuration {
    pub(super) name: &'static str,
    /// Whether the subject is given the font of icons: a bare one draws the shapes that controls
    /// draw without it.
    pub(super) icons: bool,
    /// Builds the subject, an element of the kind under test, under `root`, and gives it. May
    /// draw frames, open layers and set states.
    pub(super) show: fn(&mut Ui, ElementId, &Style) -> ElementId,
}

fn configuration(
    name: &'static str,
    show: fn(&mut Ui, ElementId, &Style) -> ElementId,
) -> Configuration {
    Configuration {
        name,
        icons: true,
        show,
    }
}

/// A configuration whose subject is given no font of icons.
fn bare(name: &'static str, show: fn(&mut Ui, ElementId, &Style) -> ElementId) -> Configuration {
    Configuration {
        name,
        icons: false,
        show,
    }
}

/// Builds `build` under `root` for `revision`, and gives the id that it returns.
fn built(
    ui: &mut Ui,
    root: ElementId,
    revision: u64,
    build: impl FnOnce(&mut Children<'_>) -> ElementId,
) -> ElementId {
    let mut subject = None;
    ui.build(root, revision, |children| subject = Some(build(children)));
    subject.expect("the builder ran")
}

/// Adds `element` alone under `root`, as the subject.
fn alone(ui: &mut Ui, root: ElementId, element: Element) -> ElementId {
    built(ui, root, 0, |children| {
        children.add("subject", element, |_| {})
    })
}

/// The state of an element, nothing but `pressed`.
fn pressed() -> ElementState {
    ElementState {
        pressed: true,
        ..ElementState::default()
    }
}

/// Selects the first `bytes` of the text of the field `field`, which holds the focus.
fn select_start(ui: &mut Ui, field: ElementId, bytes: usize) {
    ui.frame();
    ui.set_focus(Some(field));
    assert_eq!(ui.focus(), Some(field), "the field takes the focus");
    assert!(ui.select_text(field, 0, bytes), "a field");
}

/// Opens a floating layer over `root`, of the style `style`, and gives its root, which is always a
/// container.
fn floating_layer(ui: &mut Ui, root: ElementId, modal: bool, style: Style) -> ElementId {
    let layer = Layer {
        place: Place::Floating {
            over: root,
            origin: [40.0, 30.0],
            size: [220.0, 140.0],
        },
        modal,
        dismiss: false,
    };
    ui.open_layer("layer", layer, style)
}

/// A label for what a container holds, so that it holds something.
fn label(children: &mut Children<'_>, key: &'static str) -> ElementId {
    children.add(key, Element::label("Held", plain()), |_| {})
}

/// The configurations of `kind`: what it may draw, from a reading of the code that draws it.
pub(super) fn configurations(kind: KindName) -> Vec<Configuration> {
    match kind {
        KindName::Container => vec![
            configuration("a box holding a label", |ui, root, leaves| {
                built(ui, root, 0, |children| {
                    children.add(
                        "subject",
                        Element::container(sized(leaves, 120.0, 60.0)),
                        |held| {
                            label(held, "held");
                        },
                    )
                })
            }),
            configuration("a layer's root", |ui, root, leaves| {
                let layer = floating_layer(ui, root, false, shown(leaves));
                built(ui, layer, 0, |children| label(children, "held"));
                layer
            }),
            configuration("a modal layer's root", |ui, root, leaves| {
                let layer = floating_layer(ui, root, true, shown(leaves));
                built(ui, layer, 0, |children| label(children, "held"));
                layer
            }),
        ],
        KindName::Label => vec![configuration("a label", |ui, root, leaves| {
            alone(ui, root, Element::label("Name", shown(leaves)))
        })],
        KindName::Image => vec![configuration("an image", |ui, root, leaves| {
            alone(
                ui,
                root,
                Element::image(ImageKey(7), sized(leaves, 40.0, 40.0)),
            )
        })],
        KindName::Button => vec![
            configuration("a button", |ui, root, leaves| {
                alone(ui, root, Element::button("Apply", shown(leaves)))
            }),
            configuration("a button pressed", |ui, root, leaves| {
                let id = alone(ui, root, Element::button("Apply", shown(leaves)));
                ui.set_state(id, pressed());
                id
            }),
        ],
        KindName::Scroll => vec![
            configuration("content taller than the box", |ui, root, leaves| {
                scroll(ui, root, leaves, (100.0, 200.0))
            }),
            configuration("content larger both ways", |ui, root, leaves| {
                scroll(ui, root, leaves, (300.0, 200.0))
            }),
            configuration("content that fits", |ui, root, leaves| {
                scroll(ui, root, leaves, (50.0, 30.0))
            }),
        ],
        KindName::List => vec![
            configuration("rows that do not fit", |ui, root, leaves| {
                list(ui, root, leaves, 50)
            }),
            configuration("rows that fit", |ui, root, leaves| {
                list(ui, root, leaves, 2)
            }),
        ],
        KindName::TextField => vec![
            configuration(
                "a field holding nothing under a placeholder",
                |ui, root, leaves| {
                    let element = Element::text_field("", field(leaves, 200.0, 28.0));
                    alone(ui, root, element.placeholder("Name"))
                },
            ),
            configuration("a field holding text", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::text_field("Hello world", field(leaves, 200.0, 28.0)),
                )
            }),
            configuration("a field focused, text selected", |ui, root, leaves| {
                let id = alone(
                    ui,
                    root,
                    Element::text_field("Hello world", field(leaves, 200.0, 28.0)),
                );
                select_start(ui, id, 5);
                id
            }),
            configuration("a text area holding text", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::text_area("Hello\nworld", field(leaves, 200.0, 80.0)),
                )
            }),
            configuration(
                "a field of search holding nothing under a placeholder",
                |ui, root, leaves| {
                    let element = Element::search_field("", field(leaves, 200.0, 28.0));
                    alone(ui, root, element.placeholder("Search"))
                },
            ),
            configuration("a field of search holding text", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::search_field("lamp", field(leaves, 200.0, 28.0)),
                )
            }),
            bare(
                "a field of search holding text, without icons",
                |ui, root, leaves| {
                    let style = field(leaves, 200.0, 28.0);
                    alone(ui, root, Element::search_field("lamp", style))
                },
            ),
            configuration(
                "a field of search focused, text selected",
                |ui, root, leaves| {
                    let id = alone(
                        ui,
                        root,
                        Element::search_field("lamp post", field(leaves, 200.0, 28.0)),
                    );
                    select_start(ui, id, 4);
                    id
                },
            ),
        ],
        KindName::NumberField => vec![
            configuration("a number", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::number_field(1.5, 0.1, field(leaves, 160.0, 28.0)),
                )
            }),
            configuration("a number with a prefix", |ui, root, leaves| {
                let element = Element::number_field(1.5, 0.1, field(leaves, 160.0, 28.0));
                alone(ui, root, element.prefix("X"))
            }),
            configuration("a number with units", |ui, root, leaves| {
                let element = Element::number_field(1.5, 0.1, field(leaves, 160.0, 28.0));
                alone(ui, root, element.units(Units::Length))
            }),
            configuration("a number focused, text selected", |ui, root, leaves| {
                let id = alone(
                    ui,
                    root,
                    Element::number_field(1.5, 0.1, field(leaves, 160.0, 28.0)),
                );
                select_start(ui, id, 2);
                id
            }),
        ],
        KindName::CheckBox => vec![
            configuration("a check box unchecked", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::check_box("Casts shadows", false, sized(leaves, 200.0, 32.0)),
                )
            }),
            configuration("a check box checked", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::check_box("Casts shadows", true, sized(leaves, 200.0, 32.0)),
                )
            }),
            bare("a check box checked, without icons", |ui, root, leaves| {
                let style = sized(leaves, 200.0, 32.0);
                alone(ui, root, Element::check_box("Casts shadows", true, style))
            }),
        ],
        KindName::Slider => vec![configuration("a slider at half", |ui, root, leaves| {
            alone(
                ui,
                root,
                Element::slider(0.5, 0.0, 1.0, 0.0, sized(leaves, 200.0, 32.0)),
            )
        })],
        KindName::Section => vec![
            configuration("a section open", |ui, root, leaves| {
                section(ui, root, leaves, true)
            }),
            configuration("a section folded", |ui, root, leaves| {
                section(ui, root, leaves, false)
            }),
            bare("a section open, without icons", |ui, root, leaves| {
                section(ui, root, leaves, true)
            }),
            bare("a section folded, without icons", |ui, root, leaves| {
                section(ui, root, leaves, false)
            }),
        ],
        KindName::TabBar => vec![configuration("a bar of three tabs", |ui, root, leaves| {
            built(ui, root, 0, |children| {
                children.add(
                    "subject",
                    Element::tab_bar(1, sized(leaves, 240.0, 28.0)),
                    |bar| {
                        for (place, title) in
                            ["General", "Physics", "Audio"].into_iter().enumerate()
                        {
                            bar.add(place, Element::tab(title, plain_sized(80.0, 28.0)), |_| {});
                        }
                    },
                )
            })
        })],
        KindName::Tab => vec![
            configuration("a tab chosen", |ui, root, leaves| {
                tab(
                    ui,
                    root,
                    0,
                    Element::tab("Physics", sized(leaves, 80.0, 28.0)),
                )
            }),
            configuration("a tab not chosen", |ui, root, leaves| {
                tab(
                    ui,
                    root,
                    1,
                    Element::tab("Physics", sized(leaves, 80.0, 28.0)),
                )
            }),
            configuration("a tab chosen with a count", |ui, root, leaves| {
                let element = Element::counted_tab("Physics", 12, sized(leaves, 120.0, 28.0));
                tab(ui, root, 0, element)
            }),
            configuration("a tab not chosen with a count", |ui, root, leaves| {
                let element = Element::counted_tab("Physics", 12, sized(leaves, 120.0, 28.0));
                tab(ui, root, 1, element)
            }),
        ],
        KindName::Split => vec![
            configuration("two panels side by side", |ui, root, leaves| {
                split(ui, root, leaves, true)
            }),
            configuration("two panels one above the other", |ui, root, leaves| {
                split(ui, root, leaves, false)
            }),
            configuration("two panels, the handle pressed", |ui, root, leaves| {
                let id = split(ui, root, leaves, true);
                ui.set_state(id, pressed());
                id
            }),
            configuration("two panels, the handle focused", |ui, root, leaves| {
                let id = split(ui, root, leaves, true);
                ui.frame();
                ui.set_focus(Some(id));
                assert_eq!(ui.focus(), Some(id), "the split takes the focus");
                id
            }),
        ],
        KindName::Splitter => vec![
            configuration("a splitter between two children", |ui, root, leaves| {
                splitter(ui, root, leaves)
            }),
            configuration("a splitter pressed", |ui, root, leaves| {
                let id = splitter(ui, root, leaves);
                ui.set_state(id, pressed());
                id
            }),
            configuration("a splitter focused", |ui, root, leaves| {
                let id = splitter(ui, root, leaves);
                ui.frame();
                ui.set_focus(Some(id));
                assert_eq!(ui.focus(), Some(id), "the splitter takes the focus");
                id
            }),
        ],
        KindName::Menu => vec![configuration("a menu of two items", |ui, root, leaves| {
            built(ui, root, 0, |children| {
                let mut style = sized(leaves, 220.0, 64.0);
                style.layout.direction = FlexDirection::Column;
                children.add("subject", Element::menu(style), |menu| {
                    for key in ["new", "open"] {
                        let item = Element::menu_item(key, "", ItemMark::None, column());
                        menu.add(key, item, |_| {});
                    }
                })
            })
        })],
        KindName::MenuBar => vec![configuration("a bar of two menus", |ui, root, leaves| {
            built(ui, root, 0, |children| {
                let style = sized(leaves, 240.0, 28.0);
                children.add("subject", Element::menu_bar(style), |bar| {
                    for key in ["File", "Edit"] {
                        let item = Element::menu_item(key, "", ItemMark::None, plain());
                        bar.add(key, item, |_| {});
                    }
                })
            })
        })],
        KindName::MenuItem => vec![
            configuration("an item", |ui, root, leaves| {
                menu_item(ui, root, leaves, "", ItemMark::None)
            }),
            configuration("an item with a shortcut", |ui, root, leaves| {
                menu_item(ui, root, leaves, "Ctrl+S", ItemMark::None)
            }),
            configuration("an item checked", |ui, root, leaves| {
                menu_item(ui, root, leaves, "", ItemMark::Check(true))
            }),
            configuration("an item unchecked", |ui, root, leaves| {
                menu_item(ui, root, leaves, "", ItemMark::Check(false))
            }),
            configuration("an item checked with a shortcut", |ui, root, leaves| {
                menu_item(ui, root, leaves, "Ctrl+G", ItemMark::Check(true))
            }),
            configuration("an item with a submenu", |ui, root, leaves| {
                menu_item(ui, root, leaves, "", ItemMark::Submenu)
            }),
            configuration(
                "an item with a submenu and a shortcut",
                |ui, root, leaves| menu_item(ui, root, leaves, "Ctrl+R", ItemMark::Submenu),
            ),
            bare("an item checked, without icons", |ui, root, leaves| {
                menu_item(ui, root, leaves, "", ItemMark::Check(true))
            }),
            bare(
                "an item with a submenu, without icons",
                |ui, root, leaves| menu_item(ui, root, leaves, "", ItemMark::Submenu),
            ),
            configuration("an item of a bar of menus", |ui, root, leaves| {
                let mut id = None;
                built(ui, root, 0, |children| {
                    children.add("bar", Element::menu_bar(plain_sized(240.0, 28.0)), |bar| {
                        let item = Element::menu_item(
                            "File",
                            "",
                            ItemMark::None,
                            sized(leaves, 80.0, 28.0),
                        );
                        id = Some(bar.add("subject", item, |_| {}));
                    })
                });
                id.expect("built")
            }),
        ],
        KindName::Choice => vec![
            configuration("a list to choose from", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::choice(["Low", "High"], 1, sized(leaves, 160.0, 32.0)),
                )
            }),
            bare(
                "a list to choose from, without icons",
                |ui, root, leaves| {
                    let style = sized(leaves, 160.0, 32.0);
                    alone(ui, root, Element::choice(["Low", "High"], 1, style))
                },
            ),
            configuration("a list to choose from, pressed", |ui, root, leaves| {
                let id = alone(
                    ui,
                    root,
                    Element::choice(["Low", "High"], 0, sized(leaves, 160.0, 32.0)),
                );
                ui.set_state(id, pressed());
                id
            }),
        ],
        KindName::FloatingPanel => vec![
            configuration("a floating panel open", |ui, root, leaves| {
                floating_panel(ui, root, leaves, true)
            }),
            configuration("a floating panel folded", |ui, root, leaves| {
                floating_panel(ui, root, leaves, false)
            }),
            bare(
                "a floating panel open, without icons",
                |ui, root, leaves| floating_panel(ui, root, leaves, true),
            ),
            bare(
                "a floating panel folded, without icons",
                |ui, root, leaves| floating_panel(ui, root, leaves, false),
            ),
        ],
        KindName::SectionHeader => vec![configuration(
            "the box of a section's header holding a button",
            |ui, root, leaves| {
                let mut id = None;
                built(ui, root, 0, |children| {
                    children.add(
                        "section",
                        Element::section("Light", true, plain_sized(240.0, 100.0)),
                        |section| {
                            let header = Element::section_header(shown(leaves));
                            id = Some(section.add("subject", header, |slot| {
                                let reset = Element::button("Reset", plain_sized(40.0, 20.0));
                                slot.add("reset", reset, |_| {});
                            }));
                            label(section, "body");
                        },
                    )
                });
                id.expect("built")
            },
        )],
        KindName::ToggleButton => vec![
            configuration("a toggle button on", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::toggle_button("Snap", true, shown(leaves)),
                )
            }),
            configuration("a toggle button off", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::toggle_button("Snap", false, shown(leaves)),
                )
            }),
        ],
        KindName::Segments => vec![configuration(
            "a control of two segments",
            |ui, root, leaves| {
                built(ui, root, 0, |children| {
                    children.add(
                        "subject",
                        Element::segments(0, sized(leaves, 200.0, 28.0)),
                        |segments| {
                            for (place, title) in ["Move", "Turn"].into_iter().enumerate() {
                                let segment = Element::segment(title, plain_sized(100.0, 28.0));
                                segments.add(place, segment, |_| {});
                            }
                        },
                    )
                })
            },
        )],
        KindName::Segment => vec![
            configuration("a segment chosen", |ui, root, leaves| {
                segment(ui, root, leaves, 0)
            }),
            configuration("a segment not chosen", |ui, root, leaves| {
                segment(ui, root, leaves, 1)
            }),
        ],
        KindName::Progress => vec![
            configuration("a bar of progress with a share", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::progress(Some(0.4), sized(leaves, 200.0, 24.0)),
                )
            }),
            configuration(
                "a bar of progress with a share of nothing",
                |ui, root, leaves| {
                    alone(
                        ui,
                        root,
                        Element::progress(Some(0.0), sized(leaves, 200.0, 24.0)),
                    )
                },
            ),
            configuration("a bar of progress without a share", |ui, root, leaves| {
                alone(
                    ui,
                    root,
                    Element::progress(None, sized(leaves, 200.0, 24.0)),
                )
            }),
        ],
        KindName::ColorWell => vec![
            configuration("a color well of an opaque color", |ui, root, leaves| {
                let color = LinearColor::rgba(0.8, 0.3, 0.1, 1.0);
                alone(ui, root, Element::color_well(color, shown(leaves)))
            }),
            configuration("a color well of a translucent color", |ui, root, leaves| {
                let color = LinearColor::rgba(0.8, 0.3, 0.1, 0.5);
                alone(ui, root, Element::color_well(color, shown(leaves)))
            }),
            configuration("a color well with its picker open", |ui, root, leaves| {
                let color = LinearColor::rgba(0.8, 0.3, 0.1, 1.0);
                let id = alone(ui, root, Element::color_well(color, shown(leaves)));
                ui.frame();
                ui.press_well(id);
                id
            }),
        ],
        KindName::ColorPart => vec![
            configuration("the square", |ui, root, leaves| {
                color_part(ui, root, leaves, 0, false)
            }),
            configuration("the square, focus visible", |ui, root, leaves| {
                color_part(ui, root, leaves, 0, true)
            }),
            configuration("the strip of hue", |ui, root, leaves| {
                color_part(ui, root, leaves, 1, false)
            }),
            configuration("the strip of hue, focus visible", |ui, root, leaves| {
                color_part(ui, root, leaves, 1, true)
            }),
            configuration("the strip of alpha", |ui, root, leaves| {
                color_part(ui, root, leaves, 2, false)
            }),
            configuration("the strip of alpha, focus visible", |ui, root, leaves| {
                color_part(ui, root, leaves, 2, true)
            }),
        ],
        KindName::Surface => vec![configuration("a surface", |ui, root, leaves| {
            alone(
                ui,
                root,
                Element::of(
                    Kind::Surface { image: ImageKey(9) },
                    sized(leaves, 60.0, 40.0),
                ),
            )
        })],
    }
}

/// A box that scrolls, `content` logical pixels large inside it.
fn scroll(ui: &mut Ui, root: ElementId, leaves: &Style, content: (f32, f32)) -> ElementId {
    built(ui, root, 0, |children| {
        children.add(
            "subject",
            Element::scroll(sized(leaves, 100.0, 60.0)),
            |scroll| {
                scroll.add(
                    "content",
                    Element::container(plain_sized(content.0, content.1)),
                    |_| {},
                );
            },
        )
    })
}

/// A list of `rows` rows of 20 logical pixels, in a box of 100 by 60. Its first frame lays it out;
/// the next builds the rows that it shows.
fn list(ui: &mut Ui, root: ElementId, leaves: &Style, rows: usize) -> ElementId {
    let mut subject = None;
    for revision in 0..2 {
        subject = Some(built(ui, root, revision, |children| {
            children.add(
                "subject",
                Element::list(rows, 20.0, sized(leaves, 100.0, 60.0)),
                |list| {
                    for row in list.rows() {
                        list.add(row, Element::container(plain_sized(100.0, 20.0)), |_| {});
                    }
                },
            )
        }));
        ui.frame();
    }
    subject.expect("built")
}

/// A section titled `Transform`, open or folded, holding a label.
fn section(ui: &mut Ui, root: ElementId, leaves: &Style, open: bool) -> ElementId {
    built(ui, root, 0, |children| {
        let mut style = shown(leaves);
        style.layout.size = [Dimension::Pixels(240.0), Dimension::Auto];
        children.add(
            "subject",
            Element::section("Transform", open, style),
            |section| {
                label(section, "body");
            },
        )
    })
}

/// A floating panel titled `Color`, open or folded, in a floating layer, holding a label.
fn floating_panel(ui: &mut Ui, root: ElementId, leaves: &Style, open: bool) -> ElementId {
    let layer = floating_layer(ui, root, false, Style::default());
    built(ui, layer, 0, |children| {
        let panel = Element::floating_panel("Color", open, shown(leaves));
        children.add("subject", panel, |panel| {
            label(panel, "body");
        })
    })
}

/// `subject` among three tabs of a bar, the program choosing the tab at `chosen`.
fn tab(ui: &mut Ui, root: ElementId, chosen: usize, subject: Element) -> ElementId {
    let mut id = None;
    built(ui, root, 0, |children| {
        children.add(
            "bar",
            Element::tab_bar(chosen, plain_sized(320.0, 28.0)),
            |bar| {
                id = Some(bar.add(0_usize, subject, |_| {}));
                for place in 1_usize..3 {
                    bar.add(
                        place,
                        Element::tab("Other", plain_sized(80.0, 28.0)),
                        |_| {},
                    );
                }
            },
        )
    });
    id.expect("built")
}

/// A split of two panels, of 300 by 100 logical pixels.
fn split(ui: &mut Ui, root: ElementId, leaves: &Style, side_by_side: bool) -> ElementId {
    built(ui, root, 0, |children| {
        children.add(
            "subject",
            Element::split(side_by_side, 0.4, sized(leaves, 300.0, 100.0)),
            |panels| {
                for key in ["first", "second"] {
                    panels.add(key, Element::container(plain()), |held| {
                        label(held, "held");
                    });
                }
            },
        )
    })
}

/// A splitter between two children of a box of 300 by 100 logical pixels, 8 wide.
fn splitter(ui: &mut Ui, root: ElementId, leaves: &Style) -> ElementId {
    let mut id = None;
    built(ui, root, 0, |children| {
        children.add(
            "box",
            Element::container(plain_sized(300.0, 100.0)),
            |held| {
                held.add(
                    "first",
                    Element::container(plain_sized(100.0, 100.0)),
                    |_| {},
                );
                id = Some(held.add(
                    "subject",
                    Element::splitter(true, 0.4, sized(leaves, 8.0, 100.0)),
                    |_| {},
                ));
                held.add(
                    "second",
                    Element::container(plain_sized(100.0, 100.0)),
                    |_| {},
                );
            },
        )
    });
    id.expect("built")
}

/// An item of a menu, of a label `Item`, whose `shortcut` and `mark` are given.
fn menu_item(
    ui: &mut Ui,
    root: ElementId,
    leaves: &Style,
    shortcut: &str,
    mark: ItemMark,
) -> ElementId {
    let mut id = None;
    built(ui, root, 0, |children| {
        children.add("menu", Element::menu(column()), |menu| {
            id = Some(menu.add(
                "subject",
                Element::menu_item("Item", shortcut, mark, sized(leaves, 220.0, 28.0)),
                |_| {},
            ));
        })
    });
    id.expect("built")
}

/// A segment among two of a control, the program choosing the one at `chosen`.
fn segment(ui: &mut Ui, root: ElementId, leaves: &Style, chosen: usize) -> ElementId {
    let mut id = None;
    built(ui, root, 0, |children| {
        children.add(
            "segments",
            Element::segments(chosen, plain_sized(200.0, 28.0)),
            |segments| {
                id = Some(segments.add(
                    0_usize,
                    Element::segment("Move", sized(leaves, 100.0, 28.0)),
                    |_| {},
                ));
                segments.add(
                    1_usize,
                    Element::segment("Turn", plain_sized(100.0, 28.0)),
                    |_| {},
                );
            },
        )
    });
    id.expect("built")
}

/// The part `index` of the picker of a color well, the picker open: 0 the square, 1 the strip of
/// hue, 2 that of alpha. The part's own style, which the picker makes, is the one that the leaves
/// are laid over, and `focused` gives it a focus that shows.
fn color_part(
    ui: &mut Ui,
    root: ElementId,
    leaves: &Style,
    index: usize,
    focused: bool,
) -> ElementId {
    let well = alone(
        ui,
        root,
        Element::color_well(
            LinearColor::rgba(0.8, 0.3, 0.1, 0.7),
            plain_sized(60.0, 24.0),
        ),
    );
    ui.frame();
    ui.press_well(well);
    ui.frame();
    let picker = ui.owned_layer(well).expect("the picker is open");
    // The picker's layer holds one box, whose color parts are, in order, the square, the strip of
    // hue and the strip of alpha.
    let parts: Vec<ElementId> = ui
        .children(ui.children(picker)[0])
        .iter()
        .copied()
        .filter(|&child| matches!(ui.element(child).map(|e| &e.kind), Some(Kind::ColorPart(_))))
        .collect();
    let part = *parts.get(index).unwrap_or_else(|| {
        panic!(
            "the picker holds a color part {index}, but only {} of them",
            parts.len()
        )
    });
    let node = ui.node_mut(part).expect("a part");
    let style = &mut node.element.style;
    style.text = leaves.text.clone();
    style.control = leaves.control;
    style.icons = leaves.icons.clone();
    style.paint.backdrop = leaves.paint.backdrop;
    ui.mark_style(part);
    ui.set_focus(focused.then_some(part));
    part
}

/// What a display list draws, one text per primitive: a text by its characters, glyphs, places and
/// color, any other primitive by its fields, and each with its clip, transform and layer.
fn describe(list: &DisplayList) -> Vec<String> {
    list.items()
        .iter()
        .map(|item| {
            let what = match &item.primitive {
                Primitive::Text(text) => format!(
                    "text {:?} at {:?} in {:?}, lines {:?}, runs {:?}",
                    text.layout.shown(),
                    text.origin,
                    text.color,
                    text.layout.lines(),
                    text.layout.runs(),
                ),
                other => format!("{other:?}"),
            };
            format!(
                "{what}, clip {:?}, transform {:?}, layer {:?}",
                item.clip, item.transform, item.layer
            )
        })
        .collect()
}

/// What a configuration drew: the display list, described, and whether the subject was a layer's
/// root.
struct Drawing {
    items: Vec<String>,
    layer_root: bool,
}

/// What `configuration` draws in a root laid out as `direction` says, its subject's style
/// `leaves`: the subject checked to be of `kind`; two frames, so that rows laid out by the first
/// are drawn by the second; then the subject's computed style checked to hold the leaves given.
fn draw(
    kind: KindName,
    configuration: &Configuration,
    direction: LayoutDirection,
    leaves: &Style,
) -> Drawing {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    let root = ui.add_root(
        "main",
        RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        },
        Style::default(),
    );
    // A bare configuration has no font of icons: the controls draw the shapes that they draw
    // without one, a mark's square or bars, a chevron's bar, a magnifier's ring.
    let mut given = leaves.clone();
    if !configuration.icons {
        given.icons.font.clear();
    }
    let leaves = &given;
    let subject = (configuration.show)(&mut ui, root, leaves);
    let shown = ui.element(subject).map(|element| kind_name(&element.kind));
    assert_eq!(
        shown,
        Some(kind),
        "{kind:?}, {}: the subject is of another kind",
        configuration.name
    );
    let layer_root = ui.layers().contains(&subject);
    ui.frame();
    let (list, _) = ui.frame();
    // A kind that reads none of these leaves draws the same whatever they hold: the drawings
    // compared only mean something if the subject was given them.
    let computed = &ui.node(subject).expect("the subject is still there").style;
    let name = configuration.name;
    assert_eq!(
        computed.text, leaves.text,
        "{kind:?}, {name}: computed text"
    );
    assert_eq!(
        computed.control, leaves.control,
        "{kind:?}, {name}: computed control"
    );
    assert_eq!(
        computed.icons, leaves.icons,
        "{kind:?}, {name}: computed icons"
    );
    assert_eq!(
        computed.paint.backdrop, leaves.paint.backdrop,
        "{kind:?}, {name}: computed backdrop"
    );
    Drawing {
        items: describe(&list),
        layer_root,
    }
}

/// A line telling where two drawings first differ.
fn first_difference(before: &[String], after: &[String]) -> String {
    let at = before
        .iter()
        .zip(after)
        .position(|(before, after)| before != after)
        .unwrap_or(before.len().min(after.len()));
    let cut = |text: Option<&String>| {
        text.map_or_else(
            || "nothing".to_owned(),
            |text| text.chars().take(160).collect::<String>(),
        )
    };
    format!(
        "{} primitives became {}; the first that differs, at position {} (from 1): {} became {}",
        before.len(),
        after.len(),
        at + 1,
        cut(before.get(at)),
        cut(after.get(at)),
    )
}

/// The declaration that changing `leaf` of `base` makes, which the table's `reads` answers for.
fn declaration(base: &Style, leaf: &Leaf) -> StyleDeclaration {
    let mut changed = base.clone();
    (leaf.change)(&mut changed);
    let mut declarations = Vec::new();
    changed.declared(base, &mut |declaration| declarations.push(declaration));
    assert_eq!(declarations.len(), 1, "{}: one declaration", leaf.path);
    let declaration = declarations.remove(0);
    assert_eq!(declaration.path(), leaf.path, "the leaf's path");
    declaration
}

/// A configuration, in a direction, that was drawn.
struct Tried {
    /// The configuration and the direction, as failures name them.
    name: String,
    /// Whether the subject was a layer's root.
    layer_root: bool,
}

/// A leaf that changed a drawing.
struct Observation {
    group: &'static str,
    path: &'static str,
    /// The `name` of what was drawn.
    tried: String,
    /// The leaf, where it was drawn and the first difference, as a failure tells it.
    place: String,
}

/// Every leaf of [`LEAVES`] and [`BACKDROP`] tried in every configuration of `kind`, in both
/// directions: the drawings tried, and the leaves that changed one.
fn observe(kind: KindName, base: &Style) -> (Vec<Tried>, Vec<Observation>) {
    let configurations = configurations(kind);
    assert!(!configurations.is_empty(), "{kind:?} has a configuration");
    let mut tried = Vec::new();
    let mut observations = Vec::new();
    for configuration in &configurations {
        for direction in DIRECTIONS {
            let name = format!("{}, {direction:?}", configuration.name);
            let before = draw(kind, configuration, direction, base);
            // The drawing repeats: a difference below comes from the leaf alone.
            assert_eq!(
                before.items,
                draw(kind, configuration, direction, base).items,
                "{kind:?}, {name}: the same drawing twice"
            );
            assert!(
                !before.items.is_empty(),
                "{kind:?}, {name}: draws something"
            );
            for leaf in LEAVES.iter().chain([&BACKDROP]) {
                let mut changed = base.clone();
                (leaf.change)(&mut changed);
                let after = draw(kind, configuration, direction, &changed);
                if after.items != before.items {
                    observations.push(Observation {
                        group: leaf.group,
                        path: leaf.path,
                        tried: name.clone(),
                        place: format!(
                            "{} in `{name}` ({})",
                            leaf.path,
                            first_difference(&before.items, &after.items)
                        ),
                    });
                }
            }
            tried.push(Tried {
                name,
                layer_root: before.layer_root,
            });
        }
    }
    // Shown by `cargo test -- --nocapture`: what each kind was seen to read.
    let paths: BTreeSet<&str> = observations.iter().map(|seen| seen.path).collect();
    println!("{kind:?} reads {paths:?} over {} drawings", tried.len());
    (tried, observations)
}

#[test]
fn every_kind_draws_the_leaves_that_the_table_says_it_reads() {
    let variants = match KindName::type_info().kind {
        TypeKind::Enum(variants) => variants.len(),
        other => panic!("KindName is an enum, not {other:?}"),
    };
    assert_eq!(
        KindName::ALL.len(),
        variants,
        "KindName has {variants} kinds, and KindName::ALL lists {}: add the kind to it and to \
         `configurations`",
        KindName::ALL.len()
    );
    for (place, kind) in KindName::ALL.iter().enumerate() {
        assert_eq!(
            kind.index(),
            place,
            "{kind:?} lies in KindName::ALL in the enum's order"
        );
    }
    let base = base_style();
    // Every leaf that the table can name, by group.
    let groups = LEAVES
        .iter()
        .chain([&BACKDROP])
        .map(|leaf| leaf.group)
        .collect::<BTreeSet<_>>();
    let mut problems = Vec::new();
    for kind in KindName::ALL {
        let (tried, observations) = observe(kind, &base);
        for &group in &groups {
            let drawn: Vec<&Observation> = observations
                .iter()
                .filter(|seen| seen.group == group)
                .collect();
            // What `reads` is compared with, and where it comes from.
            let (listed, source) = if group == BACKDROP.group {
                // Drawn exactly where the subject is a layer's root, whatever the kind.
                for tried in &tried {
                    let is_drawn = drawn.iter().any(|seen| seen.tried == tried.name);
                    if is_drawn != tried.layer_root {
                        problems.push(format!(
                            "{kind:?}, {}: the subject {} a layer's root, and `{group}` {} the \
                             drawing",
                            tried.name,
                            if tried.layer_root { "is" } else { "is not" },
                            if is_drawn {
                                "changes"
                            } else {
                                "leaves unchanged"
                            },
                        ));
                    }
                }
                (!drawn.is_empty(), "the drawing shows")
            } else {
                let listed = theme::read_by(kind).iter().any(|(read, _)| *read == group);
                if listed && drawn.is_empty() {
                    let names: Vec<&str> = tried.iter().map(|tried| tried.name.as_str()).collect();
                    problems.push(format!(
                        "{kind:?} is said to read `{group}`, but changing it changed no drawing, \
                         in any of: {}",
                        names.join("; ")
                    ));
                }
                if !listed && let Some(seen) = drawn.first() {
                    problems.push(format!(
                        "{kind:?} is said not to read `{group}`, but it draws it: {}",
                        seen.place
                    ));
                }
                (listed, "the table's list says")
            };
            // The table's other entry point, which a rule is checked by.
            for leaf in LEAVES
                .iter()
                .chain([&BACKDROP])
                .filter(|leaf| leaf.group == group)
            {
                if theme::reads(kind, &declaration(&base, leaf)) != listed {
                    problems.push(format!(
                        "{kind:?}: `reads` says {} for `{}`, where {source} {listed}",
                        !listed, leaf.path
                    ));
                }
            }
        }
        for leaf in &ALWAYS_READ {
            if !theme::reads(kind, &declaration(&base, leaf)) {
                problems.push(format!(
                    "{kind:?}: `reads` says false for `{}`, which every kind reads",
                    leaf.path
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
