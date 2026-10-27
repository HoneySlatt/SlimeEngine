//! A person's input and an agent's actions on the tree: the element under the pointer, the pointer
//! held by a pressed button, the focus and its moves, the shortcuts, and the queue of actions that
//! the program reads (`docs/adr/0142-input-and-focus.md`).

use slime_input::{ButtonState, Key, KeyEvent, Modifiers, MouseButton, NamedKey, ScrollDelta};

use super::{ElementId, ElementState, Kind, LayoutDirection, Node, Ui};
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
    pub(super) pointer: Option<[f32; 2]>,
    /// The button or the field under the pointer.
    hovered: Option<ElementId>,
    /// The button or the field that the pointer pressed, which holds it until it is let go.
    pub(super) captured: Option<ElementId>,
    /// The mouse's buttons whose press the interface took, whose release it takes too.
    held: Vec<MouseButton>,
    /// The element that holds the focus.
    pub(super) focus: Option<ElementId>,
    /// The modal roots open, each with the focus that it took, given back when it closes.
    modals: Vec<(ElementId, Option<ElementId>)>,
    pub(super) modifiers: Modifiers,
    /// The actions that the program has not taken yet.
    pub(super) actions: Vec<Action>,
    /// The shortcuts, in the order of their tables.
    bindings: Vec<Binding>,
    /// Whether what lies under the pointer may have changed since it was found: the tree was laid
    /// out, scrolled or cut since.
    moved: bool,
    /// The thumb of a scroll bar that the pointer holds.
    pub(super) bar: Option<super::scroll_bar::Held>,
    /// Whether the last input was a press of the pointer, whose focus does not show, rather than a
    /// key, an agent's action or the program's.
    pub(super) by_pointer: bool,
    /// Where the pointer rests, and since when.
    pub(super) rest: super::tooltip::Rest,
    /// The layer of the tooltip that shows, if one does.
    pub(super) tooltip: Option<ElementId>,
    /// What held the focus before the keys gave it to a bar of menus.
    pub(super) bar_before: Option<ElementId>,
    /// Whether Alt went down, and no other key nor button since.
    alt_alone: bool,
    /// The floating panel that the pointer holds by its bar or its corner.
    panel: Option<super::panel::Grab>,
}

impl Ui {
    /// The pointer moved to `x` and `y`, in pixels of the screen from the surface's top left
    /// corner: the button or the field under it is hovered, the one before is no longer, an item
    /// of a menu that it comes over takes the focus, and a field that holds the pointer selects or
    /// drags its number. A move is never taken: a game follows the pointer too.
    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        self.interaction.pointer = Some([x, y]);
        let hovered = self.interaction.hovered;
        if let Some(held) = self.interaction.bar {
            self.drag_bar(held, x, y);
        }
        if let Some(grab) = self.interaction.panel {
            self.drag_panel(grab, x, y);
        }
        if let Some(captured) = self.interaction.captured {
            match self.node(captured).map(|node| &node.element.kind) {
                Some(Kind::TextField { .. } | Kind::NumberField { .. }) => {
                    self.field_drag(captured, x, y);
                }
                Some(Kind::Slider { .. }) => self.slider_drag(captured, x),
                Some(Kind::Split { .. }) => self.split_drag(captured, x, y),
                _ => {}
            }
        }
        self.hover();
        let on = self.element_at(x, y);
        self.rest_on(on);
        // The item of a menu that the pointer comes over takes the focus, which the keys move on
        // from, as a press would give it; one of a bar of menus opens its menu while another's is
        // open.
        if let Some(item) = self.interaction.hovered
            && Some(item) != hovered
            && self.is_menu_item(item)
        {
            if self.in_menu_bar(item) {
                self.hover_bar_item(item);
            } else if self.hover_palette_row(item) {
                // The palette's rows are chosen as the pointer passes, its field keeping the focus.
            } else {
                self.interaction.by_pointer = true;
                self.set_focus(Some(item));
            }
        }
    }

    /// The pointer left the surface: nothing is hovered any more.
    pub fn pointer_left(&mut self) {
        self.interaction.pointer = None;
        self.hover();
        self.rest_on(None);
    }

    /// A button of the mouse went down or up; gives whether the interface took it, which a game
    /// then leaves. The primary button pressed over a button presses it, which then holds the
    /// pointer: let go over it, it gives an [`ActionKind::Press`]; let go elsewhere, nothing. A
    /// button that can hold the focus takes it. Pressed over a field, it gives the field the focus
    /// and places its caret, Shift held extending the selection, and the field holds the pointer,
    /// which selects as it moves; over a field of numbers that does not hold the focus, a move
    /// past a few pixels drags its number instead. The primary button pressed elsewhere takes the
    /// focus away, a field committing, unless a modal root is open. The interface takes a press of
    /// any button over what it draws, a panel's box or a label's text as well as a button or a
    /// field, and every press while a modal root is open; and the release of a press that it took.
    pub fn pointer_button(&mut self, button: MouseButton, state: ButtonState) -> bool {
        let primary = button == MouseButton::Left;
        if state == ButtonState::Pressed {
            self.interaction.by_pointer = true;
            self.interaction.alt_alone = false;
            self.interaction.bar_before = None;
            self.input_hides_tooltip();
            // A press outside a layer that closes as the user leaves it closes it; one on a
            // floating panel raises it.
            if let Some([x, y]) = self.interaction.pointer {
                self.dismiss_outside(x, y);
                if let Some(hit) = self.element_at(x, y) {
                    self.raise_panel(hit);
                }
            }
        }
        match state {
            // A press on a scroll bar holds its thumb, or pages along its track.
            ButtonState::Pressed
                if primary
                    && let Some([x, y]) = self.interaction.pointer
                    && let Some((id, bar)) = self.bar_at(x, y) =>
            {
                self.interaction.held.retain(|&held| held != button);
                self.interaction.held.push(button);
                self.press_bar(id, bar, x, y);
                true
            }
            ButtonState::Released if primary && self.interaction.bar.take().is_some() => {
                self.interaction.held.retain(|&held| held != button);
                true
            }
            // The second button on what the interface draws asks for a context menu.
            ButtonState::Pressed
                if button == MouseButton::Right
                    && let Some([x, y]) = self.interaction.pointer
                    && (self.holds_pointer()
                        || self.pointer_target(Kind::takes_pointer).is_some()) =>
            {
                self.interaction.held.retain(|&held| held != button);
                self.interaction.held.push(button);
                let element = self.element_at(x, y);
                let scale = element.map_or(1.0, |id| self.scale_of(id));
                self.interaction.actions.push(Action {
                    element,
                    kind: ActionKind::ContextMenu {
                        at: [x / scale, y / scale],
                    },
                });
                true
            }
            ButtonState::Pressed => {
                // A press without the release of the one before lets go of what that one held.
                if primary && let Some(before) = self.interaction.captured.take() {
                    self.change_state(before, |state| state.pressed = false);
                    self.field_press_lost(before);
                }
                let target = if primary {
                    self.pointer_target(Kind::takes_pointer)
                } else {
                    None
                };
                let taken = target.is_some() || self.holds_pointer();
                self.interaction.held.retain(|&held| held != button);
                if taken {
                    self.interaction.held.push(button);
                }
                let Some(target) = target else {
                    // A press that reaches no button and no field takes the focus away, a field
                    // committing; a modal root keeps it among its own elements, and a press that
                    // the interface cannot place, the pointer gone, leaves it alone.
                    if primary
                        && self.interaction.pointer.is_some()
                        && self.interaction.modals.is_empty()
                    {
                        self.set_focus(None);
                    }
                    return taken;
                };
                self.interaction.captured = Some(target);
                let [x, y] = self.interaction.pointer.unwrap_or_default();
                // A floating panel's bar or corner, held, moves or sizes it.
                if let Some(grab) = self.panel_grab(target, x, y) {
                    self.interaction.panel = Some(grab);
                    return true;
                }
                // The mark of a field of search empties it, the field holding the focus.
                if self
                    .search_clear(target)
                    .is_some_and(|clear| inside(clear, x, y))
                {
                    self.interaction.captured = None;
                    self.set_focus(Some(target));
                    self.set_value(target, "");
                    return true;
                }
                if self.node(target).is_some_and(|node| node.field.is_some()) {
                    let shift = self.interaction.modifiers.shift;
                    self.field_press(target, x, y, shift);
                } else {
                    self.change_state(target, |state| state.pressed = true);
                    // An item of a bar of menus takes the focus from the keys alone.
                    if !self.in_menu_bar(target) {
                        self.set_focus(Some(target));
                    }
                    // A slider's thumb comes under the pointer, wherever it pressed the track.
                    self.slider_drag(target, x);
                }
                true
            }
            ButtonState::Released => {
                let Some(at) = self
                    .interaction
                    .held
                    .iter()
                    .position(|&held| held == button)
                else {
                    return false;
                };
                self.interaction.held.swap_remove(at);
                if !primary {
                    return true;
                }
                if self.interaction.panel.take().is_some() {
                    self.interaction.captured = None;
                    return true;
                }
                let Some(captured) = self.interaction.captured.take() else {
                    return true;
                };
                if self.node(captured).is_some_and(|node| node.field.is_some()) {
                    let [x, y] = self.interaction.pointer.unwrap_or_default();
                    self.field_release(captured, x, y);
                    return true;
                }
                self.change_state(captured, |state| state.pressed = false);
                // A slider's drag ends where it is, committed.
                if let Some(value) = self.slider_value(captured) {
                    self.set_slider(captured, value, true);
                    return true;
                }
                let over = self.pointer_target(Kind::takes_pointer);
                if over == Some(captured) {
                    self.perform(Action {
                        element: Some(captured),
                        kind: ActionKind::Press,
                    });
                }
                true
            }
        }
    }

    /// The wheel scrolled: the box that scrolls or the list under the pointer scrolls, a line of
    /// the wheel some 40 logical pixels. Gives whether the interface took it, which a game then
    /// leaves: when such a box or list lies under the pointer, whether or not it had room to
    /// scroll, and, as a press, over what the interface draws or while a modal root is open.
    pub fn wheel(&mut self, delta: ScrollDelta) -> bool {
        self.input_hides_tooltip();
        let Some(target) =
            self.pointer_target(|kind| matches!(kind, Kind::Scroll | Kind::List { .. }))
        else {
            return self.holds_pointer();
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
        true
    }

    /// The modifier keys held changed: shortcuts are read with them.
    pub fn modifiers(&mut self, modifiers: Modifiers) {
        self.interaction.modifiers = modifiers;
    }

    /// A key went down or up; gives whether the interface took it, which a game then leaves. A
    /// press, or a repeat, reaches first the field that holds the focus, which takes what it types
    /// and edits with; then it gives the command of a shortcut whose chord it is, in the scopes of
    /// the focus, innermost first, then `global` ([`Ui::set_shortcuts`]); else the intent that its
    /// key means: Tab and Shift+Tab next and previous, the arrows a direction, Enter and Space
    /// accept, Escape cancel. The interface takes what a field takes, a shortcut's key, the key
    /// of an intent while an element holds the focus or once it gives the focus, as Tab does with
    /// none held, and every key while a modal root is open; the intent of a key that it does not
    /// take still reaches its queue. Its releases are left to the game, which pairs them with its
    /// presses, but Alt's, which, pressed and let go alone, gives a bar of menus the focus, as F10
    /// does ([`Kind::MenuBar`]). A press hides the tooltip that shows, which Escape does alone
    /// then, and the element that it gives the focus shows its tooltip
    /// ([`Element::tooltip`](super::Element::tooltip)).
    pub fn key(&mut self, event: &KeyEvent) -> bool {
        let alt = event.logical == Key::Named(NamedKey::Alt);
        if event.state != ButtonState::Pressed {
            // Alt let go alone reaches the bar of menus.
            let alone = std::mem::take(&mut self.interaction.alt_alone);
            return alt && alone && self.toggle_menu_bar();
        }
        if alt {
            self.interaction.alt_alone = true;
            return false;
        }
        self.interaction.alt_alone = false;
        self.interaction.by_pointer = false;
        // A key hides the tooltip that shows; Escape does nothing more.
        if self.input_hides_tooltip() && event.logical == Key::Named(NamedKey::Escape) {
            return true;
        }
        let before = self.interaction.focus;
        let taken = self.press_key(event);
        // The element that the keys gave the focus shows its tooltip.
        if let Some(focus) = self.interaction.focus
            && Some(focus) != before
            && self
                .node(focus)
                .is_some_and(|node| node.element.tooltip.is_some())
        {
            self.show_tooltip(focus);
        }
        taken
    }

    /// What a key's press does, as [`Ui::key`] says, once the tooltip is hidden.
    fn press_key(&mut self, event: &KeyEvent) -> bool {
        // Escape closes the top layer that closes as the user leaves it.
        if event.logical == Key::Named(NamedKey::Escape) && self.dismiss_top() {
            return true;
        }
        // The key of context menus, or Shift+F10, asks for one at the element that holds the
        // focus.
        let context = event.logical == Key::Named(NamedKey::ContextMenu)
            || (event.logical == Key::Named(NamedKey::F10) && self.interaction.modifiers.shift);
        if context && let Some(focus) = self.interaction.focus {
            let scale = self.scale_of(focus);
            let rect = self.node(focus).map(|node| node.rect).unwrap_or_default();
            let rtl = self.root_settings(focus).map(|root| root.direction)
                == Some(super::LayoutDirection::RightToLeft);
            let x = if rtl { rect.x + rect.width } else { rect.x };
            self.interaction.actions.push(Action {
                element: Some(focus),
                kind: ActionKind::ContextMenu {
                    at: [x / scale, (rect.y + rect.height) / scale],
                },
            });
            return true;
        }
        // F10 reaches the bar of menus.
        if event.logical == Key::Named(NamedKey::F10)
            && !self.interaction.modifiers.shift
            && self.toggle_menu_bar()
        {
            return true;
        }
        // The palette's field chooses among the commands found by the arrows and Enter.
        if self.palette_key(event) {
            return true;
        }
        let modal = !self.interaction.modals.is_empty();
        if let Some(field) = self.focused_field()
            && self.field_key(field, event)
        {
            return true;
        }
        if let Some(focus) = self.interaction.focus
            && (self.slider_key(focus, event)
                || self.tab_key(focus, event)
                || self.split_key(focus, event)
                || self.menu_key(focus, event)
                || self.choice_key(focus, event))
        {
            return true;
        }
        if let Some(command) = self.shortcut(event) {
            let element = self.interaction.focus;
            self.act(Action {
                element,
                kind: ActionKind::Command(command),
            });
            return true;
        }
        let Key::Named(named) = event.logical else {
            return modal;
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
            _ => return modal,
        };
        let element = self.interaction.focus;
        self.act(Action {
            element,
            kind: ActionKind::Intent(intent),
        });
        element.is_some() || self.interaction.focus.is_some() || modal
    }

    /// Does `action`, as a person's input does, for an agent or the program: a press of a button
    /// that a person could press, not disabled, drawn, and no modal root above its own, reaches the
    /// queue; the intents of navigation move the focus; `Accept` commits the field that holds the
    /// focus, which queues what it commits, if anything; it presses the button that holds it, and
    /// reaches the queue as an intent where none can be pressed; `Cancel`, commands, and a field's
    /// changes and commits reach the queue. A press of what is not such a button, or of what
    /// names nothing, is left out.
    pub fn act(&mut self, action: Action) {
        self.interaction.by_pointer = false;
        self.perform(action);
    }

    /// Does `action`, as [`Ui::act`] says, for any input: the pointer's, which keeps whether it was
    /// the last.
    pub(super) fn perform(&mut self, action: Action) {
        match action.kind {
            ActionKind::Press => {
                let Some(id) = action
                    .element
                    .filter(|&id| self.pressable(id) && self.in_scope(id))
                else {
                    return;
                };
                // A button of a notification gives its command and closes it.
                if self.press_notice(id) {
                    return;
                }
                // A check box pressed toggles, which the program reads as its value; a section opens
                // or folds; a tab is chosen.
                match self.node(id).map(|node| &node.element.kind) {
                    Some(
                        Kind::CheckBox { .. }
                        | Kind::Section { .. }
                        | Kind::FloatingPanel { .. }
                        | Kind::ToggleButton { .. },
                    ) => self.toggle(id),
                    Some(Kind::Tab { .. } | Kind::Segment { .. }) => self.press_tab(id),
                    Some(Kind::MenuItem { .. }) => self.press_menu_item(id),
                    Some(Kind::Choice { .. }) => self.press_choice(id),
                    _ => self.interaction.actions.push(action),
                }
            }
            ActionKind::Intent(Intent::Accept) => {
                let focus = self.interaction.focus;
                if let Some(field) = self.focused_field() {
                    self.commit(field);
                    return;
                }
                match focus {
                    Some(id) if self.pressable(id) => self.perform(Action {
                        element: focus,
                        kind: ActionKind::Press,
                    }),
                    _ => self.interaction.actions.push(Action {
                        element: focus,
                        kind: ActionKind::Intent(Intent::Accept),
                    }),
                }
            }
            // Cancel closes the top layer that closes as the user leaves it, as Escape does.
            ActionKind::Intent(Intent::Cancel) if self.dismiss_top() => {}
            ActionKind::Intent(Intent::Cancel)
            | ActionKind::Command(_)
            | ActionKind::Changed(_)
            | ActionKind::Committed(_)
            | ActionKind::ContextMenu { .. }
            | ActionKind::Dismissed => {
                self.interaction.actions.push(action);
            }
            ActionKind::Intent(intent) => self.move_focus(intent),
        }
    }

    /// The actions that the program has not taken yet, in the order they came, taken.
    pub fn take_actions(&mut self) -> Vec<Action> {
        self.drop_palette_actions();
        std::mem::take(&mut self.interaction.actions)
    }

    /// The element that holds the focus.
    pub fn focus(&self) -> Option<ElementId> {
        self.interaction.focus
    }

    /// Gives the focus to `id`, a button, a field or a widget that is not disabled and is drawn, or
    /// takes it from what holds it, for `None`; a field that loses it commits. Nothing happens for an
    /// element that cannot hold it, or that a modal root above its own keeps it from.
    pub fn set_focus(&mut self, id: Option<ElementId>) {
        if let Some(id) = id
            && (!self.focusable(id) || !self.in_scope(id))
        {
            return;
        }
        if let Some(before) = self.interaction.focus.take() {
            // A field that the focus leaves commits what the user typed; its caret goes.
            if Some(before) != id && self.node(before).is_some_and(|node| node.field.is_some()) {
                self.commit(before);
                self.mark_paint(before);
            }
            self.change_state(before, |state| {
                state.focused = false;
                state.focus_visible = false;
            });
        }
        if let Some(id) = id {
            // The focus that the keyboard, an agent or the program gave shows; the pointer's does
            // not, as browsers show it.
            let visible = !self.interaction.by_pointer;
            self.interaction.focus = Some(id);
            self.change_state(id, |state| {
                state.focused = true;
                state.focus_visible = visible;
            });
            self.mark_paint(id);
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

    /// The chord that gives `command`, as a table writes it, `Ctrl+S`: of the scope `global`
    /// first, then of the first scope that gives it, in the order of [`Ui::shortcuts`]; a chord that
    /// a later table gives another command in its scope gives this one no more. None for a command
    /// that no shortcut gives.
    pub fn command_chord(&self, command: &str) -> Option<String> {
        let scopes = self.shortcuts();
        let chord = |(_, chords): &(String, Vec<(String, String)>)| {
            chords
                .iter()
                .find(|(_, each)| each == command)
                .map(|(chord, _)| chord.clone())
        };
        scopes
            .iter()
            .find(|(scope, _)| scope == "global")
            .and_then(chord)
            .or_else(|| scopes.iter().find_map(chord))
    }

    /// The shortcuts that the tables give, by scope, each scope's in the order of the tables:
    /// its name, then each chord, as a table writes it, and its command. A chord that a later
    /// table gives again in a scope is given once, with the later command.
    pub fn shortcuts(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut scopes: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for binding in &self.interaction.bindings {
            let chord = binding.chord.text();
            let at = match scopes.iter().position(|(scope, _)| *scope == binding.scope) {
                Some(at) => at,
                None => {
                    scopes.push((binding.scope.clone(), Vec::new()));
                    scopes.len() - 1
                }
            };
            let chords = &mut scopes[at].1;
            chords.retain(|(held, _)| *held != chord);
            chords.push((chord, binding.command.clone()));
        }
        scopes
    }

    /// The element that the pointer would reach at `x` and `y`, in pixels of the screen: the
    /// deepest drawn there, of the top root that holds the point, its clip holding it too, a
    /// tooltip letting it through; none below a modal root.
    pub fn element_at(&self, x: f32, y: f32) -> Option<ElementId> {
        for &root in self.roots.iter().rev() {
            // The pointer passes through a tooltip.
            if self.interaction.tooltip == Some(root) {
                continue;
            }
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
    pub(super) fn pointer_target(&self, takes: impl Fn(&Kind) -> bool) -> Option<ElementId> {
        let [x, y] = self.interaction.pointer?;
        let mut at = self.element_at(x, y);
        while let Some(id) = at {
            let node = self.node(id)?;
            // A section takes the pointer on its header alone, two panels on their handle.
            let reached = match node.element.kind {
                Kind::Section { .. } => self
                    .section_header(id)
                    .is_some_and(|header| inside(header, x, y)),
                // A floating panel on its bar and its corner.
                Kind::FloatingPanel { .. } => {
                    self.section_header(id)
                        .is_some_and(|header| inside(header, x, y))
                        || self
                            .panel_corner(id)
                            .is_some_and(|corner| inside(corner, x, y))
                }
                Kind::Split { .. } => self
                    .split_handle(id)
                    .is_some_and(|handle| inside(handle, x, y)),
                _ => true,
            };
            if reached && takes(&node.element.kind) && !node.state.disabled {
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
        holds(node, x, y).then_some(id)
    }

    /// Whether a press or the wheel where the pointer is reaches the interface, which a game then
    /// leaves: a modal root is open, or the pointer lies on what the interface draws, as its last
    /// frame drew it, a box with a fill, a border or a shadow, a text, where its lines lie, an
    /// image or a field. A box that only lays others out draws nothing, nor does an empty text.
    fn holds_pointer(&self) -> bool {
        if !self.interaction.modals.is_empty() {
            return true;
        }
        let Some([x, y]) = self.interaction.pointer else {
            return false;
        };
        if self.bar_at(x, y).is_some() {
            return true;
        }
        let mut at = self.element_at(x, y);
        while let Some(node) = at.and_then(|id| self.node(id)) {
            let drawn = node.primitives.iter().any(|(primitive, own)| {
                let clip = match own {
                    Some(own) => Some(crate::layout::intersect(node.clip, *own)),
                    None => node.clip,
                };
                inside(primitive.bounds(), x, y) && clip.is_none_or(|clip| inside(clip, x, y))
            });
            if drawn {
                return true;
            }
            at = node.parent;
        }
        false
    }

    /// Hovers the button or the field under the pointer, the one that holds the pointer alone
    /// while it does.
    fn hover(&mut self) {
        let over = self.pointer_target(Kind::takes_pointer);
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
    /// Shift+Tab to the last, and the arrows, which a game moves with, give it to none.
    fn move_focus(&mut self, intent: Intent) {
        let focus = self.interaction.focus;
        let scopes: Vec<ElementId> = match self.interaction.modals.last() {
            // Within the root of the focus, a layer above the modal root, or else the modal root.
            Some(&(modal, _)) => match focus.filter(|&focus| self.in_scope(focus)) {
                Some(focus) => self.root_of(focus).into_iter().collect(),
                None => vec![modal],
            },
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
                Intent::Next => candidates[0],
                Intent::Previous => candidates[candidates.len() - 1],
                _ => return,
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

    /// Whether `id` lies in a root that no modal root above keeps the focus from: the top modal
    /// root, or a root above it.
    pub(super) fn in_scope(&self, id: ElementId) -> bool {
        match self.interaction.modals.last() {
            // The modal root's, or a root above it: a layer that it opened, its list or its menu.
            Some(&(modal, _)) => {
                let place = |root| self.roots.iter().position(|&each| each == root);
                self.root_of(id)
                    .and_then(place)
                    .zip(place(modal))
                    .is_some_and(|(root, modal)| root >= modal)
            }
            None => true,
        }
    }

    /// Whether `id` can hold the focus: an element of a kind that the pointer reaches, a button, a
    /// field or a widget, not disabled, drawn, as its ancestors are.
    pub(super) fn focusable(&self, id: ElementId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        if !node.element.kind.takes_pointer() || node.state.disabled {
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

    /// Whether `id` can be pressed: a button, a check box, a section, a tab, an item of a menu, a
    /// list to choose from, a floating panel, a button that toggles or a segment that can hold the
    /// focus.
    pub(super) fn pressable(&self, id: ElementId) -> bool {
        self.focusable(id)
            && self.node(id).is_some_and(|node| {
                matches!(
                    node.element.kind,
                    Kind::Button { .. }
                        | Kind::CheckBox { .. }
                        | Kind::Section { .. }
                        | Kind::Tab { .. }
                        | Kind::MenuItem { .. }
                        | Kind::Choice { .. }
                        | Kind::FloatingPanel { .. }
                        | Kind::ToggleButton { .. }
                        | Kind::Segment { .. }
                )
            })
    }

    /// The elements below `root` that can hold the focus, in the order of the tree.
    pub(super) fn focusables(&self, root: ElementId) -> Vec<ElementId> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else {
                continue;
            };
            if node.style.layout.display == Display::None {
                continue;
            }
            if node.element.kind.takes_pointer() && !node.state.disabled {
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
    pub(super) fn change_state(&mut self, id: ElementId, change: impl FnOnce(&mut ElementState)) {
        if let Some(node) = self.node_mut(id) {
            let before = node.state;
            change(&mut node.state);
            if node.state != before {
                self.mark_style(id);
                self.mark_meaning(id);
            }
        }
    }

    /// Takes the focus from the root below when `root`, a modal one, opens.
    pub(super) fn opened_modal(&mut self, root: ElementId) {
        let before = self.interaction.focus;
        self.set_focus(None);
        self.interaction.modals.push((root, before));
        // What every element of the roots below takes changed.
        self.meaning_whole = true;
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
        self.meaning_whole = true;
        let interaction = &mut self.interaction;
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

/// Whether `node`'s box holds `x` and `y`, in pixels of the screen, its clip holding them too.
fn holds(node: &Node, x: f32, y: f32) -> bool {
    inside(node.rect, x, y) && node.clip.is_none_or(|clip| inside(clip, x, y))
}

/// Whether `rect` holds `x` and `y`.
pub(super) fn inside(rect: crate::Rect, x: f32, y: f32) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}
