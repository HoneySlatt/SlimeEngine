//! The clipboard as the host serves it
//! (`docs/adr/0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md`): what a field or the
//! program writes waits in the outputs; a read is asked through the outputs, numbered, and its
//! answer comes back as an event; the platform's own copy, cut and paste come as events too.

use std::collections::VecDeque;

use super::{ElementId, Kind, Ui};
use crate::action::{Action, ActionKind};
use crate::host::ReadId;

/// How many reads of the clipboard wait for their answers at most: a host that answers none keeps
/// no more, the oldest dropped first, a program's answered with no text.
const AWAITED_READS: usize = 16;

/// What the interface keeps of the clipboard between the host's readings.
#[derive(Debug, Default)]
pub(super) struct Clipboard {
    /// Whether the platform fires its own copy, cut and paste from their keys, as the web does
    /// ([`Ui::set_clipboard_events`]).
    events: bool,
    /// Set by a key of copy, cut or paste that the platform handles; read by [`Ui::event`].
    pub(super) left_to_platform: bool,
    /// The text that waits to be written.
    write: Option<String>,
    /// The reads asked and not yet told the host, oldest first.
    asked: Vec<ReadId>,
    /// The reads awaiting their answers, oldest first, each with who asked.
    awaited: VecDeque<(ReadId, Asker)>,
    /// The number of the last read asked.
    last: u64,
}

/// Who asked a read of the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asker {
    /// A field, by its key of paste: the answer is pasted in it.
    Field(ElementId),
    /// The program ([`Ui::read_clipboard`]): the answer is its action.
    Program,
}

impl Clipboard {
    /// What waits for the host: the text to write and the reads asked, taken.
    pub(super) fn take(&mut self) -> (Option<String>, Vec<ReadId>) {
        (self.write.take(), std::mem::take(&mut self.asked))
    }
}

impl Ui {
    /// Tells the interface whether the platform fires its own copy, cut and paste from their keys,
    /// as the web's `copy`, `cut` and `paste` events do: the keys of copy, cut and paste in a field
    /// are then answered [`Answer::TakenLeftToPlatform`](crate::host::Answer::TakenLeftToPlatform),
    /// and the host gives the platform's events that follow as
    /// [`Input::Copy`](crate::host::Input::Copy), [`Input::Cut`](crate::host::Input::Cut) and
    /// [`Input::Paste`](crate::host::Input::Paste). Without it, the default, the keys write the
    /// clipboard and ask to read it through the outputs.
    pub fn set_clipboard_events(&mut self, events: bool) {
        self.clipboard.events = events;
    }

    /// Asks the host to put `text` on the clipboard, in place of what it held, as a program does
    /// that copies without a field, an editor copying what is selected
    /// (`docs/adr/0162-creating-copying-and-arranging.md`): it waits in the outputs
    /// ([`Outputs::clipboard_write`](crate::host::Outputs::clipboard_write)).
    pub fn write_clipboard(&mut self, text: &str) {
        self.clipboard.write = Some(text.to_owned());
    }

    /// Asks the host to read the clipboard for the program, as an editor's paste without a field
    /// does: the read waits in the outputs
    /// ([`Outputs::clipboard_reads`](crate::host::Outputs::clipboard_reads)), and its answer
    /// reaches the program as [`ActionKind::ClipboardText`], with the read that this gives. A
    /// text that the program wrote before is what it reads, the host writing before it reads.
    /// A read dropped past the 16 that wait at most reaches the program with no text; one that the
    /// host never answers waits until newer reads push it out.
    pub fn read_clipboard(&mut self) -> ReadId {
        self.ask_read(Asker::Program)
    }

    /// Asks a read for `asker`; past [`AWAITED_READS`], the oldest is dropped.
    fn ask_read(&mut self, asker: Asker) -> ReadId {
        let clipboard = &mut self.clipboard;
        clipboard.last += 1;
        let read = ReadId(clipboard.last);
        if clipboard.awaited.len() == AWAITED_READS
            && let Some((dropped, asker)) = clipboard.awaited.pop_front()
        {
            clipboard.asked.retain(|&asked| asked != dropped);
            if asker == Asker::Program {
                self.interaction.actions.push(Action {
                    element: None,
                    kind: ActionKind::ClipboardText {
                        read: dropped,
                        text: None,
                    },
                });
            }
        }
        let clipboard = &mut self.clipboard;
        clipboard.awaited.push_back((read, asker));
        clipboard.asked.push(read);
        read
    }

    /// The focus left a field: the reads that fields asked are dropped, the person having moved
    /// on, even if the focus comes back before their answers.
    pub(super) fn drop_field_reads(&mut self) {
        let clipboard = &mut self.clipboard;
        clipboard
            .awaited
            .retain(|&(_, asker)| asker == Asker::Program);
        let awaited = &clipboard.awaited;
        clipboard
            .asked
            .retain(|read| awaited.iter().any(|(each, _)| each == read));
    }

    /// A key of copy, cut or paste, `c`, `x` or `v`, reached the field `id`, which takes it:
    /// left to the platform's events, or the selection written, a cut deleting it, or a read
    /// asked.
    pub(super) fn clipboard_key(&mut self, id: ElementId, letter: &str) {
        if self.clipboard.events {
            self.clipboard.left_to_platform = true;
        } else if letter == "v" {
            let _ = self.ask_read(Asker::Field(id));
        } else {
            self.copy_from(id, letter == "x");
        }
    }

    /// The platform's copy or cut ([`Input::Copy`](crate::host::Input::Copy)): gives whether a
    /// field held the focus, which copied.
    pub(super) fn copy_input(&mut self, cut: bool) -> bool {
        let Some(id) = self.focused_field() else {
            return false;
        };
        self.copy_from(id, cut);
        true
    }

    /// Writes what the field `id` selects, if anything, a cut deleting it, one edit.
    fn copy_from(&mut self, id: ElementId, cut: bool) {
        let selected = self
            .node(id)
            .and_then(|node| node.field.as_ref())
            .map(|field| field.selected_text().to_owned())
            .unwrap_or_default();
        if selected.is_empty() {
            return;
        }
        self.clipboard.write = Some(selected);
        if cut {
            self.delete_selection(id);
        }
    }

    /// The platform's paste ([`Input::Paste`](crate::host::Input::Paste)): gives whether a field
    /// held the focus, which pasted.
    pub(super) fn paste_input(&mut self, text: &str) -> bool {
        let Some(id) = self.focused_field() else {
            return false;
        };
        self.paste_into(id, text);
        true
    }

    /// The host's answer to the read `read`
    /// ([`Input::ClipboardText`](crate::host::Input::ClipboardText)): gives whether it was
    /// awaited.
    pub(super) fn clipboard_answer(&mut self, read: ReadId, text: Option<&str>) -> bool {
        let clipboard = &mut self.clipboard;
        let Some(at) = clipboard.awaited.iter().position(|&(each, _)| each == read) else {
            return false;
        };
        let (_, asker) = clipboard
            .awaited
            .remove(at)
            .unwrap_or((read, Asker::Program));
        clipboard.asked.retain(|&asked| asked != read);
        match asker {
            // The reads of a field whose focus left were dropped; one taken out of the tree took
            // the focus with it.
            Asker::Field(id) => {
                if let Some(text) = text
                    && self.focused_field() == Some(id)
                {
                    self.paste_into(id, text);
                }
            }
            Asker::Program => self.interaction.actions.push(Action {
                element: None,
                kind: ActionKind::ClipboardText {
                    read,
                    text: text.map(str::to_owned),
                },
            }),
        }
        true
    }

    /// Pastes `text` in the field `id`, in place of its selection, one edit: a field of one line
    /// takes a line break as a space. While an input method composes, what it composed stays as
    /// typed, its input ends, the host told that it starts again, then the text is pasted after
    /// it.
    fn paste_into(&mut self, id: ElementId, text: &str) {
        self.leave_composition(id);
        let Some(node) = self.node(id) else {
            return;
        };
        let multiline = matches!(
            node.element.kind,
            Kind::TextField {
                multiline: true,
                ..
            }
        );
        if multiline {
            self.insert_pasted(id, text);
        } else {
            self.insert_pasted(id, &text.replace(['\r', '\n'], " "));
        }
    }
}
