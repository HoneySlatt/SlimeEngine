//! The scroll bars of a box that scrolls and of a list: drawn over their content while it does not
//! fit, along their end side and their bottom, a thumb as long as the share of the content that
//! shows, which the pointer drags, and a track that a press pages along
//! (`docs/adr/0148-widgets.md`).

use super::{ElementId, Kind, LayoutDirection, Ui};
use crate::Rect;
use crate::display::{BoxPrimitive, Primitive};
use crate::layout;

/// A scroll bar of an element: its axis, its track and its thumb, in pixels of the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bar {
    /// 0 along x, 1 along y.
    pub(crate) axis: usize,
    pub(crate) track: Rect,
    pub(crate) thumb: Rect,
}

/// A thumb that the pointer holds: its element and its bar's axis, and where along the axis the
/// pointer holds it, from the thumb's start, in pixels of the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    pub(crate) element: ElementId,
    pub(crate) axis: usize,
    pub(crate) grip: f32,
}

impl Ui {
    /// The scroll bars of `id`, a box that scrolls or a list, while its content does not fit:
    /// as thick as half its control's size, inside its border, at its end side and its bottom.
    pub(crate) fn scroll_bars(&self, id: ElementId) -> Vec<Bar> {
        let scale = self.scale_of(id);
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        if !matches!(node.element.kind, Kind::Scroll | Kind::List { .. }) {
            return Vec::new();
        }
        let most = layout::scroll_most(&node.element.kind, &node.layout, scale);
        let thickness = node.style.control.size * scale / 2.0;
        let border = node.style.paint.border_width * scale;
        let rect = node.rect;
        let inside = Rect::new(
            rect.x + border,
            rect.y + border,
            (rect.width - 2.0 * border).max(0.0),
            (rect.height - 2.0 * border).max(0.0),
        );
        let rtl =
            self.root_settings(id).map(|root| root.direction) == Some(LayoutDirection::RightToLeft);
        let both = most[0] > 0.0 && most[1] > 0.0;
        let mut bars = Vec::new();
        for axis in [1, 0] {
            if most[axis] <= 0.0 {
                continue;
            }
            // Its track runs along its side, short of the other bar where both show.
            let track = if axis == 1 {
                let x = if rtl {
                    inside.x
                } else {
                    inside.x + inside.width - thickness
                };
                let length = inside.height - if both { thickness } else { 0.0 };
                Rect::new(x, inside.y, thickness, length.max(0.0))
            } else {
                let x = if rtl && both {
                    inside.x + thickness
                } else {
                    inside.x
                };
                let length = inside.width - if both { thickness } else { 0.0 };
                Rect::new(
                    x,
                    inside.y + inside.height - thickness,
                    length.max(0.0),
                    thickness,
                )
            };
            let (along, shown) = if axis == 1 {
                (track.height, inside.height)
            } else {
                (track.width, inside.width)
            };
            let length = (along * shown / (shown + most[axis]))
                .max(2.0 * thickness)
                .min(along);
            let offset = node.scroll[axis] * scale;
            let mut start = (along - length) * (offset / most[axis]).clamp(0.0, 1.0);
            // From right to left, the content starts at the right, and so does the thumb.
            if axis == 0 && rtl {
                start = along - length - start;
            }
            let thumb = if axis == 1 {
                Rect::new(track.x, track.y + start, thickness, length)
            } else {
                Rect::new(track.x + start, track.y, length, thickness)
            };
            bars.push(Bar { axis, track, thumb });
        }
        bars
    }

    /// The scroll bar under `x` and `y`, in pixels of the surface: of the element there or of the
    /// nearest above it that scrolls, whose bar holds the point.
    pub(crate) fn bar_at(&self, x: f32, y: f32) -> Option<(ElementId, Bar)> {
        let mut at = self.element_at(x, y);
        while let Some(id) = at {
            if let Some(bar) = self
                .scroll_bars(id)
                .into_iter()
                .find(|bar| super::interact::inside(bar.track, x, y))
            {
                return Some((id, bar));
            }
            at = self.node(id)?.parent;
        }
        None
    }

    /// The pointer pressed on `bar` of `id` at `x` and `y`: on its thumb, the pointer holds it,
    /// which a move drags; on its track, the content pages towards the pointer by what shows of it.
    pub(crate) fn press_bar(&mut self, id: ElementId, bar: Bar, x: f32, y: f32) {
        let at = if bar.axis == 1 { y } else { x };
        let (start, length) = if bar.axis == 1 {
            (bar.thumb.y, bar.thumb.height)
        } else {
            (bar.thumb.x, bar.thumb.width)
        };
        if at >= start && at < start + length {
            self.interaction.bar = Some(Held {
                element: id,
                axis: bar.axis,
                grip: at - start,
            });
            return;
        }
        let scale = self.scale_of(id);
        let Some(node) = self.node(id) else {
            return;
        };
        let page = if bar.axis == 1 {
            node.rect.height
        } else {
            node.rect.width
        } / scale;
        let rtl = bar.axis == 0
            && self.root_settings(id).map(|root| root.direction)
                == Some(LayoutDirection::RightToLeft);
        // Before the thumb on screen is towards the content's start, but from right to left.
        let towards_start = (at < start) != rtl;
        let mut offset = node.scroll;
        offset[bar.axis] += if towards_start { -page } else { page };
        self.scroll_to(id, offset);
    }

    /// The pointer moved to `x` and `y` while it holds a thumb: the content scrolls as far as the
    /// thumb is along its track.
    pub(crate) fn drag_bar(&mut self, held: Held, x: f32, y: f32) {
        let scale = self.scale_of(held.element);
        let Some(bar) = self
            .scroll_bars(held.element)
            .into_iter()
            .find(|bar| bar.axis == held.axis)
        else {
            return;
        };
        let Some(node) = self.node(held.element) else {
            return;
        };
        let most = layout::scroll_most(&node.element.kind, &node.layout, scale)[held.axis];
        let (at, track_start, along, length) = if held.axis == 1 {
            (y, bar.track.y, bar.track.height, bar.thumb.height)
        } else {
            (x, bar.track.x, bar.track.width, bar.thumb.width)
        };
        let room = (along - length).max(f32::EPSILON);
        let mut share = ((at - held.grip - track_start) / room).clamp(0.0, 1.0);
        if held.axis == 0
            && self.root_settings(held.element).map(|root| root.direction)
                == Some(LayoutDirection::RightToLeft)
        {
            share = 1.0 - share;
        }
        let mut offset = node.scroll;
        offset[held.axis] = share * most / scale;
        self.scroll_to(held.element, offset);
    }

    /// What the scroll bars of `id` draw, over its content: their tracks and their thumbs.
    pub(crate) fn bar_primitives(&self, id: ElementId) -> Vec<Primitive> {
        let scale = self.scale_of(id);
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let control = node.style.control;
        let filled = |rect: Rect, fill| {
            Primitive::Box(BoxPrimitive {
                rect,
                radii: [control.radius * scale; 4],
                fill,
                border_width: 0.0,
                border: fill,
            })
        };
        self.scroll_bars(id)
            .into_iter()
            .flat_map(|bar| {
                [
                    filled(bar.track, control.track),
                    filled(bar.thumb, control.mark),
                ]
            })
            .collect()
    }
}
