//! A prototype of the host's API, without a window, thrown away once M6d.5a's ADR is accepted
//! (`docs/adr/0248-the-hosts-api-cut-in-five-built-where-it-is-read.md`, "Three channels and one
//! callback"). It tries three hard cases against the real [`Ui`], and nothing else:
//!
//! - an input method's synchronous question, answered from the field's live state, in UTF-8 bytes
//!   and UTF-16 code units ([`Ui::surrounding_text`], [`Ui::range_rects`], [`Ui::character_at`]);
//! - copy, cut and paste as events that answer at once, and the paste key as a request to read
//!   the clipboard, answered by a later event ([`HostEvent::ClipboardRead`]);
//! - two targets, each with its own roots, hit test, pointer, hover and cursor
//!   ([`Ui::set_target`]).
//!
//! Events arrive one at a time through [`Ui::host_event`] and answer at once ([`Answer`]); the
//! host reads the outputs of each target after each event and each frame ([`Ui::take_outputs`]).
//! Times and sources of events, the waker and the frame of each target are not prototyped.

use std::ops::Range;
use std::sync::Arc;

use slime_input::{ButtonState, CursorShape, Ime, KeyEvent, Modifiers, MouseButton};

use super::{ElementId, Kind, Ui};
use crate::Rect;
use crate::display::DisplayList;
use crate::text::{Affinity, TextLayout, TextPosition};

/// A surface that the host draws roots into and that input comes from: a window, a canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TargetId(pub u32);

impl TargetId {
    /// The target of every root that was given none.
    pub const MAIN: Self = Self(0);
}

/// The units in which a host counts text: UTF-8 bytes, as uwui and Wayland's text input count, or
/// UTF-16 code units, as Android, Apple's systems, Windows and the web count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    /// Bytes of UTF-8.
    Utf8,
    /// Code units of UTF-16.
    Utf16,
}

impl Units {
    /// How many units `character` takes.
    fn of(self, character: char) -> usize {
        match self {
            Self::Utf8 => character.len_utf8(),
            Self::Utf16 => character.len_utf16(),
        }
    }
}

/// A place in a field's text, in both units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextIndex {
    /// In bytes of UTF-8.
    pub utf8: usize,
    /// In code units of UTF-16.
    pub utf16: usize,
}

/// What an input method asks around the caret ([`Ui::surrounding_text`]): every place is counted
/// from the start of the field's whole text, which holds what the method composes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurroundingText {
    /// The text before the selection, up to the count asked, never cutting a character.
    pub before: String,
    /// The text selected; empty for a caret.
    pub selected: String,
    /// The text after the selection, up to the count asked.
    pub after: String,
    /// Where `before` starts.
    pub start: TextIndex,
    /// The selection's side that stays as it grows; for a caret, the caret.
    pub anchor: TextIndex,
    /// The selection's side that moves; for a caret, the caret.
    pub focus: TextIndex,
    /// The text that the input method composes, if it composes.
    pub composition: Option<Range<TextIndex>>,
}

/// An event that the host gives, on one target ([`Ui::host_event`]).
#[derive(Clone, Copy, Debug)]
pub enum HostEvent<'a> {
    /// The pointer moved to `x` and `y`, in pixels of the target's surface.
    PointerMoved {
        /// Across, from the surface's left.
        x: f32,
        /// Down, from the surface's top.
        y: f32,
    },
    /// The pointer left the target's surface.
    PointerLeft,
    /// A button of the mouse went down or up.
    PointerButton {
        /// Which button.
        button: MouseButton,
        /// Down or up.
        state: ButtonState,
    },
    /// The modifier keys held changed.
    Modifiers(Modifiers),
    /// A key went down or up.
    Key(&'a KeyEvent),
    /// An input method composed, committed, or turned on or off.
    Ime(&'a Ime),
    /// The platform's copy, as the web's `copy` event: the answer carries the text to write.
    Copy,
    /// The platform's cut: the answer carries the text, which the field deletes.
    Cut,
    /// The platform's paste, with its text, as the web's `paste` event.
    Paste(&'a str),
    /// The host's answer to a request to read the clipboard ([`Outputs::read_clipboard`]): its
    /// text; none when it holds no text, or the person refused.
    ClipboardRead {
        /// The request's number.
        request: u64,
        /// The clipboard's text.
        text: Option<&'a str>,
    },
}

/// What an event answers at once, inside the platform's handler.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    /// Whether the interface took it, which a game then leaves.
    pub taken: bool,
    /// Whether the platform's own handling must go on though the interface took it: on the web,
    /// the keys of copy, cut and paste, whose events then carry the clipboard
    /// ([`Ui::set_clipboard_events`]), are not prevented.
    pub platform_acts: bool,
    /// The text that the handler writes on the clipboard: a copy's or a cut's.
    pub clipboard: Option<String>,
}

/// What the host reads of a target after each event and each frame: only what changed since it
/// last read ([`Ui::take_outputs`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outputs {
    /// The pointer's shape over the target, when it changed.
    pub cursor: Option<CursorShape>,
    /// The text to put on the clipboard: the keys of copy and cut.
    pub write_clipboard: Option<String>,
    /// The requests to read the clipboard, by number, which [`HostEvent::ClipboardRead`] answers.
    pub read_clipboard: Vec<u64>,
}

/// What the prototype keeps in the interface.
#[derive(Default)]
pub(crate) struct HostState {
    /// Whether a host drives the interface through [`Ui::host_event`]: the keys of copy, cut and
    /// paste then reach its outputs when no `Clipboard` was given, where they did nothing.
    driven: bool,
    /// Whether the platform's own events carry the clipboard, as the web's do.
    clipboard_events: bool,
    /// Set by a key that the platform must go on handling; read by [`Ui::host_event`].
    platform_acts: bool,
    /// The roots given a target; the others are on [`TargetId::MAIN`].
    targets: Vec<(ElementId, TargetId)>,
    /// The target whose pointer the interface's input holds now: its hit test reaches its roots
    /// alone.
    loaded: TargetId,
    /// The pointer of each other target, and what it hovers.
    pointers: Vec<(TargetId, Option<[f32; 2]>, Option<ElementId>)>,
    /// What waits for each target's host.
    outputs: Vec<(TargetId, Waiting)>,
    /// The number of the last request to read the clipboard.
    last_request: u64,
    /// The requests not yet answered, each with the field that asked.
    pastes: Vec<(u64, ElementId)>,
}

/// What waits for a target's host.
#[derive(Default)]
struct Waiting {
    /// The cursor that the host was last given.
    cursor: CursorShape,
    write: Option<String>,
    requests: Vec<u64>,
}

/// How many UTF-16 code units `text` takes: a character takes two when it takes four bytes of
/// UTF-8, one otherwise. Counts the bytes that start a character, and once more those that start
/// one of four bytes, without decoding.
pub fn utf16_len(text: &str) -> usize {
    text.bytes()
        .map(|byte| usize::from(byte & 0xC0 != 0x80) + usize::from(byte >= 0xF0))
        .sum()
}

/// The place of the byte `byte` of `text`, a boundary of its characters, in both units.
fn index(text: &str, byte: usize) -> TextIndex {
    TextIndex {
        utf8: byte,
        utf16: utf16_len(&text[..byte]),
    }
}

/// The byte of `text` at `at`, counted in `units`, moved back to the start of the character that
/// it falls in, or on to its end when `up`; kept within the text.
fn byte_at(text: &str, at: usize, units: Units, up: bool) -> usize {
    let mut byte = match units {
        Units::Utf8 => at.min(text.len()),
        Units::Utf16 => {
            let mut counted = 0;
            let mut found = text.len();
            for (start, character) in text.char_indices() {
                if counted >= at {
                    found = start;
                    break;
                }
                counted += character.len_utf16();
                // Within a character of two units: its start, or its end.
                if counted > at {
                    found = if up {
                        start + character.len_utf8()
                    } else {
                        start
                    };
                    break;
                }
            }
            found
        }
    };
    while !text.is_char_boundary(byte) {
        if up {
            byte += 1;
        } else {
            byte -= 1;
        }
    }
    byte
}

impl Ui {
    /// Puts `root` on `target`: input given for that target reaches its roots alone, and its
    /// outputs and display list are its own.
    pub fn set_target(&mut self, root: ElementId, target: TargetId) {
        self.host.targets.retain(|&(each, _)| each != root);
        self.host.targets.push((root, target));
    }

    /// Whether the platform's own events carry the clipboard, as the web's `copy`, `cut` and
    /// `paste` do: the keys of copy, cut and paste in a field are then taken from the game, but
    /// left to the platform, which fires those events ([`Answer::platform_acts`]).
    pub fn set_clipboard_events(&mut self, events: bool) {
        self.host.clipboard_events = events;
    }

    /// Gives the interface `event`, which came on `target`, and answers at once.
    pub fn host_event(&mut self, target: TargetId, event: HostEvent<'_>) -> Answer {
        self.host.driven = true;
        let taken = |taken: bool| Answer {
            taken,
            ..Answer::default()
        };
        match event {
            HostEvent::PointerMoved { x, y } => {
                self.load_target(target);
                self.pointer_moved(x, y);
                Answer::default()
            }
            HostEvent::PointerLeft => {
                self.load_target(target);
                self.pointer_left();
                Answer::default()
            }
            HostEvent::PointerButton { button, state } => {
                self.load_target(target);
                taken(self.pointer_button(button, state))
            }
            HostEvent::Modifiers(modifiers) => {
                self.modifiers(modifiers);
                Answer::default()
            }
            HostEvent::Key(event) => {
                self.host.platform_acts = false;
                let taken = self.key(event);
                Answer {
                    taken,
                    platform_acts: std::mem::take(&mut self.host.platform_acts),
                    clipboard: None,
                }
            }
            HostEvent::Ime(ime) => match self.focused_field_on(target) {
                Some(_) => taken(self.ime(ime)),
                None => Answer::default(),
            },
            HostEvent::Copy | HostEvent::Cut => {
                let Some(field) = self.focused_field_on(target) else {
                    return Answer::default();
                };
                let text = self.selected_text(field);
                if matches!(event, HostEvent::Cut) && !text.is_empty() {
                    self.edit_text(field, |field, _| field.delete(0..0));
                }
                Answer {
                    taken: true,
                    platform_acts: false,
                    clipboard: (!text.is_empty()).then_some(text),
                }
            }
            HostEvent::Paste(text) => match self.focused_field_on(target) {
                Some(field) => {
                    self.paste_into(field, text);
                    taken(true)
                }
                None => Answer::default(),
            },
            HostEvent::ClipboardRead { request, text } => {
                let Some(at) = self
                    .host
                    .pastes
                    .iter()
                    .position(|&(each, _)| each == request)
                else {
                    return Answer::default();
                };
                let (_, field) = self.host.pastes.remove(at);
                // The focus leaving the field drops its requests (`Ui::host_focus_moves`); a field
                // taken out of the tree took the focus with it.
                if self.focused_field() != Some(field) {
                    return Answer::default();
                }
                if let Some(text) = text {
                    self.paste_into(field, text);
                }
                taken(true)
            }
        }
    }

    /// What waits for `target`'s host, taken: the cursor where it changed, the clipboard to write,
    /// the requests to read it.
    pub fn take_outputs(&mut self, target: TargetId) -> Outputs {
        self.load_target(target);
        let cursor = self.cursor();
        let waiting = self.waiting(target);
        let changed = (waiting.cursor != cursor).then_some(cursor);
        waiting.cursor = cursor;
        Outputs {
            cursor: changed,
            write_clipboard: waiting.write.take(),
            read_clipboard: std::mem::take(&mut waiting.requests),
        }
    }

    /// What `target`'s roots draw, as the last [`Ui::frame`] drew them.
    pub fn target_list(&self, target: TargetId) -> DisplayList {
        let mut list = DisplayList::new();
        for &root in &self.roots {
            if self.target_of(root) == target {
                crate::paint::gather(self, root, &mut list);
            }
        }
        list
    }

    /// The text around the caret of the field that holds the focus on `target`, up to `before`
    /// units before the selection and `after` units after it, counted in `units`; its selection
    /// and its composition. None without such a field.
    pub fn surrounding_text(
        &self,
        target: TargetId,
        before: usize,
        after: usize,
        units: Units,
    ) -> Option<SurroundingText> {
        let id = self.focused_field_on(target)?;
        let field = self.node(id)?.field.as_ref()?;
        let text = field.text.as_str();
        let (anchor, focus) = field.ends();
        let selected = anchor.min(focus)..anchor.max(focus);
        let mut start = selected.start;
        let mut counted = 0;
        for character in text[..selected.start].chars().rev() {
            counted += units.of(character);
            if counted > before {
                break;
            }
            start -= character.len_utf8();
        }
        let mut end = selected.end;
        let mut counted = 0;
        for character in text[selected.end..].chars() {
            counted += units.of(character);
            if counted > after {
                break;
            }
            end += character.len_utf8();
        }
        Some(SurroundingText {
            before: text[start..selected.start].to_owned(),
            selected: text[selected.clone()].to_owned(),
            after: text[selected.end..end].to_owned(),
            start: index(text, start),
            anchor: index(text, anchor),
            focus: index(text, focus),
            composition: field
                .compose
                .clone()
                .map(|range| index(text, range.start)..index(text, range.end)),
        })
    }

    /// The rectangles that `range` of the text of the field that holds the focus on `target`
    /// covers, counted in `units`, in pixels of the target's surface: one for each line and piece
    /// of one direction; for an empty range, the caret there. A range that cuts a character is
    /// widened to the whole character. The field's text laid out as it is now, scrolled as the next
    /// frame will show it, from its box as last laid out. None without such a field.
    pub fn range_rects(
        &mut self,
        target: TargetId,
        range: Range<usize>,
        units: Units,
    ) -> Option<Vec<Rect>> {
        let id = self.focused_field_on(target)?;
        let text = &self.node(id)?.field.as_ref()?.text;
        let start = byte_at(text, range.start, units, false);
        let end = byte_at(text, range.end, units, true).max(start);
        let (layout, origin) = self.settled_layout(id)?;
        let placed = |rect: Rect| {
            Rect::new(
                rect.x + origin[0],
                rect.y + origin[1],
                rect.width,
                rect.height,
            )
        };
        if start == end {
            let caret = layout.caret(TextPosition {
                index: start,
                affinity: Affinity::Downstream,
            });
            return Some(vec![placed(caret)]);
        }
        Some(
            layout
                .selection(start..end)
                .into_iter()
                .map(placed)
                .collect(),
        )
    }

    /// The character of the field that holds the focus on `target` drawn at `x` and `y`, in
    /// pixels of the target's surface, within the field's box, scrolled as the next frame will show
    /// it: where it starts. None where no character is drawn.
    pub fn character_at(&mut self, target: TargetId, x: f32, y: f32) -> Option<TextIndex> {
        let id = self.focused_field_on(target)?;
        if !super::interact::inside(self.node(id)?.content_box(), x, y) {
            return None;
        }
        let (layout, origin) = self.settled_layout(id)?;
        let cluster = layout.cluster_at(x - origin[0], y - origin[1])?;
        let text = &self.node(id)?.field.as_ref()?.text;
        Some(index(text, cluster.start))
    }

    /// The layout of the field `id` and its origin, its caret placed and its text scrolled first,
    /// as the next frame's paint would: the focus given without an edit, by Tab or the program,
    /// leaves both as the field was last drawn (`paint.rs`'s `settle_field`).
    fn settled_layout(&mut self, id: ElementId) -> Option<(Arc<TextLayout>, [f32; 2])> {
        self.settle_field(id);
        self.field_layout(id)
    }

    /// A key of copy, cut or paste, `letter` `c`, `x` or `v`, reached the field `id`: gives
    /// whether the host takes it, as it does when it drives the interface and gave no
    /// `Clipboard`. Copy and cut write the outputs, paste asks for the clipboard; on the web,
    /// the platform goes on to its own event.
    pub(super) fn host_clipboard_key(&mut self, id: ElementId, letter: &str) -> bool {
        if !self.host.driven || self.has_clipboard() {
            return false;
        }
        if self.host.clipboard_events {
            self.host.platform_acts = true;
            return true;
        }
        let target = self.target_of_element(id);
        if letter == "v" {
            self.host.last_request += 1;
            let request = self.host.last_request;
            self.host.pastes.push((request, id));
            self.waiting(target).requests.push(request);
            return true;
        }
        let text = self.selected_text(id);
        if text.is_empty() {
            return true;
        }
        self.waiting(target).write = Some(text);
        if letter == "x" {
            self.edit_text(id, |field, _| field.delete(0..0));
        }
        true
    }

    /// The focus moves: the requests to read the clipboard that a field made are dropped, the
    /// person having moved on.
    pub(super) fn host_focus_moves(&mut self) {
        self.host.pastes.clear();
    }

    /// Whether the pointer of the loaded target reaches `root`: a root of that target, or any
    /// root while no root was given a target.
    pub(super) fn pointer_reaches(&self, root: ElementId) -> bool {
        self.host.targets.is_empty() || self.target_of(root) == self.host.loaded
    }

    /// Gives the interface's input `target`'s pointer and hover, keeping the target's before.
    fn load_target(&mut self, target: TargetId) {
        let loaded = self.host.loaded;
        if loaded == target {
            return;
        }
        let pointer = self.interaction.pointer.take();
        let hovered = self.interaction.hovered.take();
        self.host.pointers.retain(|&(each, ..)| each != loaded);
        self.host.pointers.push((loaded, pointer, hovered));
        let (pointer, hovered) = match self
            .host
            .pointers
            .iter()
            .position(|&(each, ..)| each == target)
        {
            Some(at) => {
                let (_, pointer, hovered) = self.host.pointers.remove(at);
                (pointer, hovered)
            }
            None => (None, None),
        };
        self.interaction.pointer = pointer;
        self.interaction.hovered = hovered;
        self.host.loaded = target;
    }

    /// The target of `root`.
    fn target_of(&self, root: ElementId) -> TargetId {
        self.host
            .targets
            .iter()
            .find(|&&(each, _)| each == root)
            .map_or(TargetId::MAIN, |&(_, target)| target)
    }

    /// The target of the root that holds `id`.
    fn target_of_element(&self, id: ElementId) -> TargetId {
        self.root_of(id)
            .map_or(TargetId::MAIN, |root| self.target_of(root))
    }

    /// The field that holds the focus, if it lies on `target`.
    fn focused_field_on(&self, target: TargetId) -> Option<ElementId> {
        self.focused_field()
            .filter(|&id| self.target_of_element(id) == target)
    }

    /// What waits for `target`'s host.
    fn waiting(&mut self, target: TargetId) -> &mut Waiting {
        let at = match self
            .host
            .outputs
            .iter()
            .position(|(each, _)| *each == target)
        {
            Some(at) => at,
            None => {
                self.host.outputs.push((target, Waiting::default()));
                self.host.outputs.len() - 1
            }
        };
        &mut self.host.outputs[at].1
    }

    /// The text selected in the field `id`.
    fn selected_text(&self, id: ElementId) -> String {
        self.node(id)
            .and_then(|node| node.field.as_ref())
            .map(|field| field.text[field.selected()].to_owned())
            .unwrap_or_default()
    }

    /// Pastes `text` in the field `id`, as its key of paste does: a field of one line takes a line
    /// break as a space.
    fn paste_into(&mut self, id: ElementId, text: &str) {
        let multiline = self.node(id).is_some_and(|node| {
            matches!(
                node.element.kind,
                Kind::TextField {
                    multiline: true,
                    ..
                }
            )
        });
        let text = if multiline {
            text.to_owned()
        } else {
            text.replace(['\r', '\n'], " ")
        };
        self.edit_text(id, |field, _| field.insert(&text, false));
    }
}
