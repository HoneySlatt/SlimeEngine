//! The tree laid out by taffy's flexbox and grid, through its traits on the tree's own arena, in
//! pixels of the screen, then mirrored for a root that lays its lines out from right to left.
//!
//! taffy does not see a list's rows: each is laid out alone, at the list's width and the height
//! of a row, and placed from its number, so that a list costs what it shows, and a change in its
//! rows lays nothing else out again.

use taffy::prelude::{
    AvailableSpace, Dimension as TaffyDimension, Layout, LengthPercentage, LengthPercentageAuto,
    NodeId, Size,
};
use taffy::{
    CacheTree, LayoutInput, LayoutOutput, Overflow, Point, RoundTree, RunMode, TraversePartialTree,
    TraverseTree, compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
    compute_hidden_layout, compute_leaf_layout, compute_root_layout, round_layout,
};

use crate::Rect;
use crate::style::{
    Align, Dimension, Display, FlexDirection, GridPlace, Justify, LayoutStyle, Sides, Track,
};
use crate::text::TextBounds;
use crate::tree::{ElementId, Key, Kind, LayoutDirection, RootSettings, Ui};

/// Lays out `root` and what lies below it: their styles converted where they changed, taffy run
/// from the root, the rows of its lists laid out, then every element placed.
pub(crate) fn lay_out(ui: &mut Ui, root: ElementId) {
    let Some(settings) = ui.node(root).and_then(|node| node.root) else {
        return;
    };
    let scale = settings.scale;
    convert(ui, root, None, settings, false);
    let available = Size {
        width: AvailableSpace::Definite(settings.size[0] * scale),
        height: AvailableSpace::Definite(settings.size[1] * scale),
    };
    let root_node = NodeId::from(root.index());
    let mut tree = Tree { ui, scale };
    compute_root_layout(&mut tree, root_node, available);
    round_layout(&mut tree, root_node);
    for list in ui.lists(root) {
        lay_out_rows(ui, list, settings);
    }
    place(ui, (root, 0.0, 0.0, None), settings);
}

/// Lays out the rows of the list `list` again, with the rows of the lists that they hold, and
/// places them: they changed, and nothing else did. Gives those lists, `list` first.
pub(crate) fn lay_out_list(ui: &mut Ui, list: ElementId) -> Vec<ElementId> {
    let Some(settings) = ui.root_settings(list) else {
        return Vec::new();
    };
    let row_height = ui.row_height(list);
    for row in ui.children(list).to_vec() {
        convert(ui, row, row_height, settings, false);
    }
    // Each list after the one that holds it, whose rows give it its box.
    let lists = ui.lists(list);
    for &list in &lists {
        lay_out_rows(ui, list, settings);
    }
    place_below(ui, list);
    lists
}

/// Converts the styles of `start` and the elements below it that changed into taffy's, `start`
/// being a row of `row_height` logical pixels when given, and converted whatever changed when
/// `forced`.
fn convert(
    ui: &mut Ui,
    start: ElementId,
    row_height: Option<f32>,
    settings: RootSettings,
    forced: bool,
) {
    let scale = settings.scale;
    let mut stack = vec![(start, row_height, forced)];
    while let Some((id, row_height, forced)) = stack.pop() {
        let Some(node) = ui.node_mut(id) else {
            continue;
        };
        if forced {
            node.cache.clear();
        }
        if node.changed.layout || forced {
            let mut style = taffy_style(&node.style.layout, scale);
            if let Some(row_height) = row_height {
                style.size = Size {
                    width: TaffyDimension::percent(1.0),
                    height: TaffyDimension::length(row_height * scale),
                };
            }
            if node.root.is_some() {
                style.size = Size {
                    width: TaffyDimension::length(settings.size[0] * scale),
                    height: TaffyDimension::length(settings.size[1] * scale),
                };
            }
            // A box that scrolls, as CSS's `overflow: hidden`, is as small as its style lets it,
            // whatever its content.
            if matches!(node.element.kind, Kind::Scroll | Kind::List { .. }) {
                style.overflow = Point {
                    x: Overflow::Hidden,
                    y: Overflow::Hidden,
                };
            }
            node.taffy = style;
        }
        let rows = match node.element.kind {
            Kind::List { row_height, .. } => Some(row_height),
            _ => None,
        };
        // A list that changed may have changed the height of its rows.
        let forced = rows.is_some() && node.changed.layout;
        stack.extend(node.children.iter().map(|&child| (child, rows, forced)));
    }
}

/// Lays out each row of the list `list` alone, at the list's width and the height of a row.
fn lay_out_rows(ui: &mut Ui, list: ElementId, settings: RootSettings) {
    let Some(row_height) = ui.row_height(list) else {
        return;
    };
    let Some(node) = ui.node(list) else {
        return;
    };
    let width = node.layout.content_box_width();
    let rows = node.children.clone();
    let scale = settings.scale;
    let mut tree = Tree { ui, scale };
    for row in rows {
        let row_node = NodeId::from(row.index());
        compute_root_layout(
            &mut tree,
            row_node,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::Definite(row_height * scale),
            },
        );
        round_layout(&mut tree, row_node);
    }
}

/// Places `id` and the elements below it again, without laying them out: its content scrolled,
/// or its rows laid out again.
pub(crate) fn place_below(ui: &mut Ui, id: ElementId) {
    let Some(settings) = ui.root_settings(id) else {
        return;
    };
    let width = settings.size[0] * settings.scale;
    let Some(node) = ui.node(id) else {
        return;
    };
    // Where its parent's content starts, before the mirror.
    let left = match settings.direction {
        LayoutDirection::LeftToRight => node.rect.x,
        LayoutDirection::RightToLeft => width - node.rect.x - node.rect.width,
    };
    let start = (
        id,
        left - node.layout.location.x,
        node.rect.y - node.layout.location.y,
        node.clip,
    );
    place(ui, start, settings);
}

/// Places the element of `start`, its parent's content starting at `start`'s point and cut to its
/// rectangle, and the elements below it: each element's box on its root, from its layout and
/// those of its ancestors, the content of a box that scrolls and of a list moved by its scroll,
/// kept within the content; the whole mirrored for a root that lays its lines out from right to
/// left; and the rectangle each is cut to, by the boxes that scroll above it. An element whose
/// box or cut moves is drawn again.
fn place(ui: &mut Ui, start: (ElementId, f32, f32, Option<Rect>), settings: RootSettings) {
    let (scale, direction) = (settings.scale, settings.direction);
    let width = settings.size[0] * scale;
    let mut stack = vec![start];
    while let Some((id, x, y, clip)) = stack.pop() {
        let Some(node) = ui.node_mut(id) else {
            continue;
        };
        node.changed.layout = false;
        let layout = node.layout;
        let (left, top) = (x + layout.location.x, y + layout.location.y);
        let mut rect = Rect::new(left, top, layout.size.width, layout.size.height);
        if direction == LayoutDirection::RightToLeft {
            rect.x = width - rect.x - rect.width;
        }
        if node.rect != rect || node.clip != clip {
            node.rect = rect;
            node.clip = clip;
            node.changed.paint = true;
        }
        node.scroll = within(node.scroll, &node.element.kind, &layout, scale);
        // Content is cut inside the border, as CSS cuts it at the padding's edge.
        let border = node.style.paint.border_width * scale;
        let inside = Rect::new(
            rect.x + border,
            rect.y + border,
            (rect.width - 2.0 * border).max(0.0),
            (rect.height - 2.0 * border).max(0.0),
        );
        // The content scrolls by whole pixels, so that it is drawn as sharp as it stands.
        let scroll = node.scroll.map(|offset| (offset * scale).round());
        let (content_x, content_y, inner) = match node.element.kind {
            Kind::Scroll => (
                left - scroll[0],
                top - scroll[1],
                Some(intersect(clip, inside)),
            ),
            Kind::List { .. } => (left, top, Some(intersect(clip, inside))),
            _ => (left, top, clip),
        };
        stack.extend(
            node.children
                .iter()
                .map(|&child| (child, content_x, content_y, inner)),
        );
        // Each row of a list at its place, from its number.
        if let Kind::List { row_height, .. } = node.element.kind {
            for child in node.children.clone() {
                let Some(row) = ui.node_mut(child) else {
                    continue;
                };
                let number = match row.key {
                    Key::Index(number) => number as f32,
                    Key::Name(_) => 0.0,
                };
                let margin = row.layout.margin;
                row.layout.location.x = layout.padding.left + margin.left;
                row.layout.location.y =
                    layout.padding.top + margin.top + (number * row_height * scale).round()
                        - scroll[1];
            }
        }
    }
}

/// `offset`, a scroll in logical pixels of an element of `kind` laid out as `layout`, kept within
/// its content: none for an element that does not scroll.
pub(crate) fn within(offset: [f32; 2], kind: &Kind, layout: &Layout, scale: f32) -> [f32; 2] {
    let most = match *kind {
        Kind::Scroll => [layout.scroll_width(), layout.scroll_height()],
        Kind::List { rows, row_height } => {
            let height =
                rows as f32 * row_height * scale + layout.padding.top + layout.padding.bottom;
            [0.0, (height - layout.size.height).max(0.0)]
        }
        _ => [0.0; 2],
    };
    [0, 1].map(|axis| offset[axis].clamp(0.0, most[axis] / scale))
}

/// `rect` cut to `clip`, when there is one.
fn intersect(clip: Option<Rect>, rect: Rect) -> Rect {
    let Some(clip) = clip else {
        return rect;
    };
    let left = rect.x.max(clip.x);
    let top = rect.y.max(clip.y);
    let right = (rect.x + rect.width).min(clip.x + clip.width);
    let bottom = (rect.y + rect.height).min(clip.y + clip.height);
    Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

/// `style` as taffy reads it, its lengths in pixels of the screen at `scale`.
fn taffy_style(style: &LayoutStyle, scale: f32) -> taffy::Style {
    let length = |value: f32| LengthPercentage::length(value * scale);
    let sides = |sides: Sides| taffy::Rect {
        left: length(sides.start),
        right: length(sides.end),
        top: length(sides.top),
        bottom: length(sides.bottom),
    };
    let margin = |sides: Sides| taffy::Rect {
        left: LengthPercentageAuto::length(sides.start * scale),
        right: LengthPercentageAuto::length(sides.end * scale),
        top: LengthPercentageAuto::length(sides.top * scale),
        bottom: LengthPercentageAuto::length(sides.bottom * scale),
    };
    let dimension = |value: Dimension| match value {
        Dimension::Auto => TaffyDimension::auto(),
        Dimension::Pixels(pixels) => TaffyDimension::length(pixels * scale),
        Dimension::Share(share) => TaffyDimension::percent(share),
    };
    let size = |values: [Dimension; 2]| Size {
        width: dimension(values[0]),
        height: dimension(values[1]),
    };
    let bound = |value: Dimension| match value {
        Dimension::Auto => LengthPercentageAuto::auto(),
        Dimension::Pixels(pixels) => LengthPercentageAuto::length(pixels * scale),
        Dimension::Share(share) => LengthPercentageAuto::percent(share),
    };
    let bounds = |values: [Dimension; 2]| Size {
        width: bound(values[0]),
        height: bound(values[1]),
    };
    let align = |align: Align| match align {
        Align::Start => taffy::AlignItems::START,
        Align::End => taffy::AlignItems::END,
        Align::Center => taffy::AlignItems::CENTER,
        Align::Stretch => taffy::AlignItems::STRETCH,
    };
    let track = |track: Track| match track {
        Track::Pixels(pixels) => taffy::prelude::length(pixels * scale),
        Track::Fraction(share) => taffy::prelude::fr(share),
        Track::Auto => taffy::prelude::auto(),
    };
    let place = |place: GridPlace| {
        let span = taffy::prelude::span(place.span.max(1));
        taffy::Line {
            start: place.start.map_or(taffy::GridPlacement::Auto, |line| {
                taffy::prelude::line(i16::try_from(line).unwrap_or(i16::MAX))
            }),
            end: span,
        }
    };
    taffy::Style {
        display: match style.display {
            Display::Flex => taffy::Display::Flex,
            Display::Grid => taffy::Display::Grid,
            Display::None => taffy::Display::None,
        },
        flex_direction: match style.direction {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        flex_wrap: if style.wrap {
            taffy::FlexWrap::Wrap
        } else {
            taffy::FlexWrap::NoWrap
        },
        justify_content: match style.justify {
            Justify::Start => taffy::JustifyContent::START,
            Justify::End => taffy::JustifyContent::END,
            Justify::Center => taffy::JustifyContent::CENTER,
            Justify::SpaceBetween => taffy::JustifyContent::SPACE_BETWEEN,
            Justify::SpaceAround => taffy::JustifyContent::SPACE_AROUND,
            Justify::SpaceEvenly => taffy::JustifyContent::SPACE_EVENLY,
        },
        align_items: align(style.align_items),
        align_self: style.align_self.map(align),
        gap: Size {
            width: length(style.gap[1]),
            height: length(style.gap[0]),
        },
        padding: sides(style.padding),
        margin: margin(style.margin),
        size: size(style.size),
        min_size: bounds(style.min_size),
        max_size: bounds(style.max_size),
        flex_grow: style.grow,
        flex_shrink: style.shrink,
        flex_basis: dimension(style.basis),
        grid_template_columns: style.columns.iter().map(|&t| track(t)).collect(),
        grid_template_rows: style.rows.iter().map(|&t| track(t)).collect(),
        grid_column: place(style.column),
        grid_row: place(style.row),
        ..taffy::Style::default()
    }
}

/// The tree as taffy walks it, at the scale of the root it lays out.
struct Tree<'a> {
    ui: &'a mut Ui,
    scale: f32,
}

/// The children that taffy lays out: none for a list, whose rows are laid out apart.
pub(crate) struct Children<'a> {
    ids: std::slice::Iter<'a, ElementId>,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        self.ids.next().map(|&id| NodeId::from(id.index()))
    }
}

impl Tree<'_> {
    fn laid_out_children(&self, node: NodeId) -> &[ElementId] {
        let node = self.ui.node_at(usize::from(node));
        match node.element.kind {
            Kind::List { .. } => &[],
            _ => &node.children,
        }
    }
}

impl TraversePartialTree for Tree<'_> {
    type ChildIter<'b>
        = Children<'b>
    where
        Self: 'b;

    fn child_ids(&self, node: NodeId) -> Self::ChildIter<'_> {
        Children {
            ids: self.laid_out_children(node).iter(),
        }
    }

    fn child_count(&self, node: NodeId) -> usize {
        self.laid_out_children(node).len()
    }

    fn get_child_id(&self, node: NodeId, index: usize) -> NodeId {
        NodeId::from(self.laid_out_children(node)[index].index())
    }
}

impl TraverseTree for Tree<'_> {}

impl taffy::LayoutPartialTree for Tree<'_> {
    type CustomIdent = String;

    type CoreContainerStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    fn get_core_container_style(&self, node: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.ui.node_at(usize::from(node)).taffy
    }

    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        self.ui.node_at_mut(usize::from(node)).unrounded = *layout;
    }

    fn compute_child_layout(&mut self, node: NodeId, inputs: LayoutInput) -> LayoutOutput {
        // Below an element whose display is none, nothing takes space.
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, node);
        }
        compute_cached_layout(self, node, inputs, |tree, node, inputs| {
            let at = tree.ui.node_at(usize::from(node));
            let display = at.taffy.display;
            if display == taffy::Display::None {
                return compute_hidden_layout(tree, node);
            }
            let text = match &at.element.kind {
                Kind::Label(text) => Some(text.clone()),
                Kind::Button { label, .. } => Some(label.clone()),
                _ => None,
            };
            if !tree.laid_out_children(node).is_empty() {
                return match display {
                    taffy::Display::Grid => compute_grid_layout(tree, node, inputs),
                    _ => compute_flexbox_layout(tree, node, inputs),
                };
            }
            let style = at.taffy.clone();
            let text_style = at.style.text.style.clone();
            let scale = tree.scale;
            compute_leaf_layout(
                inputs,
                &style,
                |_, _| 0.0,
                |known, available| {
                    let Some(text) = text else {
                        return Size::ZERO;
                    };
                    let width = known.width.or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        AvailableSpace::MinContent => Some(0.0),
                        AvailableSpace::MaxContent => None,
                    });
                    let bounds = width.map_or(TextBounds::default(), |width| {
                        TextBounds::width(width / scale)
                    });
                    let ui = &mut *tree.ui;
                    let layout = ui
                        .texts
                        .get(&mut ui.fonts, &text, &text_style, scale, bounds);
                    Size {
                        width: layout.width().ceil(),
                        height: layout.height().ceil(),
                    }
                },
            )
        })
    }
}

impl taffy::LayoutContainingBlock for Tree<'_> {
    type OofItemStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    fn get_oof_item_style(&self, node: NodeId) -> Self::OofItemStyle<'_> {
        &self.ui.node_at(usize::from(node)).taffy
    }

    // No element is placed out of its parent's flow: none is hoisted.
    fn clear_hoisted_children(&mut self, _node: NodeId) {}

    fn add_hoisted_children(&mut self, _node: NodeId, _hoisted: &[NodeId]) {}
}

impl CacheTree for Tree<'_> {
    fn cache_get(&mut self, node: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.ui.node_at_mut(usize::from(node)).cache.get(inputs)
    }

    fn cache_store(&mut self, node: NodeId, inputs: &LayoutInput, output: LayoutOutput) {
        self.ui
            .node_at_mut(usize::from(node))
            .cache
            .store(inputs, output);
    }

    fn cache_clear(&mut self, node: NodeId) {
        self.ui.node_at_mut(usize::from(node)).cache.clear();
    }
}

impl taffy::LayoutFlexboxContainer for Tree<'_> {
    type FlexboxContainerStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    type FlexboxItemStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    fn get_flexbox_container_style(&self, node: NodeId) -> Self::FlexboxContainerStyle<'_> {
        &self.ui.node_at(usize::from(node)).taffy
    }

    fn get_flexbox_child_style(&self, child: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.ui.node_at(usize::from(child)).taffy
    }
}

impl taffy::LayoutGridContainer for Tree<'_> {
    type GridContainerStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    type GridItemStyle<'b>
        = &'b taffy::Style
    where
        Self: 'b;

    fn get_grid_container_style(&self, node: NodeId) -> Self::GridContainerStyle<'_> {
        &self.ui.node_at(usize::from(node)).taffy
    }

    fn get_grid_child_style(&self, child: NodeId) -> Self::GridItemStyle<'_> {
        &self.ui.node_at(usize::from(child)).taffy
    }
}

impl RoundTree for Tree<'_> {
    fn get_unrounded_layout(&self, node: NodeId) -> Layout {
        self.ui.node_at(usize::from(node)).unrounded
    }

    fn set_final_layout(&mut self, node: NodeId, layout: &Layout) {
        self.ui.node_at_mut(usize::from(node)).layout = *layout;
    }

    fn is_out_of_flow(&self, _node: NodeId) -> bool {
        false
    }

    fn hoisted_child_count(&self, _node: NodeId) -> usize {
        0
    }

    fn get_hoisted_child_id(&self, _node: NodeId, _index: usize) -> NodeId {
        unreachable!("no element is hoisted")
    }
}
