//! The host's events and outputs on the interface
//! (`docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`):
//! [`Ui::event`] hands each event to the input's handlers, and [`Ui::outputs`] tells what the
//! host shows of the interface, the cursor and the input method, and what it asks of the
//! clipboard, as it changed.

use slime_input::CursorShape;
#[cfg(feature = "test-input")]
use slime_input::{ButtonState, Ime, KeyEvent, Modifiers, MouseButton, ScrollDelta};

use super::{ElementId, Ui};
use crate::Rect;
#[cfg(feature = "test-input")]
use crate::host::Person;
use crate::host::{Answer, Event, Input, InputMethod, Outputs, TargetId};
#[cfg(feature = "test-input")]
use crate::time::EventTime;

/// What the host was last told of the main target's outputs ([`Ui::outputs`]), against which the
/// next reading tells what changed.
#[derive(Debug, Default)]
pub(super) struct Told {
    cursor: CursorShape,
    /// The field whose input the host was last told started, while it takes text.
    field: Option<ElementId>,
    /// Whether that field lost the focus since, which ended what it composed.
    ended: bool,
    /// The caret's box last told, while a field takes text.
    area: Option<Rect>,
}

impl Told {
    /// The field that held the focus lost it: its input ends, and starts again if it takes the
    /// focus back before the host reads the outputs.
    pub(super) fn field_left(&mut self) {
        self.ended = true;
    }
}

impl Ui {
    /// Gives the interface an input event that came to `event.target` at `event.time`, and answers
    /// at once whether it took it, which a game then leaves ([`Input`] says what each does). The
    /// interface's time moves to the event's, never back, so that what it starts, the pointer's
    /// rest before a tooltip shows or a submenu opens, starts then. An event for a target other
    /// than [`TargetId::MAIN`], on which no root draws, is not taken and changes nothing but the
    /// time.
    pub fn event(&mut self, event: Event<'_>) -> Answer {
        self.now = self.now.max(event.time.at);
        if event.target != TargetId::MAIN {
            return Answer::NotTaken;
        }
        let taken = match event.input {
            Input::PointerMoved { x, y } => {
                self.pointer_moved_input(x, y);
                false
            }
            Input::PointerLeft => {
                self.pointer_left_input();
                false
            }
            Input::PointerButton { button, state } => self.pointer_button_input(button, state),
            Input::Wheel(delta) => self.wheel_input(delta),
            Input::Modifiers(modifiers) => {
                self.interaction.modifiers = modifiers;
                false
            }
            Input::Key(key) => {
                self.clipboard.left_to_platform = false;
                let taken = self.key_input(key);
                if std::mem::take(&mut self.clipboard.left_to_platform) {
                    return Answer::TakenLeftToPlatform;
                }
                taken
            }
            Input::InputMethod(ime) => self.ime_input(ime),
            Input::DeleteAround {
                before,
                after,
                unit,
            } => self.delete_around(before, after, unit),
            Input::Copy => self.copy_input(false),
            Input::Cut => self.copy_input(true),
            Input::Paste(text) => self.paste_input(text),
            Input::ClipboardText { read, text } => self.clipboard_answer(read, text),
        };
        Answer::of(taken)
    }

    /// What `target` shows of the interface and changed since the host last read it: the
    /// pointer's shape where the pointer is; the input method, on as a field's input starts, off
    /// once no field holds the focus; the caret's box of the field that takes text; the text to
    /// write on the clipboard and the reads asked of it. The host reads them after each frame, and
    /// after an event whose effect it shows at once, a key of paste, whose read a platform that
    /// asks the person before a program reads lets through only in answer to the key; a target
    /// other than [`TargetId::MAIN`] has none. They are the host's alone: what another reader
    /// takes, the host is not told.
    pub fn outputs(&mut self, target: TargetId) -> Outputs {
        if target != TargetId::MAIN {
            return Outputs::default();
        }
        let cursor = self.cursor();
        let field = self.focused_field();
        let area = field
            .and_then(|field| self.node(field)?.field.as_ref())
            .map(|field| field.caret);
        let told = &mut self.told;
        let input_method = match field {
            Some(_) if field != told.field || told.ended => Some(InputMethod::On),
            None if told.field.is_some() => Some(InputMethod::Off),
            _ => None,
        };
        let input_area = if input_method == Some(InputMethod::On) || area != told.area {
            area
        } else {
            None
        };
        told.field = field;
        told.ended = false;
        told.area = area;
        let cursor = (cursor != told.cursor).then(|| {
            told.cursor = cursor;
            cursor
        });
        let (clipboard_write, clipboard_reads) = self.clipboard.take();
        Outputs {
            cursor,
            input_method,
            input_area,
            clipboard_write,
            clipboard_reads,
        }
    }
}

#[cfg(feature = "test-input")]
impl Person for Ui {
    fn pointer_moved(&mut self, x: f32, y: f32) {
        let _ = self.person(Input::PointerMoved { x, y });
    }

    fn pointer_left(&mut self) {
        let _ = self.person(Input::PointerLeft);
    }

    fn pointer_button(&mut self, button: MouseButton, state: ButtonState) -> bool {
        self.person(Input::PointerButton { button, state }).taken()
    }

    fn wheel(&mut self, delta: ScrollDelta) -> bool {
        self.person(Input::Wheel(delta)).taken()
    }

    fn modifiers(&mut self, modifiers: Modifiers) {
        let _ = self.person(Input::Modifiers(modifiers));
    }

    fn key(&mut self, key: &KeyEvent) -> bool {
        self.person(Input::Key(key)).taken()
    }

    fn ime(&mut self, ime: &Ime) -> bool {
        self.person(Input::InputMethod(ime)).taken()
    }

    fn serve_clipboard(&mut self, board: &mut Option<String>) -> Outputs {
        let outputs = self.outputs(TargetId::MAIN);
        if let Some(text) = &outputs.clipboard_write {
            *board = Some(text.clone());
        }
        for &read in &outputs.clipboard_reads {
            let text = board.as_deref();
            let _ = self.person(Input::ClipboardText { read, text });
        }
        outputs
    }
}

#[cfg(feature = "test-input")]
impl Ui {
    /// `input` given on the main target at the interface's latest time ([`Person`]).
    fn person(&mut self, input: Input<'_>) -> Answer {
        let time = EventTime::simulated(self.now);
        self.event(Event::new(TargetId::MAIN, time, input))
    }
}
