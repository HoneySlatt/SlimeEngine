//! Layers (`docs/adr/0150-layers.md`): roots that the framework places beside an element, at a
//! point, or where a floating panel stands, over the root of what they stand over; that close as
//! the user leaves them, a press outside them and outside what opened them, or Escape; and that
//! give the focus back to what held it as they close.

use super::{ElementId, Kind, RootSettings, Ui};
use crate::Rect;
use crate::action::{Action, ActionKind};
use crate::style::Style;

/// The room left between a layer and the element that it stands beside, in logical pixels.
const GAP: f32 = 4.0;

/// Where a layer stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Beside `element`, on its `side`, aligned to the `along` edge of it: a popup, a menu, a list
    /// to choose from. It takes the size of its content; where its side lacks room, it stands on
    /// the other side, and it moves along the element's edge to stay within the window.
    Beside {
        /// What it stands beside, which closes it as it goes.
        element: ElementId,
        /// The side of the element that it stands on.
        side: Side,
        /// The edge of the element's side that it lines up with.
        along: Along,
    },
    /// Its top start corner at `point`, in logical pixels of the surface of `over`'s root: a
    /// context menu at the pointer. It takes the size of its content, and turns up or toward the
    /// start where it lacks room.
    At {
        /// An element of the root whose surface, scale and direction it takes, which closes it as
        /// it goes.
        over: ElementId,
        /// Where its top start corner goes, in logical pixels.
        point: [f32; 2],
    },
    /// At `origin`, of `size`, in logical pixels of the surface of `over`'s root: a floating panel,
    /// which the user moves and sizes then.
    Floating {
        /// An element of the root whose surface, scale and direction it takes.
        over: ElementId,
        /// Where its top left corner goes, in logical pixels.
        origin: [f32; 2],
        /// Its width and height, in logical pixels.
        size: [f32; 2],
    },
}

/// The side of an element that a layer stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Below it.
    Below,
    /// Above it.
    Above,
    /// Before its start: its left from left to right.
    Start,
    /// After its end: its right from left to right.
    End,
}

/// The edge of an element's side that a layer lines up with: along a line for a layer above or
/// below it, from top to bottom for one before its start or after its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    /// Its start: the layer's start on the element's, or its top on the element's top.
    Start,
    /// Its middle.
    Center,
    /// Its end: the layer's end on the element's, or its bottom on the element's bottom.
    End,
}

/// A layer that the framework places ([`Ui::open_layer`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// Where it stands.
    pub place: Place,
    /// Whether it is modal: while it is open, the roots below it take no input, and the focus stays
    /// among its elements, as a modal root's does.
    pub modal: bool,
    /// Whether it closes as the user leaves it: a press outside it and outside the element that it
    /// stands beside, or Escape while the focus is in it, on that element or nowhere.
    pub dismiss: bool,
}

/// What the interface keeps of a layer, on its root.
pub(crate) struct Held {
    pub(crate) layer: Layer,
    /// The widget that opened it and owns it, which closes it; none for one that the program
    /// opened.
    pub(crate) owner: Option<ElementId>,
    /// The box that its anchor had as it was last placed, in pixels of the surface.
    anchored: Rect,
    /// The element that held the focus as it opened, which takes it back as it closes.
    before: Option<ElementId>,
    /// Whether its first element that takes the focus takes it once built: it opened by the
    /// keyboard, an agent or the program.
    focus_first: bool,
    /// Whether it is a tooltip, which the pointer passes through and whose closing tells no one.
    pub(crate) tooltip: bool,
}

impl Place {
    /// The element that it stands beside or over.
    fn anchor(&self) -> ElementId {
        match *self {
            Self::Beside { element, .. } => element,
            Self::At { over, .. } | Self::Floating { over, .. } => over,
        }
    }
}

impl Ui {
    /// Opens a layer, a root over the others of `style`, placed as `layer` says each frame: built
    /// as any root is ([`Ui::build`]), and closed by [`Ui::remove`] or, for a layer that closes as
    /// the user leaves it, by the interface, which tells the program
    /// ([`ActionKind::Dismissed`]). It takes the scale and the direction of the root that holds
    /// what it stands beside or over, and closes with it. A layer opened while the last input was
    /// not a press of the pointer gives its first element that takes the focus the focus, once
    /// built.
    pub fn open_layer(&mut self, layer: Layer, style: Style) -> ElementId {
        let root = self.add_layer(layer, style);
        // A button or an item of a menu shows that what it opened is open.
        if let Place::Beside { element, .. } = layer.place
            && self.opens_layers(element)
        {
            self.change_state(element, |state| state.expanded = true);
        }
        root
    }

    /// Adds the root of a layer, as [`Ui::open_layer`] says, without telling what it stands beside.
    pub(crate) fn add_layer(&mut self, layer: Layer, style: Style) -> ElementId {
        let settings = self
            .root_of(layer.place.anchor())
            .and_then(|root| self.node(root)?.root)
            .unwrap_or(RootSettings {
                size: [1.0, 1.0],
                scale: 1.0,
                direction: super::LayoutDirection::LeftToRight,
                origin: [0.0; 2],
                modal: false,
            });
        let before = self.interaction.focus;
        let focus_first = !self.interaction.by_pointer;
        let root = self.add_root(
            RootSettings {
                size: self.surface_size(),
                origin: [0.0; 2],
                modal: layer.modal,
                ..settings
            },
            style,
        );
        if let Some(node) = self.node_mut(root) {
            node.layer = Some(Box::new(Held {
                layer,
                owner: None,
                anchored: Rect::default(),
                before,
                focus_first,
                tooltip: false,
            }));
        }
        root
    }

    /// Opens a layer as [`Ui::open_layer`] does, that the widget `owner` owns: it closes it, and
    /// reads that it closed, rather than the program.
    pub(crate) fn open_owned_layer(
        &mut self,
        owner: ElementId,
        layer: Layer,
        style: Style,
    ) -> ElementId {
        let root = self.add_layer(layer, style);
        if let Some(held) = self.node_mut(root).and_then(|node| node.layer.as_mut()) {
            held.owner = Some(owner);
            // The widget gives the focus as it builds the layer.
            held.focus_first = false;
        }
        root
    }

    /// The widget that owns the layer `root`; none for one that the program opened, or a root that
    /// is no layer.
    pub(crate) fn layer_owner(&self, root: ElementId) -> Option<ElementId> {
        self.node(root)?.layer.as_ref()?.owner
    }

    /// The layer that the widget `owner` owns, open.
    pub(crate) fn owned_layer(&self, owner: ElementId) -> Option<ElementId> {
        self.layers()
            .into_iter()
            .find(|&root| self.layer_owner(root) == Some(owner))
    }

    /// Moves the layer `root` to `place`, as the user moving or sizing a floating panel does: it
    /// is laid out and placed again at the next frame. Nothing happens for a root that is no
    /// layer.
    pub fn set_layer_place(&mut self, root: ElementId, place: Place) {
        let Some(held) = self.node_mut(root).and_then(|node| node.layer.as_mut()) else {
            return;
        };
        if held.layer.place == place {
            return;
        }
        held.layer.place = place;
        self.mark_layout(root);
    }

    /// The size of the window's surface, in logical pixels: as far as the roots that are no layers
    /// reach, from its top left corner, a dialog placed within the window's root included.
    pub(crate) fn surface_size(&self) -> [f32; 2] {
        self.roots
            .iter()
            .filter_map(|&root| self.node(root).filter(|node| node.layer.is_none())?.root)
            .fold([1.0_f32; 2], |size, root| {
                [
                    size[0].max(root.origin[0] + root.size[0]),
                    size[1].max(root.origin[1] + root.size[1]),
                ]
            })
    }

    /// Where the layer `root` stands now; none for a root that is no layer.
    pub fn layer_place(&self, root: ElementId) -> Option<Place> {
        Some(self.node(root)?.layer.as_ref()?.layer.place)
    }

    /// The layers, from the bottom: the roots that the interface places.
    pub(crate) fn layers(&self) -> Vec<ElementId> {
        self.roots
            .iter()
            .copied()
            .filter(|&root| self.node(root).is_some_and(|node| node.layer.is_some()))
            .collect()
    }

    /// Whether `root` is a layer that takes the size of its content.
    pub(crate) fn fits_content(&self, root: ElementId) -> bool {
        self.node(root)
            .and_then(|node| node.layer.as_ref())
            .is_some_and(|held| !matches!(held.layer.place, Place::Floating { .. }))
    }

    /// Gives each layer the scale and the direction of the root that it stands over, and the size
    /// of the window's surface, and closes those whose anchor is gone. Run before a frame lays out.
    pub(crate) fn settle_layers(&mut self) {
        for root in self.layers() {
            let Some(held) = self.node(root).and_then(|node| node.layer.as_ref()) else {
                continue;
            };
            let anchor = held.layer.place.anchor();
            let base = self
                .root_of(anchor)
                .filter(|&base| base != root)
                .and_then(|base| self.node(base)?.root);
            let Some(base) = base else {
                self.close_layer(root, true);
                continue;
            };
            let Some(settings) = self.node(root).and_then(|node| node.root) else {
                continue;
            };
            let wanted = RootSettings {
                size: self.surface_size(),
                origin: settings.origin,
                modal: settings.modal,
                ..base
            };
            if wanted != settings {
                self.set_root(root, wanted);
            }
        }
    }

    /// Lays out again the layers whose anchor moved since they were placed: what moved it, a
    /// layout or a scroll, ran this frame. Gives them.
    pub(crate) fn follow_anchors(&mut self) -> Vec<ElementId> {
        let mut moved = Vec::new();
        for root in self.layers() {
            let Some(held) = self.node(root).and_then(|node| node.layer.as_ref()) else {
                continue;
            };
            let anchored = held.anchored;
            let Some(rect) = self.node(held.layer.place.anchor()).map(|node| node.rect) else {
                continue;
            };
            if rect != anchored {
                self.mark_layout(root);
                crate::layout::lay_out(self, root);
                moved.push(root);
            }
        }
        moved
    }

    /// Places the layer `root`, whose content was laid out: its origin, in logical pixels, as its
    /// place asks, within the surface of the root that it stands over; the settings to place it
    /// with. None for a root that is no layer.
    pub(crate) fn place_layer(&mut self, root: ElementId) -> Option<RootSettings> {
        let node = self.node(root)?;
        let held = node.layer.as_ref()?;
        let settings = node.root?;
        let scale = settings.scale;
        let size = [node.layout.size.width, node.layout.size.height];
        let place = held.layer.place;
        let anchor = self.node(place.anchor())?.rect;
        // Within the window's surface, whose size every layer takes, a submenu's too.
        let bounds = Rect::new(0.0, 0.0, settings.size[0] * scale, settings.size[1] * scale);
        let rtl = settings.direction == super::LayoutDirection::RightToLeft;
        let gap = GAP * scale;
        let origin = match place {
            Place::Beside { side, along, .. } => {
                beside(anchor, size, bounds, side, along, gap, rtl)
            }
            Place::At { point, .. } => {
                let at = [point[0] * scale, point[1] * scale];
                // A menu at the pointer turns toward the start, or up, where it lacks room.
                let mut x = if rtl { at[0] - size[0] } else { at[0] };
                if x + size[0] > bounds.x + bounds.width {
                    x = at[0] - size[0];
                }
                if x < bounds.x {
                    x = at[0];
                }
                let mut y = at[1];
                if y + size[1] > bounds.y + bounds.height {
                    y = at[1] - size[1];
                }
                within([x, y], size, bounds)
            }
            Place::Floating { origin, .. } => {
                within([origin[0] * scale, origin[1] * scale], size, bounds)
            }
        };
        let settings = RootSettings {
            origin: [origin[0] / scale, origin[1] / scale],
            ..settings
        };
        let node = self.node_mut(root)?;
        node.root = Some(settings);
        if let Some(held) = node.layer.as_mut() {
            held.anchored = anchor;
        }
        Some(settings)
    }

    /// Closes the dismissable layers above the root that a press at `x` and `y` falls in, but those
    /// beside the element that it falls in, down to the first modal root, which keeps the press:
    /// they close as the user leaves them.
    pub(crate) fn dismiss_outside(&mut self, x: f32, y: f32) {
        let hit = self.element_at(x, y);
        let hit_root = hit.and_then(|id| self.root_of(id));
        let below = hit_root
            .and_then(|root| self.roots.iter().position(|&each| each == root))
            .unwrap_or(0);
        let above: Vec<ElementId> = self.roots.iter().skip(below + 1).copied().collect();
        for root in above.into_iter().rev() {
            // A modal root keeps the press: neither it nor what lies below closes.
            if self
                .node(root)
                .and_then(|node| node.root)
                .is_some_and(|settings| settings.modal)
            {
                return;
            }
            let Some(held) = self.node(root).and_then(|node| node.layer.as_ref()) else {
                continue;
            };
            if !held.layer.dismiss {
                continue;
            }
            // A press on the element that a layer stands beside reaches it, which closes the
            // layer as it likes; a layer at a point or floating has no such element.
            if let Place::Beside { element, .. } = held.layer.place
                && hit.is_some_and(|hit| self.within(hit, element))
            {
                continue;
            }
            self.close_layer(root, true);
        }
    }

    /// Closes the top layer that closes as the user leaves it, for Escape, while the focus is in
    /// it, on what opened it, or nowhere: whether one did.
    pub(crate) fn dismiss_top(&mut self) -> bool {
        let focus = self.interaction.focus;
        for root in self.roots.clone().into_iter().rev() {
            let Some(held) = self.node(root).and_then(|node| node.layer.as_ref()) else {
                if self
                    .node(root)
                    .and_then(|node| node.root)
                    .is_some_and(|root| root.modal)
                {
                    return false;
                }
                continue;
            };
            if !held.layer.dismiss {
                continue;
            }
            let opener = held.layer.place.anchor();
            let reaches = focus.is_none_or(|focus| {
                self.root_of(focus) == Some(root) || self.within(focus, opener)
            });
            if reaches {
                self.close_layer(root, true);
                return true;
            }
            return false;
        }
        false
    }

    /// Closes the layer `root` and the layers that stand over what it holds, the focus going back
    /// to what held it as it opened; `dismissed` when the user left it, which tells the program,
    /// for a layer that it opened.
    pub(crate) fn close_layer(&mut self, root: ElementId, dismissed: bool) {
        let Some(held) = self.node_mut(root).and_then(|node| node.layer.take()) else {
            return;
        };
        // What stands over what it holds goes with it, from the top.
        for other in self.layers().into_iter().rev() {
            let anchor = self
                .node(other)
                .and_then(|node| node.layer.as_ref())
                .map(|held| held.layer.place.anchor());
            if anchor.is_some_and(|anchor| self.root_of(anchor) == Some(root)) {
                self.close_layer(other, dismissed);
            }
        }
        let focused_inside = self
            .interaction
            .focus
            .is_some_and(|focus| self.root_of(focus) == Some(root));
        self.remove(root);
        if focused_inside {
            let back = held
                .before
                .filter(|&before| self.focusable(before))
                .or_else(|| Some(held.layer.place.anchor()).filter(|&a| self.focusable(a)));
            self.set_focus(back);
        }
        if held.tooltip {
            self.interaction.tooltip = None;
            return;
        }
        if let Place::Beside { element, .. } = held.layer.place
            && held.owner.is_none()
            && self.opens_layers(element)
        {
            self.change_state(element, |state| state.expanded = false);
        }
        match held.owner {
            Some(owner) => self.owned_layer_closed(owner),
            None if dismissed => self.interaction.actions.push(Action {
                element: Some(root),
                kind: ActionKind::Dismissed,
            }),
            None => {}
        }
    }

    /// Gives the first element of each layer that opened by the keyboard the focus, once its
    /// builder has given it elements.
    pub(crate) fn focus_new_layers(&mut self) {
        for root in self.layers() {
            let wanted = self
                .node(root)
                .and_then(|node| node.layer.as_ref())
                .is_some_and(|held| held.focus_first);
            if !wanted || self.children(root).is_empty() {
                continue;
            }
            if let Some(held) = self.node_mut(root).and_then(|node| node.layer.as_mut()) {
                held.focus_first = false;
            }
            if let Some(first) = self.first_focusable(root) {
                self.set_focus(Some(first));
            }
        }
    }

    /// The first element below `root`, in the order of the tree, that can hold the focus.
    pub(crate) fn first_focusable(&self, root: ElementId) -> Option<ElementId> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let node = self.node(id)?;
            if id != root && self.focusable(id) {
                return Some(id);
            }
            if !matches!(node.element.kind, Kind::List { .. }) || id == root {
                stack.extend(node.children.iter().rev().copied());
            }
        }
        None
    }

    /// Whether `id` is a button or an item of a menu, which shows that the layer open beside it is
    /// open.
    fn opens_layers(&self, id: ElementId) -> bool {
        self.node(id).is_some_and(|node| {
            matches!(
                node.element.kind,
                Kind::Button { .. } | Kind::MenuItem { .. }
            )
        })
    }

    /// What a widget does as the layer that it owns closes: it is no longer open.
    fn owned_layer_closed(&mut self, owner: ElementId) {
        self.change_state(owner, |state| state.expanded = false);
    }

    /// Whether `id` is `ancestor` or lies below it.
    pub(crate) fn within(&self, id: ElementId, ancestor: ElementId) -> bool {
        let mut at = Some(id);
        while let Some(each) = at {
            if each == ancestor {
                return true;
            }
            at = self.node(each).and_then(|node| node.parent);
        }
        false
    }
}

/// The top left corner of a layer of `size` beside `anchor`, on `side`, along its `along` edge,
/// `gap` apart, turned to the other side where `bounds` leave no room, and kept within `bounds`.
fn beside(
    anchor: Rect,
    size: [f32; 2],
    bounds: Rect,
    side: Side,
    along: Along,
    gap: f32,
    rtl: bool,
) -> [f32; 2] {
    let (right, bottom) = (bounds.x + bounds.width, bounds.y + bounds.height);
    // Start and end from the root's direction: a side across the line, and the alignment along
    // it of a layer above or below; beside its start or end, the alignment runs from top to bottom.
    let (side, along) = if rtl {
        let side = match side {
            Side::Start => Side::End,
            Side::End => Side::Start,
            other => other,
        };
        let along = match (side, along) {
            (Side::Below | Side::Above, Along::Start) => Along::End,
            (Side::Below | Side::Above, Along::End) => Along::Start,
            (_, other) => other,
        };
        (side, along)
    } else {
        (side, along)
    };
    let line_up = |start: f32, length: f32, own: f32| match along {
        Along::Start => start,
        Along::Center => start + (length - own) / 2.0,
        Along::End => start + length - own,
    };
    let origin = match side {
        Side::Below | Side::Above => {
            let below = anchor.y + anchor.height + gap;
            let above = anchor.y - gap - size[1];
            let y = match side {
                Side::Below if below + size[1] > bottom && above >= bounds.y => above,
                Side::Above if above < bounds.y && below + size[1] <= bottom => below,
                Side::Below => below,
                _ => above,
            };
            [line_up(anchor.x, anchor.width, size[0]), y]
        }
        Side::Start | Side::End => {
            let after = anchor.x + anchor.width + gap;
            let before = anchor.x - gap - size[0];
            let x = match side {
                Side::End if after + size[0] > right && before >= bounds.x => before,
                Side::Start if before < bounds.x && after + size[0] <= right => after,
                Side::End => after,
                _ => before,
            };
            [x, line_up(anchor.y, anchor.height, size[1])]
        }
    };
    within(origin, size, bounds)
}

/// `origin` moved as little as keeps a box of `size` within `bounds`, its top left corner first
/// where it is larger.
fn within(origin: [f32; 2], size: [f32; 2], bounds: Rect) -> [f32; 2] {
    let x = origin[0]
        .min(bounds.x + bounds.width - size[0])
        .max(bounds.x);
    let y = origin[1]
        .min(bounds.y + bounds.height - size[1])
        .max(bounds.y);
    [x, y]
}
