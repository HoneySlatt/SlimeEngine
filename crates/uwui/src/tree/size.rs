//! Each target's size and scale, which its host gives and each root on it fills, and the box that a
//! root lays out in
//! (`docs/adr/0268-each-target-its-size-from-its-host-a-root-fills-it-stacked-roots-take-where-they-act.md`).

use super::{ElementId, LayoutDirection, Ui};
use crate::host::{TargetId, TargetSize};

/// The box that a root lays out in and its elements are placed by: its target's size and scale as
/// its roots were last laid out, its direction, and its place on the target, a layer's or none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RootBox {
    /// Its target's width and height, in logical pixels.
    pub(crate) size: [f32; 2],
    /// Its target's physical pixels per logical pixel.
    pub(crate) scale: f32,
    /// The direction of its lines.
    pub(crate) direction: LayoutDirection,
    /// Where its top left corner lies on its target, in logical pixels: a layer's place, else the
    /// corner.
    pub(crate) origin: [f32; 2],
}

impl Ui {
    /// Gives `target` its image's size and its scale, which each root on it, and each layer over
    /// them, fills: a host gives it before the target's first frame, and again whenever either
    /// changes, as a window is resized or moved to a screen of another scale. Its roots are laid
    /// out again at the target's next frame ([`Ui::frame`]), drawn again whole for another scale; a
    /// size given again unchanged does nothing, and asks for no frame.
    ///
    /// A size is kept for a target that has no root yet, without making it known to input. A
    /// target whose host gave none lays its roots out at one pixel, at scale 1. A host whose window
    /// has no area, minimized, gives nothing ([`TargetSize::new`] refuses it): the target keeps
    /// its size, and its roots their layout. A host gives the size of the image that it draws the
    /// target's display list into, which may be smaller than the window; a position beyond it
    /// falls outside every root.
    pub fn set_target_size(&mut self, target: TargetId, size: TargetSize) {
        let record = self.frame_record(target);
        if record.given == Some(size) {
            return;
        }
        record.given = Some(size);
        if record.laid_out != size {
            record.changed = true;
        }
    }

    /// The size that `target`'s host last gave ([`Ui::set_target_size`]), which its next frame lays
    /// its roots out at; none before it gave one.
    pub fn target_size(&self, target: TargetId) -> Option<TargetSize> {
        self.frames
            .iter()
            .find(|each| each.target == target)
            .and_then(|each| each.given)
    }

    /// The size that `target`'s next frame lays its roots out at: the one its host last gave, or the
    /// one they were last laid out at.
    pub(crate) fn coming_size(&self, target: TargetId) -> TargetSize {
        self.target_size(target)
            .unwrap_or_else(|| self.laid_out_size(target))
    }

    /// The size that `target`'s roots were last laid out at: what their boxes are in, between a
    /// size given and the frame that lays them out.
    pub(crate) fn laid_out_size(&self, target: TargetId) -> TargetSize {
        self.frames
            .iter()
            .find(|each| each.target == target)
            .map_or(TargetSize::UNGIVEN, |each| each.laid_out)
    }

    /// Makes the size given to `t` the one its roots are laid out at, as its frame starts: each
    /// root and layer on it laid out again, and drawn again whole for another scale.
    pub(crate) fn settle_size(&mut self, t: TargetId) {
        let Some(record) = self.frames.iter_mut().find(|each| each.target == t) else {
            return;
        };
        let Some(given) = record.given else {
            return;
        };
        let before = record.laid_out;
        if before == given {
            return;
        }
        record.laid_out = given;
        for at in 0..self.roots.len() {
            let root = self.roots[at];
            if self.root_target(root) != t {
                continue;
            }
            if before.scale() == given.scale() {
                self.mark_layout(root);
            } else {
                // Every length that taffy holds, and every primitive, is in pixels of the screen.
                self.mark_all(root);
            }
        }
    }

    /// The box of the root that `id` lies under; none for an id that names nothing.
    pub(crate) fn root_box(&self, id: ElementId) -> Option<RootBox> {
        let node = self.node(self.root_of(id)?)?;
        let settings = node.root?;
        let size = self.laid_out_size(node.drawn_on);
        Some(RootBox {
            size: size.logical(),
            scale: size.scale(),
            direction: settings.direction,
            origin: node.layer.as_ref().map_or([0.0; 2], |held| held.origin),
        })
    }
}
