//! The host's questions, answered from the interface's live state, the edits of the same step
//! included, as an input method asks them in the middle of a keystroke
//! (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`, "Questions"). They are about the
//! field that holds the focus, and change nothing that the person sees: a question may lay the
//! field's text out, or settle its scroll as the next frame would, which is why each takes
//! `&mut self`.

use std::borrow::Cow;

use super::interact::inside;
use super::{ElementId, Ui};
use crate::Rect;
use crate::host::{RangeBox, TargetId, TextAround, TextQuery, TextUnit};

impl Ui {
    /// The text around the caret of the field that holds the focus on `target`, as an input method
    /// asks it ([`TextQuery`] says what it holds and how it counts); none without such a field.
    pub fn text_around(&mut self, target: TargetId, query: TextQuery) -> Option<TextAround> {
        let id = self.focused_field(target)?;
        let field = self.node(id)?.field.as_ref()?;
        let (anchor, focus) = field.ends();
        // Without the composition, the text as committed, the caret where the composition began.
        let (text, anchor, focus, composition) = match field.compose.clone() {
            Some(range) if !query.composition => {
                let mut text = field.text.clone();
                text.replace_range(range.clone(), "");
                (Cow::Owned(text), range.start, range.start, None)
            }
            composition => (
                Cow::Borrowed(field.text.as_str()),
                anchor,
                focus,
                composition,
            ),
        };
        let unit = query.unit;
        // The selection and the composition kept whole, the bounds counted from their ends.
        let kept = composition
            .clone()
            .map_or(anchor.min(focus)..anchor.max(focus), |range| {
                range.start.min(anchor).min(focus)..range.end.max(anchor).max(focus)
            });
        let mut begin = kept.start;
        let mut counted = 0;
        for character in text[..kept.start].chars().rev() {
            counted += unit.of(character);
            if counted > query.before {
                break;
            }
            begin -= character.len_utf8();
        }
        let mut end = kept.end;
        let mut counted = 0;
        for character in text[kept.end..].chars() {
            counted += unit.of(character);
            if counted > query.after {
                break;
            }
            end += character.len_utf8();
        }
        // The excerpt's place counted once from the text's start; the rest within the excerpt.
        let within = |byte: usize| unit.count(&text[begin..byte]);
        Some(TextAround {
            start: unit.count(&text[..begin]),
            anchor: within(anchor),
            focus: within(focus),
            composition: composition.map(|range| within(range.start)..within(range.end)),
            text: text[begin..end].to_owned(),
        })
    }

    /// The box of the first line of `range` of the text of the field that holds the focus on
    /// `target`, counted in `unit`, in the target's physical pixels, and where in the range that
    /// line's part ends, as macOS's `firstRect(forCharacterRange:actualRange:)` gives it: a host
    /// that wants every line asks again from there. Places count the field's text with what an input
    /// method composes, as macOS counts it. An empty range gives the caret's box there. A
    /// range that cuts a character is widened to the whole character; one past the text's end
    /// stops there. The field's text is laid out as it is now, scrolled as the next frame will
    /// show it, from its box as last laid out. None without such a field.
    pub fn range_box(
        &mut self,
        target: TargetId,
        range: std::ops::Range<usize>,
        unit: TextUnit,
    ) -> Option<RangeBox> {
        let id = self.focused_field(target)?;
        let text = &self.node(id)?.field.as_ref()?.text;
        let start = unit.byte(text, range.start, false);
        let end = unit.byte(text, range.end, true).max(start);
        let (layout, origin) = self.settled_layout(id)?;
        let (rect, ends) = layout.first_line_box(start..end);
        let text = &self.node(id)?.field.as_ref()?.text;
        let end = if ends == end {
            range.end.max(range.start)
        } else {
            unit.count(&text[..ends])
        };
        Some(RangeBox {
            rect: Rect::new(
                rect.x + origin[0],
                rect.y + origin[1],
                rect.width,
                rect.height,
            ),
            end,
        })
    }

    /// Where the character of the field that holds the focus on `target` drawn at `x` and `y`
    /// starts, in the target's physical pixels and counted in `unit`, within the field's box,
    /// scrolled as the next frame will show it, counted in the field's text with what an input
    /// method composes. None where no character is drawn, as macOS and
    /// Windows answer off every character, or without such a field.
    pub fn character_at(
        &mut self,
        target: TargetId,
        x: f32,
        y: f32,
        unit: TextUnit,
    ) -> Option<usize> {
        let id = self.focused_field(target)?;
        if !inside(self.node(id)?.content_box(), x, y) {
            return None;
        }
        let (layout, origin) = self.settled_layout(id)?;
        let cluster = layout.cluster_at(x - origin[0], y - origin[1])?;
        let text = &self.node(id)?.field.as_ref()?.text;
        Some(unit.count(&text[..cluster.start]))
    }

    /// Whether the point `x` and `y` of `target`, in its physical pixels, is the interface's, by
    /// the rule by which a press there is taken: what it draws there, what takes the pointer
    /// there, a button, a field or a surface, or any point while a modal root of `target` is open;
    /// none of a target that the interface does not know. A host
    /// lets the rest through, a click to what lies beneath a window that lets it, or a drop. It
    /// tells no kind of region: the parts of a window's bar that uwui draws, its caption, edges
    /// and buttons, wait for the decorations of the desktop's host
    /// (`docs/adr/0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md`).
    pub fn covers(&self, target: TargetId, x: f32, y: f32) -> bool {
        self.interaction.knows(target) && self.takes_press_at(target, x, y)
    }

    /// The layout of the field `id` and its origin, its caret placed and its text scrolled first,
    /// as the next frame's paint would: the focus given without an edit, by Tab or the program,
    /// leaves both as the field was last drawn.
    fn settled_layout(
        &mut self,
        id: ElementId,
    ) -> Option<(std::sync::Arc<crate::text::TextLayout>, [f32; 2])> {
        self.settle_field(id);
        self.field_layout(id)
    }
}
