//! The cursor's shapes on X11
//! (`docs/adr/0313-x11s-keyboard-through-xinput2-and-libxkbcommon-x11-its-cursor-themed-its-scale-from-xft-dpi.md`):
//! each shape from the person's cursor theme, through `x11rb`'s cursor loader, at the theme and
//! size that `XCURSOR_THEME` and `XCURSOR_SIZE` name, else the X resources' `Xcursor.theme` and
//! `Xcursor.size`, else a size from `Xft.dpi`; where no theme holds it, or where the theme named
//! is `core`, the server's core cursor font's glyph, from the layer's own table (`x11rb` 0.13.2's
//! gives the font's glyphs at half their numbers). Each shape is made once, when first shown, and
//! set on the window as its cursor.

use slime_input::CursorShape;
use x11rb::NONE;
use x11rb::connection::Connection;
use x11rb::cursor::Handle;
use x11rb::protocol::xproto::{self, ChangeWindowAttributesAux, ConnectionExt as _};
use x11rb::resource_manager::Database;

/// The names that a shape is looked up by in the theme, in order: the CSS name that themes of
/// today give, then X11's older name, which is also the core font's.
fn names(shape: CursorShape) -> &'static [&'static str] {
    match shape {
        CursorShape::Text => &["text", "xterm"],
        CursorShape::ResizeHorizontal => &["ew-resize", "sb_h_double_arrow"],
        CursorShape::ResizeVertical => &["ns-resize", "sb_v_double_arrow"],
        // A shape added to `slime-input` that the layer does not map yet shows the default.
        _ => &["default", "left_ptr"],
    }
}

/// The glyph of the shape in the server's core cursor font, `cursorfont.h`'s: its mask is the next
/// glyph.
fn core_glyph(shape: CursorShape) -> u16 {
    match shape {
        CursorShape::Text => 152,
        CursorShape::ResizeHorizontal => 108,
        CursorShape::ResizeVertical => 116,
        _ => 68,
    }
}

/// The window's cursor: the shapes made, and the one shown.
pub(super) struct Cursors {
    window: xproto::Window,
    /// None where the server's RENDER cannot be asked, or the theme named is `core`: the core
    /// font's glyphs alone.
    handle: Option<Handle>,
    made: Vec<(CursorShape, xproto::Cursor)>,
    shown: CursorShape,
}

impl Cursors {
    /// The cursors of `window`, on the screen `screen`, from the theme that the environment and
    /// `resources` name.
    pub(super) fn new(
        connection: &impl Connection,
        screen: usize,
        window: xproto::Window,
        resources: &Database,
    ) -> Self {
        Self {
            window,
            handle: handle(connection, screen, resources),
            made: Vec::new(),
            shown: CursorShape::Default,
        }
    }

    /// Shows `shape` over the window: made first if it never was. The shape shown already asks
    /// nothing of the server.
    pub(super) fn show(&mut self, connection: &impl Connection, shape: CursorShape) {
        let cursor = match self.made.iter().find(|(made, _)| *made == shape) {
            Some(_) if shape == self.shown => return,
            Some(&(_, cursor)) => cursor,
            None => {
                let cursor = self.make(connection, shape);
                self.made.push((shape, cursor));
                cursor
            }
        };
        self.shown = shape;
        let _ = connection.change_window_attributes(
            self.window,
            &ChangeWindowAttributesAux::new().cursor(cursor),
        );
    }

    /// Takes the theme and size that the X resources now name, `resources`: every shape is made
    /// again from them, the one shown at once.
    pub(super) fn resources_changed(
        &mut self,
        connection: &impl Connection,
        screen: usize,
        resources: &Database,
    ) {
        for (_, cursor) in self.made.drain(..) {
            if cursor != NONE {
                let _ = connection.free_cursor(cursor);
            }
        }
        self.handle = handle(connection, screen, resources);
        self.show(connection, self.shown);
    }

    /// The cursor of `shape`, from the theme, else the core font; none, the server's, where both
    /// fail.
    fn make(&self, connection: &impl Connection, shape: CursorShape) -> xproto::Cursor {
        let themed = self.handle.as_ref().and_then(|handle| {
            names(shape)
                .iter()
                .map(|name| handle.load_cursor(connection, name))
                .find_map(|loaded| match loaded {
                    Ok(cursor) if cursor != NONE => Some(cursor),
                    Ok(_) => None,
                    Err(error) => {
                        tracing::debug!(%error, "a cursor of the theme does not load");
                        None
                    }
                })
        });
        themed
            .or_else(|| core_cursor(connection, core_glyph(shape)))
            .unwrap_or(NONE)
    }
}

/// x11rb's loader of themed cursors on `screen`, or none where RENDER cannot be asked or the
/// theme named is `core`.
fn handle(connection: &impl Connection, screen: usize, resources: &Database) -> Option<Handle> {
    let named = std::env::var("XCURSOR_THEME").ok();
    if theme_is_core(named.as_deref(), resources) {
        return None;
    }
    let loaded = Handle::new(connection, screen, resources)
        .map_err(x11rb::errors::ReplyOrIdError::from)
        .and_then(|cookie| cookie.reply());
    loaded
        .inspect_err(|error| {
            tracing::warn!(%error, "the cursor's theme is not read: the core font's cursors show");
        })
        .ok()
}

/// Whether the cursor's theme named is `core`, the server's font: `XCURSOR_THEME`, the
/// environment's `named`, first, else the X resources' `Xcursor.theme`, as `x11rb`'s loader reads
/// them.
fn theme_is_core(named: Option<&str>, resources: &Database) -> bool {
    named.or_else(|| resources.get_string("Xcursor.theme", "")) == Some("core")
}

/// The core cursor font's glyph `glyph`, black on white, as X11's older programs show it.
fn core_cursor(connection: &impl Connection, glyph: u16) -> Option<xproto::Cursor> {
    let font = xproto::FontWrapper::open_font(connection, b"cursor").ok()?;
    let cursor = connection.generate_id().ok()?;
    connection
        .create_glyph_cursor(
            cursor,
            font.font(),
            font.font(),
            glyph,
            glyph + 1,
            0,
            0,
            0,
            u16::MAX,
            u16::MAX,
            u16::MAX,
        )
        .ok()?;
    Some(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_theme_is_read_as_x11rbs_loader_reads_it() {
        let core = Database::new_from_data(b"Xcursor.theme: core\n");
        let themed = Database::new_from_data(b"Xcursor.theme: Adwaita\n");
        let none = Database::new_from_data(b"");
        assert!(theme_is_core(Some("core"), &none));
        assert!(theme_is_core(None, &core));
        // The environment first.
        assert!(!theme_is_core(Some("Adwaita"), &core));
        assert!(theme_is_core(Some("core"), &themed));
        assert!(!theme_is_core(None, &themed));
        assert!(!theme_is_core(None, &none));
    }

    #[test]
    fn each_shape_has_its_names_and_its_glyph() {
        assert_eq!(names(CursorShape::Text), ["text", "xterm"]);
        assert_eq!(names(CursorShape::Default), ["default", "left_ptr"]);
        // The glyphs of `cursorfont.h`: XC_xterm, XC_sb_h_double_arrow, XC_sb_v_double_arrow,
        // XC_left_ptr.
        assert_eq!(core_glyph(CursorShape::Text), 152);
        assert_eq!(core_glyph(CursorShape::ResizeHorizontal), 108);
        assert_eq!(core_glyph(CursorShape::ResizeVertical), 116);
        assert_eq!(core_glyph(CursorShape::Default), 68);
    }
}
