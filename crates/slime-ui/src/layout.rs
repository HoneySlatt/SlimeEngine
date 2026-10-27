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
    // A section that starts to open was laid out open, to measure the height that it grows to:
    // laid out again at the height that it starts from.
    if ui.measure_reveals(root) {
        convert(ui, root, None, settings, false);
        let mut tree = Tree { ui, scale };
        compute_root_layout(&mut tree, root_node, available);
        round_layout(&mut tree, root_node);
    }
    for list in ui.lists(root) {
        lay_out_rows(ui, list, settings);
    }
    // A layer is placed once its content is laid out, beside what it stands beside.
    let settings = ui.place_layer(root).unwrap_or(settings);
    let origin = settings.origin.map(|at| at * scale);
    place(ui, (root, origin[0], origin[1], None), settings);
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
        let header = section_header(ui, id, scale);
        let panel = split_panel(ui, id);
        let revealed = ui.reveal_height(id);
        let fits = ui.fits_content(id);
        let floating = ui.floating_size(id);
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
            // A root is as large as its surface, but a layer that takes the size of its content,
            // and a floating one, of its own size, as tall as its content where its panel is
            // folded.
            if let Some(size) = floating {
                let length = |side: Option<f32>| {
                    side.map_or(TaffyDimension::auto(), |side| {
                        TaffyDimension::length(side * scale)
                    })
                };
                style.size = Size {
                    width: length(size[0]),
                    height: length(size[1]),
                };
            } else if node.root.is_some() && !fits {
                style.size = Size {
                    width: TaffyDimension::length(settings.size[0] * scale),
                    height: TaffyDimension::length(settings.size[1] * scale),
                };
            }
            // A floating panel fills its layer, as tall as its bar once folded.
            if let Kind::FloatingPanel { .. } = node.element.kind {
                style.size = Size {
                    width: TaffyDimension::percent(1.0),
                    height: if node.state.expanded {
                        TaffyDimension::percent(1.0)
                    } else {
                        TaffyDimension::auto()
                    },
                };
            }
            // A section's header lies above its children, within its padding.
            if let Some(header) = header {
                let top = style.padding.top.into_raw().value();
                style.padding.top = LengthPercentage::length(top + header);
            }
            // Two panels and a handle: the panels in a row or a column, the handle the gap
            // between them; the first takes its share, the second the rest, the others nothing.
            if let Kind::Split { side_by_side, .. } = node.element.kind {
                style.display = taffy::Display::Flex;
                style.flex_direction = if side_by_side {
                    taffy::FlexDirection::Row
                } else {
                    taffy::FlexDirection::Column
                };
                style.align_items = taffy::AlignItems::STRETCH;
                let handle = LengthPercentage::length(node.style.control.size * scale / 2.0);
                style.gap = Size {
                    width: handle,
                    height: handle,
                };
            }
            match panel {
                Some(Panel::First(share)) => {
                    style.flex_basis = TaffyDimension::percent(share);
                    style.flex_grow = 0.0;
                    style.flex_shrink = 1.0;
                }
                Some(Panel::Second) => {
                    style.flex_basis = TaffyDimension::length(0.0);
                    style.flex_grow = 1.0;
                    style.flex_shrink = 1.0;
                }
                None => {}
            }
            // A section that grows or shrinks takes its height now, what it holds cut to it.
            if let Some(height) = revealed {
                style.size.height = TaffyDimension::length(height);
                style.overflow.y = Overflow::Hidden;
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

/// What a panel of two panels and a handle takes.
enum Panel {
    /// The first panel, which takes this share of their length.
    First(f32),
    /// The second, which takes the rest.
    Second,
}

/// What `id` takes as a panel of two panels and a handle; none for an element that is no such
/// panel. A child past the second is folded, its style of no display (`Ui::folded`).
fn split_panel(ui: &Ui, id: ElementId) -> Option<Panel> {
    let parent = ui.node(id)?.parent?;
    let share = ui.split_share(parent)?;
    match ui.children(parent).iter().position(|&child| child == id)? {
        0 => Some(Panel::First(share)),
        1 => Some(Panel::Second),
        _ => None,
    }
}

/// The height of the header of `id`, a section or a floating panel, in pixels of the screen at
/// `scale`: its title's, or its mark's where it is taller. None for another element.
fn section_header(ui: &mut Ui, id: ElementId, scale: f32) -> Option<f32> {
    let node = ui.node(id)?;
    let (Kind::Section { title, .. } | Kind::FloatingPanel { title, .. }) = &node.element.kind
    else {
        return None;
    };
    let title = title.clone();
    let text_style = node.style.text.style.clone();
    let mark = node.style.control.size * scale;
    let text = ui
        .texts
        .get(
            &mut ui.fonts,
            &title,
            &text_style,
            scale,
            TextBounds::default(),
        )
        .height()
        .ceil();
    Some(text.max(mark))
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
    // A mirror across the root's box, at its place on the surface.
    let across = 2.0 * settings.origin[0] * settings.scale + settings.size[0] * settings.scale;
    let Some(node) = ui.node(id) else {
        return;
    };
    // Where its parent's content starts, before the mirror.
    let left = match settings.direction {
        LayoutDirection::LeftToRight => node.rect.x,
        LayoutDirection::RightToLeft => across - node.rect.x - node.rect.width,
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
    // A mirror across the root's box, at its place on the surface: as wide as its surface, or as
    // its content for a layer that takes the size of its content.
    let width = ui
        .root_of(start.0)
        .and_then(|root| ui.node(root))
        .map_or(settings.size[0] * scale, |root| root.layout.size.width);
    let across = 2.0 * settings.origin[0] * scale + width;
    let mut stack = vec![start];
    // The elements that moved, or whose cut changed, which changes where they show to whoever
    // reads what the interface means; a root whose box changed, what shows of all it holds.
    let mut moved = Vec::new();
    while let Some((id, x, y, clip)) = stack.pop() {
        let Some(node) = ui.node_mut(id) else {
            continue;
        };
        node.changed.layout = false;
        let layout = node.layout;
        let (left, top) = (x + layout.location.x, y + layout.location.y);
        let mut rect = Rect::new(left, top, layout.size.width, layout.size.height);
        if direction == LayoutDirection::RightToLeft {
            rect.x = across - rect.x - rect.width;
        }
        if node.rect != rect || node.clip != clip {
            node.rect = rect;
            node.clip = clip;
            node.changed.paint = true;
            moved.push((id, node.root.is_some()));
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
            // A section that grows or shrinks cuts what it holds to its height now.
            Kind::Section { .. } if node.reveal.is_some() => {
                (left, top, Some(intersect(clip, inside)))
            }
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
    for (id, root) in moved {
        if root {
            ui.mark_meaning_below(id);
        } else {
            ui.mark_meaning(id);
        }
    }
}

/// `offset`, a scroll in logical pixels of an element of `kind` laid out as `layout`, kept within
/// its content: none for an element that does not scroll.
pub(crate) fn within(offset: [f32; 2], kind: &Kind, layout: &Layout, scale: f32) -> [f32; 2] {
    let most = scroll_most(kind, layout, scale);
    [0, 1].map(|axis| offset[axis].clamp(0.0, most[axis] / scale))
}

/// How far the content of an element of `kind` laid out as `layout` scrolls at most, along x and
/// y, in pixels of the screen at `scale`: 0 for an element that does not scroll, or whose content
/// fits.
pub(crate) fn scroll_most(kind: &Kind, layout: &Layout, scale: f32) -> [f32; 2] {
    match *kind {
        Kind::Scroll => [layout.scroll_width(), layout.scroll_height()],
        Kind::List { rows, row_height } => {
            let height =
                rows as f32 * row_height * scale + layout.padding.top + layout.padding.bottom;
            [0.0, (height - layout.size.height).max(0.0)]
        }
        _ => [0.0; 2],
    }
}

/// `rect` cut to `clip`, when there is one.
pub(crate) fn intersect(clip: Option<Rect>, rect: Rect) -> Rect {
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
            // What a control lays out besides its text: a check box's box and the gap after it,
            // as wide as half of it; a slider is eight thumbs wide, a thumb tall; an item of a
            // menu, its mark and its chevron, each with its gap, whether it shows them or not, so
            // that a menu's labels line up, and its shortcut, two marks after its label, unless a
            // bar of menus holds it; a list to choose from, its mark and the gap before it.
            let control = at.style.control.size * tree.scale;
            // An item of a bar of menus lays out its label alone.
            let in_bar = at
                .parent
                .and_then(|parent| tree.ui.node(parent))
                .is_some_and(|parent| matches!(parent.element.kind, Kind::MenuBar));
            let (mut beside, least) = match at.element.kind {
                Kind::CheckBox { .. } => (control * 1.5, control),
                Kind::Slider { .. } => (control * 8.0, control),
                Kind::MenuItem { .. } if in_bar => (0.0, 0.0),
                Kind::MenuItem { .. } => (control * 3.0, control),
                Kind::Choice { .. } => (control * 1.5, control),
                Kind::Progress { .. } => (control * 8.0, control / 2.0),
                _ => (0.0, 0.0),
            };
            if let Kind::MenuItem { shortcut, .. } = &at.element.kind
                && !shortcut.is_empty()
                && !in_bar
            {
                let (shortcut, text_style, scale) =
                    (shortcut.clone(), at.style.text.style.clone(), tree.scale);
                let ui = &mut *tree.ui;
                let width = ui
                    .texts
                    .get(
                        &mut ui.fonts,
                        &shortcut,
                        &text_style,
                        scale,
                        TextBounds::default(),
                    )
                    .width();
                beside += control * 2.0 + width.ceil();
            }
            // A tab's count, after its title, in a box half a mark wider than its digits.
            let at = tree.ui.node_at(usize::from(node));
            if let Kind::Tab {
                count: Some(count), ..
            } = at.element.kind
            {
                let (text_style, scale) = (at.style.text.style.clone(), tree.scale);
                let ui = &mut *tree.ui;
                let width = ui
                    .texts
                    .get(
                        &mut ui.fonts,
                        &count.to_string(),
                        &text_style,
                        scale,
                        TextBounds::default(),
                    )
                    .width();
                beside += width.ceil() + control;
            }
            // A list to choose from is as wide as its widest option.
            let at = tree.ui.node_at(usize::from(node));
            let widest = match &at.element.kind {
                Kind::Choice { options, .. } => {
                    let (options, text_style, scale) =
                        (options.clone(), at.style.text.style.clone(), tree.scale);
                    let ui = &mut *tree.ui;
                    let mut widest = (0.0, String::new());
                    for option in options {
                        let width = ui
                            .texts
                            .get(
                                &mut ui.fonts,
                                &option,
                                &text_style,
                                scale,
                                TextBounds::default(),
                            )
                            .width();
                        if width > widest.0 {
                            widest = (width, option);
                        }
                    }
                    Some(widest.1)
                }
                _ => None,
            };
            let at = tree.ui.node_at(usize::from(node));
            let text = match &at.element.kind {
                Kind::Choice { .. } => widest,
                Kind::Label(text) => Some(text.clone()),
                Kind::Button { label, .. }
                | Kind::CheckBox { label, .. }
                | Kind::Tab { title: label, .. }
                | Kind::MenuItem { label, .. }
                | Kind::ToggleButton { label, .. }
                | Kind::Segment { title: label } => Some(label.clone()),
                // A field as tall as a line of its text, even empty.
                Kind::TextField { .. } | Kind::NumberField { .. } => {
                    at.field.as_ref().map(|field| match field.text.as_str() {
                        "" => " ".to_owned(),
                        text => text.to_owned(),
                    })
                }
                _ => None,
            };
            // A field of one line does not break its text: it scrolls it; nor does an item of a
            // menu, nor a list to choose from.
            let breaks = !matches!(
                at.element.kind,
                Kind::TextField {
                    multiline: false,
                    ..
                } | Kind::NumberField { .. }
                    | Kind::MenuItem { .. }
                    | Kind::Choice { .. }
            );
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
                        return Size {
                            width: beside,
                            height: least,
                        };
                    };
                    let width = known
                        .width
                        .or(match available.width {
                            AvailableSpace::Definite(width) => Some(width),
                            AvailableSpace::MinContent => Some(0.0),
                            AvailableSpace::MaxContent => None,
                        })
                        .map(|width| (width - beside).max(0.0));
                    let bounds = width
                        .filter(|_| breaks)
                        .map_or(TextBounds::default(), |width| {
                            TextBounds::width(width / scale)
                        });
                    let ui = &mut *tree.ui;
                    let layout = ui
                        .texts
                        .get(&mut ui.fonts, &text, &text_style, scale, bounds);
                    Size {
                        width: layout.width().ceil() + beside,
                        height: layout.height().ceil().max(least),
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
