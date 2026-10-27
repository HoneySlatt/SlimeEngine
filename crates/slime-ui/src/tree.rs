//! The tree of elements: an interface that lasts from one frame to the next, built by code when
//! what it shows changes, laid out by flexbox and grid, and drawn as a display list
//! (`docs/adr/0132-the-engines-ui-framework.md`, `docs/adr/0140-the-tree-of-elements.md`).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use slime_color::LinearColor;
use slime_reflect::{Reflect, Registry, Value};

use crate::DocumentError;
use crate::Rect;
use crate::display::{DisplayList, ImageKey, ImageKind, Primitive};
use crate::layout;
use crate::paint;
use crate::style::{Display, Style};
use crate::text::{Fonts, TEXT_CACHE_BUDGET, TextCache};
use crate::theme::{self, KindName, Rule, StateName, StyleSheet, Theme};

mod choice;
mod control;
mod field;
mod interact;
mod layer;
mod meaning;
mod menu;
mod motion;
mod panel;
mod scroll_bar;
mod tooltip;

pub use field::Clipboard;
use interact::Interaction;
pub use layer::{Along, Layer, Place, Side};
pub use meaning::{
    ElementAction, FieldText, Meaning, MeaningChanges, MeaningNode, RUN_CHARACTERS, Role,
    RunCharacter, TextRun, ValueRange,
};

/// An element of a [`Ui`]: the place it holds in the tree's arena, and the generation of that
/// place, so that the id of an element removed names no other that takes its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId {
    index: u32,
    generation: u32,
}

impl ElementId {
    /// Its place in the arena, which taffy names it by.
    pub(crate) fn index(self) -> usize {
        self.index as usize
    }
}

/// What names an element among its siblings, which whoever builds it gives: a name, or a number,
/// as the rows of a list are named by theirs. Keys are unique among siblings; the keys from a root
/// down to an element are its path, which names it for agents, tests and saved layouts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A name.
    Name(Arc<str>),
    /// A number.
    Index(u64),
}

impl From<&str> for Key {
    fn from(name: &str) -> Self {
        Self::Name(name.into())
    }
}

impl From<String> for Key {
    fn from(name: String) -> Self {
        Self::Name(name.into())
    }
}

impl From<u64> for Key {
    fn from(index: u64) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for Key {
    fn from(index: usize) -> Self {
        Self::Index(index as u64)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => formatter.write_str(name),
            Self::Index(index) => write!(formatter, "#{index}"),
        }
    }
}

/// What an element is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A box that holds others, laid out as its style says.
    Container,
    /// A text, laid out in the element's box, less its padding, from its top: the box is as
    /// large as the text unless its style or its parent says otherwise, and lines break at its
    /// width.
    Label(String),
    /// A part of an image, drawn into the element's box, less its padding.
    Image {
        /// The image, which the program that draws the display list gives.
        image: ImageKey,
        /// The part drawn, in fractions of the image's width and height.
        part: Rect,
        /// How its texels are drawn.
        kind: ImageKind,
        /// The color of a coverage, or, of an image of colors, its alpha alone.
        color: LinearColor,
    },
    /// A button: its label, in the middle of its box from top to bottom, its lines aligned as its
    /// text's style says. Rules style its states ([`ElementState`]).
    Button {
        /// Its label.
        label: String,
    },
    /// A box whose children scroll inside it, cut at its edges.
    Scroll,
    /// A list of rows of one height, of which only those that show are built: its builder asks
    /// [`Children::rows`] which, and adds each under its number, a [`Key::Index`]. Each row is
    /// laid out alone, at the list's width and the height of a row, and placed by its number. When
    /// other rows show, the builder that gives the rows runs again: the list's own, for a list in
    /// a slot ([`Children::add_slot`]), which then costs what it shows, or the one that made it. A
    /// new list shows its rows from the frame after it is first laid out, when that builder runs
    /// again; so does a list that scrolls to other rows, unless it scrolls before [`Ui::build`].
    List {
        /// How many rows it has.
        rows: usize,
        /// The height of each, in logical pixels.
        row_height: f32,
    },
    /// A field of text, which the user types in, selects, copies and composes through an input
    /// method, its text laid out as a label's: one line, which scrolls to keep the caret in view,
    /// or several, broken at its width. The field keeps what the user types; the program reads it
    /// as actions ([`ActionKind::Changed`](crate::action::ActionKind::Changed),
    /// [`ActionKind::Committed`](crate::action::ActionKind::Committed)).
    TextField {
        /// Its text, as the program gives it: it replaces what the field holds only when it
        /// differs from the later of the text that the program gave before and the one that the
        /// field last committed, and from what the field holds. Building the field again keeps
        /// what the user typed, and so does giving back each change or commit; giving the text
        /// given before, an empty one after a message sent, or the one that the program had for
        /// a text that it refuses, replaces what was committed.
        text: String,
        /// Whether it holds several lines, Enter typing a line break.
        multiline: bool,
        /// Whether it is a field of search (`docs/adr/0151-rich-widgets.md`): a magnifier before
        /// its text and, once it holds something, a mark after it that a press empties it by, as a
        /// person who deleted its text and committed would. A field of several lines is none.
        search: bool,
    },
    /// A field of a number, which shows the number whole, as Rust writes it, and which the user
    /// types in, or drags left and right. As a field of text, it keeps what the user types, and
    /// the program reads it as actions, a number.
    NumberField {
        /// Its number, as the program gives it, under the same rule as a field of text's text.
        value: f64,
        /// How much a drag of one logical pixel adds to it; a drag gives multiples of it.
        step: f64,
        /// A short text before its number, `X`, in its control's mark color, which the pointer
        /// drags to change the number whether or not the field holds the focus
        /// (`docs/adr/0151-rich-widgets.md`); empty for none.
        prefix: String,
    },
    /// A check box: its box, checked or not, then its label, in the middle of its box from top to
    /// bottom. A press, or Enter or Space while it holds the focus, toggles it, which the program
    /// reads as actions ([`FieldValue::Checked`](crate::action::FieldValue::Checked)). It keeps
    /// what the user toggled; the program's state replaces it when it differs from the state that
    /// the program gave before. Rules select it checked by the state `Checked`.
    CheckBox {
        /// Its label.
        label: String,
        /// Whether it is checked, as the program gives it.
        checked: bool,
    },
    /// A slider: a track from its least value to its most, the part up to its value filled, and a
    /// thumb at its value, which the pointer drags, or presses elsewhere on the track, and the
    /// arrows, Home, End, Page Up and Page Down move while it holds the focus, by multiples of its
    /// step from its least value. It keeps the user's value, as a field of numbers does, which the
    /// program reads as actions, a number.
    Slider {
        /// Its value, as the program gives it, under the same rule as a check box's state; kept
        /// within its least and most values.
        value: f64,
        /// Its least value.
        min: f64,
        /// Its most value, at least its least.
        max: f64,
        /// The values it takes are its least value and multiples of this from it; 0 for any.
        step: f64,
    },
    /// A section that folds: a header, its title after a mark that shows whether it is open,
    /// then, while it is open, its children, laid out as a container's below the header. A press
    /// on the header, or Enter or Space while it holds the focus, opens or folds it; what it holds
    /// is then not laid out, drawn nor reached. It keeps whether the user opened it, as a check box
    /// keeps its state, and rules select it open by the state `Expanded`.
    Section {
        /// Its title.
        title: String,
        /// Whether it is open, as the program gives it.
        open: bool,
    },
    /// A bar of tabs, laid out as a container, whose children are its tabs
    /// ([`Kind::Tab`]): one is chosen, which a press on another, or the arrows, Home and End while
    /// a tab holds the focus, changes. It keeps the user's choice, as a check box keeps its state;
    /// a new choice runs again the builder that made the bar, which shows the chosen tab's panel
    /// ([`Children::chosen`]).
    TabBar {
        /// The place of the chosen tab among the bar's, as the program gives it.
        chosen: usize,
    },
    /// A tab of a bar of tabs ([`Kind::TabBar`]): its title, in the middle of its box, as a
    /// button's label, and its count after it, in a small box, where it has one. Rules select the
    /// chosen one by the state `Selected`.
    Tab {
        /// Its title.
        title: String,
        /// A count that it shows after its title, in a small box, which its name holds too
        /// (`docs/adr/0151-rich-widgets.md`); none for none.
        count: Option<u64>,
    },
    /// Two panels, its first two children, side by side or one above the other, and a handle
    /// between them, as thick as half its control's size, which the pointer drags, and the arrows
    /// move while it holds the focus: the share of its length that the first takes. It keeps the
    /// user's share, as a slider keeps its value; children past the second are not laid out.
    Split {
        /// Whether its panels lie side by side, the handle upright; one above the other otherwise.
        side_by_side: bool,
        /// The share of its length that its first panel takes, from 0 to 1, as the program gives
        /// it; the second takes the rest, less the handle.
        share: f32,
    },
    /// A menu, laid out as a container, that holds its items ([`Kind::MenuItem`]), and boxes
    /// between them that only part them (`docs/adr/0150-layers.md`). The arrows up and down move
    /// the focus among its items, around, Home and End to the first and the last. A layer that the
    /// program opens holds it, beside what opens it or at the pointer ([`Ui::open_layer`]).
    Menu,
    /// A bar of menus, laid out as a container, whose items ([`Kind::MenuItem`]) open its menus
    /// below them: their labels alone, in the middle of their boxes, as buttons' (`docs/adr/0150-
    /// layers.md`). A press on an item opens its menu, the program opening it beside the item, and
    /// closes it once open; while a menu of the bar is open, the pointer over another item opens
    /// that one's. The arrows towards a line's start and end move the focus among its items,
    /// around, and between its menus while one is open; the arrows up and down open the menu of
    /// the item that holds the focus. F10, or Alt pressed and let go alone, gives its first item
    /// the focus, and Escape gives it back to what held it; a press on an item of its menus gives
    /// it back too. Its items take the focus from the keys alone.
    MenuBar,
    /// An item of a menu: a mark, its label, the shortcut of its command, then a chevron where it
    /// opens a submenu, in the middle of its box from top to bottom. The pointer over it gives it
    /// the focus. A press, or Enter or Space while it holds the focus, reaches the program, and
    /// closes the menus that the program opened, from the first that led to it, telling the
    /// program that they closed ([`ActionKind::Dismissed`](crate::action::ActionKind::Dismissed)),
    /// so that the item that the press names no longer exists as the program reads it;
    /// an item that opens a submenu stays, and the arrow towards the end of a line presses it too,
    /// the program opening the submenu beside it. In a submenu, the arrow towards the start closes
    /// it, the focus back on its item. Rules select it checked by the state `Checked`, and an item
    /// whose submenu is open by the state `Expanded`.
    MenuItem {
        /// Its label.
        label: String,
        /// The chord of its command's shortcut, as the user reads it, `Ctrl+S`; empty for none.
        shortcut: String,
        /// What it shows besides its label.
        mark: ItemMark,
    },
    /// A panel that floats over a window: a bar of its title, after a mark that shows whether it
    /// is open, then, while it is open, its children, laid out as a container's below the bar, as
    /// a section's are (`docs/adr/0150-layers.md`). The program opens a floating layer
    /// ([`Place::Floating`]) and builds the panel as its only child, which fills it. The pointer
    /// drags the bar to move the layer, and the corner at the end of its bottom to size it; a
    /// press on its mark, or Enter or Space while it holds the focus, folds it to its bar, the
    /// layer as tall as the bar then, and opens it again; a press anywhere on it raises its layer
    /// above the others. The place and the size that the user leaves are the layer's
    /// ([`Ui::layer_place`], [`Ui::set_layer_place`]); whether it is open is kept as a section's,
    /// and rules select it open by the state `Expanded`.
    FloatingPanel {
        /// Its title.
        title: String,
        /// Whether it is open, as the program gives it.
        open: bool,
    },
    /// A button that stays pressed (`docs/adr/0151-rich-widgets.md`): its label, in the middle of
    /// its box, as a button's. A press, or Enter or Space while it holds the focus, turns it on or
    /// off, which the program reads as actions
    /// ([`FieldValue::Checked`](crate::action::FieldValue::Checked)); it keeps what the user
    /// turned under a check box's rule, and rules select it on by the state `Checked`.
    ToggleButton {
        /// Its label.
        label: String,
        /// Whether it is on, as the program gives it.
        on: bool,
    },
    /// A control of segments, laid out as a container, whose children are its segments
    /// ([`Kind::Segment`]), of which one is chosen (`docs/adr/0151-rich-widgets.md`): a press on
    /// another, or the arrows, Home and End while a segment holds the focus, chooses it, as a bar
    /// of tabs does, and the program reads the place of the chosen segment as actions
    /// ([`FieldValue::Chosen`](crate::action::FieldValue::Chosen)), under a check box's rule.
    Segments {
        /// The place of the chosen segment among its segments, as the program gives it.
        chosen: usize,
    },
    /// A segment of a control of segments ([`Kind::Segments`]): its title, in the middle of its
    /// box. Rules select the chosen one by the state `Selected`.
    Segment {
        /// Its title.
        title: String,
    },
    /// A bar of progress (`docs/adr/0151-rich-widgets.md`): a track, its part up to its share
    /// filled; without a share, a part of it that moves across it with the interface's clock, while
    /// what it shows does not know how far it went. It takes neither the pointer nor the focus.
    Progress {
        /// How far it went, from 0 to 1; none while it is not known.
        share: Option<f32>,
    },
    /// A list to choose from: its chosen option, in the middle of its box from top to bottom, as
    /// wide as its widest, and a mark at its end. A press, or Enter, Space, Alt+Down or F4 while it
    /// holds the focus, opens the list of its options below it, in a layer of its own, a menu
    /// ([`Kind::Menu`]) whose items are its options, the chosen one marked and given the focus when
    /// the keyboard opened it; the arrows and the pointer move through it, a press or Enter
    /// chooses, Escape, a press outside or a press on the list to choose from closes it unchanged.
    /// Closed, the arrows up and down choose the option before or after it, Home and End the first
    /// and the last. It keeps the user's choice, which the program reads as actions
    /// ([`FieldValue::Chosen`](crate::action::FieldValue::Chosen)), under a check box's rule; rules
    /// select it open by the state `Expanded`.
    Choice {
        /// Its options.
        options: Vec<String>,
        /// The place of the chosen option among them, as the program gives it; kept within them.
        chosen: usize,
    },
}

/// What an item of a menu ([`Kind::MenuItem`]) shows besides its label and its shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMark {
    /// Nothing: an item that runs its command.
    None,
    /// A tick, where it is checked, as the program gives it: an item that turns something on or
    /// off, which the program turns as it reads the press.
    Check(bool),
    /// A chevron: an item that opens a submenu.
    Submenu,
}

impl Kind {
    /// Whether a press of the pointer reaches it, and the focus: a button, a field, a check box, a
    /// slider, a section, a tab, two panels and their handle, an item of a menu, a list to choose
    /// from, a floating panel, a button that toggles or a segment.
    pub(crate) fn takes_pointer(&self) -> bool {
        matches!(
            self,
            Self::Button { .. }
                | Self::TextField { .. }
                | Self::NumberField { .. }
                | Self::CheckBox { .. }
                | Self::Slider { .. }
                | Self::Section { .. }
                | Self::Tab { .. }
                | Self::Split { .. }
                | Self::MenuItem { .. }
                | Self::Choice { .. }
                | Self::FloatingPanel { .. }
                | Self::ToggleButton { .. }
                | Self::Segment { .. }
        )
    }
}

/// The state of an element that its builder does not give: whether the pointer is over it, it is
/// pressed, disabled or holds the focus. The interface's input sets whether it is hovered, pressed
/// and focused, and the program whether it is disabled ([`Ui::set_state`]); rules select elements
/// by it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ElementState {
    /// The pointer is over it.
    pub hovered: bool,
    /// It is pressed.
    pub pressed: bool,
    /// It takes no input.
    pub disabled: bool,
    /// It holds the focus: the keys reach it first. The interface sets it ([`Ui::set_focus`]).
    pub focused: bool,
    /// It holds a focus that shows: one that the keyboard, an agent or the program gave, not a
    /// press of the pointer (`docs/adr/0149-the-looks-foundations.md`). The interface sets it.
    pub focus_visible: bool,
    /// It is checked: a check box, as the program gave it or the user toggled it.
    pub checked: bool,
    /// It is open: a section, as the program gave it or the user opened it.
    pub expanded: bool,
    /// It is the chosen tab of its bar.
    pub selected: bool,
}

/// An element as a builder describes it: what it is, its classes and its style. The tree keeps
/// the element it built before for the same key and kind, and redoes of it only what changed.
///
/// Its style is the rules' that select it, in their order, the fields that `style` sets apart from
/// [`Style::default`] in place of theirs ([`Ui::set_style`]): the code keeps what it sets, and a
/// field that it leaves at its default takes the rules'. A color, an array, a list and an option
/// are each one field, set whole: `size` set to `[Pixels(120.0), Auto]` sets both its sizes.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// What it is.
    pub kind: Kind,
    /// The classes that rules select it by.
    pub classes: Vec<String>,
    /// How it is laid out and drawn, over what the rules give.
    pub style: Style,
    /// What it is to whoever reads the interface without seeing it; `None` for what its kind
    /// makes it ([`Ui::meaning`]).
    pub role: Option<Role>,
    /// Its name to whoever reads the interface without seeing it; `None` for a button's label or a
    /// text's, or no name.
    pub name: Option<String>,
    /// The text that shows below it once the pointer has rested on it for half a second of the
    /// interface's clock, or once the keys gave it the focus, and that screen readers read as its
    /// description (`docs/adr/0150-layers.md`); `None` for none.
    pub tooltip: Option<String>,
    /// The text that a field shows, in its control's track color, while it holds nothing, and that
    /// screen readers read as its placeholder (`docs/adr/0151-rich-widgets.md`); `None` for none,
    /// and for an element that is no field.
    pub placeholder: Option<String>,
}

impl Element {
    /// A box that holds others.
    pub fn container(style: Style) -> Self {
        Self::of(Kind::Container, style)
    }

    /// A text.
    pub fn label(text: impl Into<String>, style: Style) -> Self {
        Self::of(Kind::Label(text.into()), style)
    }

    /// The whole of an image of colors, drawn as it is.
    pub fn image(image: ImageKey, style: Style) -> Self {
        let kind = Kind::Image {
            image,
            part: Rect::new(0.0, 0.0, 1.0, 1.0),
            kind: ImageKind::Color,
            color: LinearColor::WHITE,
        };
        Self::of(kind, style)
    }

    /// A button with its label.
    pub fn button(label: impl Into<String>, style: Style) -> Self {
        Self::of(
            Kind::Button {
                label: label.into(),
            },
            style,
        )
    }

    /// A box whose children scroll inside it.
    pub fn scroll(style: Style) -> Self {
        Self::of(Kind::Scroll, style)
    }

    /// A list of `rows` rows of `row_height` logical pixels, of which only those that show are
    /// built.
    pub fn list(rows: usize, row_height: f32, style: Style) -> Self {
        Self::of(Kind::List { rows, row_height }, style)
    }

    /// A field of one line of text, holding `text`.
    pub fn text_field(text: impl Into<String>, style: Style) -> Self {
        let kind = Kind::TextField {
            text: text.into(),
            multiline: false,
            search: false,
        };
        Self::of(kind, style)
    }

    /// A field of several lines of text, holding `text`.
    pub fn text_area(text: impl Into<String>, style: Style) -> Self {
        let kind = Kind::TextField {
            text: text.into(),
            multiline: true,
            search: false,
        };
        Self::of(kind, style)
    }

    /// A field of search, of one line, holding `text`.
    pub fn search_field(text: impl Into<String>, style: Style) -> Self {
        let kind = Kind::TextField {
            text: text.into(),
            multiline: false,
            search: true,
        };
        Self::of(kind, style)
    }

    /// A field of a number, holding `value`, which a drag of one logical pixel changes by `step`.
    /// What the user types reads as Rust reads a number, its decimals after a full stop: `2,5`
    /// reads as none.
    pub fn number_field(value: f64, step: f64, style: Style) -> Self {
        let kind = Kind::NumberField {
            value,
            step,
            prefix: String::new(),
        };
        Self::of(kind, style)
    }

    /// A field of a number, as [`Element::number_field`] makes one, with `prefix` before its
    /// number, which the pointer drags: `X` for a vector's first component.
    pub fn prefixed_number_field(
        prefix: impl Into<String>,
        value: f64,
        step: f64,
        style: Style,
    ) -> Self {
        let kind = Kind::NumberField {
            value,
            step,
            prefix: prefix.into(),
        };
        Self::of(kind, style)
    }

    /// A check box with its label, checked or not.
    pub fn check_box(label: impl Into<String>, checked: bool, style: Style) -> Self {
        let kind = Kind::CheckBox {
            label: label.into(),
            checked,
        };
        Self::of(kind, style)
    }

    /// A bar of tabs, the tab at `chosen` among those that its builder adds chosen.
    pub fn tab_bar(chosen: usize, style: Style) -> Self {
        Self::of(Kind::TabBar { chosen }, style)
    }

    /// A tab of a bar of tabs, with its title.
    pub fn tab(title: impl Into<String>, style: Style) -> Self {
        Self::of(
            Kind::Tab {
                title: title.into(),
                count: None,
            },
            style,
        )
    }

    /// A tab of a bar of tabs, with its title and the count that it shows after it.
    pub fn counted_tab(title: impl Into<String>, count: u64, style: Style) -> Self {
        Self::of(
            Kind::Tab {
                title: title.into(),
                count: Some(count),
            },
            style,
        )
    }

    /// A button that stays pressed, with its label, on or off.
    pub fn toggle_button(label: impl Into<String>, on: bool, style: Style) -> Self {
        let kind = Kind::ToggleButton {
            label: label.into(),
            on,
        };
        Self::of(kind, style)
    }

    /// A control of segments, the segment at `chosen` among those that its builder adds chosen.
    pub fn segments(chosen: usize, style: Style) -> Self {
        Self::of(Kind::Segments { chosen }, style)
    }

    /// A segment of a control of segments, with its title.
    pub fn segment(title: impl Into<String>, style: Style) -> Self {
        Self::of(
            Kind::Segment {
                title: title.into(),
            },
            style,
        )
    }

    /// A bar of progress at `share`, from 0 to 1, or moving without one, for none.
    pub fn progress(share: Option<f32>, style: Style) -> Self {
        Self::of(Kind::Progress { share }, style)
    }

    /// Two panels side by side, or one above the other, the first taking `share` of its length;
    /// its builder adds them.
    pub fn split(side_by_side: bool, share: f32, style: Style) -> Self {
        Self::of(
            Kind::Split {
                side_by_side,
                share,
            },
            style,
        )
    }

    /// A section that folds, with its title, open or not.
    pub fn section(title: impl Into<String>, open: bool, style: Style) -> Self {
        let kind = Kind::Section {
            title: title.into(),
            open,
        };
        Self::of(kind, style)
    }

    /// A slider from `min` to `max`, at `value`, by multiples of `step` from `min`, 0 for any
    /// value.
    pub fn slider(value: f64, min: f64, max: f64, step: f64, style: Style) -> Self {
        Self::of(
            Kind::Slider {
                value,
                min,
                max,
                step,
            },
            style,
        )
    }

    /// A menu, whose builder adds its items.
    pub fn menu(style: Style) -> Self {
        Self::of(Kind::Menu, style)
    }

    /// A bar of menus, whose builder adds its items, each of [`ItemMark::Submenu`], which opens its
    /// menu: an item of another mark in a bar is pressed as a menu's item is, opening nothing.
    pub fn menu_bar(style: Style) -> Self {
        Self::of(Kind::MenuBar, style)
    }

    /// An item of a menu, with its label, the chord of its shortcut as the user reads it, empty for
    /// none, and its mark.
    pub fn menu_item(
        label: impl Into<String>,
        shortcut: impl Into<String>,
        mark: ItemMark,
        style: Style,
    ) -> Self {
        let kind = Kind::MenuItem {
            label: label.into(),
            shortcut: shortcut.into(),
            mark,
        };
        Self::of(kind, style)
    }

    /// A panel that floats over a window, with its title, open or not.
    pub fn floating_panel(title: impl Into<String>, open: bool, style: Style) -> Self {
        let kind = Kind::FloatingPanel {
            title: title.into(),
            open,
        };
        Self::of(kind, style)
    }

    /// A list to choose from, of `options`, the one at `chosen` chosen.
    pub fn choice(options: Vec<String>, chosen: usize, style: Style) -> Self {
        Self::of(Kind::Choice { options, chosen }, style)
    }

    /// An element of `kind` and `style`, of no class.
    pub fn of(kind: Kind, style: Style) -> Self {
        Self {
            kind,
            classes: Vec::new(),
            style,
            role: None,
            name: None,
            tooltip: None,
            placeholder: None,
        }
    }

    /// The element, holding the class `class` too.
    #[must_use]
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// The element, of the role `role` ([`Ui::meaning`]).
    #[must_use]
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// The element, named `name` for whoever reads the interface without seeing it: a container
    /// so named means something, a [`Role::Group`] unless it has another role ([`Ui::meaning`]).
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The element, with a tooltip of `text` (the field [`tooltip`](Self::tooltip)): the
    /// interface shows it in a layer below the element, which the pointer passes through, a
    /// container of the class `tooltip` holding a label.
    #[must_use]
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The element, a field, showing `text` while it holds nothing (the field
    /// [`placeholder`](Self::placeholder)).
    #[must_use]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }
}

/// The direction in which a root lays out its lines: its children's start and end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayoutDirection {
    /// The start is at the left.
    #[default]
    LeftToRight,
    /// The start is at the right: the layout mirrored.
    RightToLeft,
}

/// A root of the tree: the surface it lays out on, a window or any target that the program draws
/// its display list into.
///
/// Roots are stacked in the order they are added, the last on top: drawn last, and taking the
/// pointer first. A popup, a menu or a dialog is a root of its own, over the window's, at the place
/// that the program gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootSettings {
    /// Its width and height, in logical pixels.
    pub size: [f32; 2],
    /// The pixels of the screen per logical pixel, a finite number above 0.
    pub scale: f32,
    /// The direction of its lines.
    pub direction: LayoutDirection,
    /// Where its top left corner lies on its surface, in logical pixels: `[0.0, 0.0]` for a root
    /// that fills a window, a popup's place otherwise.
    pub origin: [f32; 2],
    /// Whether it is modal, as the root is added: while it is there, the roots below it take no
    /// input, and the focus stays among its elements, given back to the element that held it when
    /// it is removed.
    pub modal: bool,
}

/// What changed in an element since it was last laid out and drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Changed {
    /// Its style: the rules that select it are matched again, and its style made again.
    pub(crate) style: bool,
    /// Its layout, or its children's: the root is laid out again.
    pub(crate) layout: bool,
    /// How it is drawn.
    pub(crate) paint: bool,
    /// What it means: its role, name, value, states, actions, bounds, the elements that mean
    /// something nearest below it, or a field's text. Kept for what the interface last gave of its
    /// meaning's changes ([`Ui::take_meaning_changes`]).
    pub(crate) meaning: bool,
}

/// What a frame did ([`Ui::frame`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// The builders that ran, of those that [`Ui::build`] was given.
    pub built: usize,
    /// The elements whose rules were matched again, and their style made again.
    pub styled: usize,
    /// The roots laid out again.
    pub layouts: usize,
    /// The lists whose rows alone were laid out again, their roots not.
    pub lists: usize,
    /// The elements drawn again.
    pub painted: usize,
    /// The elements in the tree.
    pub elements: usize,
    /// Whether a transition goes on after this frame, the pointer rests where a tooltip will show
    /// or a submenu open, or a bar of progress without a share moves: a program that draws only
    /// when something changes draws the next frame too.
    pub animating: bool,
}

/// An element in the arena.
pub(crate) struct Node {
    /// The number that names it to whoever reads what the interface means, given to no other
    /// element of the interface ([`MeaningNode::number`]).
    pub(crate) number: u64,
    pub(crate) key: Key,
    pub(crate) parent: Option<ElementId>,
    pub(crate) children: Vec<ElementId>,
    pub(crate) element: Element,
    pub(crate) state: ElementState,
    /// The root's settings, for a root.
    pub(crate) root: Option<RootSettings>,
    /// The revision its children were last built for, for an element that a builder fills.
    pub(crate) built: Option<u64>,
    /// Whether its builder runs again whatever the revision: a list below it shows other rows.
    pub(crate) rebuild: bool,
    /// The element whose builder made it, which runs again when it is a list that shows other rows.
    pub(crate) owner: Option<ElementId>,
    /// The rows that a list built.
    pub(crate) rows_built: Range<usize>,
    /// How far its content is scrolled, in logical pixels, for a box that scrolls or a list.
    pub(crate) scroll: [f32; 2],
    pub(crate) changed: Changed,
    /// What flexbox and grid know of it.
    pub(crate) taffy: taffy::Style,
    pub(crate) cache: taffy::Cache,
    pub(crate) unrounded: taffy::Layout,
    pub(crate) layout: taffy::Layout,
    /// Its box on its root, in pixels of the screen, and the rectangle it is cut to.
    pub(crate) rect: Rect,
    pub(crate) clip: Option<Rect>,
    /// Its style: the rules' that select it, and its element's.
    pub(crate) style: Arc<Style>,
    /// What it draws, kept until it changes, each with the rectangle it is cut to besides its
    /// element's clip: a field's text, cut to its content box.
    pub(crate) primitives: Vec<(Primitive, Option<Rect>)>,
    /// What a field keeps: the text that the user types, its selection and its undo.
    pub(crate) field: Option<Box<field::Field>>,
    /// What a check box or a slider keeps: the value that the program last gave, and a slider's
    /// own.
    pub(crate) control: Option<control::Control>,
    /// Whether it was styled once: a new element shows its style at once, without a transition.
    pub(crate) styled: bool,
    /// The transition of its colors and opacity under way, its style its interpolation.
    pub(crate) motion: Option<Box<motion::Motion>>,
    /// A section's growing or shrinking under way.
    pub(crate) reveal: Option<motion::Reveal>,
    /// What the interface keeps of a layer, for a root that it places.
    pub(crate) layer: Option<Box<layer::Held>>,
}

/// A place in the arena.
struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// An interface: its roots, their trees of elements, the fonts their texts are laid out with, and
/// what each element lays out and draws, kept from one frame to the next.
///
/// A program builds its elements with [`Ui::build`] each frame, which runs a builder only when the
/// revision it gives moved; elements built again are matched with those before by their keys, so
/// that what the builder does not give, a scroll, a state, survives. [`Ui::frame`] then lays out
/// the roots where something changed, places again what scrolled, draws again the elements that
/// changed or moved, and gives the display list. A frame where nothing changed lays out and draws
/// nothing; it gathers the display list from what each element drew before.
///
/// Lengths are logical pixels in styles and pixels of the screen, logical pixels times the root's
/// scale, in what a frame gives: boxes, clips and the display list.
///
/// A person's input reaches it as the window gives it, the pointer's moves and buttons, the wheel,
/// the modifiers and the keys ([`Ui::pointer_moved`], [`Ui::pointer_button`], [`Ui::wheel`],
/// [`Ui::modifiers`], [`Ui::key`]), which become *actions*; an agent sends actions themselves
/// ([`Ui::act`]). The program reads them from a queue ([`Ui::take_actions`]).
///
/// ```
/// use slime_ui::Rect;
/// use slime_ui::style::{Dimension, Style};
/// use slime_ui::text::Fonts;
/// use slime_ui::tree::{Element, LayoutDirection, RootSettings, Ui};
///
/// let mut ui = Ui::new(Fonts::new());
/// let settings = RootSettings {
///     size: [400.0, 300.0],
///     scale: 2.0,
///     direction: LayoutDirection::RightToLeft,
///     origin: [0.0; 2],
///     modal: false,
/// };
/// let root = ui.add_root(settings, Style::default());
/// let mut style = Style::default();
/// style.layout.size = [Dimension::Pixels(100.0), Dimension::Pixels(50.0)];
/// let mut panel = None;
/// ui.build(root, 1, |children| {
///     panel = Some(children.add("panel", Element::container(style), |_| {}));
/// });
/// let (_list, stats) = ui.frame();
/// assert_eq!(stats.layouts, 1);
/// // At the start of a line laid out from right to left, in pixels of the screen.
/// assert_eq!(ui.rect(panel.unwrap()), Some(Rect::new(600.0, 0.0, 200.0, 100.0)));
/// // Nothing changed: nothing laid out nor drawn.
/// let (_list, stats) = ui.frame();
/// assert_eq!((stats.layouts, stats.painted), (0, 0));
/// ```
pub struct Ui {
    slots: Vec<Slot>,
    free: Vec<u32>,
    roots: Vec<ElementId>,
    pub(crate) fonts: Fonts,
    pub(crate) texts: TextCache,
    built: usize,
    /// The lists whose rows changed, and nothing else: their rows are laid out again alone.
    relaid: Vec<ElementId>,
    /// The elements whose content scrolled: what lies below them is placed again.
    scrolled: Vec<ElementId>,
    /// The elements whose style is to be made again.
    restyled: Vec<ElementId>,
    /// The rules of the style sheets that style the elements, their theme's values in them.
    rules: Vec<Rule>,
    /// What a person's input reached, and the actions that wait for the program.
    interaction: Interaction,
    /// The system's clipboard, which its fields copy to and paste from.
    clipboard: Option<Box<dyn Clipboard>>,
    /// The types that style sheets hold, and the default style as reflection holds it.
    registry: Registry,
    default_style: Value,
    /// The styles made, by the rules that select their elements, then the elements' own styles.
    styles: HashMap<Vec<u32>, Vec<(Style, Arc<Style>)>>,
    /// The number that the next element, or run of a field's text, is given: from 1, so that 0
    /// names none.
    next_number: u64,
    /// The elements whose meaning changed since its changes were last taken.
    meant: Vec<ElementId>,
    /// Whether the next changes of the meaning are the whole tree: none were taken yet, a modal
    /// root changed what every element takes, or more changed than the tree holds.
    meaning_whole: bool,
    /// Whether the elements that mean something at the top of the tree changed.
    top_changed: bool,
    /// The bars of progress without a share that the last frame drew, which move with the clock and
    /// are drawn again each frame.
    spinning: Vec<ElementId>,
    /// The interface's clock: the time that its program gave it ([`Ui::advance`]).
    now: Duration,
    /// Whether transitions end at once ([`Ui::set_reduced_motion`]).
    reduced_motion: bool,
    /// The elements whose transition is under way.
    moving: Vec<ElementId>,
}

impl Ui {
    /// An interface without roots, whose texts are laid out with `fonts`, and kept within
    /// [`TEXT_CACHE_BUDGET`].
    pub fn new(fonts: Fonts) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            roots: Vec::new(),
            fonts,
            texts: TextCache::new(TEXT_CACHE_BUDGET),
            built: 0,
            relaid: Vec::new(),
            scrolled: Vec::new(),
            restyled: Vec::new(),
            interaction: Interaction::default(),
            clipboard: None,
            rules: Vec::new(),
            registry: theme::registry(),
            default_style: Style::default().to_value(),
            styles: HashMap::new(),
            next_number: 1,
            meant: Vec::new(),
            meaning_whole: true,
            top_changed: false,
            spinning: Vec::new(),
            now: Duration::ZERO,
            reduced_motion: false,
            moving: Vec::new(),
        }
    }

    /// The fonts that its texts are laid out with, which the program that draws its display list
    /// rasterizes them with.
    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// The fonts, to register more.
    pub fn fonts_mut(&mut self) -> &mut Fonts {
        &mut self.fonts
    }

    /// Adds a root: a surface of `settings`, its element a container of `style`, whose size is
    /// the root's.
    ///
    /// # Panics
    ///
    /// If the settings' scale is not a finite number above 0.
    pub fn add_root(&mut self, settings: RootSettings, style: Style) -> ElementId {
        assert_scale(settings.scale);
        let mut node = Node::new(
            Key::Index(self.roots.len() as u64),
            Element::container(style),
        );
        node.root = Some(settings);
        let id = self.insert(node);
        self.roots.push(id);
        self.restyled.push(id);
        self.top_changed = true;
        if settings.modal {
            self.opened_modal(id);
        }
        id
    }

    /// Changes a root's size, scale or direction: it is laid out again, and, for another scale or
    /// direction, drawn again whole.
    ///
    /// # Panics
    ///
    /// If `root` is not a root of this interface, or the settings' scale is not a finite number
    /// above 0.
    pub fn set_root(&mut self, root: ElementId, settings: RootSettings) {
        assert_scale(settings.scale);
        let node = self.node_mut(root).expect("a root of this interface");
        let before = node
            .root
            .replace(settings)
            .expect("a root of this interface");
        if before.scale != settings.scale || before.direction != settings.direction {
            // Every length that taffy holds, and every primitive, is in pixels of the screen, and
            // every corner and shadow mirrored by the direction.
            self.mark_all(root);
        } else if before != settings {
            self.mark_layout(root);
        }
    }

    /// Removes `id`, a root or an element, with its children; their ids name nothing from then
    /// on. A layer goes with the layers that stand over what it holds, telling the program nothing
    /// ([`Ui::open_layer`]); the layers that stood beside or over what another removal took close
    /// at the next frame, which tells it ([`ActionKind::Dismissed`](crate::action::ActionKind::Dismissed)).
    /// Nothing happens for an id that names nothing.
    pub fn remove(&mut self, id: ElementId) {
        let Some(node) = self.node(id) else {
            return;
        };
        // A layer goes with the layers that stand over what it holds.
        if node.layer.is_some() {
            self.close_layer(id, false);
            return;
        }
        if let Some(parent) = node.parent {
            if let Some(parent) = self.node_mut(parent) {
                parent.children.retain(|&child| child != id);
            }
            self.children_changed(parent);
        } else {
            self.top_changed = true;
            // The roots above it take the numbers below theirs, which their elements' paths begin
            // with.
            let above: Vec<ElementId> = self
                .roots
                .iter()
                .skip_while(|&&root| root != id)
                .skip(1)
                .copied()
                .collect();
            for root in above {
                self.mark_meaning_below(root);
            }
        }
        self.roots.retain(|&root| root != id);
        self.free_subtree(id);
        self.forget(id);
    }

    /// Builds the children of `parent` with `build`, unless they were built for `revision`
    /// already, and no list among them shows rows that it did not build: what the builder reads,
    /// the world's tick or a counter of the program's, moves the revision. The children that it
    /// adds are matched by their keys with those it added before: one of the same key and kind is
    /// kept, with its state, and gets the new description, laid out or drawn again only where it
    /// changed; the others are made, and those it no longer adds are removed.
    ///
    /// # Panics
    ///
    /// If `parent` names nothing, or the builder gives two siblings the same key.
    pub fn build(
        &mut self,
        parent: ElementId,
        revision: u64,
        build: impl FnOnce(&mut Children<'_>),
    ) {
        let node = self.node(parent).expect("a parent in this interface");
        if node.built == Some(revision) && !node.rebuild {
            return;
        }
        self.built += 1;
        let mut children = Children::new(self, parent, parent);
        build(&mut children);
        children.finish();
        let node = self.node_mut(parent).expect("built above");
        node.built = Some(revision);
        node.rebuild = false;
    }

    /// Styles the elements with the rules of `sheets`, in their order and the order of their
    /// files, their names given the values of `themes`, stacked: of the themes that give a name, the
    /// last gives its value, and the values that name it follow it. Every element's style is made
    /// again at the next [`Ui::frame`]. A rule that selects an element sets the fields of its style
    /// that it gives; a later rule, those it gives in turn; the element's own style, the fields
    /// that it sets apart from their defaults, a color, an array, a list and an option each one
    /// field. The styles made are kept and shared, for up to 64 elements' own styles that the same
    /// rules
    /// select: past them, the style of such an element is made again each time, more slowly.
    ///
    /// # Errors
    ///
    /// [`DocumentError`] if a style sheet or a theme is not a document of their types, names a
    /// value that no theme gives or that holds another type, a value names itself through others,
    /// or a color function is given what it does not take: the interface keeps the style it had.
    pub fn set_style(
        &mut self,
        sheets: &[StyleSheet],
        themes: &[Theme],
    ) -> Result<(), DocumentError> {
        self.rules = theme::rules(&self.registry, sheets, themes)?;
        self.styles.clear();
        for root in self.roots.clone() {
            self.mark_style_subtree(root);
        }
        Ok(())
    }

    /// Sets the state of `id`, which rules select elements by: its style is made again. Whether it
    /// holds the focus, and whether its focus shows, stay as they were, the focus's
    /// ([`Ui::set_focus`]); whether it is hovered or pressed, which the input sets too, holds until
    /// the input next changes it. A disabled element loses the focus at the next [`Ui::frame`].
    /// Nothing happens for an id that names nothing.
    pub fn set_state(&mut self, id: ElementId, state: ElementState) {
        if let Some(node) = self.node_mut(id) {
            let state = ElementState {
                focused: node.state.focused,
                focus_visible: node.state.focus_visible,
                ..state
            };
            if node.state == state {
                return;
            }
            node.state = state;
            self.mark_style(id);
            self.mark_meaning(id);
        }
    }

    /// Scrolls the content of `id`, a box that scrolls or a list, to `offset` logical pixels from
    /// its start and top, kept within its content as last laid out, and again when it is next laid
    /// out: an element not laid out yet keeps it until then. A list that then shows other rows has
    /// its builder run again at the next [`Ui::build`]. Nothing happens for an id that names
    /// nothing.
    pub fn scroll_to(&mut self, id: ElementId, offset: [f32; 2]) {
        let scale = self.scale_of(id);
        let Some(node) = self.node_mut(id) else {
            return;
        };
        // An element not laid out yet has no content to keep the offset within: its layout will.
        let offset = if node.changed.layout {
            offset
        } else {
            layout::within(offset, &node.element.kind, &node.layout, scale)
        };
        if node.scroll != offset {
            node.scroll = offset;
            if !self.scrolled.contains(&id) {
                self.scrolled.push(id);
            }
            self.update_rows(id);
        }
    }

    /// The state of `id`.
    pub fn state(&self, id: ElementId) -> Option<ElementState> {
        self.node(id).map(|node| node.state)
    }

    /// How far the content of `id` is scrolled, in logical pixels.
    pub fn scroll(&self, id: ElementId) -> Option<[f32; 2]> {
        self.node(id).map(|node| node.scroll)
    }

    /// The box of `id` on its root after the last [`Ui::frame`], in pixels of the screen.
    pub fn rect(&self, id: ElementId) -> Option<Rect> {
        self.node(id).map(|node| node.rect)
    }

    /// The style of `id` after the last [`Ui::frame`]: the rules' that select it, and its
    /// element's.
    pub fn style(&self, id: ElementId) -> Option<&Style> {
        self.node(id).map(|node| &*node.style)
    }

    /// What `id` is, as its builder last described it.
    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.node(id).map(|node| &node.element)
    }

    /// The children of `id`, in their order.
    pub fn children(&self, id: ElementId) -> &[ElementId] {
        self.node(id).map_or(&[], |node| node.children.as_slice())
    }

    /// The keys from `id`'s root down to it, the root's left out: its path.
    pub fn path(&self, id: ElementId) -> Option<Vec<Key>> {
        let mut path = Vec::new();
        let mut at = id;
        loop {
            let node = self.node(at)?;
            match node.parent {
                Some(parent) => {
                    path.push(node.key.clone());
                    at = parent;
                }
                None => break,
            }
        }
        path.reverse();
        Some(path)
    }

    /// The element at `path` below `root`.
    pub fn find(&self, root: ElementId, path: &[Key]) -> Option<ElementId> {
        let mut at = root;
        for key in path {
            at = *self
                .node(at)?
                .children
                .iter()
                .find(|&&child| self.node(child).is_some_and(|node| node.key == *key))?;
        }
        Some(at)
    }

    /// How many elements the tree holds, its roots included.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Whether it holds no element.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Lays out the roots where something changed, draws again what changed, and gives what the
    /// interface draws, its roots in the order they were added, and what it did.
    pub fn frame(&mut self) -> (DisplayList, FrameStats) {
        let mut stats = FrameStats {
            built: std::mem::take(&mut self.built),
            elements: self.len(),
            ..FrameStats::default()
        };
        // Transitions move on to the clock; then styles: a style that changed lays its element
        // out again. The input's states follow what they made, a focus hidden or disabled dropped,
        // and are styled in turn.
        self.step_motions();
        // A bar of progress without a share moves with the clock.
        for id in std::mem::take(&mut self.spinning) {
            self.mark_paint(id);
        }
        // The pointer resting long enough shows a tooltip or opens a submenu.
        self.settle_rest();
        // Layers take their surface from the root that they stand over, those whose anchor went
        // close, and those opened by the keyboard give their first element the focus.
        self.settle_layers();
        self.focus_new_layers();
        stats.styled += self.restyle_marked();
        self.settle_input();
        stats.styled += self.restyle_marked();
        let mut laid_out = Vec::new();
        let mut lists = Vec::new();
        for root in self.roots.clone() {
            if self.node(root).is_some_and(|node| node.changed.layout) {
                layout::lay_out(self, root);
                stats.layouts += 1;
                laid_out.push(root);
                lists.extend(self.lists(root));
            }
        }
        for list in std::mem::take(&mut self.relaid) {
            // A list of a root laid out above has had its rows laid out with it.
            if self
                .root_of(list)
                .is_some_and(|root| !laid_out.contains(&root))
            {
                lists.extend(layout::lay_out_list(self, list));
                stats.lists += 1;
            }
        }
        for id in std::mem::take(&mut self.scrolled) {
            layout::place_below(self, id);
            lists.push(id);
        }
        // A layer follows its anchor, which a layout or a scroll may have moved.
        let followed = self.follow_anchors();
        stats.layouts += followed.len();
        lists.extend(followed);
        if stats.layouts + stats.lists > 0 || !lists.is_empty() {
            self.moved();
        }
        for list in lists {
            self.update_rows(list);
        }
        for root in self.roots.clone() {
            stats.painted += paint::paint(self, root);
        }
        let mut list = DisplayList::new();
        for root in &self.roots {
            paint::gather(self, *root, &mut list);
        }
        stats.animating = !self.moving.is_empty() || self.resting() || !self.spinning.is_empty();
        (list, stats)
    }

    pub(crate) fn node(&self, id: ElementId) -> Option<&Node> {
        let slot = self.slots.get(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_ref())
            .flatten()
    }

    pub(crate) fn node_mut(&mut self, id: ElementId) -> Option<&mut Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_mut())
            .flatten()
    }

    /// The node at an arena index that taffy names, which holds one.
    pub(crate) fn node_at(&self, index: usize) -> &Node {
        self.slots[index].node.as_ref().expect("an element there")
    }

    pub(crate) fn node_at_mut(&mut self, index: usize) -> &mut Node {
        self.slots[index].node.as_mut().expect("an element there")
    }

    fn insert(&mut self, mut node: Node) -> ElementId {
        node.number = self.number();
        let id = match self.free.pop() {
            Some(index) => {
                let slot = &mut self.slots[index as usize];
                slot.node = Some(node);
                ElementId {
                    index,
                    generation: slot.generation,
                }
            }
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    node: Some(node),
                });
                ElementId {
                    index: (self.slots.len() - 1) as u32,
                    generation: 0,
                }
            }
        };
        self.mark_meaning(id);
        id
    }

    /// A number that names an element, or a run of a field's text, to whoever reads what the
    /// interface means, never given before.
    pub(crate) fn number(&mut self) -> u64 {
        let number = self.next_number;
        self.next_number += 1;
        number
    }

    fn free_subtree(&mut self, id: ElementId) {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return;
        };
        if slot.generation != id.generation {
            return;
        }
        let Some(node) = slot.node.take() else {
            return;
        };
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        for child in node.children {
            self.free_subtree(child);
        }
    }

    /// Makes again the styles marked to be, and counts those that it made.
    fn restyle_marked(&mut self) -> usize {
        let marked = std::mem::take(&mut self.restyled);
        marked.into_iter().filter(|&id| self.restyle(id)).count()
    }

    /// Marks the style of `id` made again at the next frame.
    fn mark_style(&mut self, id: ElementId) {
        if let Some(node) = self.node_mut(id)
            && !node.changed.style
        {
            node.changed.style = true;
            self.restyled.push(id);
        }
    }

    /// Marks the style of `id` and of every element below it made again: rules may select them by
    /// an ancestor.
    fn mark_style_subtree(&mut self, id: ElementId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            self.mark_style(id);
            if let Some(node) = self.node(id) {
                stack.extend(node.children.iter().copied());
            }
        }
    }

    /// Makes the style of `id` again from the rules that select it and its element's own, and marks
    /// it laid out or drawn again as the style changed; `false` for an id that names nothing.
    fn restyle(&mut self, id: ElementId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        let selected: Vec<u32> = (0..self.rules.len())
            .filter(|&rule| self.selects(&self.rules[rule], id))
            .map(|rule| rule as u32)
            .collect();
        let code = &node.element.style;
        // Elements of one style that the same rules select share what they make.
        let made = self
            .styles
            .get(&selected)
            .and_then(|made| made.iter().find(|(own, _)| own == code))
            .map(|(_, style)| Arc::clone(style));
        let mut style = match made {
            Some(style) => style,
            None => {
                let code = code.clone();
                let style = Arc::new(self.compose(&selected, &code));
                let made = self.styles.entry(selected).or_default();
                if made.len() < STYLES_PER_RULES {
                    made.push((code, Arc::clone(&style)));
                }
                style
            }
        };
        // What a folded section holds is not laid out, drawn nor reached, as an element of no
        // display: its style says so, which every walk of the tree reads.
        if self.folded(id) && style.layout.display != Display::None {
            let mut hidden = (*style).clone();
            hidden.layout.display = Display::None;
            style = Arc::new(hidden);
        }
        let node = self.node_mut(id).expect("styled above");
        node.changed.style = false;
        let target = Arc::clone(node.target());
        if *target != *style {
            // The border's width cuts what scrolls inside: placed again with a layout.
            let layout = target.layout != style.layout
                || target.text.style != style.text.style
                || target.paint.border_width != style.paint.border_width;
            let shown = target.layout.display != style.layout.display;
            let first = !node.styled;
            node.styled = true;
            node.changed.paint = true;
            self.show_style(id, style, first);
            if layout {
                self.mark_layout(id);
            }
            // A field's text is laid out in its style; an element hidden or shown again takes
            // what it holds out of the meaning, or puts it back.
            self.mark_meaning(id);
            if shown {
                self.mark_meaning_below(id);
                self.mark_meaning_above(id);
            }
        } else {
            node.styled = true;
            if node.motion.is_none() {
                node.style = style;
            }
        }
        true
    }

    /// The style that the rules `selected`, in their order, then `code`, the fields that it sets
    /// apart from their defaults, make.
    fn compose(&self, selected: &[u32], code: &Style) -> Style {
        if selected.is_empty() {
            return code.clone();
        }
        let mut value = self.default_style.clone();
        let code_value = code.to_value();
        let mut own = self
            .registry
            .difference("UiStyle", &self.default_style, &code_value, true)
            .expect("a style");
        if let Some(own) = &mut own {
            whole_colors(&self.registry, "UiStyle", own, &code_value);
        }
        for part in selected
            .iter()
            .map(|&rule| &self.rules[rule as usize].style)
            .chain(own.as_ref())
        {
            self.registry
                .overlay("UiStyle", &mut value, part)
                .expect("a part of a style, read as one");
        }
        Style::from_value(&value).expect("a style")
    }

    /// Whether `rule` selects `id`.
    fn selects(&self, rule: &Rule, id: ElementId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        let selector = &rule.selector;
        let kind = kind_name(&node.element.kind);
        if selector.kind.is_some_and(|wanted| wanted != kind)
            || !selector
                .classes
                .iter()
                .all(|class| node.element.classes.contains(class))
            || selector
                .key
                .as_ref()
                .is_some_and(|key| !named(&node.key, key))
            || !selector.states.iter().all(|state| match state {
                StateName::Hovered => node.state.hovered,
                StateName::Pressed => node.state.pressed,
                StateName::Disabled => node.state.disabled,
                StateName::Focused => node.state.focused,
                StateName::FocusVisible => node.state.focus_visible,
                StateName::Checked => node.state.checked,
                StateName::Expanded => node.state.expanded,
                StateName::Selected => node.state.selected,
            })
        {
            return false;
        }
        let Some(ancestor) = &selector.inside else {
            return true;
        };
        let mut at = node.parent;
        while let Some(id) = at {
            let Some(node) = self.node(id) else {
                return false;
            };
            if ancestor
                .kind
                .is_none_or(|wanted| wanted == kind_name(&node.element.kind))
                && ancestor
                    .class
                    .as_ref()
                    .is_none_or(|class| node.element.classes.contains(class))
            {
                return true;
            }
            at = node.parent;
        }
        false
    }

    /// Marks `id` laid out again, with its ancestors, up to its root, or up to the list it is a
    /// row of: a list lays out its rows alone.
    pub(crate) fn mark_layout(&mut self, id: ElementId) {
        let mut at = id;
        loop {
            let Some(node) = self.node_mut(at) else {
                return;
            };
            node.changed.layout = true;
            node.cache.clear();
            let Some(parent) = node.parent else {
                return;
            };
            if self.row_height(parent).is_some() {
                self.relay(parent);
                return;
            }
            at = parent;
        }
    }

    /// Marks `id` and every element below it laid out and drawn again.
    fn mark_all(&mut self, id: ElementId) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        node.changed.layout = true;
        node.changed.paint = true;
        node.cache.clear();
        for child in node.children.clone() {
            self.mark_all(child);
        }
    }

    /// Marks `parent` laid out again, its children having changed: or only its rows, for a list;
    /// and the elements that mean something nearest below the one above it that does.
    fn children_changed(&mut self, parent: ElementId) {
        self.mark_meaning_up(parent);
        if self.row_height(parent).is_some() {
            self.relay(parent);
        } else {
            self.mark_layout(parent);
        }
    }

    /// Asks for the rows of the list `list` to be laid out again at the next frame.
    fn relay(&mut self, list: ElementId) {
        if !self.relaid.contains(&list) {
            self.relaid.push(list);
        }
    }

    /// The height of the rows of `id`, a list, in logical pixels.
    pub(crate) fn row_height(&self, id: ElementId) -> Option<f32> {
        match self.node(id)?.element.kind {
            Kind::List { row_height, .. } => Some(row_height),
            _ => None,
        }
    }

    /// The lists below `id`, itself included.
    pub(crate) fn lists(&self, id: ElementId) -> Vec<ElementId> {
        let mut lists = Vec::new();
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.node(at) else {
                continue;
            };
            if matches!(node.element.kind, Kind::List { .. }) {
                lists.push(at);
            }
            stack.extend(node.children.iter().copied());
        }
        lists
    }

    /// The rows that the list `id` shows, from its box when last laid out and its scroll.
    pub(crate) fn visible_rows(&self, id: ElementId) -> Range<usize> {
        let Some(node) = self.node(id) else {
            return 0..0;
        };
        let Kind::List { rows, row_height } = node.element.kind else {
            return 0..0;
        };
        if row_height <= 0.0 || rows == 0 {
            return 0..0;
        }
        let scale = self.scale_of(id);
        let top = node.scroll[1] - node.layout.padding.top / scale;
        let bottom = top + node.layout.size.height / scale;
        let first = ((top / row_height).floor().max(0.0) as usize).min(rows);
        let last = ((bottom / row_height).ceil().max(0.0) as usize).min(rows);
        first..last.max(first)
    }

    /// Asks the builder of the list `id` to run again when it shows other rows than it built.
    fn update_rows(&mut self, id: ElementId) {
        let visible = self.visible_rows(id);
        let Some(node) = self.node(id) else {
            return;
        };
        if !matches!(node.element.kind, Kind::List { .. }) || node.rows_built == visible {
            return;
        }
        // The builder that gives its rows: its own, for a list in a slot, or the one that made it.
        let builder = if node.built.is_some() {
            Some(id)
        } else {
            node.owner
        };
        if let Some(builder) = builder
            && let Some(builder) = self.node_mut(builder)
        {
            builder.rebuild = true;
        }
    }

    /// The scale of the root that `id` lies under.
    pub(crate) fn scale_of(&self, id: ElementId) -> f32 {
        self.root_settings(id)
            .map_or(1.0, |settings| settings.scale)
    }

    /// The settings of the root that `id` lies under: its scale and its direction among them.
    pub fn root_settings(&self, id: ElementId) -> Option<RootSettings> {
        self.node(self.root_of(id)?)?.root
    }

    /// The root that `id` lies under.
    pub(crate) fn root_of(&self, id: ElementId) -> Option<ElementId> {
        let mut at = id;
        loop {
            match self.node(at)?.parent {
                Some(parent) => at = parent,
                None => return Some(at),
            }
        }
    }
}

impl Node {
    fn new(key: Key, element: Element) -> Self {
        let field = field::given(&element.kind).map(|text| Box::new(field::Field::new(text)));
        let mut state = ElementState::default();
        let control = control::Control::new(&element.kind, &mut state);
        Self {
            number: 0,
            key,
            parent: None,
            children: Vec::new(),
            style: Arc::new(element.style.clone()),
            element,
            state,
            root: None,
            built: None,
            rebuild: false,
            owner: None,
            rows_built: 0..0,
            scroll: [0.0; 2],
            changed: Changed {
                style: true,
                layout: true,
                paint: true,
                meaning: false,
            },
            taffy: taffy::Style::default(),
            cache: taffy::Cache::new(),
            unrounded: taffy::Layout::with_order(0),
            layout: taffy::Layout::with_order(0),
            rect: Rect::default(),
            clip: None,
            primitives: Vec::new(),
            field,
            control,
            styled: false,
            motion: None,
            reveal: None,
            layer: None,
        }
    }

    /// The style that it goes to: its own, or the end of its transition under way.
    pub(crate) fn target(&self) -> &Arc<Style> {
        self.motion
            .as_ref()
            .map_or(&self.style, |motion| &motion.target)
    }

    /// Its content box, its box less its padding, in pixels of the screen.
    pub(crate) fn content_box(&self) -> Rect {
        let padding = self.layout.padding;
        Rect::new(
            self.rect.x + padding.left,
            self.rect.y + padding.top,
            self.layout.content_box_width(),
            self.layout.content_box_height(),
        )
    }
}

/// The children that a builder gives an element ([`Ui::build`]), matched with those it gave
/// before by their keys.
pub struct Children<'a> {
    ui: &'a mut Ui,
    parent: ElementId,
    /// The element whose builder runs.
    owner: ElementId,
    /// The children before, by their keys.
    before: HashMap<Key, ElementId>,
    /// The children given, in their order.
    given: Vec<ElementId>,
    /// Their keys.
    keys: HashSet<Key>,
}

impl<'a> Children<'a> {
    fn new(ui: &'a mut Ui, parent: ElementId, owner: ElementId) -> Self {
        let before = ui
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|child| Some((ui.node(child)?.key.clone(), child)))
            .collect();
        Self {
            ui,
            parent,
            owner,
            before,
            given: Vec::new(),
            keys: HashSet::new(),
        }
    }

    /// Adds the child `element` under `key`, its own children given by `children`: the child of
    /// that key and kind built before, kept, or a new one.
    ///
    /// # Panics
    ///
    /// If a sibling given before has the same key.
    pub fn add(
        &mut self,
        key: impl Into<Key>,
        element: Element,
        children: impl FnOnce(&mut Children<'_>),
    ) -> ElementId {
        let id = self.place(key.into(), element);
        let mut grandchildren = Children::new(self.ui, id, self.owner);
        children(&mut grandchildren);
        grandchildren.finish();
        id
    }

    /// Adds the child `element` under `key`, as [`Children::add`] does, whose children another
    /// builder gives, through [`Ui::build`] on the id it returns: a panel that its own revision
    /// builds again. Its children are kept as they are.
    ///
    /// # Panics
    ///
    /// If a sibling given before has the same key.
    pub fn add_slot(&mut self, key: impl Into<Key>, element: Element) -> ElementId {
        self.place(key.into(), element)
    }

    /// Puts the child `element` under `key` among the children given.
    fn place(&mut self, key: Key, element: Element) -> ElementId {
        assert!(
            self.keys.insert(key.clone()),
            "two siblings of the key `{key}`"
        );
        // One of another kind under the key is removed, and another made.
        let kept = self.before.remove(&key).and_then(|before| {
            let same = self.ui.node(before).is_some_and(|node| {
                std::mem::discriminant(&node.element.kind) == std::mem::discriminant(&element.kind)
            });
            if !same {
                self.ui.free_subtree(before);
                self.ui.forget(before);
            }
            same.then_some(before)
        });
        let id = match kept {
            Some(id) => {
                let meant_before = self.ui.means(id);
                let node = self.ui.node_mut(id).expect("kept");
                // A field takes the program's text when it differs from the later of what the
                // program gave before and what the field last committed, and keeps the user's
                // otherwise. It is read at each build, the element unchanged too: the text that
                // the program gave before, given again once the user committed another, replaces
                // it.
                let given = field::given(&element.kind);
                if let (Some(text), Some(field)) = (given, node.field.as_mut())
                    && field.give(&text, &element.kind)
                {
                    self.ui.mark_layout(id);
                    self.ui.mark_paint(id);
                    self.ui.mark_meaning(id);
                }
                // A check box's or a slider's value, under the same rule.
                let node = self.ui.node_mut(id).expect("kept");
                if let Some(control) = node.control.as_mut()
                    && control.give(&element.kind, &mut node.state)
                {
                    self.ui.mark_style(id);
                    self.ui.mark_paint(id);
                    self.ui.mark_meaning(id);
                }
                let node = self.ui.node_mut(id).expect("kept");
                if node.element != element {
                    let layout = lays_out_again(&node.element, &element);
                    let style = node.element.style != element.style;
                    let classes = node.element.classes != element.classes;
                    // Its role, its name and its tooltip, which it does not draw, change only what
                    // it means.
                    let drawn = node.element.kind != element.kind
                        || style
                        || classes
                        || node.element.placeholder != element.placeholder;
                    let meant = node.element.kind != element.kind
                        || node.element.role != element.role
                        || node.element.name != element.name
                        || node.element.tooltip != element.tooltip
                        || node.element.placeholder != element.placeholder;
                    node.element = element;
                    if meant {
                        self.ui.mark_meaning(id);
                        // Made to mean something, or nothing, it changes what the element above it
                        // that means something holds.
                        if self.ui.means(id) != meant_before {
                            self.ui.mark_meaning_above(id);
                        }
                    }
                    let node = self.ui.node_mut(id).expect("kept");
                    if drawn {
                        node.changed.paint = true;
                    }
                    if layout {
                        self.ui.mark_layout(id);
                    }
                    // Rules may select the elements below by an ancestor's class.
                    if classes {
                        self.ui.mark_style_subtree(id);
                    } else if style {
                        self.ui.mark_style(id);
                    }
                }
                id
            }
            None => {
                let mut node = Node::new(key, element);
                node.parent = Some(self.parent);
                node.owner = Some(self.owner);
                let id = self.ui.insert(node);
                self.ui.restyled.push(id);
                id
            }
        };
        self.given.push(id);
        id
    }

    /// For the children of a list, the rows that show: those to add, keyed by their numbers.
    /// Empty for another element, or a list not yet laid out.
    pub fn rows(&self) -> Range<usize> {
        self.ui.visible_rows(self.parent)
    }

    /// The place of the chosen tab of `bar`, a bar of tabs that this builder added, among its
    /// tabs: the builder shows its panel. A control of segments' and a list to choose from's choice
    /// too; 0 for another element.
    pub fn chosen(&self, bar: ElementId) -> usize {
        self.ui.chosen(bar).unwrap_or(0)
    }

    /// Puts the children given in place, removes those not given, and marks the parent laid out
    /// again if they changed.
    fn finish(self) {
        let Self {
            ui,
            parent,
            before,
            given,
            ..
        } = self;
        let removed = !before.is_empty();
        for (_, child) in before {
            ui.free_subtree(child);
            ui.forget(child);
        }
        let node = ui.node_mut(parent).expect("the parent is there");
        if removed || node.children != given {
            node.children = given;
            ui.children_changed(parent);
        }
        // A bar's tabs show which is chosen.
        ui.mark_chosen(parent);
        // A list's builder gave the rows that it shows.
        if ui.row_height(parent).is_some() {
            let visible = ui.visible_rows(parent);
            ui.node_mut(parent).expect("the parent is there").rows_built = visible;
        }
    }
}

/// Refuses a root's scale that is not a finite number above 0, which no length can be divided by.
fn assert_scale(scale: f32) {
    assert!(
        scale.is_finite() && scale > 0.0,
        "a root's scale must be a finite number above 0, not {scale}"
    );
}

/// Whether changing what `before` holds into what `after` holds lays the element out again, rather
/// than drawing it again alone: a change of style is weighed once the style is made again.
fn lays_out_again(before: &Element, after: &Element) -> bool {
    match (&before.kind, &after.kind) {
        (Kind::Label(before), Kind::Label(after)) => before != after,
        (Kind::Button { label: before }, Kind::Button { label: after }) => before != after,
        (Kind::CheckBox { label: before, .. }, Kind::CheckBox { label: after, .. })
        | (Kind::Section { title: before, .. }, Kind::Section { title: after, .. })
        | (Kind::FloatingPanel { title: before, .. }, Kind::FloatingPanel { title: after, .. })
        | (Kind::Segment { title: before }, Kind::Segment { title: after })
        | (Kind::ToggleButton { label: before, .. }, Kind::ToggleButton { label: after, .. }) => {
            before != after
        }
        (
            Kind::Tab {
                title: before,
                count: count_before,
            },
            Kind::Tab {
                title: after,
                count: count_after,
            },
        ) => before != after || count_before != count_after,
        (
            Kind::MenuItem {
                label: before,
                shortcut: shortcut_before,
                ..
            },
            Kind::MenuItem {
                label: after,
                shortcut: shortcut_after,
                ..
            },
        ) => before != after || shortcut_before != shortcut_after,
        (
            Kind::Choice {
                options: before, ..
            },
            Kind::Choice { options: after, .. },
        ) => before != after,
        (Kind::List { .. }, Kind::List { .. }) => before.kind != after.kind,
        // The room that a field keeps for its marks and its prefix.
        (
            Kind::TextField {
                search: before,
                multiline: lines_before,
                ..
            },
            Kind::TextField {
                search: after,
                multiline: lines_after,
                ..
            },
        ) => before != after || lines_before != lines_after,
        (Kind::NumberField { prefix: before, .. }, Kind::NumberField { prefix: after, .. }) => {
            before != after
        }
        _ => false,
    }
}

/// Whether `text` names `key`, as a rule names a key: a name as it is, a number after `#`.
fn named(key: &Key, text: &str) -> bool {
    match key {
        Key::Name(name) => **name == *text,
        Key::Index(index) => {
            text.strip_prefix('#')
                .and_then(|digits| digits.parse::<u64>().ok())
                == Some(*index)
        }
    }
}

/// The kind of an element, as a rule names it.
fn kind_name(kind: &Kind) -> KindName {
    match kind {
        Kind::Container => KindName::Container,
        Kind::Label(_) => KindName::Label,
        Kind::Image { .. } => KindName::Image,
        Kind::Button { .. } => KindName::Button,
        Kind::Scroll => KindName::Scroll,
        Kind::List { .. } => KindName::List,
        Kind::TextField { .. } => KindName::TextField,
        Kind::NumberField { .. } => KindName::NumberField,
        Kind::CheckBox { .. } => KindName::CheckBox,
        Kind::Slider { .. } => KindName::Slider,
        Kind::Section { .. } => KindName::Section,
        Kind::TabBar { .. } => KindName::TabBar,
        Kind::Tab { .. } => KindName::Tab,
        Kind::Split { .. } => KindName::Split,
        Kind::Menu => KindName::Menu,
        Kind::MenuBar => KindName::MenuBar,
        Kind::MenuItem { .. } => KindName::MenuItem,
        Kind::Choice { .. } => KindName::Choice,
        Kind::FloatingPanel { .. } => KindName::FloatingPanel,
        Kind::ToggleButton { .. } => KindName::ToggleButton,
        Kind::Segments { .. } => KindName::Segments,
        Kind::Segment { .. } => KindName::Segment,
        Kind::Progress { .. } => KindName::Progress,
    }
}

/// The styles made that are kept for one set of rules, past which an element's style that the
/// others are not is made again each time: what elements of styles of their own, sized each its
/// way, would otherwise keep.
const STYLES_PER_RULES: usize = 64;

/// Makes whole each color that `part`, a part of a value of the type `ty`, holds a part of, as
/// `full`, the whole value, holds it: a color is one property of a style, which an element's own
/// style sets whole, not some of its components over a rule's.
fn whole_colors(registry: &Registry, ty: &str, part: &mut Value, full: &Value) {
    if ty == "LinearColor" {
        *part = full.clone();
        return;
    }
    let Some(registration) = registry.get(ty) else {
        return;
    };
    let (slime_reflect::Kind::Struct(fields), Value::Struct(parts), Value::Struct(wholes)) =
        (&registration.info().kind, part, full)
    else {
        return;
    };
    for (name, part) in parts {
        let field = fields.iter().find(|field| field.name == *name);
        let whole = wholes.iter().find(|(whole, _)| whole == name);
        if let (Some(field), Some((_, whole))) = (field, whole) {
            whole_colors(registry, &field.ty, part, whole);
        }
    }
}
