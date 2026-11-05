//! The rectangle that the framework speaks in.

/// A rectangle: its top left corner and its size, y down, as the renderer's canvas and its image
/// parts give theirs (`docs/adr/0139-text-drawn-and-the-display-list.md`). In pixels of the screen,
/// unless what holds it says otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Its left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// Its width.
    pub width: f32,
    /// Its height.
    pub height: f32,
}

impl Rect {
    /// The rectangle of `width` × `height` whose top left corner is (`x`, `y`).
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
