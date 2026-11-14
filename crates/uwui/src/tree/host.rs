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

/// What the host was last told of a target's outputs ([`Ui::outputs`]), against which the next
/// reading tells what changed.
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
    /// rest before a tooltip shows or a submenu opens, starts then. A key, a button or the window's
    /// focus given makes its target the active one ([`Ui::active_target`]).
    ///
    /// An event reaches the roots of its own target alone. The interface knows a target from its
    /// first root, [`TargetId::MAIN`] from the start: an event for a target that it does not know
    /// is not taken and changes nothing but the time. A known target whose roots are gone keeps
    /// its input: its events reach no element, a release whose press it took is taken, and a key
    /// reaches the shortcuts that need no focus. An answer of the clipboard, which names its read,
    /// is taken on any target
    /// (`docs/adr/0257-targets-known-from-their-first-root-a-moved-root-below-its-new-targets-modal-root.md`).
    pub fn event(&mut self, event: Event<'_>) -> Answer {
        self.now = self.now.max(event.time.at);
        let t = event.target;
        if let Input::ClipboardText { read, text } = event.input {
            return Answer::of(self.clipboard_answer(read, text));
        }
        if !self.interaction.knows(t) {
            return Answer::NotTaken;
        }
        if matches!(event.input, Input::Key(_) | Input::PointerButton { .. }) {
            self.interaction.active = t;
        }
        let taken = match event.input {
            Input::PointerMoved { x, y } => {
                self.pointer_moved_input(t, x, y);
                false
            }
            Input::PointerLeft => {
                self.pointer_left_input(t);
                false
            }
            Input::PointerButton { button, state } => self.pointer_button_input(t, button, state),
            Input::Wheel(delta) => self.wheel_input(t, delta),
            Input::Modifiers(modifiers) => {
                self.interaction.of_mut(t).modifiers = modifiers;
                false
            }
            Input::Focus(true) => {
                self.interaction.active = t;
                false
            }
            Input::Focus(false) => {
                self.focus_lost_input(t);
                false
            }
            Input::Key(key) => {
                self.clipboard.left_to_platform = false;
                let taken = self.key_input(t, key);
                if std::mem::take(&mut self.clipboard.left_to_platform) {
                    return Answer::TakenLeftToPlatform;
                }
                taken
            }
            Input::InputMethod(ime) => self.ime_input(t, ime),
            Input::DeleteAround {
                before,
                after,
                unit,
            } => self.delete_around(t, before, after, unit),
            Input::Copy => self.copy_input(t, false),
            Input::Cut => self.copy_input(t, true),
            Input::Paste(text) => self.paste_input(t, text),
            Input::ClipboardText { .. } => unreachable!("answered above"),
        };
        Answer::of(taken)
    }

    /// What `target` shows of the interface and changed since the host last read it: the
    /// pointer's shape where its pointer is; the input method, on as a field's input starts, off
    /// once no field holds its focus; the caret's box of its field that takes text; the text to
    /// write on the clipboard and the reads asked of it, a field's on its own target and the
    /// program's on the active one. The host reads them after each frame, and after an event
    /// whose effect it shows at once, a key of paste, whose read a platform that asks the person
    /// before a program reads lets through only in answer to the key; a target that the interface
    /// does not know has none. They are the host's alone: what another reader takes, the host is not
    /// told.
    pub fn outputs(&mut self, target: TargetId) -> Outputs {
        if !self.interaction.knows(target) {
            return Outputs::default();
        }
        let cursor = self.cursor(target);
        let field = self.focused_field(target);
        let area = field
            .and_then(|field| self.node(field)?.field.as_ref())
            .map(|field| field.caret);
        let told = &mut self.interaction.of_mut(target).told;
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
        let (clipboard_write, clipboard_reads) = self.clipboard.take(target);
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
