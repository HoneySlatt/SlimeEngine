//! A presentation's interface connected to its window: the window's events given to the
//! interface first, as the host's events of uwui, and what it asks the window to show, the
//! pointer's shape and the input method, and the system's clipboard to write and read
//! (`docs/adr/0145-text-edited.md`, `docs/adr/0249-the-hosts-api-its-items-and-their-table.md`).

use slime_platform::{Clipboard, CursorShape, Event, Window};
use uwui::host::{self, Input, InputMethod, ReadId, TargetId};
use uwui::time::EventTime;
use uwui::tree::Ui;

/// What a presentation's interface is given of the window's events: each as the host's event of
/// uwui on the window's target, but the pointer's moves, of which it holds the last until another
/// of the interface's events comes or the next frame starts, so that the moves of a frame reach
/// the interface as one
/// (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`): a fast mouse's four moves for
/// each frame at 240 Hz cost the interface one hit test, not four.
#[derive(Debug, Default)]
pub(crate) struct WindowInput {
    /// The pointer's last move not yet given, and when it came.
    moved: Option<(f32, f32, EventTime)>,
}

impl WindowInput {
    /// Gives `event`, which came at `time`, to `ui`, and gives whether the interface took it,
    /// which the presentation's [`on_event`](crate::Presentation::on_event) and the game's input
    /// then do not see: a press over what the interface draws, and its release, a key that a
    /// field, a shortcut or the focus takes, a wheel over a box that scrolls or a list, or over
    /// what the interface draws, every press, key and wheel while a modal root is open, what an
    /// input method composes for a field. The pointer's moves, the modifiers, the window's focus
    /// and the releases of keys reach the game too; a move is held, and given before the
    /// interface's next event or the next frame, so that the game sees it first. The window's
    /// other events, raw motion and the rest, are not the interface's, and leave a held move held.
    pub(crate) fn give(&mut self, ui: &mut Ui, event: &Event, time: EventTime) -> bool {
        let input = match event {
            Event::CursorMoved { x, y } => {
                self.moved = Some((*x as f32, *y as f32, time));
                return false;
            }
            Event::CursorLeft => Input::PointerLeft,
            Event::MouseButton { button, state } => Input::PointerButton {
                button: *button,
                state: *state,
            },
            Event::MouseWheel(delta) => Input::Wheel(*delta),
            Event::ModifiersChanged(modifiers) => Input::Modifiers(*modifiers),
            Event::Key(key) => Input::Key(key),
            Event::Ime(ime) => Input::InputMethod(ime),
            Event::Focused(focused) => Input::Focus(*focused),
            _ => return false,
        };
        self.give_moved(ui);
        ui.event(host::Event::new(TargetId::MAIN, time, input))
            .taken()
    }

    /// Gives `ui` the pointer's last move, if one is held, at the time that it came: before a
    /// frame starts, so that the frame shows what it hovers.
    pub(crate) fn give_moved(&mut self, ui: &mut Ui) {
        if let Some((x, y, time)) = self.moved.take() {
            let moved = Input::PointerMoved { x, y };
            let _ = ui.event(host::Event::new(TargetId::MAIN, time, moved));
        }
    }
}

/// What the window shows of the interface's outputs, rather than its own arrow and no input
/// method, and the system's clipboard that it serves them.
#[derive(Debug, Default)]
pub(crate) struct Shown {
    /// Whether an interface's outputs reached the window since it last showed its own.
    by_interface: bool,
    /// Whether the window allows the input method, which is forbidden only while it is allowed.
    ime_allowed: bool,
    /// The system's clipboard, opened as an interface's outputs are first shown, as before uwui's
    /// clipboard went through them, so that the compositors' series reads whether the engine has
    /// one in each: none where the system gives none, which the log tells once.
    clipboard: Option<Option<Clipboard>>,
}

impl Shown {
    /// Does on `window` what `ui`'s outputs ask since they were last read, at `time`: the
    /// pointer's shape; the input method forbidden, or forbidden then allowed as a field's input
    /// starts, which is meant to end what it composed for the field before, though no input
    /// method was checked with it yet; the caret's box, beside which it places its window; the
    /// text written on the system's clipboard, then each read of it answered at once, with none
    /// where the system gives no clipboard. Without an interface, the arrow and the input method
    /// forbidden. The run shows them after each frame, an image drawn or not, and right after
    /// each event that the interface took, so that a paste lands in the step of its key, which
    /// macOS is expected to let a program read without asking the person, not yet checked. Gives
    /// whether it answered a read, whose paste the next frame shows. winit 0.30 tells an input
    /// method no field's purpose but a password's and a terminal's, which uwui has none of.
    pub(crate) fn show(&mut self, window: &Window, ui: Option<&mut Ui>, time: EventTime) -> bool {
        let Some(ui) = ui else {
            if std::mem::take(&mut self.by_interface) {
                window.set_cursor(CursorShape::Default);
                self.allow_ime(window, false);
            }
            return false;
        };
        self.by_interface = true;
        let _ = self.clipboard();
        let outputs = ui.outputs(TargetId::MAIN);
        if let Some(cursor) = outputs.cursor {
            window.set_cursor(cursor);
        }
        if let Some(input_method) = outputs.input_method {
            self.allow_ime(window, false);
            if input_method == InputMethod::On {
                self.allow_ime(window, true);
            }
        }
        if let Some(area) = outputs.input_area {
            window.set_ime_area(area.x, area.y, area.width.max(1.0), area.height);
        }
        self.serve_clipboard(ui, outputs.clipboard_write, &outputs.clipboard_reads, time)
    }

    /// Writes `write` on the system's clipboard, then answers each of `reads` at `time`, so that
    /// a read asked after a write reads it; gives whether it answered any.
    fn serve_clipboard(
        &mut self,
        ui: &mut Ui,
        write: Option<String>,
        reads: &[ReadId],
        time: EventTime,
    ) -> bool {
        if write.is_none() && reads.is_empty() {
            return false;
        }
        let mut clipboard = self.clipboard();
        if let (Some(text), Some(clipboard)) = (write, clipboard.as_mut()) {
            clipboard.set_text(&text);
        }
        // Read once for every read that waits: they asked the same clipboard in the same step.
        let text = match reads {
            [] => None,
            _ => clipboard.and_then(|clipboard| clipboard.text()),
        };
        for &read in reads {
            let answer = Input::ClipboardText {
                read,
                text: text.as_deref(),
            };
            let _ = ui.event(host::Event::new(TargetId::MAIN, time, answer));
        }
        !reads.is_empty()
    }

    /// The system's clipboard, opened the first time; none where the system gives none.
    fn clipboard(&mut self) -> Option<&mut Clipboard> {
        self.clipboard
            .get_or_insert_with(|| match Clipboard::open() {
                Ok(clipboard) => Some(clipboard),
                Err(error) => {
                    tracing::warn!(%error, "fields will copy and paste nothing");
                    None
                }
            })
            .as_mut()
    }

    /// Allows the input method, or forbids it, unless the window already does.
    fn allow_ime(&mut self, window: &Window, allowed: bool) {
        if self.ime_allowed != allowed {
            window.set_ime_allowed(allowed);
            self.ime_allowed = allowed;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use slime_platform::{ButtonState, Key, KeyCode, KeyEvent, MouseButton};
    use uwui::style::Dimension;
    use uwui::text::Fonts;
    use uwui::time::Instant;
    use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings};

    use super::*;

    /// At the clock's zero.
    const ZERO: EventTime = EventTime::received(Instant::ZERO);

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

    /// A press or a release of the primary button.
    fn press(state: ButtonState) -> Event {
        Event::MouseButton {
            button: MouseButton::Left,
            state,
        }
    }

    /// An interface of 400 by 300 pixels holding a field of 200 by 30 at its top left corner,
    /// then a button as wide below it, laid out: the interface, the field and the button.
    fn interface() -> (Ui, ElementId, ElementId) {
        let mut ui = Ui::new(Fonts::new());
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| {
            s.direction(uwui::style::FlexDirection::Column)
        });
        let size = |s: uwui::style::StyleBuilder| {
            s.width(Dimension::Pixels(200.0))
                .height(Dimension::Pixels(30.0))
        };
        let (mut field, mut button) = (None, None);
        ui.build(root, 0, |children| {
            field = Some(children.add("field", Element::text_field("").style(size), |_| {}));
            button = Some(children.add("button", Element::button("go").style(size), |_| {}));
        });
        ui.frame(TargetId::MAIN);
        (ui, field.expect("built"), button.expect("built"))
    }

    /// The window's events reach the interface first; it takes a press on a field, the keys that
    /// the field types while it holds the focus, and leaves the game the rest.
    #[test]
    fn the_interface_takes_what_reaches_its_elements_and_leaves_the_game_the_rest() {
        let (mut ui, _, _) = interface();
        let mut input = WindowInput::default();
        // Over nothing: the game's.
        assert!(!input.give(&mut ui, &Event::CursorMoved { x: 300.0, y: 200.0 }, ZERO));
        assert!(!input.give(&mut ui, &press(ButtonState::Pressed), ZERO));
        assert!(!input.give(&mut ui, &press(ButtonState::Released), ZERO));
        assert!(
            !input.give(&mut ui, &typed("w"), ZERO),
            "no element holds the focus"
        );
        // On the field: the interface's, and the keys it types.
        input.give(&mut ui, &Event::CursorMoved { x: 10.0, y: 10.0 }, ZERO);
        assert!(input.give(&mut ui, &press(ButtonState::Pressed), ZERO));
        assert!(input.give(&mut ui, &press(ButtonState::Released), ZERO));
        assert!(input.give(&mut ui, &typed("w"), ZERO));
        assert!(!input.give(&mut ui, &Event::Focused(true), ZERO));
    }

    /// The window's focus reaches the interface before the releases that the platform makes as it
    /// leaves: Alt pressed, then the focus lost as Alt and Tab leave the window, gives the bar of
    /// menus nothing at Alt's release
    /// (`docs/adr/0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md`).
    #[test]
    fn the_windows_focus_reaches_the_interface() {
        use slime_platform::NamedKey;
        let mut ui = Ui::new(Fonts::new());
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| s);
        ui.build(root, 0, |children| {
            children.add("bar", Element::menu_bar(), |bar| {
                let item = Element::menu_item("File", "", uwui::tree::ItemMark::Submenu);
                bar.add("file", item, |_| {});
            });
        });
        ui.frame(TargetId::MAIN);
        let alt = |state| {
            Event::Key(KeyEvent {
                physical: KeyCode::AltLeft,
                logical: Key::Named(NamedKey::Alt),
                text: None,
                state,
                repeat: false,
            })
        };
        let mut input = WindowInput::default();
        input.give(&mut ui, &alt(ButtonState::Pressed), ZERO);
        assert!(!input.give(&mut ui, &Event::Focused(false), ZERO));
        input.give(&mut ui, &alt(ButtonState::Released), ZERO);
        assert_eq!(ui.focus(), None, "the bar took no focus");
    }

    /// The pointer's moves reach the interface as one, the last, at the time it came: when another
    /// of the interface's events comes, or the frame starts.
    #[test]
    fn the_pointers_moves_reach_the_interface_as_one_before_the_next_event_or_frame() {
        let (mut ui, field, button) = interface();
        let mut input = WindowInput::default();
        let hovered = |ui: &Ui, id| ui.state(id).is_some_and(|state| state.hovered);
        let at = |millis| EventTime::received(Instant::ZERO + Duration::from_millis(millis));
        input.give(&mut ui, &Event::CursorMoved { x: 10.0, y: 10.0 }, at(1));
        input.give(&mut ui, &Event::CursorMoved { x: 10.0, y: 40.0 }, at(2));
        assert!(!hovered(&ui, field) && !hovered(&ui, button), "held");
        // The window's own events leave it held.
        let raw = Event::RawMotion { delta: [1.0, 0.0] };
        assert!(!input.give(&mut ui, &raw, at(3)));
        assert!(!hovered(&ui, button), "still held");
        // A press reaches the button that the last move came over, after that move.
        assert!(input.give(&mut ui, &press(ButtonState::Pressed), at(4)));
        assert!(hovered(&ui, button) && !hovered(&ui, field));
        assert!(input.give(&mut ui, &press(ButtonState::Released), at(5)));
        // A move held until the frame, then given once: the pointer moved away since stays away.
        input.give(&mut ui, &Event::CursorMoved { x: 10.0, y: 10.0 }, at(6));
        assert!(hovered(&ui, button));
        input.give_moved(&mut ui);
        assert!(hovered(&ui, field) && !hovered(&ui, button));
        let away = host::Input::PointerMoved { x: 300.0, y: 280.0 };
        let _ = ui.event(host::Event::new(TargetId::MAIN, at(7), away));
        input.give_moved(&mut ui);
        assert!(!hovered(&ui, field), "given once");
    }

    /// A held move reaches the interface at the time it came, not the frame's: the pointer's rest
    /// over a button with a tooltip starts then.
    #[test]
    fn a_held_move_reaches_the_interface_at_the_time_it_came() {
        let mut ui = Ui::new(Fonts::new());
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| s);
        ui.build(root, 0, |children| {
            let button = Element::button("go").tooltip("Go").style(|s| {
                s.width(Dimension::Pixels(200.0))
                    .height(Dimension::Pixels(30.0))
            });
            children.add("button", button, |_| {});
        });
        ui.frame(TargetId::MAIN);
        let moved_at = Instant::ZERO + Duration::from_secs(1);
        let mut input = WindowInput::default();
        let moved = Event::CursorMoved { x: 10.0, y: 10.0 };
        input.give(&mut ui, &moved, EventTime::received(moved_at));
        // The frame starts later, and takes the move first.
        input.give_moved(&mut ui);
        ui.set_frame_shown(
            TargetId::MAIN,
            uwui::time::FrameShown::simulated(moved_at + Duration::from_millis(100)),
        );
        ui.frame(TargetId::MAIN);
        // A tooltip shows after half a second of rest, from the move, not from the frame.
        assert_eq!(
            ui.next_frame_due(TargetId::MAIN),
            Some(moved_at + Duration::from_millis(500))
        );
    }
}
