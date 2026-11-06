//! What the prototypes of SlimeEngine's milestone M6d.1b write their screens with, as the screens'
//! ideal code in uwui's repository names it (`docs/ideal-code/README.md` there): elements made
//! without a style, which the style sheets style by their classes, and the control's [`Program`].
//! Built on a branch for the prototypes, then thrown away
//! (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::time::Instant;

use crate::style::{Dimension, FlexDirection, LayoutStyle, Style};
use crate::tree::{self, Kind};

pub use crate::action::{Action, ActionKind, FieldValue, Intent};
pub use crate::display::ImageKey;
pub use crate::tree::{Children, ElementId, Ui};

/// An application in the control, today's rebuild by keys: the host calls, at each frame,
/// `advance` with the frame's time, before the frame's input is handled, then `act` for each
/// action of the input, then `build`, then lays out and draws.
pub trait Program {
    /// Builds the interface under `root`: each builder through [`Ui::build`], which runs it when
    /// the revision it is given moved.
    fn build(&mut self, ui: &mut Ui, root: ElementId);

    /// Does what `action` asks.
    fn act(&mut self, ui: &mut Ui, action: Action) {
        let _ = (ui, action);
    }

    /// Takes the frame's time, `now`.
    fn advance(&mut self, ui: &mut Ui, now: Instant) {
        let _ = (ui, now);
    }
}

/// Makes elements without a style: the style sheets style them by their classes. Each gives a
/// [`tree::Element`].
pub struct Element;

impl Element {
    /// A box that lays its children out in a column.
    pub fn column() -> tree::Element {
        tree::Element::container(laid_out(FlexDirection::Column))
    }

    /// A box that lays its children out in a row.
    pub fn row() -> tree::Element {
        tree::Element::container(Style::default())
    }

    /// A box whose children lie over each other, the last on top.
    pub fn stack() -> tree::Element {
        tree::Element::of(Kind::Stack, Style::default())
    }

    /// A box that takes presses and holds what its builder adds.
    pub fn pressable() -> tree::Element {
        tree::Element::of(Kind::Pressable, laid_out(FlexDirection::Column))
    }

    /// A text.
    pub fn label(text: impl Into<String>) -> tree::Element {
        tree::Element::label(text, Style::default())
    }

    /// A button, its label `text`.
    pub fn button(text: impl Into<String>) -> tree::Element {
        tree::Element::button(text, Style::default())
    }

    /// An image, named by its key.
    pub fn image(image: ImageKey) -> tree::Element {
        tree::Element::image(image, Style::default())
    }

    /// A field of search, holding `text`.
    pub fn search_field(text: impl Into<String>) -> tree::Element {
        tree::Element::search_field(text, Style::default())
    }

    /// A list of `rows` rows of `row_height` logical pixels, which builds those that show.
    pub fn list(rows: usize, row_height: f32) -> tree::Element {
        tree::Element::list(rows, row_height, Style::default())
    }

    /// A grid of `cells` cells of `cell` logical pixels, which builds those that show.
    pub fn grid(cells: usize, cell: [f32; 2]) -> tree::Element {
        tree::Element::grid(cells, cell, Style::default())
    }

    /// A wave of progress, `share` played, its sine at `phase`: 480 by 24 logical pixels, which
    /// no rule changes.
    pub fn wave(share: f32, phase: f32) -> tree::Element {
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(480.0), Dimension::Pixels(24.0)];
        tree::Element::of(Kind::Wave { share, phase }, style)
    }
}

/// A style that lays children out in `direction`, and sets nothing else.
fn laid_out(direction: FlexDirection) -> Style {
    Style {
        layout: LayoutStyle {
            direction,
            ..LayoutStyle::default()
        },
        ..Style::default()
    }
}
