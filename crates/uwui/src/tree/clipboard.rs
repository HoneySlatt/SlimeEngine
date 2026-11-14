//! The clipboard as the host serves it
//! (`docs/adr/0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md`): what a field or the
//! program writes waits in the outputs; a read is asked through the outputs, numbered, and its
//! answer comes back as an event; the platform's own copy, cut and paste come as events too.

use std::collections::VecDeque;

use super::{ElementId, Kind, Ui};
use crate::action::{Action, ActionKind};
use crate::host::{ReadId, TargetId};

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
    /// The text that waits to be written, the last for each target whose outputs tell it.
    write: Vec<(TargetId, String)>,
    /// The reads asked and not yet told the host, oldest first, each with the target whose
    /// outputs tell it.
    asked: Vec<(ReadId, TargetId)>,
    /// The reads awaiting their answers, oldest first, each with who asked, and on which target.
    awaited: VecDeque<(ReadId, Asker, TargetId)>,
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
    /// What waits for `target`'s host: the text to write and the reads asked, taken.
    pub(super) fn take(&mut self, target: TargetId) -> (Option<String>, Vec<ReadId>) {
        let write = self
            .write
            .iter()
            .position(|(each, _)| *each == target)
            .map(|at| self.write.swap_remove(at).1);
        let mut reads = Vec::new();
        self.asked.retain(|&(read, each)| {
            let ours = each == target;
            if ours {
                reads.push(read);
            }
            !ours
        });
        (write, reads)
    }

    /// Puts `text` to be written in `target`'s outputs, in place of what waited there.
    fn put(&mut self, target: TargetId, text: String) {
        self.write.retain(|(each, _)| *each != target);
        self.write.push((target, text));
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
    /// (`docs/adr/0162-creating-copying-and-arranging.md`): it waits in the outputs of the active
    /// target, where the person's gesture was ([`Ui::active_target`],
    /// [`Outputs::clipboard_write`](crate::host::Outputs::clipboard_write)).
    pub fn write_clipboard(&mut self, text: &str) {
        self.clipboard.put(self.interaction.active, text.to_owned());
    }

    /// Asks the host to read the clipboard for the program, as an editor's paste without a field
    /// does: the read waits in the outputs of the active target ([`Ui::active_target`],
    /// [`Outputs::clipboard_reads`](crate::host::Outputs::clipboard_reads)), and its answer
    /// reaches the program as [`ActionKind::ClipboardText`], with the read that this gives. A
    /// text that the program wrote before is what it reads, the host writing before it reads.
    /// A read dropped past the 16 that wait at most reaches the program with no text; one that the
    /// host never answers waits until newer reads push it out.
    pub fn read_clipboard(&mut self) -> ReadId {
        self.ask_read(Asker::Program, self.interaction.active)
    }

    /// Asks a read for `asker` in `target`'s outputs; past [`AWAITED_READS`], the oldest is
    /// dropped.
    fn ask_read(&mut self, asker: Asker, target: TargetId) -> ReadId {
        let clipboard = &mut self.clipboard;
        clipboard.last += 1;
        let read = ReadId(clipboard.last);
        if clipboard.awaited.len() == AWAITED_READS
            && let Some((dropped, asker, _)) = clipboard.awaited.pop_front()
        {
            clipboard.asked.retain(|&(asked, _)| asked != dropped);
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
        clipboard.awaited.push_back((read, asker, target));
        clipboard.asked.push((read, target));
        read
    }

    /// `t`'s focus left a field: the reads that fields of `t` asked are dropped, the person having
    /// moved on, even if the focus comes back before their answers.
    pub(super) fn drop_field_reads(&mut self, t: TargetId) {
        let clipboard = &mut self.clipboard;
        clipboard
            .awaited
            .retain(|&(_, asker, target)| asker == Asker::Program || target != t);
        let awaited = &clipboard.awaited;
        clipboard
            .asked
            .retain(|(read, _)| awaited.iter().any(|(each, _, _)| each == read));
    }

    /// A key of copy, cut or paste, `c`, `x` or `v`, reached the field `id`, which takes it:
    /// left to the platform's events, or the selection written, a cut deleting it, or a read
    /// asked.
    pub(super) fn clipboard_key(&mut self, id: ElementId, letter: &str) {
        if self.clipboard.events {
            self.clipboard.left_to_platform = true;
        } else if letter == "v" {
            let _ = self.ask_read(Asker::Field(id), self.input_target(id));
        } else {
            self.copy_from(id, letter == "x");
        }
    }

    /// The platform's copy or cut ([`Input::Copy`](crate::host::Input::Copy)): gives whether a
    /// field held the focus, which copied.
    pub(super) fn copy_input(&mut self, t: TargetId, cut: bool) -> bool {
        let Some(id) = self.focused_field(t) else {
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
        self.clipboard.put(self.input_target(id), selected);
        if cut {
            self.delete_selection(id);
        }
    }

    /// The platform's paste ([`Input::Paste`](crate::host::Input::Paste)): gives whether a field
    /// held the focus, which pasted.
    pub(super) fn paste_input(&mut self, t: TargetId, text: &str) -> bool {
        let Some(id) = self.focused_field(t) else {
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
        let Some(at) = clipboard
            .awaited
            .iter()
            .position(|&(each, _, _)| each == read)
        else {
            return false;
        };
        let (_, asker, target) =
            clipboard
                .awaited
                .remove(at)
                .unwrap_or((read, Asker::Program, TargetId::MAIN));
        clipboard.asked.retain(|&(asked, _)| asked != read);
        match asker {
            // The reads of a field whose focus left were dropped; one taken out of the tree took
            // the focus with it. The answer pastes whatever target it came on.
            Asker::Field(id) => {
                if let Some(text) = text
                    && self.focused_field(target) == Some(id)
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
