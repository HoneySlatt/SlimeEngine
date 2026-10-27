//! Floating panels (`docs/adr/0150-layers.md`): the bar that the pointer drags to move a panel's
//! layer, the corner that it drags to size it, and the press that raises the layer above the
//! others.

use super::{ElementId, Kind, Place, Ui};
use crate::Rect;

/// The least size of a floating panel that the pointer sizes, in its control's sizes.
const LEAST_SIZE: [f32; 2] = [8.0, 4.0];

/// A floating panel that the pointer holds by its bar or its corner.
#[derive(Clone, Copy, Debug)]
pub(super) struct Grab {
    /// The panel's layer.
    root: ElementId,
    /// Whether the pointer holds its corner, which sizes it, rather than its bar, which moves it.
    sizing: bool,
    /// Where the pointer pressed, in pixels of the screen.
    from: [f32; 2],
    /// The layer's place as the pointer pressed, in logical pixels: its origin and its size.
    origin: [f32; 2],
    size: [f32; 2],
}

impl Ui {
    /// What the pointer pressing the floating panel `id` at `x` and `y` holds: its corner, or its
    /// bar but its mark, which a press folds. None elsewhere, or for another element.
    pub(super) fn panel_grab(&self, id: ElementId, x: f32, y: f32) -> Option<Grab> {
        let node = self.node(id)?;
        if !matches!(node.element.kind, Kind::FloatingPanel { .. }) {
            return None;
        }
        let root = self.root_of(id)?;
        let Some(Place::Floating { origin, size, .. }) = self.layer_place(root) else {
            return None;
        };
        let grab = |sizing| Grab {
            root,
            sizing,
            from: [x, y],
            origin,
            size,
        };
        if self
            .panel_corner(id)
            .is_some_and(|corner| super::interact::inside(corner, x, y))
        {
            return Some(grab(true));
        }
        let header = self.section_header(id)?;
        let mark = self.panel_mark(id)?;
        (super::interact::inside(header, x, y) && !super::interact::inside(mark, x, y))
            .then(|| grab(false))
    }

    /// The pointer that holds a floating panel moved to `x` and `y`: its layer moves, or is sized,
    /// by as much, no smaller than a few of its control's sizes.
    pub(super) fn drag_panel(&mut self, grab: Grab, x: f32, y: f32) {
        let scale = self.scale_of(grab.root);
        let moved = [(x - grab.from[0]) / scale, (y - grab.from[1]) / scale];
        let Some(Place::Floating { over, .. }) = self.layer_place(grab.root) else {
            return;
        };
        let rtl = self.root_settings(grab.root).map(|root| root.direction)
            == Some(super::LayoutDirection::RightToLeft);
        let place = if grab.sizing {
            let least = self
                .children(grab.root)
                .first()
                .and_then(|&panel| self.node(panel))
                .map_or(0.0, |panel| panel.style.control.size);
            // From right to left, the corner is at the left: the panel grows towards it.
            let width_moved = if rtl { -moved[0] } else { moved[0] };
            let size = [
                (grab.size[0] + width_moved).max(least * LEAST_SIZE[0]),
                (grab.size[1] + moved[1]).max(least * LEAST_SIZE[1]),
            ];
            let origin = if rtl {
                [grab.origin[0] + grab.size[0] - size[0], grab.origin[1]]
            } else {
                grab.origin
            };
            Place::Floating { over, origin, size }
        } else {
            // Kept within the window, as it is placed.
            let window = self
                .root_of(over)
                .and_then(|base| self.node(base)?.root)
                .map_or([f32::INFINITY; 2], |base| base.size);
            let height = self
                .node(grab.root)
                .map_or(grab.size[1], |node| node.rect.height / scale);
            let origin = [
                (grab.origin[0] + moved[0])
                    .min(window[0] - grab.size[0])
                    .max(0.0),
                (grab.origin[1] + moved[1]).min(window[1] - height).max(0.0),
            ];
            Place::Floating {
                over,
                origin,
                size: grab.size,
            }
        };
        self.set_layer_place(grab.root, place);
    }

    /// The corner of the open floating panel `id` that the pointer drags to size it, at the end of
    /// its bottom, a control's size square, in pixels of the surface.
    pub(crate) fn panel_corner(&self, id: ElementId) -> Option<Rect> {
        let node = self.node(id)?;
        if !matches!(node.element.kind, Kind::FloatingPanel { .. }) || !node.state.expanded {
            return None;
        }
        let side = node.style.control.size * self.scale_of(id);
        let rect = node.rect;
        let x = match self.root_settings(id)?.direction {
            super::LayoutDirection::LeftToRight => rect.x + rect.width - side,
            super::LayoutDirection::RightToLeft => rect.x,
        };
        Some(Rect::new(x, rect.y + rect.height - side, side, side))
    }

    /// The mark of the floating panel `id`'s bar, which a press folds it by, in pixels of the
    /// surface.
    fn panel_mark(&self, id: ElementId) -> Option<Rect> {
        let header = self.section_header(id)?;
        let side = self.node(id)?.style.control.size * self.scale_of(id);
        let x = match self.root_settings(id)?.direction {
            super::LayoutDirection::LeftToRight => header.x,
            super::LayoutDirection::RightToLeft => header.x + header.width - side,
        };
        Some(Rect::new(x, header.y, side, header.height))
    }

    /// Raises the floating layer that holds `id` above the others, a press having reached it.
    pub(super) fn raise_panel(&mut self, id: ElementId) {
        let Some(root) = self.root_of(id) else {
            return;
        };
        if !matches!(self.layer_place(root), Some(Place::Floating { .. }))
            || self.roots.last() == Some(&root)
        {
            return;
        }
        self.roots.retain(|&each| each != root);
        self.roots.push(root);
        self.top_changed = true;
    }

    /// The size of the floating layer `root`, in logical pixels, the panel that it holds folded to
    /// its bar leaving its height to its content; none for another root.
    pub(crate) fn floating_size(&self, root: ElementId) -> Option<[Option<f32>; 2]> {
        let Some(Place::Floating { size, .. }) = self.layer_place(root) else {
            return None;
        };
        let folded = self
            .children(root)
            .first()
            .and_then(|&panel| self.node(panel))
            .is_some_and(|panel| {
                matches!(panel.element.kind, Kind::FloatingPanel { .. }) && !panel.state.expanded
            });
        Some([Some(size[0]), (!folded).then_some(size[1])])
    }
}
