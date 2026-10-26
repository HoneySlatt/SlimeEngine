//! A presentation's interface connected to its window: the window's events given to the
//! interface first, its clipboard, the pointer's shape and the input method
//! (`docs/adr/0145-text-edited.md`).

use std::sync::{Arc, Mutex, PoisonError};

use slime_platform::{Clipboard, CursorShape, Event, Window};
use slime_ui::Rect;
use slime_ui::tree::{ElementId, Ui};

/// Gives `event` to `ui` as the interface's calls, and gives whether the interface took it, which
/// the presentation's [`on_event`](crate::Presentation::on_event) and the game's input then do not
/// see: a press over what the interface draws, and its release, a key that a field, a shortcut or
/// the focus takes, a wheel over a box that scrolls or a list, or over what the interface draws,
/// every press, key and wheel while a modal root is open, what an input method composes for a
/// field. The pointer's moves, the modifiers and the releases of keys reach the game too.
pub(crate) fn give(ui: &mut Ui, event: &Event) -> bool {
    match event {
        Event::CursorMoved { x, y } => {
            ui.pointer_moved(*x as f32, *y as f32);
            false
        }
        Event::CursorLeft => {
            ui.pointer_left();
            false
        }
        Event::MouseButton { button, state } => ui.pointer_button(*button, *state),
        Event::MouseWheel(delta) => ui.wheel(*delta),
        Event::ModifiersChanged(modifiers) => {
            ui.modifiers(*modifiers);
            false
        }
        Event::Key(key) => ui.key(key),
        Event::Ime(ime) => ui.ime(ime),
        _ => false,
    }
}

/// The system's clipboard, shared by the interfaces that a run's presentation gives.
#[derive(Clone)]
pub(crate) struct SharedClipboard(Arc<Mutex<Clipboard>>);

impl SharedClipboard {
    /// The system's clipboard; none, which the log tells, when the system gives none.
    pub(crate) fn open() -> Option<Self> {
        match Clipboard::open() {
            Ok(clipboard) => Some(Self(Arc::new(Mutex::new(clipboard)))),
            Err(error) => {
                tracing::warn!(%error, "fields will copy and paste nothing");
                None
            }
        }
    }

    /// Gives `ui` the clipboard, unless it has one.
    pub(crate) fn give(&self, ui: &mut Ui) {
        if !ui.has_clipboard() {
            ui.set_clipboard(Box::new(self.clone()));
        }
    }
}

impl slime_ui::tree::Clipboard for SharedClipboard {
    fn text(&mut self) -> Option<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).text()
    }

    fn set_text(&mut self, text: &str) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_text(text);
    }
}

/// What the window shows of the interface, as last set: set again only when it changes.
#[derive(Debug, Default)]
pub(crate) struct Shown {
    cursor: CursorShape,
    /// The field that takes text and its caret's box, the input method allowed; none, forbidden.
    text_input: Option<(ElementId, Rect)>,
}

impl Shown {
    /// Shows on `window` what `ui` asks after a frame: the pointer's shape, and the input method
    /// allowed, beside the caret, while a field takes text; without an interface, the arrow and
    /// the input method forbidden. When the focus goes from one field to another, the input method
    /// is forbidden then allowed again, meant to end what it composed for the first, which no
    /// input method was checked with yet.
    pub(crate) fn update(&mut self, window: &Window, ui: Option<&Ui>) {
        let cursor = ui.map_or(CursorShape::Default, Ui::cursor);
        if cursor != self.cursor {
            window.set_cursor(cursor);
            self.cursor = cursor;
        }
        let text_input = ui.and_then(|ui| ui.focus().zip(ui.text_input()));
        let field = |shown: Option<(ElementId, Rect)>| shown.map(|(field, _)| field);
        if field(text_input) != field(self.text_input) {
            if self.text_input.is_some() {
                window.set_ime_allowed(false);
            }
            if text_input.is_some() {
                window.set_ime_allowed(true);
            }
        }
        if let Some((_, caret)) = text_input
            && text_input != self.text_input
        {
            window.set_ime_area(caret.x, caret.y, caret.width.max(1.0), caret.height);
        }
        self.text_input = text_input;
    }
}

#[cfg(test)]
mod tests {
    use slime_platform::{ButtonState, Key, KeyCode, KeyEvent, MouseButton};
    use slime_ui::style::{Dimension, Style};
    use slime_ui::text::Fonts;
    use slime_ui::tree::{Element, LayoutDirection, RootSettings};

    use super::*;

    /// A press of a key that types `text`.
    fn typed(text: &str) -> Event {
        Event::Key(KeyEvent {
            physical: KeyCode::KeyW,
            logical: Key::Character(text.to_owned()),
            text: Some(text.to_owned()),
            state: ButtonState::Pressed,
            repeat: false,
        })
    }

    /// The window's events reach the interface first; it takes a press on a field, the keys that
    /// the field types while it holds the focus, and leaves the game the rest.
    #[test]
    fn the_interface_takes_what_reaches_its_elements_and_leaves_the_game_the_rest() {
        let mut ui = Ui::new(Fonts::new());
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, Style::default());
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(200.0), Dimension::Pixels(30.0)];
        ui.build(root, 0, |children| {
            children.add("field", Element::text_field("", style), |_| {});
        });
        ui.frame();
        let press = |state| Event::MouseButton {
            button: MouseButton::Left,
            state,
        };
        // Over nothing: the game's.
        assert!(!give(&mut ui, &Event::CursorMoved { x: 300.0, y: 200.0 }));
        assert!(!give(&mut ui, &press(ButtonState::Pressed)));
        assert!(!give(&mut ui, &press(ButtonState::Released)));
        assert!(!give(&mut ui, &typed("w")), "no element holds the focus");
        // On the field: the interface's, and the keys it types.
        give(&mut ui, &Event::CursorMoved { x: 10.0, y: 10.0 });
        assert!(give(&mut ui, &press(ButtonState::Pressed)));
        assert!(give(&mut ui, &press(ButtonState::Released)));
        assert!(give(&mut ui, &typed("w")));
        assert!(!give(&mut ui, &Event::Focused(true)));
    }
}
