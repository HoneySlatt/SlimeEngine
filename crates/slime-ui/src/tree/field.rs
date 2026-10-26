//! Fields of text and of numbers: what the user types, selects, copies and composes through an
//! input method, kept by the field and read by the program as actions
//! (`docs/adr/0145-text-edited.md`).
//!
//! A field's text is laid out as a label's, by [`Fonts::lay_out`](crate::text::Fonts::lay_out),
//! and parley moves its selection on that layout, by clusters, words and lines, in text of both
//! directions; the field keeps its own text, the text that an input method composes, and its undo.

use std::ops::Range;
use std::sync::Arc;

use parley::{Affinity as ParleyAffinity, Cursor, Layout, Selection};
use slime_input::{CursorShape, Ime, Key, KeyEvent, NamedKey};

use super::meaning::{FieldText, RUN_CHARACTERS, RunCharacter, TextRun};
use super::{ElementId, Kind, Ui};
use crate::Rect;
use crate::action::{Action, ActionKind, FieldValue};
use crate::shortcuts;
use crate::text::{Affinity, Alignment, TextBounds, TextLayout, TextPosition};

/// How many edits a field undoes.
const UNDO_DEPTH: usize = 100;

/// How far the pointer moves, in pixels of the screen, before a press on a field of numbers that
/// does not hold the focus drags its number rather than gives it the focus.
const DRAG_THRESHOLD: f32 = 3.0;

/// The system's clipboard, of text, as an interface reads and writes it ([`Ui::set_clipboard`]):
/// the platform's, which `slime-app` gives, or a test's own.
pub trait Clipboard: Send {
    /// The text that it holds; none when it holds no text.
    fn text(&mut self) -> Option<String>;
    /// Puts `text` in it, in place of what it held.
    fn set_text(&mut self, text: &str);
}

/// What a field keeps between two events.
pub(crate) struct Field {
    /// What it holds, with the text that an input method composes while it composes.
    pub(crate) text: String,
    /// The program's text, as it last gave it, or the text that the field last committed: a field
    /// of numbers' number, as Rust writes it.
    given: String,
    /// The selection, as parley moves it on the layout of `text`; collapsed, the caret.
    selection: Selection,
    /// The selection that an edit leaves, by bytes, an anchor and a focus, set on the layout of
    /// the text that the edit made once it is laid out.
    pending: Option<(usize, usize)>,
    /// The bytes of `text` that an input method composes, not yet committed.
    pub(crate) compose: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Whether the last edit was typed, or a step of a drag, which the next typed edit, or the next
    /// step of the drag, joins in one undo.
    typing: bool,
    /// Whether the user changed it since it took the focus, or since its last commit.
    edited: bool,
    /// How far its text is scrolled to keep the caret in view, in pixels of the screen.
    pub(crate) offset: [f32; 2],
    /// The caret's box, in pixels of the surface, as the field was last laid out: where the input
    /// method places its window.
    pub(crate) caret: Rect,
    /// A press on a field of numbers that does not hold the focus, which drags its number once the
    /// pointer moves past [`DRAG_THRESHOLD`].
    drag: Option<Drag>,
    /// The numbers of the runs of its text, as whoever reads what the interface means reads them
    /// ([`TextRun::number`](super::TextRun::number)), the first run's first: a run keeps its number
    /// while the text changes.
    pub(crate) runs: Vec<u64>,
}

/// What a field held, which an undo gives back.
struct Snapshot {
    text: String,
    anchor: usize,
    focus: usize,
}

/// A press on a field of numbers.
#[derive(Clone, Copy)]
struct Drag {
    /// Where the pointer pressed, along x, in pixels of the screen.
    x: f32,
    /// The number then.
    value: f64,
    /// Whether the pointer moved past the threshold: the number is dragged.
    moved: bool,
}

impl Field {
    /// A field holding `text`, its caret at its end.
    pub(crate) fn new(text: String) -> Self {
        let end = text.len();
        Self {
            given: text.clone(),
            text,
            selection: Selection::default(),
            pending: Some((end, end)),
            compose: None,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            edited: false,
            offset: [0.0; 2],
            caret: Rect::default(),
            drag: None,
            runs: Vec::new(),
        }
    }

    /// The program gives `text`, for a field of `kind`: it replaces what the field holds when it
    /// differs from the later of the text that the program gave before and the one that the field
    /// last committed, and from what the field holds, as the program reads it: a program that
    /// gives back each change keeps the user's spelling of a number and what an input method
    /// composes. The field forgets its undo then, and that the user changed it. Gives whether it
    /// replaced it.
    pub(crate) fn give(&mut self, text: &str, kind: &Kind) -> bool {
        if self.given == text {
            return false;
        }
        self.given = text.to_owned();
        if self
            .value(kind)
            .is_some_and(|value| value_text(&value) == text)
        {
            return false;
        }
        self.text = text.to_owned();
        self.compose = None;
        self.undo.clear();
        self.redo.clear();
        self.typing = false;
        self.edited = false;
        self.pending = Some((text.len(), text.len()));
        true
    }

    /// The bytes selected, from the first to the last.
    fn selected(&self) -> Range<usize> {
        if self.pending.is_some() {
            let (anchor, focus) = self.pending.unwrap_or_default();
            return anchor.min(focus)..anchor.max(focus);
        }
        self.selection.text_range()
    }

    /// Keeps what the field holds to undo the edit that follows, unless it is typed after a typed
    /// one, which joins it.
    fn remember(&mut self, typing: bool) {
        if !(typing && self.typing) {
            let (anchor, focus) = self.ends();
            self.undo.push(Snapshot {
                text: self.text.clone(),
                anchor,
                focus,
            });
            if self.undo.len() > UNDO_DEPTH {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = typing;
    }

    /// The selection's anchor and focus, by bytes.
    fn ends(&self) -> (usize, usize) {
        self.pending.unwrap_or((
            self.selection.anchor().index(),
            self.selection.focus().index(),
        ))
    }

    /// Replaces the bytes `range` with `with`, the caret after it, and marks the field edited.
    fn replace(&mut self, range: Range<usize>, with: &str) {
        self.text.replace_range(range.clone(), with);
        let caret = range.start + with.len();
        self.pending = Some((caret, caret));
        self.edited = true;
    }

    /// Takes out the text that an input method composes, the caret where it began; gives where it
    /// began, if it composed any.
    fn uncompose(&mut self) -> Option<usize> {
        let range = self.compose.take()?;
        self.text.replace_range(range.clone(), "");
        self.pending = Some((range.start, range.start));
        Some(range.start)
    }

    /// Replaces the selection, or the text being composed, with `with`, as typing or a paste does;
    /// gives whether that changed the text, the text composed left aside.
    fn insert(&mut self, with: &str, typing: bool) -> bool {
        // The text being composed goes first, so that an undo gives back the text before it.
        let range = match self.uncompose() {
            Some(start) => start..start,
            None => self.selected(),
        };
        if range.is_empty() && with.is_empty() {
            return false;
        }
        self.remember(typing);
        self.replace(range, with);
        true
    }

    /// Deletes the selection, or, collapsed, the bytes `otherwise`; gives whether it deleted any.
    fn delete(&mut self, otherwise: Range<usize>) -> bool {
        let range = match self.selected() {
            range if !range.is_empty() => range,
            _ => otherwise,
        };
        if range.is_empty() {
            return false;
        }
        self.remember(false);
        self.replace(range, "");
        true
    }

    /// Gives back what the field held before its last edit, or after its last undo; gives whether
    /// there was one to give back.
    fn undo(&mut self, redo: bool) -> bool {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        let (anchor, focus) = self.pending.unwrap_or((
            self.selection.anchor().index(),
            self.selection.focus().index(),
        ));
        to.push(Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            anchor,
            focus,
        });
        self.compose = None;
        self.pending = Some((snapshot.anchor, snapshot.focus));
        self.typing = false;
        self.edited = true;
        true
    }

    /// What the field holds, as the program reads it: a field of numbers' number, if its text
    /// reads as a finite one.
    fn value(&self, kind: &Kind) -> Option<FieldValue> {
        let text = match &self.compose {
            Some(range) => {
                let mut text = self.text.clone();
                text.replace_range(range.clone(), "");
                text
            }
            None => self.text.clone(),
        };
        match kind {
            Kind::NumberField { .. } => text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map(FieldValue::Number),
            _ => Some(FieldValue::Text(text)),
        }
    }
}

/// The text that the program gives a field of `kind`; none for an element that is no field.
pub(crate) fn given(kind: &Kind) -> Option<String> {
    match kind {
        Kind::TextField { text, .. } => Some(text.clone()),
        Kind::NumberField { value, .. } => Some(number_text(*value)),
        _ => None,
    }
}

/// `value` as a field of numbers shows it: whole, as Rust writes it.
pub(crate) fn number_text(value: f64) -> String {
    format!("{value}")
}

/// The text that the program gives to keep a field as `value` left it.
fn value_text(value: &FieldValue) -> String {
    match value {
        FieldValue::Text(text) => text.clone(),
        FieldValue::Number(number) => number_text(*number),
        FieldValue::Checked(checked) => checked.to_string(),
    }
}

/// `value`, a sum of `terms` and their multiples, as a drag or a slider's steps give it, rounded to
/// as many decimals as the term written with the most has, as Rust writes them shortest: what
/// adding them up in binary leaves past them, `0.30000000000000004`, is dropped. Terms of more than
/// 15 decimals, and a value too large to round, leave it as it is.
pub(crate) fn rounded(value: f64, terms: &[f64]) -> f64 {
    let decimals = terms
        .iter()
        .map(|term| {
            number_text(term.abs())
                .split_once('.')
                .map_or(0, |(_, decimals)| decimals.len())
        })
        .max()
        .unwrap_or(0);
    // Past 15 decimals, a power of ten loses what it is meant to keep.
    if decimals > 15 {
        return value;
    }
    let scale = 10f64.powi(decimals as i32);
    let rounded = (value * scale).round() / scale;
    if rounded.is_finite() { rounded } else { value }
}

impl Ui {
    /// Gives the interface the system's clipboard, which its fields copy to and paste from:
    /// without one, they copy and paste nothing.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = Some(clipboard);
    }

    /// Whether the interface has a clipboard ([`Ui::set_clipboard`]).
    pub fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    /// The text that the field `id` holds now, as the user left it, the text that an input
    /// method composes included; none for an element that is no field.
    pub fn field_text(&self, id: ElementId) -> Option<&str> {
        self.node(id)?
            .field
            .as_ref()
            .map(|field| field.text.as_str())
    }

    /// Gives the field `id` the text `text`, as a person who replaced all that it holds and
    /// committed would: the text that an input method composes goes, the replacement is one edit,
    /// which the field's undo takes back, then the field commits, the program reading
    /// [`ActionKind::Changed`] then [`ActionKind::Committed`], the same text given again included;
    /// an empty text given to an empty field changes nothing, and is read as nothing. A field of
    /// one line takes a line
    /// break as a space, as a paste does. Gives whether the field took it: not an element that is
    /// no field, nor a field that cannot hold the focus now, disabled, hidden or below a modal
    /// root, nor a field of numbers given a text that reads as no finite number, which keeps what
    /// it holds. A slider takes a text that reads as a finite number, kept within its bounds and on
    /// its steps, the program reading the change, if any, then the commit; two panels, the share of
    /// the first, kept from 0 to 1, which tells the program nothing. The focus stays where it is.
    pub fn set_value(&mut self, id: ElementId, text: &str) -> bool {
        if !self.focusable(id) || !self.in_scope(id) {
            return false;
        }
        let Some(node) = self.node(id) else {
            return false;
        };
        // A slider takes a finite number, kept within its bounds and on its steps; two panels,
        // their first panel's share.
        match node.element.kind {
            Kind::Slider { .. } => {
                return match text.trim().parse::<f64>() {
                    Ok(number) if number.is_finite() => self.set_slider(id, number, true),
                    _ => false,
                };
            }
            Kind::Split { .. } => {
                return match text.trim().parse::<f32>() {
                    Ok(share) if share.is_finite() => self.set_share(id, share),
                    _ => false,
                };
            }
            _ => {}
        }
        let kind = &node.element.kind;
        let multiline = matches!(
            kind,
            Kind::TextField {
                multiline: true,
                ..
            }
        );
        let number = matches!(kind, Kind::NumberField { .. });
        if node.field.is_none()
            || (number
                && !text
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|number| number.is_finite()))
        {
            return false;
        }
        let text = if multiline {
            text.to_owned()
        } else {
            text.replace(['\r', '\n'], " ")
        };
        self.edit_text(id, |field, _| {
            field.uncompose();
            field.pending = Some((0, field.text.len()));
            field.insert(&text, false)
        });
        self.commit(id);
        true
    }

    /// Selects the bytes of the field `id` from `anchor` to `focus`, as the pointer or the keys
    /// would, each moved back to the start of the character that it falls in, and kept within the
    /// text; collapsed, it places the caret. Gives whether `id` is a field, whether or not it could
    /// hold the focus now: the caller checks that it takes
    /// [`ElementAction::SetValue`](super::ElementAction::SetValue) when a person's rules apply.
    /// The focus stays where it is.
    pub fn select_text(&mut self, id: ElementId, anchor: usize, focus: usize) -> bool {
        self.edit_field(id, |field, layout| {
            let start = |byte: usize| {
                let mut byte = byte.min(field.text.len());
                while !field.text.is_char_boundary(byte) {
                    byte -= 1;
                }
                byte
            };
            field.selection = selection(layout, start(anchor), start(focus));
            field.typing = false;
        })
        .is_some()
    }

    /// The text of the field `id` as a screen reader reads it: its runs, line by line, and its
    /// selection, laid out as the field draws it, from its box as last laid out. A run that the
    /// field did not have before is given a number never given before, which it keeps. None for an
    /// element that is no field.
    pub fn field_runs(&mut self, id: ElementId) -> Option<FieldText> {
        let (layout, origin) = self.field_layout(id)?;
        let field = self.node(id)?.field.as_ref()?;
        let (anchor, focus) = field.ends();
        let mut runs = text_runs(&layout, origin);
        let field = self.node_mut(id)?.field.as_mut()?;
        let mut numbers = std::mem::take(&mut field.runs);
        while numbers.len() < runs.len() {
            numbers.push(self.number());
        }
        for (run, &number) in runs.iter_mut().zip(&numbers) {
            run.number = number;
        }
        self.node_mut(id)?.field.as_mut()?.runs = numbers;
        Some(FieldText {
            anchor: place_in(&runs, anchor),
            focus: place_in(&runs, focus),
            runs,
        })
    }

    /// The shape that the pointer takes where it is: the I-beam over a field of text and over a
    /// field of numbers that holds the focus, the arrows of a drag over one that does not, or
    /// while one is dragged; the arrow elsewhere.
    pub fn cursor(&self) -> CursorShape {
        if let Some(captured) = self.interaction.captured
            && self
                .node(captured)
                .and_then(|node| node.field.as_ref())
                .is_some_and(|field| field.drag.is_some_and(|drag| drag.moved))
        {
            return CursorShape::ResizeHorizontal;
        }
        // Over the handle between two panels, or while it is dragged: the arrows along it.
        let split = self
            .interaction
            .captured
            .or_else(|| self.pointer_target(|kind| matches!(kind, Kind::Split { .. })));
        if let Some(Kind::Split { side_by_side, .. }) = split
            .and_then(|id| self.node(id))
            .map(|node| &node.element.kind)
        {
            return if *side_by_side {
                CursorShape::ResizeHorizontal
            } else {
                CursorShape::ResizeVertical
            };
        }
        let over = self.pointer_target(|kind| {
            matches!(kind, Kind::TextField { .. } | Kind::NumberField { .. })
        });
        match over.and_then(|id| Some((id, self.node(id)?))) {
            Some((_, node)) if matches!(node.element.kind, Kind::TextField { .. }) => {
                CursorShape::Text
            }
            Some((id, _)) if self.interaction.focus == Some(id) => CursorShape::Text,
            Some(_) => CursorShape::ResizeHorizontal,
            None => CursorShape::Default,
        }
    }

    /// Where text is typed, when a field holds the focus: its caret's box, in pixels of the
    /// surface, beside which an input method places its window. The program allows the input
    /// method while it is some, and forbids it otherwise.
    pub fn text_input(&self) -> Option<Rect> {
        let focus = self.interaction.focus?;
        self.node(focus)?.field.as_ref().map(|field| field.caret)
    }

    /// An input method composed or committed text, or turned off: the field that holds the focus
    /// shows what it composes, under a line, in place of its selection, which a composition
    /// deletes as it starts, and takes what it commits as typed text. Gives whether a field took
    /// it.
    pub fn ime(&mut self, ime: &Ime) -> bool {
        let Some(focus) = self.focused_field() else {
            return false;
        };
        self.edit_text(focus, |field, _| match ime {
            Ime::Preedit { text, cursor } => {
                let mut changed = false;
                let start = match field.uncompose() {
                    Some(start) => start,
                    // An empty composition where none was, which input methods send before each
                    // commit, changes nothing.
                    None if text.is_empty() => return false,
                    None => {
                        // A composition that starts over a selection deletes it, as typing does:
                        // an edit, which the commit joins.
                        let selected = field.selected();
                        if !selected.is_empty() {
                            field.remember(false);
                            field.replace(selected.clone(), "");
                            field.typing = true;
                            changed = true;
                        }
                        selected.start
                    }
                };
                field.text.insert_str(start, text);
                field.compose = (!text.is_empty()).then(|| start..start + text.len());
                let caret = start + cursor.map_or(text.len(), |(_, end)| end.min(text.len()));
                field.pending = Some((caret, caret));
                changed
            }
            Ime::Commit(text) => field.insert(text, true),
            Ime::Disabled => {
                field.uncompose();
                false
            }
            Ime::Enabled => false,
        });
        true
    }

    /// Commits the field `id`, as Enter, the focus leaving, the release of a drag or an agent's
    /// `Accept` does: the text that an input method composes goes, and what the field holds
    /// reaches the queue as [`ActionKind::Committed`] if the user changed it since it took the
    /// focus or since its last commit; the program's text is compared with it from then on
    /// ([`Kind::TextField`]). A field of numbers whose text reads as no number shows again the
    /// number that the program gave or that it last committed, the later of the two, and gives
    /// none.
    pub(crate) fn commit(&mut self, id: ElementId) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let kind = node.element.kind.clone();
        let Some(field) = node.field.as_mut() else {
            return;
        };
        let mut relaid = field.uncompose().is_some();
        let mut committed = None;
        if std::mem::take(&mut field.edited) {
            match field.value(&kind) {
                Some(value) => {
                    field.given = value_text(&value);
                    committed = Some(value);
                }
                None => {
                    field.text = field.given.clone();
                    field.pending = Some((field.text.len(), field.text.len()));
                    field.undo.clear();
                    field.redo.clear();
                    relaid = true;
                }
            }
        }
        if relaid {
            self.mark_layout(id);
            self.mark_paint(id);
            self.mark_meaning(id);
        }
        if let Some(value) = committed {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Committed(value),
            });
        }
    }

    /// A key reached the field `id`, which holds the focus: gives whether the field took it. It
    /// takes what it types and edits with, a dead key and a key that the engine does not name,
    /// which type through an input method, and leaves Tab, Escape, the keys that it has no use
    /// for and, in a field of one line, the up and down arrows, to the focus and the shortcuts.
    /// While
    /// an input method composes, the keys that reach the field are the input method's: it takes
    /// them all, and edits with none.
    pub(crate) fn field_key(&mut self, id: ElementId, event: &KeyEvent) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        if node
            .field
            .as_ref()
            .is_some_and(|field| field.compose.is_some())
        {
            return true;
        }
        let multiline = matches!(
            node.element.kind,
            Kind::TextField {
                multiline: true,
                ..
            }
        );
        let modifiers = self.interaction.modifiers;
        let shift = modifiers.shift;
        let control = modifiers.control && !modifiers.alt;
        // The letter of a chord, as shortcuts read it.
        let letter = match &event.logical {
            Key::Character(character) if character.is_ascii() => character.to_lowercase(),
            Key::Character(_) => shortcuts::position_letter(event).unwrap_or_default(),
            _ => String::new(),
        };
        if control {
            let clipboard = |ui: &mut Self, cut: bool| {
                let selected = ui
                    .node(id)
                    .and_then(|node| node.field.as_ref())
                    .map(|field| field.text[field.selected()].to_owned())
                    .unwrap_or_default();
                // Without a clipboard, a cut deletes nothing that it could not give back.
                let Some(clipboard) = ui.clipboard.as_mut() else {
                    return;
                };
                if selected.is_empty() {
                    return;
                }
                clipboard.set_text(&selected);
                if cut {
                    ui.edit_text(id, |field, _| field.delete(0..0));
                }
            };
            match letter.as_str() {
                "a" => {
                    self.edit_field(id, |field, layout| {
                        field.selection = Selection::new(
                            Cursor::from_byte_index(layout, 0, ParleyAffinity::Downstream),
                            Cursor::from_byte_index(
                                layout,
                                field.text.len(),
                                ParleyAffinity::Upstream,
                            ),
                        );
                        field.typing = false;
                    });
                    return true;
                }
                "c" => {
                    clipboard(self, false);
                    return true;
                }
                "x" => {
                    clipboard(self, true);
                    return true;
                }
                "v" => {
                    let pasted = self
                        .clipboard
                        .as_mut()
                        .and_then(|clipboard| clipboard.text());
                    if let Some(mut text) = pasted {
                        if !multiline {
                            text = text.replace(['\r', '\n'], " ");
                        }
                        self.edit_text(id, |field, _| field.insert(&text, false));
                    }
                    return true;
                }
                "z" | "y" => {
                    let redo = letter == "y" || shift;
                    self.edit_text(id, |field, _| field.undo(redo));
                    return true;
                }
                _ => {}
            }
        }
        if let Key::Named(named) = &event.logical {
            type Move = fn(&Selection, &Layout<()>, bool) -> Selection;
            let moved: Option<Move> = match named {
                NamedKey::ArrowLeft if control => Some(Selection::previous_visual_word),
                NamedKey::ArrowLeft => Some(Selection::previous_visual),
                NamedKey::ArrowRight if control => Some(Selection::next_visual_word),
                NamedKey::ArrowRight => Some(Selection::next_visual),
                NamedKey::ArrowUp if multiline => Some(Selection::previous_line),
                NamedKey::ArrowDown if multiline => Some(Selection::next_line),
                NamedKey::Home if control => Some(|selection, layout, extend| {
                    let start = Cursor::from_byte_index(layout, 0, ParleyAffinity::Downstream);
                    if extend {
                        selection.extend(start)
                    } else {
                        start.into()
                    }
                }),
                NamedKey::Home => Some(Selection::line_start),
                NamedKey::End if control => Some(|selection, layout, extend| {
                    // The text's end, where its last line ends.
                    let length = layout
                        .lines()
                        .last()
                        .map_or(0, |line| line.text_range().end);
                    let end = Cursor::from_byte_index(layout, length, ParleyAffinity::Upstream);
                    if extend {
                        selection.extend(end)
                    } else {
                        end.into()
                    }
                }),
                NamedKey::End => Some(Selection::line_end),
                _ => None,
            };
            if let Some(moved) = moved {
                self.edit_field(id, |field, layout| {
                    field.selection = moved(&field.selection, layout, shift);
                    field.typing = false;
                });
                return true;
            }
            match named {
                NamedKey::Backspace | NamedKey::Delete => {
                    let back = *named == NamedKey::Backspace;
                    self.edit_text(id, |field, layout| {
                        let caret = field.selection.focus();
                        let other = match (back, control) {
                            (true, true) => caret.previous_logical_word(layout).index(),
                            (true, false) => caret
                                .upstream_cluster(layout)
                                .map_or(caret.index(), |cluster| cluster.text_range().start),
                            (false, true) => caret.next_logical_word(layout).index(),
                            (false, false) => caret
                                .downstream_cluster(layout)
                                .map_or(caret.index(), |cluster| cluster.text_range().end),
                        };
                        field.delete(caret.index().min(other)..caret.index().max(other))
                    });
                    return true;
                }
                NamedKey::Enter if multiline && !control => {
                    self.edit_text(id, |field, _| field.insert("\n", true));
                    return true;
                }
                NamedKey::Enter => {
                    self.commit(id);
                    return true;
                }
                _ => {}
            }
        }
        // Text typed: without Ctrl or Alt, or with both, as AltGr gives them on Windows.
        let typed = event
            .text
            .as_deref()
            .filter(|text| !text.is_empty() && !text.chars().any(char::is_control));
        if let Some(text) = typed
            && modifiers.control == modifiers.alt
            && !modifiers.super_key
        {
            let text = text.to_owned();
            self.edit_text(id, |field, _| field.insert(&text, true));
            return true;
        }
        // The keys that type through an input method without text of their own: a dead key, which
        // the key after it combines with, and one that the engine does not name, with which an
        // input method begins to compose on Windows, before it says so.
        matches!(event.logical, Key::Dead(_) | Key::Unidentified)
    }

    /// The pointer pressed on the field `id`, at `x` and `y` in pixels of the surface, `shift`
    /// held or not: a field of text, or of numbers that holds the focus, takes the focus and
    /// places its caret there, or extends its selection; a field of numbers that does not hold
    /// it waits to see whether the press drags its number.
    pub(crate) fn field_press(&mut self, id: ElementId, x: f32, y: f32, shift: bool) {
        let number = self
            .node(id)
            .is_some_and(|node| matches!(node.element.kind, Kind::NumberField { .. }));
        if number && self.interaction.focus != Some(id) {
            let value = self.field_number(id);
            if let Some(field) = self.node_mut(id).and_then(|node| node.field.as_mut()) {
                field.drag = Some(Drag {
                    x,
                    value,
                    moved: false,
                });
                field.typing = false;
            }
            return;
        }
        self.set_focus(Some(id));
        self.place_caret(id, x, y, shift);
    }

    /// The pointer moved to `x` and `y` while the field `id` holds it: a drag selects, or, on a
    /// field of numbers past the threshold, changes the number by its step for each logical pixel.
    pub(crate) fn field_drag(&mut self, id: ElementId, x: f32, y: f32) {
        let scale = self.scale_of(id);
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let step = match node.element.kind {
            Kind::NumberField { step, .. } => step,
            _ => 0.0,
        };
        let Some(field) = node.field.as_mut() else {
            return;
        };
        let Some(drag) = field.drag.as_mut() else {
            self.place_caret(id, x, y, true);
            return;
        };
        if !drag.moved && (x - drag.x).abs() < DRAG_THRESHOLD {
            return;
        }
        drag.moved = true;
        let steps = ((x - drag.x) / scale).round() as f64;
        let text = number_text(rounded(drag.value + steps * step, &[drag.value, step]));
        if text == field.text {
            return;
        }
        // The steps of one drag, one undo.
        field.remember(true);
        field.pending = Some((text.len(), text.len()));
        field.text = text;
        field.edited = true;
        self.mark_layout(id);
        self.mark_paint(id);
        self.mark_meaning(id);
        self.changed(id);
    }

    /// The pointer let go of the field `id`, which held it: a drag of a field of numbers commits
    /// its number; a press that did not drag gives the field the focus, its caret where it was,
    /// when the pointer is let go over the field, and nothing elsewhere, as a button.
    pub(crate) fn field_release(&mut self, id: ElementId, x: f32, y: f32) {
        match self.field_let_go(id) {
            Some(drag) if drag.moved => self.commit(id),
            Some(_) if self.pointer_target(Kind::takes_pointer) == Some(id) => {
                self.set_focus(Some(id));
                self.place_caret(id, x, y, false);
            }
            _ => {}
        }
    }

    /// The next press came without the release of the one on the field `id`: a drag of its number
    /// commits what it gave, as its release would.
    pub(crate) fn field_press_lost(&mut self, id: ElementId) {
        if self.field_let_go(id).is_some_and(|drag| drag.moved) {
            self.commit(id);
        }
    }

    /// Forgets the press on the field `id`; gives the drag that it began, for a field of numbers.
    fn field_let_go(&mut self, id: ElementId) -> Option<Drag> {
        let field = self.node_mut(id)?.field.as_mut()?;
        field.typing = false;
        field.drag.take()
    }

    /// Places the caret of the field `id` at `x` and `y`, in pixels of the surface, or extends its
    /// selection there.
    fn place_caret(&mut self, id: ElementId, x: f32, y: f32, extend: bool) {
        let Some(origin) = self.field_origin(id) else {
            return;
        };
        let (x, y) = (x - origin[0], y - origin[1]);
        self.edit_field(id, |field, layout| {
            field.selection = if extend {
                field.selection.extend_to_point(layout, x, y)
            } else {
                Selection::from_point(layout, x, y)
            };
            field.typing = false;
        });
    }

    /// The field that holds the focus, if one does.
    pub(crate) fn focused_field(&self) -> Option<ElementId> {
        let focus = self.interaction.focus?;
        self.node(focus)?.field.as_ref().map(|_| focus)
    }

    /// The number that the field of numbers `id` holds, or the program's where its text reads as
    /// none.
    fn field_number(&self, id: ElementId) -> f64 {
        let Some(node) = self.node(id) else {
            return 0.0;
        };
        let given = match node.element.kind {
            Kind::NumberField { value, .. } => value,
            _ => 0.0,
        };
        match node
            .field
            .as_ref()
            .and_then(|field| field.value(&node.element.kind))
        {
            Some(FieldValue::Number(number)) => number,
            _ => given,
        }
    }

    /// Runs `edit` on the field `id` as [`Ui::edit_field`] does, and queues what the field holds
    /// then when `edit` gives that it changed it.
    fn edit_text(&mut self, id: ElementId, edit: impl FnOnce(&mut Field, &Layout<()>) -> bool) {
        if self.edit_field(id, edit) == Some(true) {
            self.changed(id);
        }
    }

    /// Queues what the field `id` holds as [`ActionKind::Changed`], unless it reads as nothing.
    fn changed(&mut self, id: ElementId) {
        let Some(node) = self.node(id) else {
            return;
        };
        let value = node
            .field
            .as_ref()
            .and_then(|field| field.value(&node.element.kind));
        if let Some(value) = value {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Changed(value),
            });
        }
    }

    /// The layout of what the field `id` holds, as it draws it, and where its origin lies, in
    /// pixels of the surface.
    pub(crate) fn field_layout(&mut self, id: ElementId) -> Option<(Arc<TextLayout>, [f32; 2])> {
        let scale = self.scale_of(id);
        let node = self.node(id)?;
        let field = node.field.as_ref()?;
        let multiline = matches!(
            node.element.kind,
            Kind::TextField {
                multiline: true,
                ..
            }
        );
        let content = node.content_box();
        let bounds = if multiline {
            TextBounds::width(content.width / scale)
        } else {
            TextBounds::default()
        };
        let text = field.text.clone();
        let offset = field.offset;
        let style = node.style.text.style.clone();
        let layout = self
            .texts
            .get(&mut self.fonts, &text, &style, scale, bounds)
            .clone();
        if multiline {
            return Some((layout, [content.x - offset[0], content.y - offset[1]]));
        }
        // One line, laid out as wide as its text, placed across its box as its alignment says,
        // from the end of a text written from right to left, the caret's width kept in the box,
        // and in the middle of its height, as a button's label.
        let room = (content.width - layout.width() - scale.max(1.0)).max(0.0);
        let x = match (style.alignment, layout.is_right_to_left()) {
            (Alignment::Center, _) => room / 2.0,
            (Alignment::End, false) | (Alignment::Start | Alignment::Justify, true) => room,
            _ => 0.0,
        };
        let y = (content.height - layout.height()) / 2.0;
        Some((
            layout,
            [content.x + x - offset[0], content.y + y - offset[1]],
        ))
    }

    /// Where the text of the field `id` starts, in pixels of the surface.
    fn field_origin(&mut self, id: ElementId) -> Option<[f32; 2]> {
        self.field_layout(id).map(|(_, origin)| origin)
    }

    /// Runs `edit` on the field `id`, with the layout of what it holds; then lays out again what
    /// it holds, if `edit` changed it, sets the selection that the edit left, scrolls the text to
    /// keep the caret in view, and marks the field drawn again, and laid out again for a new text.
    /// Gives what `edit` gives; none for an element that is no field.
    fn edit_field<R>(
        &mut self,
        id: ElementId,
        edit: impl FnOnce(&mut Field, &Layout<()>) -> R,
    ) -> Option<R> {
        let (layout, _) = self.field_layout(id)?;
        let field = self.node_mut(id)?.field.as_mut()?;
        if let Some((anchor, focus)) = field.pending.take() {
            field.selection = selection(layout.shown_layout(), anchor, focus);
        }
        let before = field.text.len();
        let result = edit(field, layout.shown_layout());
        let relaid = field.pending.is_some() || field.text.len() != before;
        if relaid {
            self.mark_layout(id);
        }
        self.settle_field(id);
        self.mark_paint(id);
        // Its text or its selection, which whoever reads it without seeing it follows.
        self.mark_meaning(id);
        Some(result)
    }

    /// Sets the selection that an edit of the field `id` left, on the layout of its new text, and
    /// scrolls its text to keep the caret in view, from the field's box as last laid out.
    pub(crate) fn settle_field(&mut self, id: ElementId) {
        let scale = self.scale_of(id);
        let Some((layout, origin)) = self.field_layout(id) else {
            return;
        };
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let multiline = matches!(
            node.element.kind,
            Kind::TextField {
                multiline: true,
                ..
            }
        );
        let content = node.content_box();
        let Some(field) = node.field.as_mut() else {
            return;
        };
        if let Some((anchor, focus)) = field.pending.take() {
            field.selection = selection(layout.shown_layout(), anchor, focus);
        }
        let focus = field.selection.focus();
        let position = TextPosition {
            index: focus.index(),
            affinity: match focus.affinity() {
                ParleyAffinity::Downstream => Affinity::Downstream,
                ParleyAffinity::Upstream => Affinity::Upstream,
            },
        };
        let caret = layout.caret(position);
        // Where the caret would be, the text not scrolled: the caret, as wide as it is drawn, kept
        // within the box; a single line, in the middle of its box, does not scroll up and down.
        let unscrolled = [origin[0] + field.offset[0], origin[1] + field.offset[1]];
        let axes = if multiline { 2 } else { 1 };
        if !multiline {
            field.offset[1] = 0.0;
        }
        for axis in 0..axes {
            let (at, size, extent, room) = match axis {
                0 => (
                    caret.x,
                    caret.width.max(scale.max(1.0)),
                    layout.width(),
                    content.width,
                ),
                _ => (caret.y, caret.height, layout.height(), content.height),
            };
            let offset = &mut field.offset[axis];
            if at - *offset < 0.0 {
                *offset = at;
            } else if at + size - *offset > room {
                *offset = at + size - room;
            }
            *offset = offset.clamp(0.0, (extent + size - room).max(0.0));
        }
        field.caret = Rect::new(
            unscrolled[0] - field.offset[0] + caret.x,
            unscrolled[1] - field.offset[1] + caret.y,
            caret.width,
            caret.height,
        );
    }

    /// Marks `id` drawn again.
    pub(crate) fn mark_paint(&mut self, id: ElementId) {
        if let Some(node) = self.node_mut(id) {
            node.changed.paint = true;
        }
    }

    /// What a field draws, besides its box: the selection's fill, its text, the line under what an
    /// input method composes and, while it holds the focus, its caret, each cut to its content
    /// box.
    pub(crate) fn field_primitives(&mut self, id: ElementId) -> Vec<crate::display::Primitive> {
        use crate::display::{BoxPrimitive, Primitive, TextPrimitive};

        let focused = self.interaction.focus == Some(id);
        let scale = self.scale_of(id);
        let Some((layout, origin)) = self.field_layout(id) else {
            return Vec::new();
        };
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let Some(field) = node.field.as_ref() else {
            return Vec::new();
        };
        let look = &node.style.text;
        let shifted = |rect: Rect| {
            Rect::new(
                rect.x + origin[0],
                rect.y + origin[1],
                rect.width,
                rect.height,
            )
        };
        let filled = |rect: Rect, fill| {
            Primitive::Box(BoxPrimitive {
                rect,
                radii: [0.0; 4],
                fill,
                border_width: 0.0,
                border: fill,
            })
        };
        let mut primitives = Vec::new();
        let selected = field.selection.text_range();
        if focused && !selected.is_empty() {
            for rect in layout.selection(selected) {
                primitives.push(filled(shifted(rect), look.selection));
            }
        }
        primitives.push(Primitive::Text(TextPrimitive {
            origin,
            layout: layout.clone(),
            color: look.color,
        }));
        if let Some(range) = &field.compose {
            for rect in layout.selection(range.clone()) {
                let rect = shifted(rect);
                let line = Rect::new(rect.x, rect.y + rect.height - scale, rect.width, scale);
                primitives.push(filled(line, look.color));
            }
        }
        if focused {
            let caret = field.caret;
            primitives.push(filled(
                Rect::new(caret.x, caret.y, scale.max(1.0), caret.height),
                look.color,
            ));
        }
        primitives
    }
}

/// The most bytes that a character of a [`TextRun`] holds: screen readers count a character's
/// length in a byte.
const CHARACTER_BYTES: usize = 255;

/// The runs of the text laid out in `layout`, its origin at `origin` in pixels of the surface,
/// numbered 0 ([`Ui::field_runs`]): each line's pieces of one direction, in the order of the text,
/// cut every [`RUN_CHARACTERS`] characters, and an empty run for an empty line.
fn text_runs(layout: &TextLayout, origin: [f32; 2]) -> Vec<TextRun> {
    let text = layout.shown();
    let parley = layout.shown_layout();
    let mut runs = Vec::new();
    for line in parley.lines() {
        let metrics = line.metrics();
        let (top, height) = (
            metrics.block_min_coord,
            metrics.block_max_coord - metrics.block_min_coord,
        );
        // Each run's clusters as they lie, left to right from the line's start, as its glyphs are
        // placed; then in the order they are read.
        let mut x = metrics.offset + metrics.inline_min_coord;
        let mut pieces = Vec::new();
        for run in line.runs() {
            let mut clusters = Vec::new();
            for cluster in run.visual_clusters() {
                let advance = cluster.advance();
                // Where the field's moves by words stop.
                let word = cluster.is_word_boundary() && !cluster.is_space_or_nbsp();
                // A cluster longer than a byte counts, which only marks heaped on a letter make, is
                // cut into characters at the boundaries of its code points, the first advancing.
                let range = cluster.text_range();
                let mut start = range.start;
                while start < range.end {
                    let mut end = (start + CHARACTER_BYTES).min(range.end);
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    let first = start == range.start;
                    clusters.push((
                        start..end,
                        x,
                        if first { advance } else { 0.0 },
                        word && first,
                    ));
                    start = end;
                }
                x += advance;
            }
            clusters.sort_by_key(|(range, ..)| range.start);
            if !clusters.is_empty() {
                pieces.push((run.is_rtl(), clusters));
            }
        }
        pieces.sort_by_key(|(_, clusters)| clusters[0].0.start);
        if pieces.is_empty() {
            let x = metrics.offset + metrics.inline_min_coord;
            runs.push(TextRun {
                number: 0,
                text: String::new(),
                start: line.text_range().start,
                right_to_left: parley.is_rtl(),
                bounds: Rect::new(origin[0] + x, origin[1] + top, 0.0, height),
                characters: Vec::new(),
                word_starts: Vec::new(),
            });
            continue;
        }
        for (right_to_left, clusters) in pieces {
            for chunk in clusters.chunks(RUN_CHARACTERS) {
                let left = chunk
                    .iter()
                    .map(|(_, x, ..)| *x)
                    .fold(f32::INFINITY, f32::min);
                let right = chunk
                    .iter()
                    .map(|(_, x, advance, _)| x + advance)
                    .fold(f32::NEG_INFINITY, f32::max);
                runs.push(TextRun {
                    number: 0,
                    text: chunk
                        .iter()
                        .map(|(range, ..)| &text[range.clone()])
                        .collect(),
                    start: chunk[0].0.start,
                    right_to_left,
                    bounds: Rect::new(origin[0] + left, origin[1] + top, right - left, height),
                    characters: chunk
                        .iter()
                        .map(|(range, x, advance, _)| RunCharacter {
                            length: range.len(),
                            position: if right_to_left {
                                right - (x + advance)
                            } else {
                                x - left
                            },
                            width: *advance,
                        })
                        .collect(),
                    word_starts: chunk
                        .iter()
                        .enumerate()
                        .filter(|(_, (.., word))| *word)
                        .map(|(place, _)| place)
                        .collect(),
                });
            }
        }
    }
    runs
}

/// The place of the byte `byte` of a field's text among its runs `runs`: the run whose text holds
/// it and the character that it starts, or falls in; at the text's end, the last run's end.
fn place_in(runs: &[TextRun], byte: usize) -> (usize, usize) {
    for (index, run) in runs.iter().enumerate() {
        let mut at = run.start;
        for (place, character) in run.characters.iter().enumerate() {
            if byte < at + character.length {
                if byte >= run.start {
                    return (index, place);
                }
                break;
            }
            at += character.length;
        }
    }
    let last = runs.len().saturating_sub(1);
    (last, runs.last().map_or(0, |run| run.characters.len()))
}

/// The selection from `anchor` to `focus`, bytes of the text laid out in `layout`.
fn selection(layout: &Layout<()>, anchor: usize, focus: usize) -> Selection {
    let affinity = |index: usize, other: usize| {
        if index > other {
            ParleyAffinity::Upstream
        } else {
            ParleyAffinity::Downstream
        }
    };
    Selection::new(
        Cursor::from_byte_index(layout, anchor, affinity(anchor, focus)),
        Cursor::from_byte_index(layout, focus, affinity(focus, anchor)),
    )
}
