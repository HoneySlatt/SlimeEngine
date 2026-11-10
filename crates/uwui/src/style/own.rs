//! An element's own style, written in code: the declarations that its code set, whatever their
//! values, for its base and for its states, with the place of the call that wrote them
//! (`docs/adr/0231-an-elements-own-style-written-in-code.md`).

use std::marker::PhantomData;
use std::panic::Location;

use slime_color::LinearColor;

use super::{
    Align, ControlStyleDeclaration, Dimension, Display, Easing, FlexDirection,
    GridPlaceDeclaration, IconsDeclaration, Justify, LayoutStyleDeclaration, PaintStyleDeclaration,
    Same, Shadow, Sides, SidesDeclaration, StyleDeclaration, TextLookDeclaration, Track,
    TransitionDeclaration,
};
use crate::text::{Alignment, Direction, TextStyleDeclaration};
use crate::theme::StateName;

/// What an element's code wrote of its style ([`Element::style`](crate::tree::Element::style)):
/// for its base and for each of its states, the declarations that it set, in order, whatever their
/// values. The interface styles the element with them over the rules, in the *mask order*
/// ([`Ui::set_style`](crate::tree::Ui::set_style)). Empty by default, holding nothing.
#[derive(Clone, Debug, Default)]
pub struct OwnStyle {
    declarations: Vec<Own>,
}

/// One declaration of an own style: the state it holds in, `None` for the base, and the place of
/// the call that wrote it.
#[derive(Clone, Debug)]
pub(crate) struct Own {
    state: Option<StateName>,
    declaration: StyleDeclaration,
    place: &'static Location<'static>,
}

/// The states that an own style may write, in the order that they apply, the last winning
/// (`docs/adr/0231-an-elements-own-style-written-in-code.md`).
pub(crate) const OWN_STATES: [StateName; 8] = [
    StateName::Hovered,
    StateName::Focused,
    StateName::FocusVisible,
    StateName::Pressed,
    StateName::Checked,
    StateName::Expanded,
    StateName::Selected,
    StateName::Disabled,
];

/// The bit of `state` in a set of own states, by its place in [`OWN_STATES`].
pub(crate) fn state_bit(state: StateName) -> u8 {
    let place = OWN_STATES
        .iter()
        .position(|own| *own == state)
        .expect("every state is an own state");
    1 << place
}

impl OwnStyle {
    /// Whether it sets nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// The places of the calls that wrote it, each once, in the order of the calls: what a live
    /// inspector or an agent opens to change it. A place is the compiler's, its path as the build
    /// gave it: relative to the workspace for its members, absolute for others, with `\` or `/`
    /// on Windows.
    #[must_use]
    pub fn places(&self) -> Vec<&'static Location<'static>> {
        let mut places: Vec<&'static Location<'static>> = Vec::new();
        for own in &self.declarations {
            if !places.contains(&own.place) {
                places.push(own.place);
            }
        }
        places
    }

    /// An own style of `declarations`, each in its state, `None` for the base, written here: what
    /// the tests of the cascade write, whatever the leaf.
    #[cfg(test)]
    #[track_caller]
    pub(crate) fn of(
        declarations: impl IntoIterator<Item = (Option<StateName>, StyleDeclaration)>,
    ) -> Self {
        let place = Location::caller();
        Self {
            declarations: declarations
                .into_iter()
                .map(|(state, declaration)| Own {
                    state,
                    declaration,
                    place,
                })
                .collect(),
        }
    }

    /// Adds what `builder` wrote after what it holds.
    pub(crate) fn extend(&mut self, builder: StyleBuilder) {
        if self.declarations.is_empty() {
            // The builder's list taken whole: no second allocation for an element's one call.
            self.declarations = builder.declarations;
        } else {
            self.declarations.extend(builder.declarations);
        }
    }

    /// The declarations of its base, in order.
    pub(crate) fn base(&self) -> impl Iterator<Item = &StyleDeclaration> {
        self.declarations
            .iter()
            .filter(|own| own.state.is_none())
            .map(|own| &own.declaration)
    }

    /// The declarations of `state`, in order.
    pub(crate) fn in_state(&self, state: StateName) -> impl Iterator<Item = &StyleDeclaration> {
        self.declarations
            .iter()
            .filter(move |own| own.state == Some(state))
            .map(|own| &own.declaration)
    }

    /// The states that it writes, as a set of [`state_bit`]s.
    pub(crate) fn states(&self) -> u8 {
        self.declarations
            .iter()
            .filter_map(|own| own.state)
            .fold(0, |states, state| states | state_bit(state))
    }

    /// Whether `other` writes the same declarations in the same states, their values compared by
    /// their bits, wherever they were written: such own styles make the same style.
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.declarations.len() == other.declarations.len()
            && self
                .declarations
                .iter()
                .zip(&other.declarations)
                .all(|(a, b)| a.state == b.state && a.declaration.same(&b.declaration))
    }
}

/// Two own styles are equal when they write the same declarations in the same states, their values
/// compared by their bits, at the same places: an element whose style moved in its code is given
/// again, so that its places are the code's.
impl PartialEq for OwnStyle {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
            && self
                .declarations
                .iter()
                .zip(&other.declarations)
                .all(|(a, b)| a.place == b.place)
    }
}

/// What writes an own style, one call for each field that it sets: `S` says where it writes,
/// [`Base`], a state that lays out, [`State`], or a state of the pointer, [`PointerState`], which
/// offers no field that lays out. [`Element::style`](crate::tree::Element::style),
/// [`Ui::add_root`](crate::tree::Ui::add_root) and [`Ui::open_layer`](crate::tree::Ui::open_layer)
/// give one to the function that writes
/// (`docs/adr/0231-an-elements-own-style-written-in-code.md`).
///
/// ```
/// use uwui::style::{Dimension, StyleBuilder};
/// use uwui::tree::Element;
/// use slime_color::LinearColor;
///
/// let surface = LinearColor::rgba(0.1, 0.1, 0.12, 1.0);
/// let lighter = LinearColor::rgba(0.16, 0.16, 0.18, 1.0);
/// let save = Element::button("Save").class("primary").style(|s| {
///     s.padding(12.0)
///         .width(Dimension::Pixels(120.0))
///         .background(surface)
///         .hover(|s| s.background(lighter))
/// });
/// // A helper writes a part of a style that several elements share.
/// fn column(s: StyleBuilder) -> StyleBuilder {
///     s.direction(uwui::style::FlexDirection::Column)
/// }
/// let panel = Element::container().style(column);
/// assert!(!panel.style.is_empty());
/// ```
///
/// A hover or a press cannot change the element's box (`docs/adr/0222-what-a-theme-is.md`): their
/// builder offers no field that lays out, and the compiler refuses the call.
///
/// ```compile_fail,E0599
/// use uwui::style::Dimension;
/// use uwui::tree::Element;
///
/// let grows = Element::button("Grows").style(|s| s.hover(|s| s.width(Dimension::Pixels(200.0))));
/// ```
#[derive(Debug)]
#[must_use]
pub struct StyleBuilder<S = Base> {
    declarations: Vec<Own>,
    state: Option<StateName>,
    place: &'static Location<'static>,
    writes: PhantomData<S>,
}

/// An own style's base: what holds in every state.
#[derive(Debug)]
pub struct Base;

/// An own style's state that may lay out: focused, its focus shown, checked, expanded, selected,
/// disabled.
#[derive(Debug)]
pub struct State;

/// An own style's state of the pointer, hovered or pressed, which does not change the element's
/// box (`docs/adr/0222-what-a-theme-is.md`).
#[derive(Debug)]
pub struct PointerState;

/// Where a [`StyleBuilder`] may set the fields that lay out: its base and the states that are not
/// the pointer's.
pub trait LaysOut: sealed::Sealed {}
impl LaysOut for Base {}
impl LaysOut for State {}

mod sealed {
    /// Keeps [`LaysOut`](super::LaysOut) to uwui's own kinds of builders.
    pub trait Sealed {}
    impl Sealed for super::Base {}
    impl Sealed for super::State {}
}

impl StyleBuilder {
    /// A builder of an own style's base, its declarations written at `place`.
    pub(crate) fn new(place: &'static Location<'static>) -> Self {
        Self {
            declarations: Vec::new(),
            state: None,
            place,
            writes: PhantomData,
        }
    }

    /// The declarations that `write` sets in `state`, after those before.
    fn in_state<T>(
        mut self,
        state: StateName,
        write: impl FnOnce(StyleBuilder<T>) -> StyleBuilder<T>,
    ) -> Self {
        let inner = StyleBuilder {
            declarations: std::mem::take(&mut self.declarations),
            state: Some(state),
            place: self.place,
            writes: PhantomData,
        };
        self.declarations = write(inner).declarations;
        self
    }

    /// What holds while the pointer is over the element: only a kind that takes the pointer, a
    /// button, a field or a control, is ever hovered. A hover set in code wins over the look's
    /// press too: an element given one should give [`StyleBuilder::press`].
    pub fn hover(
        self,
        write: impl FnOnce(StyleBuilder<PointerState>) -> StyleBuilder<PointerState>,
    ) -> Self {
        self.in_state(StateName::Hovered, write)
    }

    /// What holds while the element is pressed: a button or a control, not a field, which a press
    /// focuses.
    pub fn press(
        self,
        write: impl FnOnce(StyleBuilder<PointerState>) -> StyleBuilder<PointerState>,
    ) -> Self {
        self.in_state(StateName::Pressed, write)
    }

    /// What holds while the element holds the focus, however it was given.
    pub fn focus(self, write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>) -> Self {
        self.in_state(StateName::Focused, write)
    }

    /// What holds while the element holds a focus that shows: one that the keyboard, an agent or
    /// the program gave.
    pub fn focus_visible(
        self,
        write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>,
    ) -> Self {
        self.in_state(StateName::FocusVisible, write)
    }

    /// What holds while the element is checked.
    pub fn checked(self, write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>) -> Self {
        self.in_state(StateName::Checked, write)
    }

    /// What holds while the element is open.
    pub fn expanded(self, write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>) -> Self {
        self.in_state(StateName::Expanded, write)
    }

    /// What holds while the element is chosen: the tab or the segment of its bar, a row of a list
    /// that the program selected.
    pub fn selected(self, write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>) -> Self {
        self.in_state(StateName::Selected, write)
    }

    /// What holds while the element takes no input.
    pub fn disabled(self, write: impl FnOnce(StyleBuilder<State>) -> StyleBuilder<State>) -> Self {
        self.in_state(StateName::Disabled, write)
    }
}

impl<S> StyleBuilder<S> {
    /// Adds `declaration` in the builder's state.
    fn set(mut self, declaration: StyleDeclaration) -> Self {
        self.declarations.push(Own {
            state: self.state,
            declaration,
            place: self.place,
        });
        self
    }

    /// Adds the declarations of the leaves of `style` whose bits differ from [`Style::default`]'s,
    /// in the builder's state: what an own style written as a whole `Style` set before
    /// `docs/adr/0231-an-elements-own-style-written-in-code.md`, which uwui's own tests of the
    /// cascade and of what each kind reads still write.
    #[cfg(test)]
    pub(crate) fn with_style(self, style: &crate::style::Style) -> Self {
        let mut declarations = Vec::new();
        style.declared(&crate::style::Style::default(), &mut |declaration| {
            declarations.push(declaration);
        });
        declarations.into_iter().fold(self, Self::set)
    }
}

/// The setters of a style's leaves, each one declaration; and, for the test that finds one setter
/// for every leaf and none that lays out offered to a state of the pointer, the function `$list`
/// that gives their declarations.
macro_rules! setters {
    ($list:ident; $(
        $(#[$doc:meta])*
        $name:ident($value:ident: $ty:ty) => $declaration:expr;
    )*) => {
        $(
            $(#[$doc])*
            pub fn $name(self, $value: $ty) -> Self {
                self.set($declaration)
            }
        )*

        /// The declarations of these setters, each given a value of its type.
        #[cfg(test)]
        pub(crate) fn $list() -> Vec<StyleDeclaration> {
            vec![$({
                let $value: $ty = tests::Sample::sample();
                $declaration
            }),*]
        }
    };
}

/// The setters of what lays out, which a state of the pointer does not offer.
impl<S: LaysOut> StyleBuilder<S> {
    setters! {
        laying_out_declarations;
        /// How it lays its children out, or whether it is laid out at all.
        display(value: Display) => StyleDeclaration::Layout(LayoutStyleDeclaration::Display(value));
        /// The axis along which a flexbox lays its children out, and their order.
        direction(value: FlexDirection) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Direction(value));
        /// Whether a flexbox's children wrap onto more lines when they do not fit.
        wrap(value: bool) => StyleDeclaration::Layout(LayoutStyleDeclaration::Wrap(value));
        /// How children share the space along the main axis.
        justify(value: Justify) => StyleDeclaration::Layout(LayoutStyleDeclaration::Justify(value));
        /// How children are placed across the main axis, unless they say otherwise.
        align_items(value: Align) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::AlignItems(value));
        /// How it is placed across its parent's main axis; its parent's `align_items` for none.
        align_self(value: Option<Align>) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::AlignSelf(value));
        /// The space between its rows, in logical pixels.
        row_gap(value: f32) => StyleDeclaration::Layout(LayoutStyleDeclaration::Gap(0, value));
        /// The space between its columns, in logical pixels.
        column_gap(value: f32) => StyleDeclaration::Layout(LayoutStyleDeclaration::Gap(1, value));
        /// The space inside its edge where a line starts.
        padding_start(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Padding(SidesDeclaration::Start(value)));
        /// The space inside its edge where a line ends.
        padding_end(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Padding(SidesDeclaration::End(value)));
        /// The space inside its top edge.
        padding_top(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Padding(SidesDeclaration::Top(value)));
        /// The space inside its bottom edge.
        padding_bottom(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Padding(SidesDeclaration::Bottom(value)));
        /// The space outside its edge where a line starts.
        margin_start(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Margin(SidesDeclaration::Start(value)));
        /// The space outside its edge where a line ends.
        margin_end(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Margin(SidesDeclaration::End(value)));
        /// The space outside its top edge.
        margin_top(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Margin(SidesDeclaration::Top(value)));
        /// The space outside its bottom edge.
        margin_bottom(value: f32) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Margin(SidesDeclaration::Bottom(value)));
        /// Its width.
        width(value: Dimension) => StyleDeclaration::Layout(LayoutStyleDeclaration::Size(0, value));
        /// Its height.
        height(value: Dimension) => StyleDeclaration::Layout(LayoutStyleDeclaration::Size(1, value));
        /// Its smallest width.
        min_width(value: Dimension) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::MinSize(0, value));
        /// Its smallest height.
        min_height(value: Dimension) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::MinSize(1, value));
        /// Its largest width.
        max_width(value: Dimension) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::MaxSize(0, value));
        /// Its largest height.
        max_height(value: Dimension) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::MaxSize(1, value));
        /// How much of the space left along its parent's main axis it takes, against its siblings.
        grow(value: f32) => StyleDeclaration::Layout(LayoutStyleDeclaration::Grow(value));
        /// How much it gives up when its siblings lack space.
        shrink(value: f32) => StyleDeclaration::Layout(LayoutStyleDeclaration::Shrink(value));
        /// Its size along its parent's main axis before it grows or shrinks.
        basis(value: Dimension) => StyleDeclaration::Layout(LayoutStyleDeclaration::Basis(value));
        /// A grid's columns, from the start, set whole.
        columns(value: Vec<Track>) => StyleDeclaration::Layout(LayoutStyleDeclaration::Columns(value));
        /// A grid's rows, from the top, set whole.
        rows(value: Vec<Track>) => StyleDeclaration::Layout(LayoutStyleDeclaration::Rows(value));
        /// The column line of its grid parent that it starts at, numbered from 1; after the
        /// children before it for none.
        column_start(value: Option<u16>) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Column(GridPlaceDeclaration::Start(value)));
        /// How many of its grid parent's columns it spans, at least 1.
        column_span(value: u16) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Column(GridPlaceDeclaration::Span(value)));
        /// The row line of its grid parent that it starts at, numbered from 1; after the children
        /// before it for none.
        row_start(value: Option<u16>) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Row(GridPlaceDeclaration::Start(value)));
        /// How many of its grid parent's rows it spans, at least 1.
        row_span(value: u16) =>
            StyleDeclaration::Layout(LayoutStyleDeclaration::Row(GridPlaceDeclaration::Span(value)));
        /// Its border's width, in logical pixels, inside its edges.
        border_width(value: f32) => StyleDeclaration::Paint(PaintStyleDeclaration::BorderWidth(value));
        /// Its text's font families, in order of preference, set whole.
        fonts(value: Vec<String>) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Fonts(value)));
        /// Its text's size, the font's em, in logical pixels.
        font_size(value: f32) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Size(value)));
        /// Its text's weight, from 100 to 900.
        font_weight(value: f32) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Weight(value)));
        /// Whether its text is italic.
        italic(value: bool) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Italic(value)));
        /// The height of its text's lines, as a multiple of the fonts' own.
        line_height(value: f32) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::LineHeight(value)));
        /// Where its text's lines go across its box.
        text_align(value: Alignment) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Alignment(value)));
        /// The direction its text is written in.
        text_direction(value: Direction) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Direction(value)));
        /// Its text's language, a BCP 47 tag; none for no language.
        locale(value: Option<String>) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Locale(value)));
        /// The OpenType features that its text's fonts turn on, by their tags, set whole.
        font_features(value: Vec<String>) =>
            StyleDeclaration::Text(TextLookDeclaration::Style(TextStyleDeclaration::Features(value)));
        /// The size of the part of its control that the pointer takes, in logical pixels.
        control_size(value: f32) => StyleDeclaration::Control(ControlStyleDeclaration::Size(value));
    }

    /// The same space inside each of its edges, or each its own: `padding(12.0)`.
    pub fn padding(self, sides: impl Into<Sides>) -> Self {
        let Sides {
            start,
            end,
            top,
            bottom,
        } = sides.into();
        self.padding_start(start)
            .padding_end(end)
            .padding_top(top)
            .padding_bottom(bottom)
    }

    /// The same space outside each of its edges, or each its own.
    pub fn margin(self, sides: impl Into<Sides>) -> Self {
        let Sides {
            start,
            end,
            top,
            bottom,
        } = sides.into();
        self.margin_start(start)
            .margin_end(end)
            .margin_top(top)
            .margin_bottom(bottom)
    }

    /// The same space between its rows and between its columns.
    pub fn gap(self, space: f32) -> Self {
        self.row_gap(space).column_gap(space)
    }
}

/// The setters of what only draws, which every builder offers.
impl<S> StyleBuilder<S> {
    setters! {
        drawing_declarations;
        /// Its box's fill.
        background(value: LinearColor) =>
            StyleDeclaration::Paint(PaintStyleDeclaration::Background(value));
        /// Its border's color.
        border_color(value: LinearColor) =>
            StyleDeclaration::Paint(PaintStyleDeclaration::Border(value));
        /// The radius of its corner at the top, where a line starts, in logical pixels.
        radius_top_start(value: f32) => StyleDeclaration::Paint(PaintStyleDeclaration::Radii(0, value));
        /// The radius of its corner at the top, where a line ends.
        radius_top_end(value: f32) => StyleDeclaration::Paint(PaintStyleDeclaration::Radii(1, value));
        /// The radius of its corner at the bottom, where a line ends.
        radius_bottom_end(value: f32) => StyleDeclaration::Paint(PaintStyleDeclaration::Radii(2, value));
        /// The radius of its corner at the bottom, where a line starts.
        radius_bottom_start(value: f32) =>
            StyleDeclaration::Paint(PaintStyleDeclaration::Radii(3, value));
        /// The shadow it casts, set whole; none for none.
        shadow(value: Option<Shadow>) => StyleDeclaration::Paint(PaintStyleDeclaration::Shadow(value));
        /// How opaque it and what it holds are drawn, from 0 to 1.
        opacity(value: f32) => StyleDeclaration::Paint(PaintStyleDeclaration::Opacity(value));
        /// The fill that a layer's root lays over the window below it.
        backdrop(value: LinearColor) => StyleDeclaration::Paint(PaintStyleDeclaration::Backdrop(value));
        /// Its text's color.
        text_color(value: LinearColor) => StyleDeclaration::Text(TextLookDeclaration::Color(value));
        /// The fill behind the text that a field holds selected.
        selection_color(value: LinearColor) =>
            StyleDeclaration::Text(TextLookDeclaration::Selection(value));
        /// The radius of its control's corners, in logical pixels.
        control_radius(value: f32) => StyleDeclaration::Control(ControlStyleDeclaration::Radius(value));
        /// Its control's track: a check box unchecked, a slider's track, a field's placeholder.
        track_color(value: LinearColor) => StyleDeclaration::Control(ControlStyleDeclaration::Track(value));
        /// Its control's fill: a check box checked, a slider up to its value.
        fill_color(value: LinearColor) => StyleDeclaration::Control(ControlStyleDeclaration::Fill(value));
        /// Its control's mark: a tick, a thumb, a chevron.
        mark_color(value: LinearColor) => StyleDeclaration::Control(ControlStyleDeclaration::Mark(value));
        /// How long a change of its style takes, in seconds.
        transition_duration(value: f32) =>
            StyleDeclaration::Transition(TransitionDeclaration::Duration(value));
        /// How a change of its style goes over that time.
        transition_easing(value: Easing) =>
            StyleDeclaration::Transition(TransitionDeclaration::Easing(value));
        /// The family of the font of icons that its control draws from; empty for its shapes.
        icon_font(value: String) => StyleDeclaration::Icons(IconsDeclaration::Font(value));
        /// The glyph of a tick.
        icon_tick(value: String) => StyleDeclaration::Icons(IconsDeclaration::Tick(value));
        /// The glyph of a chevron down.
        icon_down(value: String) => StyleDeclaration::Icons(IconsDeclaration::Down(value));
        /// The glyph of a chevron towards the right.
        icon_right(value: String) => StyleDeclaration::Icons(IconsDeclaration::Right(value));
        /// The glyph of a chevron towards the left.
        icon_left(value: String) => StyleDeclaration::Icons(IconsDeclaration::Left(value));
        /// The glyph of a magnifier.
        icon_search(value: String) => StyleDeclaration::Icons(IconsDeclaration::Search(value));
        /// The glyph of a cross that empties a field of search.
        icon_clear(value: String) => StyleDeclaration::Icons(IconsDeclaration::Clear(value));
    }

    /// The same radius at each of its corners.
    pub fn radius(self, radius: f32) -> Self {
        self.radius_top_start(radius)
            .radius_top_end(radius)
            .radius_bottom_end(radius)
            .radius_bottom_start(radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    /// A value of a leaf's type, which the lists of setters give their declarations.
    pub(super) trait Sample {
        fn sample() -> Self;
    }

    macro_rules! sample_by_default {
        ($($ty:ty),* $(,)?) => {
            $(impl Sample for $ty {
                fn sample() -> Self {
                    Self::default()
                }
            })*
        };
    }

    sample_by_default!(
        Display,
        FlexDirection,
        bool,
        Justify,
        Align,
        f32,
        Dimension,
        u16,
        Alignment,
        Direction,
        Easing,
        String,
        Option<Align>,
        Option<u16>,
        Option<String>,
        Option<Shadow>,
        Vec<Track>,
        Vec<String>,
    );

    impl Sample for LinearColor {
        fn sample() -> Self {
            LinearColor::WHITE
        }
    }

    #[test]
    fn every_leaf_has_one_setter_offered_to_a_state_of_the_pointer_unless_it_lays_out() {
        let laying_out = StyleBuilder::<Base>::laying_out_declarations();
        let drawing = StyleBuilder::<PointerState>::drawing_declarations();
        assert!(
            laying_out.iter().all(StyleDeclaration::lays_out),
            "the setters that a state of the pointer lacks lay out"
        );
        let offered: Vec<String> = drawing
            .iter()
            .filter(|declaration| declaration.lays_out())
            .map(StyleDeclaration::path)
            .collect();
        assert!(
            offered.is_empty(),
            "a state of the pointer is offered setters that lay out: {offered:?}"
        );
        let mut leaves: Vec<usize> = laying_out
            .iter()
            .chain(&drawing)
            .map(StyleDeclaration::leaf)
            .collect();
        leaves.sort_unstable();
        assert_eq!(leaves, (0..Style::LEAVES).collect::<Vec<_>>());
    }
}
