//! A person's input and an agent's actions on the tree: the element under the pointer, the pointer
//! held by a pressed button, the focus and its moves, the shortcuts, and the queue of actions that
//! the program reads (`docs/adr/0142-input-and-focus.md`).

use slime_input::{ButtonState, Key, KeyEvent, Modifiers, MouseButton, NamedKey, ScrollDelta};

use super::{ElementId, ElementState, Kind, LayoutDirection, Ui};
use crate::DocumentError;
use crate::action::{Action, ActionKind, Intent};
use crate::shortcuts::{self, Binding, Shortcuts};
use crate::style::Display;

/// How far a line of the wheel scrolls, in logical pixels.
const WHEEL_LINE: f32 = 40.0;

/// What the interface keeps of a person's input between two events.
#[derive(Default)]
pub(super) struct Interaction {
    /// Where the pointer is, in pixels of the screen; none once it left.
    pointer: Option<[f32; 2]>,
    /// The button under the pointer.
    hovered: Option<ElementId>,
    /// The button that the pointer pressed, which holds it until it is let go.
    captured: Option<ElementId>,
    /// The element that holds the focus.
    focus: Option<ElementId>,
    /// The modal roots open, each with the focus that it took, given back when it closes.
    modals: Vec<(ElementId, Option<ElementId>)>,
    modifiers: Modifiers,
    /// The actions that the program has not taken yet.
    actions: Vec<Action>,
    /// The shortcuts, in the order of their tables.
    bindings: Vec<Binding>,
    /// Whether what lies under the pointer may have changed since it was found: the tree was laid
    /// out, scrolled or cut since.
    moved: bool,
}

impl Ui {
    /// The pointer moved to `x` and `y`, in pixels of the screen from the surface's top left
    /// corner: the button under it is hovered, the one before is no longer.
    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        self.interaction.pointer = Some([x, y]);
        self.hover();
    }

    /// The pointer left the surface: nothing is hovered any more.
    pub fn pointer_left(&mut self) {
        self.interaction.pointer = None;
        self.hover();
    }

    /// A button of the mouse went down or up. The primary button pressed over a button presses
    /// it, which then holds the pointer: let go over it, it gives an [`ActionKind::Press`]; let go
    /// elsewhere, nothing. A button that can hold the focus takes it.
    pub fn pointer_button(&mut self, button: MouseButton, state: ButtonState) {
        if button != MouseButton::Left {
            return;
        }
        match state {
            ButtonState::Pressed => {
                // A press without the release of the one before lets go of what that one held.
                if let Some(before) = self.interaction.captured.take() {
                    self.change_state(before, |state| state.pressed = false);
                }
                let Some(target) = self.pointer_target(|kind| matches!(kind, Kind::Button { .. }))
                else {
                    return;
                };
                self.interaction.captured = Some(target);
                self.change_state(target, |state| state.pressed = true);
                self.set_focus(Some(target));
            }
            ButtonState::Released => {
                let Some(captured) = self.interaction.captured.take() else {
                    return;
                };
                self.change_state(captured, |state| state.pressed = false);
                let over = self.pointer_target(|kind| matches!(kind, Kind::Button { .. }));
                if over == Some(captured) {
                    self.act(Action {
                        element: Some(captured),
                        kind: ActionKind::Press,
                    });
                }
            }
        }
    }

    /// The wheel scrolled: the box that scrolls or the list under the pointer scrolls, a line of
    /// the wheel some 40 logical pixels.
    pub fn wheel(&mut self, delta: ScrollDelta) {
        let Some(target) =
            self.pointer_target(|kind| matches!(kind, Kind::Scroll | Kind::List { .. }))
        else {
            return;
        };
        let scale = self.scale_of(target);
        // The wheel turned up scrolls the content down: towards its start.
        let [x, y] = match delta {
            ScrollDelta::Lines { x, y } => [x * WHEEL_LINE, y * WHEEL_LINE],
            ScrollDelta::Pixels { x, y } => [x as f32 / scale, y as f32 / scale],
        };
        // A wheel that moves the content right shows what lies left of it: its start from left to
        // right, its end from right to left, where the offset, counted from the start, grows.
        let x = match self.root_settings(target).map(|root| root.direction) {
            Some(LayoutDirection::RightToLeft) => -x,
            _ => x,
        };
        let [at_x, at_y] = self.scroll(target).unwrap_or_default();
        self.scroll_to(target, [at_x - x, at_y - y]);
    }

    /// The modifier keys held changed: shortcuts are read with them.
    pub fn modifiers(&mut self, modifiers: Modifiers) {
        self.interaction.modifiers = modifiers;
    }

    /// A key went down or up. A press, or a repeat, gives first the command of a shortcut whose
    /// chord it is, in the scopes of the focus, innermost first, then `global`
    /// ([`Ui::set_shortcuts`]); else the intent that its key means: Tab and Shift+Tab next and
    /// previous, the arrows a direction, Enter and Space accept, Escape cancel.
    pub fn key(&mut self, event: &KeyEvent) {
        if event.state != ButtonState::Pressed {
            return;
        }
        if let Some(command) = self.shortcut(event) {
            let element = self.interaction.focus;
            self.act(Action {
                element,
                kind: ActionKind::Command(command),
            });
            return;
        }
        let Key::Named(named) = event.logical else {
            return;
        };
        let intent = match named {
            NamedKey::Tab if self.interaction.modifiers.shift => Intent::Previous,
            NamedKey::Tab => Intent::Next,
            NamedKey::ArrowUp => Intent::Up,
            NamedKey::ArrowDown => Intent::Down,
            NamedKey::ArrowLeft => Intent::Left,
            NamedKey::ArrowRight => Intent::Right,
            NamedKey::Enter | NamedKey::Space => Intent::Accept,
            NamedKey::Escape => Intent::Cancel,
            _ => return,
        };
        let element = self.interaction.focus;
        self.act(Action {
            element,
            kind: ActionKind::Intent(intent),
        });
    }

    /// Does `action`, as a person's input does, for an agent or the program: a press of a button
    /// that a person could press, not disabled, drawn, and no modal root above its own, reaches the
    /// queue; the intents of navigation move the focus; `Accept` presses the button that holds the
    /// focus, and reaches the queue as an intent where none can be pressed; `Cancel` and commands
    /// reach the queue. A press of what is not such a button, or of what names nothing, is left
    /// out.
    pub fn act(&mut self, action: Action) {
        match action.kind {
            ActionKind::Press => {
                if action
                    .element
                    .is_some_and(|id| self.focusable(id) && self.in_scope(id))
                {
                    self.interaction.actions.push(action);
                }
            }
            ActionKind::Intent(Intent::Accept) => {
                let focus = self.interaction.focus;
                let kind = match focus {
                    Some(id) if self.focusable(id) => ActionKind::Press,
                    _ => ActionKind::Intent(Intent::Accept),
                };
                self.interaction.actions.push(Action {
                    element: focus,
                    kind,
                });
            }
            ActionKind::Intent(Intent::Cancel) | ActionKind::Command(_) => {
                self.interaction.actions.push(action);
            }
            ActionKind::Intent(intent) => self.move_focus(intent),
        }
    }

    /// The actions that the program has not taken yet, in the order they came, taken.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.interaction.actions)
    }

    /// The element that holds the focus.
    pub fn focus(&self) -> Option<ElementId> {
        self.interaction.focus
    }

    /// Gives the focus to `id`, a button that is not disabled and is drawn, or takes it from what
    /// holds it, for `None`. Nothing happens for an element that cannot hold it, or that a modal
    /// root above its own keeps it from.
    pub fn set_focus(&mut self, id: Option<ElementId>) {
        if let Some(id) = id
            && (!self.focusable(id) || !self.in_scope(id))
        {
            return;
        }
        if let Some(before) = self.interaction.focus.take() {
            self.change_state(before, |state| state.focused = false);
        }
        if let Some(id) = id {
            self.interaction.focus = Some(id);
            self.change_state(id, |state| state.focused = true);
        }
    }

    /// Reads the shortcuts of `tables`, in their order, those of a later table winning over an
    /// earlier one's for one chord in one scope.
    ///
    /// # Errors
    ///
    /// [`DocumentError`] if a table is not a document of scopes, of chords and the names of
    /// commands, or a chord names no key or modifier: the interface keeps the shortcuts it had.
    pub fn set_shortcuts(&mut self, tables: &[Shortcuts]) -> Result<(), DocumentError> {
        self.interaction.bindings = shortcuts::bindings(tables)?;
        Ok(())
    }

    /// The element that the pointer would reach at `x` and `y`, in pixels of the screen: the
    /// deepest drawn there, of the top root that holds the point, its clip holding it too; none
    /// below a modal root.
    pub fn element_at(&self, x: f32, y: f32) -> Option<ElementId> {
        for &root in self.roots.iter().rev() {
            if let Some(hit) = self.hit(root, x, y) {
                return Some(hit);
            }
            if self
                .node(root)
                .and_then(|node| node.root)
                .is_some_and(|root| root.modal)
            {
                return None;
            }
        }
        None
    }

    /// The element under the pointer, or its nearest ancestor, of a kind that `takes`.
    fn pointer_target(&self, takes: impl Fn(&Kind) -> bool) -> Option<ElementId> {
        let [x, y] = self.interaction.pointer?;
        let mut at = self.element_at(x, y);
        while let Some(id) = at {
            let node = self.node(id)?;
            if takes(&node.element.kind) && !node.state.disabled {
                return Some(id);
            }
            at = node.parent;
        }
        None
    }

    /// The deepest element below `id`, itself included, drawn at `x` and `y`.
    fn hit(&self, id: ElementId, x: f32, y: f32) -> Option<ElementId> {
        let node = self.node(id)?;
        if node.style.layout.display == Display::None {
            return None;
        }
        // Children drawn later lie on top.
        for &child in node.children.iter().rev() {
            if let Some(hit) = self.hit(child, x, y) {
                return Some(hit);
            }
        }
        let inside = |rect: crate::Rect| {
            x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
        };
        (inside(node.rect) && node.clip.is_none_or(inside)).then_some(id)
    }

    /// Hovers the button under the pointer, the one that holds the pointer alone while it does.
    fn hover(&mut self) {
        let over = self.pointer_target(|kind| matches!(kind, Kind::Button { .. }));
        let hovered = match self.interaction.captured {
            Some(captured) => (over == Some(captured)).then_some(captured),
            None => over,
        };
        if hovered != self.interaction.hovered {
            if let Some(before) = self.interaction.hovered {
                self.change_state(before, |state| state.hovered = false);
            }
            if let Some(now) = hovered {
                self.change_state(now, |state| state.hovered = true);
            }
            self.interaction.hovered = hovered;
        }
    }

    /// The command of the shortcut whose chord `event` is, with the modifiers held.
    fn shortcut(&self, event: &KeyEvent) -> Option<String> {
        let modifiers = self.interaction.modifiers;
        let mut scopes: Vec<&str> = Vec::new();
        let mut at = self.interaction.focus;
        while let Some(id) = at {
            let node = self.node(id)?;
            scopes.extend(node.element.classes.iter().map(String::as_str));
            at = node.parent;
        }
        scopes.push("global");
        scopes.iter().find_map(|scope| {
            self.interaction
                .bindings
                .iter()
                .rev()
                .find(|binding| binding.scope == *scope && binding.chord.matches(event, modifiers))
                .map(|binding| binding.command.clone())
        })
    }

    /// Moves the focus as `intent` says, among the elements of its root, or of the top modal root.
    /// Without a focus, among those of the top root that has any: Tab gives it to the first,
    /// Shift+Tab to the last.
    fn move_focus(&mut self, intent: Intent) {
        let focus = self.interaction.focus;
        let scopes: Vec<ElementId> = match self.interaction.modals.last() {
            Some(&(modal, _)) => vec![modal],
            None => match focus.and_then(|focus| self.root_of(focus)) {
                Some(root) => vec![root],
                None => self.roots.iter().rev().copied().collect(),
            },
        };
        let Some(candidates) = scopes
            .into_iter()
            .map(|root| self.focusables(root))
            .find(|found| !found.is_empty())
        else {
            return;
        };
        let Some(at) = focus.and_then(|focus| candidates.iter().position(|&id| id == focus)) else {
            let first = match intent {
                Intent::Previous => candidates[candidates.len() - 1],
                _ => candidates[0],
            };
            self.set_focus(Some(first));
            return;
        };
        let next = match intent {
            Intent::Next => Some(candidates[(at + 1) % candidates.len()]),
            Intent::Previous => Some(candidates[(at + candidates.len() - 1) % candidates.len()]),
            Intent::Up | Intent::Down | Intent::Left | Intent::Right => {
                self.nearest(candidates[at], &candidates, intent)
            }
            Intent::Accept | Intent::Cancel => None,
        };
        if let Some(next) = next {
            self.set_focus(Some(next));
        }
    }

    /// Whether `id` lies in a root that no modal root above keeps the focus from.
    fn in_scope(&self, id: ElementId) -> bool {
        match self.interaction.modals.last() {
            Some(&(modal, _)) => self.root_of(id) == Some(modal),
            None => true,
        }
    }

    /// Whether `id` can hold the focus: a button, not disabled, drawn, as its ancestors are.
    fn focusable(&self, id: ElementId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        if !matches!(node.element.kind, Kind::Button { .. }) || node.state.disabled {
            return false;
        }
        let mut at = Some(node);
        while let Some(node) = at {
            if node.style.layout.display == Display::None {
                return false;
            }
            at = node.parent.and_then(|parent| self.node(parent));
        }
        true
    }

    /// The elements below `root` that can hold the focus, in the order of the tree.
    fn focusables(&self, root: ElementId) -> Vec<ElementId> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else {
                continue;
            };
            if node.style.layout.display == Display::None {
                continue;
            }
            if matches!(node.element.kind, Kind::Button { .. }) && !node.state.disabled {
                found.push(id);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        found
    }

    /// The candidate nearest to `from` on screen in the direction of `intent`: of those whose
    /// middle lies that way, the one nearest along that way, a step aside counting twice.
    fn nearest(
        &self,
        from: ElementId,
        candidates: &[ElementId],
        intent: Intent,
    ) -> Option<ElementId> {
        let middle = |id: ElementId| {
            self.node(id).map(|node| {
                let rect = node.rect;
                [rect.x + rect.width / 2.0, rect.y + rect.height / 2.0]
            })
        };
        let [x, y] = middle(from)?;
        candidates
            .iter()
            .filter(|&&id| id != from)
            .filter_map(|&id| {
                let [cx, cy] = middle(id)?;
                let (along, aside) = match intent {
                    Intent::Up => (y - cy, cx - x),
                    Intent::Down => (cy - y, cx - x),
                    Intent::Left => (x - cx, cy - y),
                    _ => (cx - x, cy - y),
                };
                (along > 0.5).then_some((id, along + 2.0 * aside.abs()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Changes the state of `id` by `change`, its style made again if it changed. Unlike
    /// [`Ui::set_state`], it sets the states that the interface keeps, the focus's among them.
    fn change_state(&mut self, id: ElementId, change: impl FnOnce(&mut ElementState)) {
        if let Some(node) = self.node_mut(id) {
            let before = node.state;
            change(&mut node.state);
            if node.state != before {
                self.mark_style(id);
            }
        }
    }

    /// Takes the focus from the root below when `root`, a modal one, opens.
    pub(super) fn opened_modal(&mut self, root: ElementId) {
        let before = self.interaction.focus;
        self.set_focus(None);
        self.interaction.modals.push((root, before));
    }

    /// Forgets what `removed`, an element taken out of the tree with what it held, leaves behind:
    /// the hover, the pointer held, the focus; a modal root closed gives back the focus it took.
    pub(super) fn forget(&mut self, removed: ElementId) {
        self.interaction.moved = true;
        let alive = |slots: &[super::Slot], id: ElementId| {
            slots
                .get(id.index as usize)
                .is_some_and(|slot| slot.generation == id.generation && slot.node.is_some())
        };
        let interaction = &mut self.interaction;
        for id in [
            &mut interaction.hovered,
            &mut interaction.captured,
            &mut interaction.focus,
        ] {
            if id.is_some_and(|id| !alive(&self.slots, id)) {
                *id = None;
            }
        }
        let Some(at) = interaction
            .modals
            .iter()
            .position(|&(root, _)| root == removed)
        else {
            return;
        };
        let (_, before) = interaction.modals.remove(at);
        match interaction.modals.get_mut(at) {
            // Closed below another, it leaves the focus that it took to the one above, when what
            // that one took went with it.
            Some((_, above)) => {
                if above.is_none_or(|id| !alive(&self.slots, id)) {
                    *above = before;
                }
            }
            None => {
                if interaction.focus.is_none() {
                    self.set_focus(before);
                }
            }
        }
    }

    /// Keeps the input's states true to the tree, as a frame begins and its styles are made: a
    /// focus that its element can no longer hold is dropped, the top modal root takes the focus
    /// when nothing of its own holds it, and the hover follows what moved under the pointer.
    pub(super) fn settle_input(&mut self) {
        if let Some(focus) = self.interaction.focus
            && !self.focusable(focus)
        {
            self.set_focus(None);
        }
        if let Some(&(modal, _)) = self.interaction.modals.last() {
            let inside = self
                .interaction
                .focus
                .is_some_and(|focus| self.root_of(focus) == Some(modal));
            if !inside && let Some(&first) = self.focusables(modal).first() {
                self.set_focus(Some(first));
            }
        }
        if std::mem::take(&mut self.interaction.moved) {
            self.hover();
        }
    }

    /// Marks what lies under the pointer to be found again as the next frame begins: the tree
    /// was laid out or scrolled.
    pub(super) fn moved(&mut self) {
        self.interaction.moved = true;
    }
}
