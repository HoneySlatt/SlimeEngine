//! Check boxes, sliders and sections that fold: the value that the user gives, or whether a section
//! is open, kept by the element as a field keeps its text (`docs/adr/0148-widgets.md`).

use slime_color::LinearColor;
use slime_input::{Key, KeyEvent, NamedKey};

use super::{ElementId, ElementState, ItemMark, Kind, LayoutDirection, Ui};
use crate::Rect;
use crate::action::{Action, ActionKind, FieldValue};
use crate::display::{BoxPrimitive, Primitive, TextPrimitive};
use crate::style::Icons;
use crate::text::{TextBounds, TextLayout, TextStyle};

/// How many steps Page Up and Page Down move a slider by.
const PAGE_STEPS: f64 = 10.0;

/// What a check box or a slider keeps between two builds, and what an item of a menu shows of the
/// program's state.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Control {
    /// A check box, a section or an item of a menu: the state that the program last gave; its
    /// state is its element's ([`ElementState::checked`], [`ElementState::expanded`]).
    Toggle {
        /// The state that the program last gave.
        given: bool,
    },
    /// A bar of tabs or a list to choose from: the place of its chosen tab or option, and the one
    /// that the program last gave.
    Choice {
        /// The place of the chosen tab among the bar's tabs, or of the chosen option among the
        /// list's options.
        chosen: usize,
        /// The place that the program last gave.
        given: usize,
    },
    /// Two panels and a handle: the share of its length that the first takes, and the share that
    /// the program last gave.
    Share {
        /// The first panel's share, from 0 to 1.
        share: f32,
        /// The share that the program last gave.
        given: f32,
    },
    /// A slider: its value, and the value that the program last gave.
    Range {
        /// Its value, within its bounds and on its steps.
        value: f64,
        /// The value that the program last gave.
        given: f64,
    },
}

impl Control {
    /// What an element of `kind` keeps as it is made, its state set in `state`; none for an element
    /// that is no check box nor slider.
    pub(crate) fn new(kind: &Kind, state: &mut ElementState) -> Option<Self> {
        match *kind {
            Kind::CheckBox { checked, .. } => {
                state.checked = checked;
                Some(Self::Toggle { given: checked })
            }
            Kind::Section { open, .. } | Kind::FloatingPanel { open, .. } => {
                state.expanded = open;
                Some(Self::Toggle { given: open })
            }
            Kind::ToggleButton { on, .. } => {
                state.checked = on;
                Some(Self::Toggle { given: on })
            }
            // An item of a menu shows the program's state alone: the program turns it.
            Kind::MenuItem { mark, .. } => {
                let checked = mark == ItemMark::Check(true);
                state.checked = checked;
                Some(Self::Toggle { given: checked })
            }
            Kind::TabBar { chosen } | Kind::Segments { chosen } => Some(Self::Choice {
                chosen,
                given: chosen,
            }),
            Kind::Choice {
                ref options,
                chosen,
            } => {
                let chosen = chosen.min(options.len().saturating_sub(1));
                Some(Self::Choice {
                    chosen,
                    given: chosen,
                })
            }
            Kind::Split { share, .. } => Some(Self::Share {
                share: unit(share),
                given: share,
            }),
            Kind::Slider {
                value,
                min,
                max,
                step,
            } => Some(Self::Range {
                value: snapped(value, min, max, step),
                given: value,
            }),
            _ => None,
        }
    }

    /// The program gives `kind` again: its value replaces what the element holds when it differs
    /// from the value that the program gave before, and the user's is kept otherwise. Gives
    /// whether what the element shows changed.
    pub(crate) fn give(&mut self, kind: &Kind, state: &mut ElementState) -> bool {
        match (self, kind) {
            (
                Self::Toggle { given },
                Kind::CheckBox { checked, .. } | Kind::ToggleButton { on: checked, .. },
            ) => {
                if *given == *checked {
                    return false;
                }
                *given = *checked;
                let changed = state.checked != *checked;
                state.checked = *checked;
                changed
            }
            (Self::Toggle { given }, Kind::MenuItem { mark, .. }) => {
                let checked = *mark == ItemMark::Check(true);
                if *given == checked {
                    return false;
                }
                *given = checked;
                state.checked = checked;
                true
            }
            (
                Self::Toggle { given },
                Kind::Section { open, .. } | Kind::FloatingPanel { open, .. },
            ) => {
                if *given == *open {
                    return false;
                }
                *given = *open;
                let changed = state.expanded != *open;
                state.expanded = *open;
                changed
            }
            (Self::Share { share, given }, &Kind::Split { share: program, .. }) => {
                if *given == program {
                    return false;
                }
                *given = program;
                let changed = *share != unit(program);
                *share = unit(program);
                changed
            }
            (
                Self::Choice { chosen, given },
                Kind::Choice {
                    options,
                    chosen: program,
                },
            ) => {
                let last = options.len().saturating_sub(1);
                let program = (*program).min(last);
                // The user's choice stays among the options, which may be fewer now.
                if *given == program {
                    let kept = (*chosen).min(last);
                    let changed = kept != *chosen;
                    *chosen = kept;
                    return changed;
                }
                *given = program;
                let changed = *chosen != program;
                *chosen = program;
                changed
            }
            (
                Self::Choice { chosen, given },
                &Kind::TabBar { chosen: program } | &Kind::Segments { chosen: program },
            ) => {
                if *given == program {
                    return false;
                }
                *given = program;
                let changed = *chosen != program;
                *chosen = program;
                changed
            }
            (
                Self::Range { value, given },
                &Kind::Slider {
                    value: program,
                    min,
                    max,
                    step,
                },
            ) => {
                let kept = snapped(*value, min, max, step);
                let shown = if *given == program {
                    kept
                } else {
                    *given = program;
                    snapped(program, min, max, step)
                };
                let changed = shown != *value;
                *value = shown;
                changed
            }
            _ => false,
        }
    }
}

/// `value` kept within `min` and `max`, on the nearest multiple of `step` from `min`; a step of 0
/// keeps any value. A bound that is no number does not bound it from its side, nor does it give
/// steps; a value that is no number is `min`, or 0 without it. A value on the steps has no more
/// decimals than the step and `min` are written with: a step of `0.1` gives `0.6`, not the
/// `0.6000000000000001` that six steps of it add up to.
pub(crate) fn snapped(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let min = if min.is_nan() { f64::NEG_INFINITY } else { min };
    let max = if max.is_nan() {
        f64::INFINITY
    } else {
        max.max(min)
    };
    let mut value = match (value.is_nan(), min.is_finite()) {
        (false, _) => value,
        (true, true) => min,
        (true, false) => 0.0,
    };
    if step > 0.0 && step.is_finite() && min.is_finite() {
        value = min + ((value - min) / step).round() * step;
        value = super::field::rounded(value, &[min, step]);
    }
    value.clamp(min, max)
}

/// `share` within 0 and 1; 0 for one that is no number.
fn unit(share: f32) -> f32 {
    if share.is_nan() {
        0.0
    } else {
        share.clamp(0.0, 1.0)
    }
}

/// A slider's bounds and step, from its kind.
fn bounds(kind: &Kind) -> Option<(f64, f64, f64)> {
    match *kind {
        Kind::Slider { min, max, step, .. } => Some((min, max.max(min), step)),
        _ => None,
    }
}

impl Ui {
    /// Opens the section or the floating panel `id`, or folds it, as a press on its header does:
    /// what it holds is laid out, drawn and reached again, or no longer. Nothing happens for
    /// another element.
    pub fn set_open(&mut self, id: ElementId, open: bool) {
        let Some(node) = self.node(id) else {
            return;
        };
        if !matches!(
            node.element.kind,
            Kind::Section { .. } | Kind::FloatingPanel { .. }
        ) || node.state.expanded == open
        {
            return;
        }
        let children = node.children.clone();
        self.change_state(id, |state| state.expanded = open);
        self.mark_paint(id);
        self.mark_layout(id);
        // It grows or shrinks to its height over its transition, if its style has one.
        self.reveal(id, open);
        // Its children's styles say whether they are laid out, drawn and reached: shown at once as
        // it opens, hidden once it has folded.
        for child in children {
            self.mark_style(child);
        }
    }

    /// Whether `id` lies in a folded section or floating panel, or past the second child of two
    /// panels: it is not laid out, drawn nor reached.
    pub(crate) fn folded(&self, id: ElementId) -> bool {
        let Some(parent) = self.node(id).and_then(|node| node.parent) else {
            return false;
        };
        let Some(node) = self.node(parent) else {
            return false;
        };
        // The slot of a header stays in it, open or folded.
        let slot = self
            .node(id)
            .is_some_and(|child| matches!(child.element.kind, Kind::SectionHeader));
        match node.element.kind {
            Kind::Section { .. } | Kind::FloatingPanel { .. } if !slot => {
                !node.state.expanded && !self.folding(parent)
            }
            Kind::Split { .. } => node.children.iter().position(|&child| child == id) > Some(1),
            _ => false,
        }
    }

    /// The header of the section or the bar of the floating panel `id`, in pixels of the surface:
    /// the top of its box within its padding, as tall as the layout makes room for it above its
    /// children.
    pub(crate) fn section_header(&self, id: ElementId) -> Option<Rect> {
        let node = self.node(id)?;
        if !matches!(
            node.element.kind,
            Kind::Section { .. } | Kind::FloatingPanel { .. }
        ) {
            return None;
        }
        let scale = self.scale_of(id);
        let padding = node.style.layout.padding;
        let top = padding.top * scale;
        let height = (node.layout.padding.top - top).max(0.0);
        let content = node.content_box();
        Some(Rect::new(
            content.x,
            node.rect.y + top,
            content.width,
            height,
        ))
    }

    /// The place of the chosen tab of the bar of tabs `id` among its tabs, of the chosen segment of
    /// the control of segments `id`, or of the chosen option of the list to choose from `id` among
    /// its options, as the user left it; none for another element.
    pub fn chosen(&self, id: ElementId) -> Option<usize> {
        match self.node(id)?.control? {
            Control::Choice { chosen, .. } => Some(chosen),
            _ => None,
        }
    }

    /// Chooses the tab at `place` of the bar of tabs `bar`, as a press on it does: the builder that
    /// made the bar runs again at the next build, to show its panel. For a control of segments, the
    /// segment at `place` is chosen, which the program reads as actions, changed then committed.
    /// Nothing happens for a place past its tabs, or an element that is neither.
    pub fn choose_tab(&mut self, bar: ElementId, place: usize) {
        let Some(node) = self.node_mut(bar) else {
            return;
        };
        let tabs = node.children.len();
        let owner = node.owner;
        let segments = matches!(node.element.kind, Kind::Segments { .. });
        let Some(Control::Choice { chosen, .. }) = node.control.as_mut() else {
            return;
        };
        if place >= tabs || *chosen == place {
            return;
        }
        *chosen = place;
        self.mark_chosen(bar);
        if segments {
            for kind in [ActionKind::Changed, ActionKind::Committed] {
                self.interaction.actions.push(Action {
                    element: Some(bar),
                    kind: kind(FieldValue::Chosen(place)),
                });
            }
        } else if let Some(owner) = owner.and_then(|owner| self.node_mut(owner)) {
            owner.rebuild = true;
        }
    }

    /// Shows which tab of `bar`, a bar of tabs, is chosen, in its tabs' states.
    pub(crate) fn mark_chosen(&mut self, bar: ElementId) {
        let Some(chosen) = self.chosen(bar) else {
            return;
        };
        let tabs = self.children(bar).to_vec();
        for (place, tab) in tabs.into_iter().enumerate() {
            self.change_state(tab, |state| state.selected = place == chosen);
        }
    }

    /// The bar of the tab `tab`, and the tab's place among its tabs; or the control of the segment
    /// `tab`, and its place.
    fn tab_place(&self, tab: ElementId) -> Option<(ElementId, usize)> {
        let node = self.node(tab)?;
        if !matches!(node.element.kind, Kind::Tab { .. } | Kind::Segment { .. }) {
            return None;
        }
        let bar = node.parent?;
        let place = self.children(bar).iter().position(|&each| each == tab)?;
        Some((bar, place))
    }

    /// Chooses the tab `tab`, as a press on it does.
    pub(crate) fn press_tab(&mut self, tab: ElementId) {
        if let Some((bar, place)) = self.tab_place(tab) {
            self.choose_tab(bar, place);
        }
    }

    /// A key reached the tab `tab`, which holds the focus: the arrows towards a line's end and
    /// start, Home and End choose the tab there, which takes the focus. Gives whether it took the
    /// key.
    pub(crate) fn tab_key(&mut self, tab: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = &event.logical else {
            return false;
        };
        let Some((bar, place)) = self.tab_place(tab) else {
            return false;
        };
        let last = self.children(bar).len().saturating_sub(1);
        let forward = self.root_settings(tab).map(|root| root.direction)
            != Some(LayoutDirection::RightToLeft);
        let next = |place: usize| (place + 1).min(last);
        let previous = |place: usize| place.saturating_sub(1);
        let target = match named {
            NamedKey::ArrowRight if forward => next(place),
            NamedKey::ArrowRight => previous(place),
            NamedKey::ArrowLeft if forward => previous(place),
            NamedKey::ArrowLeft => next(place),
            NamedKey::Home => 0,
            NamedKey::End => last,
            _ => return false,
        };
        self.choose_tab(bar, target);
        if let Some(&chosen) = self.children(bar).get(target) {
            self.set_focus(Some(chosen));
        }
        true
    }

    /// The share of the length of `split`, two panels and a handle, that its first panel takes, as
    /// the user left it; none for an element that is no such split.
    pub fn split_share(&self, split: ElementId) -> Option<f32> {
        match self.node(split)?.control? {
            Control::Share { share, .. } => Some(share),
            _ => None,
        }
    }

    /// Sets the share of the length of `split` that its first panel takes, within 0 and 1: it is
    /// laid out again. Gives whether `split` is two panels and a handle.
    pub fn set_share(&mut self, split: ElementId, share: f32) -> bool {
        let Some(node) = self.node_mut(split) else {
            return false;
        };
        let Some(Control::Share { share: held, .. }) = node.control.as_mut() else {
            return false;
        };
        let share = unit(share);
        if *held != share {
            *held = share;
            let children = node.children.clone();
            self.mark_layout(split);
            self.mark_paint(split);
            self.mark_meaning(split);
            for child in children {
                self.mark_layout(child);
            }
        }
        true
    }

    /// The handle of `split`, between its panels, in pixels of the surface: where the first panel
    /// ends, as thick as the gap that the layout leaves.
    pub(crate) fn split_handle(&self, split: ElementId) -> Option<Rect> {
        let node = self.node(split)?;
        let Kind::Split { side_by_side, .. } = node.element.kind else {
            return None;
        };
        let first = self.node(*node.children.first()?)?.rect;
        let content = node.content_box();
        let thickness = node.style.control.size * self.scale_of(split) / 2.0;
        Some(if side_by_side {
            let rtl = self.root_settings(split).map(|root| root.direction)
                == Some(LayoutDirection::RightToLeft);
            let x = if rtl {
                first.x - thickness
            } else {
                first.x + first.width
            };
            Rect::new(x, content.y, thickness, content.height)
        } else {
            Rect::new(content.x, first.y + first.height, content.width, thickness)
        })
    }

    /// The pointer pressed, or moved while it holds the handle of `split`, at `x` and `y` in pixels
    /// of the surface: the first panel's share ends there, in the handle's middle.
    pub(crate) fn split_drag(&mut self, split: ElementId, x: f32, y: f32) {
        let Some(node) = self.node(split) else {
            return;
        };
        let Kind::Split { side_by_side, .. } = node.element.kind else {
            return;
        };
        let content = node.content_box();
        let half = node.style.control.size * self.scale_of(split) / 4.0;
        let rtl = self.root_settings(split).map(|root| root.direction)
            == Some(LayoutDirection::RightToLeft);
        let share = match (side_by_side, rtl) {
            (true, false) => (x - half - content.x) / content.width,
            (true, true) => (content.x + content.width - x - half) / content.width,
            (false, _) => (y - half - content.y) / content.height,
        };
        if share.is_finite() {
            self.set_share(split, share);
        }
    }

    /// A key reached `split`, which holds the focus: the arrows along it move its handle by a
    /// twentieth of its length, Home and End to its ends. Gives whether it took the key.
    pub(crate) fn split_key(&mut self, split: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = &event.logical else {
            return false;
        };
        let Some(node) = self.node(split) else {
            return false;
        };
        let Kind::Split { side_by_side, .. } = node.element.kind else {
            return false;
        };
        let Some(share) = self.split_share(split) else {
            return false;
        };
        let forward = if self.root_settings(split).map(|root| root.direction)
            == Some(LayoutDirection::RightToLeft)
        {
            -0.05
        } else {
            0.05
        };
        let target = match named {
            NamedKey::ArrowRight if side_by_side => share + forward,
            NamedKey::ArrowLeft if side_by_side => share - forward,
            NamedKey::ArrowDown if !side_by_side => share + 0.05,
            NamedKey::ArrowUp if !side_by_side => share - 0.05,
            NamedKey::Home => 0.0,
            NamedKey::End => 1.0,
            _ => return false,
        };
        self.set_share(split, target);
        true
    }

    /// The value of the slider `id`, as the user left it; none for an element that is no slider.
    pub fn slider_value(&self, id: ElementId) -> Option<f64> {
        match self.node(id)?.control? {
            Control::Range { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Toggles the check box or the button that toggles `id`, as a press does: the program reads
    /// [`ActionKind::Changed`] then [`ActionKind::Committed`], whether it is checked now. A section
    /// opens or folds, which only the interface keeps.
    pub(crate) fn toggle(&mut self, id: ElementId) {
        let Some(node) = self.node(id) else {
            return;
        };
        if matches!(
            node.element.kind,
            Kind::Section { .. } | Kind::FloatingPanel { .. }
        ) {
            let open = !node.state.expanded;
            self.set_open(id, open);
            return;
        }
        if !matches!(
            node.element.kind,
            Kind::CheckBox { .. } | Kind::ToggleButton { .. }
        ) {
            return;
        }
        let checked = !node.state.checked;
        self.change_state(id, |state| state.checked = checked);
        self.mark_paint(id);
        for kind in [ActionKind::Changed, ActionKind::Committed] {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: kind(FieldValue::Checked(checked)),
            });
        }
    }

    /// Sets the slider `id` to `value`, kept within its bounds and on its steps: the program reads
    /// [`ActionKind::Changed`] when the value changed, and [`ActionKind::Committed`] after it for
    /// `commit`. Gives whether `id` is a slider.
    pub(crate) fn set_slider(&mut self, id: ElementId, value: f64, commit: bool) -> bool {
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let Some((min, max, step)) = bounds(&node.element.kind) else {
            return false;
        };
        let Some(Control::Range { value: held, .. }) = node.control.as_mut() else {
            return false;
        };
        let value = snapped(value, min, max, step);
        let changed = *held != value;
        *held = value;
        if changed {
            self.mark_paint(id);
            self.mark_meaning(id);
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Changed(FieldValue::Number(value)),
            });
        }
        if commit {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Committed(FieldValue::Number(value)),
            });
        }
        true
    }

    /// The pointer pressed, or moved while it holds the slider `id`, at `x` in pixels of the
    /// surface: the value under it, the thumb's middle there.
    pub(crate) fn slider_drag(&mut self, id: ElementId, x: f32) {
        let Some((track, _)) = self.slider_geometry(id) else {
            return;
        };
        let Some(node) = self.node(id) else {
            return;
        };
        let Some((min, max, _)) = bounds(&node.element.kind) else {
            return;
        };
        let mut along = if track.width > 0.0 {
            ((x - track.x) / track.width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.root_settings(id).map(|root| root.direction) == Some(LayoutDirection::RightToLeft) {
            along = 1.0 - along;
        }
        self.set_slider(id, min + f64::from(along) * (max - min), false);
    }

    /// A key reached the slider `id`, which holds the focus: the arrows move it by a step, towards
    /// its most value up and towards its end, Page Up and Page Down by ten, Home and End to its
    /// bounds, each committed. Gives whether it took the key; a slider of no step moves by a
    /// hundredth of its range.
    pub(crate) fn slider_key(&mut self, id: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = &event.logical else {
            return false;
        };
        let Some(node) = self.node(id) else {
            return false;
        };
        let Some((min, max, step)) = bounds(&node.element.kind) else {
            return false;
        };
        let Some(value) = self.slider_value(id) else {
            return false;
        };
        let step = if step > 0.0 {
            step
        } else {
            (max - min) / 100.0
        };
        // Towards the end of a line: the right, or the left from right to left.
        let forward = match self.root_settings(id).map(|root| root.direction) {
            Some(LayoutDirection::RightToLeft) => -1.0,
            _ => 1.0,
        };
        let target = match named {
            NamedKey::ArrowRight => value + forward * step,
            NamedKey::ArrowLeft => value - forward * step,
            NamedKey::ArrowUp => value + step,
            NamedKey::ArrowDown => value - step,
            NamedKey::PageUp => value + PAGE_STEPS * step,
            NamedKey::PageDown => value - PAGE_STEPS * step,
            NamedKey::Home => min,
            NamedKey::End => max,
            _ => return false,
        };
        self.set_slider(id, target, true);
        true
    }

    /// The track of the slider `id`, the line that the thumb's middle runs along, from its least
    /// value's end, and the thumb, in pixels of the surface.
    fn slider_geometry(&self, id: ElementId) -> Option<(Rect, Rect)> {
        let scale = self.scale_of(id);
        let node = self.node(id)?;
        let (min, max, _) = bounds(&node.element.kind)?;
        let value = self.slider_value(id)?;
        let content = node.content_box();
        let size = (node.style.control.size * scale).min(content.height.max(1.0));
        let track = Rect::new(
            content.x + size / 2.0,
            content.y + content.height / 2.0,
            (content.width - size).max(0.0),
            0.0,
        );
        // Bounds that are no number, or infinite, place the thumb at the start.
        let mut along = Some(((value - min) / (max - min)) as f32)
            .filter(|along| max > min && along.is_finite())
            .unwrap_or(0.0);
        if self.root_settings(id).map(|root| root.direction) == Some(LayoutDirection::RightToLeft) {
            along = 1.0 - along;
        }
        let middle = track.x + along * track.width;
        let thumb = Rect::new(
            middle - size / 2.0,
            content.y + (content.height - size) / 2.0,
            size,
            size,
        );
        Some((track, thumb))
    }

    /// What a check box or a slider draws, besides its box: a check box's box, its mark and its
    /// label; a slider's track, the part of it up to its value, and its thumb; an item of a menu's
    /// mark, label, shortcut and chevron.
    pub(crate) fn control_primitives(&mut self, id: ElementId) -> Vec<Primitive> {
        let scale = self.scale_of(id);
        let direction = self
            .root_settings(id)
            .map_or(LayoutDirection::LeftToRight, |root| root.direction);
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let control = node.style.control;
        let icons = node.style.icons.clone();
        // The glyph towards a line's end: a chevron for what is folded or opens a submenu.
        let towards_end = match direction {
            LayoutDirection::LeftToRight => icons.right.clone(),
            LayoutDirection::RightToLeft => icons.left.clone(),
        };
        let content = node.content_box();
        let filled = |rect: Rect, radius: f32, fill| {
            Primitive::Box(BoxPrimitive {
                rect,
                radii: [radius * scale; 4],
                fill,
                border_width: 0.0,
                border: fill,
            })
        };
        let mut primitives = Vec::new();
        match &node.element.kind {
            Kind::CheckBox { label, .. } => {
                let checked = node.state.checked;
                let size = control.size * scale;
                let x = match direction {
                    LayoutDirection::LeftToRight => content.x,
                    LayoutDirection::RightToLeft => content.x + content.width - size,
                };
                let boxed = Rect::new(x, content.y + (content.height - size) / 2.0, size, size);
                let fill = if checked { control.fill } else { control.track };
                primitives.push(filled(boxed, control.radius, fill));
                let label = label.clone();
                if checked {
                    let inset = size * 0.3;
                    let mark = Rect::new(
                        boxed.x + inset,
                        boxed.y + inset,
                        size - 2.0 * inset,
                        size - 2.0 * inset,
                    );
                    let tick =
                        self.icon(&icons, &icons.tick, boxed, size * 0.8, scale, control.mark);
                    primitives.push(
                        tick.unwrap_or_else(|| filled(mark, control.radius / 2.0, control.mark)),
                    );
                }
                let Some(node) = self.node(id) else {
                    return primitives;
                };
                let gap = size / 2.0;
                let width = (content.width - size - gap).max(0.0);
                let look = node.style.text.clone();
                let layout: std::sync::Arc<TextLayout> = self
                    .texts
                    .get(
                        &mut self.fonts,
                        &label,
                        &look.style,
                        scale,
                        TextBounds::width(width / scale),
                    )
                    .clone();
                let area = match direction {
                    LayoutDirection::LeftToRight => content.x + size + gap,
                    LayoutDirection::RightToLeft => content.x,
                };
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [
                        beside(direction, &layout, area, width),
                        content.y + (content.height - layout.height()) / 2.0,
                    ],
                    layout,
                    color: look.color,
                }));
            }
            Kind::Section { title, .. } | Kind::FloatingPanel { title, .. } => {
                let panel = matches!(node.element.kind, Kind::FloatingPanel { .. });
                let open = node.state.expanded;
                let title = title.clone();
                let look = node.style.text.clone();
                let Some(header) = self.section_header(id) else {
                    return primitives;
                };
                let size = control.size * scale;
                let x = match direction {
                    LayoutDirection::LeftToRight => header.x,
                    LayoutDirection::RightToLeft => header.x + header.width - size,
                };
                let mark = Rect::new(x, header.y + (header.height - size) / 2.0, size, size);
                // A chevron down, open, towards the end, folded; else, on a box, a minus, open,
                // a plus, folded: two bars of the mark's color.
                let glyph = if open { &icons.down } else { &towards_end };
                if let Some(chevron) = self.icon(&icons, glyph, mark, size, scale, control.mark) {
                    primitives.push(chevron);
                } else {
                    primitives.push(filled(mark, control.radius, control.track));
                    let (long, thin) = (size * 0.6, (size * 0.12).max(scale));
                    let middle = [mark.x + size / 2.0, mark.y + size / 2.0];
                    primitives.push(filled(
                        Rect::new(middle[0] - long / 2.0, middle[1] - thin / 2.0, long, thin),
                        0.0,
                        control.mark,
                    ));
                    if !open {
                        primitives.push(filled(
                            Rect::new(middle[0] - thin / 2.0, middle[1] - long / 2.0, thin, long),
                            0.0,
                            control.mark,
                        ));
                    }
                }
                let gap = size / 2.0;
                // The title leaves the room that the header's slot takes, and a gap before it.
                let slot = self
                    .children(id)
                    .iter()
                    .filter_map(|&child| self.node(child))
                    .find(|child| matches!(child.element.kind, Kind::SectionHeader))
                    .map_or(0.0, |child| child.rect.width + gap);
                let width = (header.width - size - gap - slot).max(0.0);
                let layout: std::sync::Arc<TextLayout> = self
                    .texts
                    .get(
                        &mut self.fonts,
                        &title,
                        &look.style,
                        scale,
                        TextBounds::width(width / scale),
                    )
                    .clone();
                let area = match direction {
                    LayoutDirection::LeftToRight => header.x + size + gap,
                    LayoutDirection::RightToLeft => header.x,
                };
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [
                        beside(direction, &layout, area, width),
                        header.y + (header.height - layout.height()) / 2.0,
                    ],
                    layout,
                    color: look.color,
                }));
                // A floating panel's corner that sizes it: two bars along its edges.
                if panel
                    && open
                    && let Some(corner) = self.panel_corner(id)
                {
                    let (long, thin) = (corner.width * 0.6, (corner.width * 0.12).max(scale));
                    let (right, bottom) = (corner.x + corner.width, corner.y + corner.height);
                    let edge = match direction {
                        LayoutDirection::LeftToRight => right - thin,
                        LayoutDirection::RightToLeft => corner.x,
                    };
                    let along = match direction {
                        LayoutDirection::LeftToRight => right - long,
                        LayoutDirection::RightToLeft => corner.x,
                    };
                    primitives.push(filled(
                        Rect::new(edge, bottom - long, thin, long),
                        0.0,
                        control.track,
                    ));
                    primitives.push(filled(
                        Rect::new(along, bottom - thin, long, thin),
                        0.0,
                        control.track,
                    ));
                }
            }
            Kind::MenuItem {
                label,
                shortcut,
                mark,
            } => {
                let (label, shortcut, mark) = (label.clone(), shortcut.clone(), *mark);
                let look = node.style.text.clone();
                // In a bar of menus, its label alone, in the middle of its box.
                if self.in_menu_bar(id) {
                    let layout: std::sync::Arc<TextLayout> = self
                        .texts
                        .get(
                            &mut self.fonts,
                            &label,
                            &look.style,
                            scale,
                            TextBounds::default(),
                        )
                        .clone();
                    primitives.push(Primitive::Text(TextPrimitive {
                        origin: [
                            content.x + (content.width - layout.width()) / 2.0,
                            content.y + (content.height - layout.height()) / 2.0,
                        ],
                        layout,
                        color: look.color,
                    }));
                    return primitives;
                }
                let size = control.size * scale;
                let gap = size / 2.0;
                let middle = content.y + content.height / 2.0;
                // The mark at the start of a line, the chevron at its end.
                let (start, end) = match direction {
                    LayoutDirection::LeftToRight => (content.x, content.x + content.width - size),
                    LayoutDirection::RightToLeft => (content.x + content.width - size, content.x),
                };
                // A tick, or a dot, where it is checked; a chevron, or a bar, where it opens a
                // submenu.
                if mark == ItemMark::Check(true) {
                    let place = Rect::new(start, middle - size / 2.0, size, size);
                    let tick = self.icon(&icons, &icons.tick, place, size, scale, control.mark);
                    let dot = size * 0.4;
                    let at = Rect::new(start + (size - dot) / 2.0, middle - dot / 2.0, dot, dot);
                    primitives.push(tick.unwrap_or_else(|| filled(at, control.size, control.mark)));
                }
                if mark == ItemMark::Submenu {
                    let place = Rect::new(end, middle - size / 2.0, size, size);
                    let chevron = self.icon(&icons, &towards_end, place, size, scale, control.mark);
                    let (thin, long) = ((size * 0.15).max(scale), size * 0.5);
                    let at = Rect::new(end + (size - thin) / 2.0, middle - long / 2.0, thin, long);
                    primitives.push(chevron.unwrap_or_else(|| filled(at, 0.0, control.mark)));
                }
                let shortcut = (!shortcut.is_empty()).then(|| {
                    self.texts
                        .get(
                            &mut self.fonts,
                            &shortcut,
                            &look.style,
                            scale,
                            TextBounds::default(),
                        )
                        .clone()
                });
                let room = shortcut
                    .as_ref()
                    .map_or(0.0, |layout| layout.width() + 2.0 * size);
                let width = (content.width - 2.0 * (size + gap) - room).max(0.0);
                let layout: std::sync::Arc<TextLayout> = self
                    .texts
                    .get(
                        &mut self.fonts,
                        &label,
                        &look.style,
                        scale,
                        TextBounds::default(),
                    )
                    .clone();
                let area = match direction {
                    LayoutDirection::LeftToRight => content.x + size + gap,
                    LayoutDirection::RightToLeft => content.x + content.width - size - gap - width,
                };
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [
                        beside(direction, &layout, area, width),
                        middle - layout.height() / 2.0,
                    ],
                    layout,
                    color: look.color,
                }));
                // The shortcut against the chevron's gap, at the end of the line.
                if let Some(layout) = shortcut {
                    let x = match direction {
                        LayoutDirection::LeftToRight => end - gap - layout.width(),
                        LayoutDirection::RightToLeft => end + size + gap,
                    };
                    primitives.push(Primitive::Text(TextPrimitive {
                        origin: [x, middle - layout.height() / 2.0],
                        layout,
                        color: control.track,
                    }));
                }
            }
            Kind::Choice { options, .. } => {
                let shown = self
                    .chosen(id)
                    .and_then(|chosen| options.get(chosen))
                    .cloned()
                    .unwrap_or_default();
                let look = node.style.text.clone();
                let size = control.size * scale;
                let gap = size / 2.0;
                let middle = content.y + content.height / 2.0;
                // The mark at the end of a line: a chevron down, or a bar across, for a list that
                // opens below.
                let end = match direction {
                    LayoutDirection::LeftToRight => content.x + content.width - size,
                    LayoutDirection::RightToLeft => content.x,
                };
                let place = Rect::new(end, middle - size / 2.0, size, size);
                let chevron = self.icon(&icons, &icons.down, place, size, scale, control.mark);
                let (long, thin) = (size * 0.5, (size * 0.15).max(scale));
                let at = Rect::new(end + (size - long) / 2.0, middle - thin / 2.0, long, thin);
                primitives.push(chevron.unwrap_or_else(|| filled(at, 0.0, control.mark)));
                let width = (content.width - size - gap).max(0.0);
                let layout: std::sync::Arc<TextLayout> = self
                    .texts
                    .get(
                        &mut self.fonts,
                        &shown,
                        &look.style,
                        scale,
                        TextBounds::default(),
                    )
                    .clone();
                let area = match direction {
                    LayoutDirection::LeftToRight => content.x,
                    LayoutDirection::RightToLeft => content.x + size + gap,
                };
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [
                        beside(direction, &layout, area, width),
                        middle - layout.height() / 2.0,
                    ],
                    layout,
                    color: look.color,
                }));
            }
            Kind::Tab {
                title,
                count: Some(count),
            } => {
                // Its title and its count in a box after it, together in the middle of its box.
                let (title, count) = (title.clone(), count.to_string());
                let look = node.style.text.clone();
                let size = control.size * scale;
                let layout = |ui: &mut Ui, text: &str| -> std::sync::Arc<TextLayout> {
                    ui.texts
                        .get(
                            &mut ui.fonts,
                            text,
                            &look.style,
                            scale,
                            TextBounds::default(),
                        )
                        .clone()
                };
                let (title, count) = (layout(self, &title), layout(self, &count));
                let gap = size / 2.0;
                let badge = Rect::new(0.0, 0.0, count.width() + size / 2.0, count.height());
                let width = title.width() + gap + badge.width;
                let start = content.x + (content.width - width) / 2.0;
                let middle = content.y + content.height / 2.0;
                let (title_x, badge_x) = match direction {
                    LayoutDirection::LeftToRight => (start, start + title.width() + gap),
                    LayoutDirection::RightToLeft => (start + badge.width + gap, start),
                };
                let badge = Rect::new(
                    badge_x,
                    middle - badge.height / 2.0,
                    badge.width,
                    badge.height,
                );
                primitives.push(filled(badge, badge.height / 2.0 / scale, control.track));
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [badge.x + size / 4.0, badge.y],
                    layout: count,
                    color: control.mark,
                }));
                primitives.push(Primitive::Text(TextPrimitive {
                    origin: [title_x, middle - title.height() / 2.0],
                    layout: title,
                    color: look.color,
                }));
            }
            Kind::Progress { share } => {
                let share = *share;
                let thickness = content.height.min(control.size * scale / 2.0).max(scale);
                let track = Rect::new(
                    content.x,
                    content.y + (content.height - thickness) / 2.0,
                    content.width,
                    thickness,
                );
                primitives.push(filled(track, control.radius, control.track));
                // Its share from a line's start; without one, a third of the track that crosses it
                // every second and a half of the clock, drawn again each frame while it shows, or
                // still in its middle for less motion.
                let (from, length) = match share {
                    Some(share) => (0.0, share.clamp(0.0, 1.0)),
                    None if self.reduced_motion || !self.shown(id) => (0.35, 0.3),
                    None => {
                        let turn = (self.now.as_secs_f32() / 1.5).fract();
                        self.spinning.push(id);
                        (turn * 1.3 - 0.3, 0.3)
                    }
                };
                let (from, to) = (from.max(0.0), (from + length).min(1.0));
                if to > from {
                    let x = match direction {
                        LayoutDirection::LeftToRight => track.x + from * track.width,
                        LayoutDirection::RightToLeft => track.x + (1.0 - to) * track.width,
                    };
                    let part = Rect::new(x, track.y, (to - from) * track.width, thickness);
                    primitives.push(filled(part, control.radius, control.fill));
                }
            }
            Kind::Split { .. } => {
                if let Some(handle) = self.split_handle(id) {
                    let fill = if node.state.pressed || node.state.focused {
                        control.fill
                    } else {
                        control.track
                    };
                    primitives.push(filled(handle, 0.0, fill));
                }
            }
            Kind::Slider { .. } => {
                let Some((track, thumb)) = self.slider_geometry(id) else {
                    return primitives;
                };
                let thickness = (control.size * scale / 4.0).max(scale);
                let line = Rect::new(track.x, track.y - thickness / 2.0, track.width, thickness);
                primitives.push(filled(line, control.radius, control.track));
                let middle = thumb.x + thumb.width / 2.0;
                let up_to = match direction {
                    LayoutDirection::LeftToRight => {
                        Rect::new(line.x, line.y, middle - line.x, thickness)
                    }
                    LayoutDirection::RightToLeft => {
                        Rect::new(middle, line.y, line.x + line.width - middle, thickness)
                    }
                };
                primitives.push(filled(up_to, control.radius, control.fill));
                primitives.push(filled(thumb, control.radius, control.mark));
            }
            _ => {}
        }
        primitives
    }
}

impl Ui {
    /// Whether `id` shows: neither it nor an ancestor has a display of none.
    fn shown(&self, id: ElementId) -> bool {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.node(id)) {
            if node.style.layout.display == crate::style::Display::None {
                return false;
            }
            at = node.parent;
        }
        true
    }

    /// The glyph `glyph` of the font of `icons`, `size` pixels tall, in the middle of `rect`, of
    /// `color`; none where `icons` names no font, or `glyph` is empty.
    pub(super) fn icon(
        &mut self,
        icons: &Icons,
        glyph: &str,
        rect: Rect,
        size: f32,
        scale: f32,
        color: LinearColor,
    ) -> Option<Primitive> {
        if icons.font.is_empty() || glyph.is_empty() {
            return None;
        }
        let style = TextStyle {
            fonts: vec![icons.font.clone()],
            size: size / scale,
            ..TextStyle::default()
        };
        let layout = self
            .texts
            .get(&mut self.fonts, glyph, &style, scale, TextBounds::default())
            .clone();
        let drawn = layout.drawn_box();
        Some(Primitive::Text(TextPrimitive {
            origin: [
                rect.x + (rect.width - drawn.width) / 2.0 - drawn.x,
                rect.y + (rect.height - layout.height()) / 2.0,
            ],
            layout,
            color,
        }))
    }
}

/// The x of the origin of `layout`, a check box's label or a section's title, laid out across the
/// area of `width` pixels from `area`, which places its glyphs against the area's side that faces
/// the box or the mark: its left in a root laid out from left to right, its right from right to
/// left, whatever the direction of the text, which aligns it to its own start.
fn beside(direction: LayoutDirection, layout: &TextLayout, area: f32, width: f32) -> f32 {
    let drawn = layout.drawn_box();
    match direction {
        LayoutDirection::LeftToRight => area - drawn.x,
        LayoutDirection::RightToLeft => area + width - drawn.x - drawn.width,
    }
}
