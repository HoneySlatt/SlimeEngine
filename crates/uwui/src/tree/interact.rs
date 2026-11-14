//! A person's input and an agent's actions on the tree: the element under the pointer, the pointer
//! held by a pressed button, the focus and its moves, the shortcuts, and the queue of actions that
//! the program reads (`docs/adr/0142-input-and-focus.md`).

use std::sync::LazyLock;

use slime_input::{ButtonState, Key, KeyEvent, Modifiers, MouseButton, NamedKey, ScrollDelta};

use super::field::DRAG_THRESHOLD;
use super::{ElementId, ElementState, Kind, LayoutDirection, Node, Place, Ui};
use crate::DocumentErrors;
use crate::action::{Action, ActionKind, Intent, SurfaceInput};
use crate::host::TargetId;
use crate::shortcuts::{self, Binding, Platform, Shortcuts};
use crate::style::Display;

/// How far a line of the wheel scrolls, in logical pixels.
const WHEEL_LINE: f32 = 40.0;

/// A drag under way ([`Ui::dragging`], `docs/adr/0168-rearranging-the-hierarchy.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dragging {
    /// The element dragged, which may have been built away since the drag started, as a list's row
    /// scrolled out of sight is: the program keeps what it dragged.
    pub source: ElementId,
    /// The target of drops under the pointer, where it would drop; none over nothing that takes
    /// drops.
    pub target: Option<ElementId>,
    /// Where in the target, from its top, 0, to its bottom, 1.
    pub at: f32,
}

/// What the interface keeps of a person's input between two events: what every target shares, and
/// each target's own (`docs/adr/0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md`).
#[derive(Default)]
pub(super) struct Interaction {
    /// Each known target's input, made with its first root, or with the first event or reading of
    /// [`TargetId::MAIN`].
    targets: Vec<(TargetId, TargetInput)>,
    /// The target of the last key or button: what names no element follows it.
    pub(super) active: TargetId,
    /// The actions that the program has not taken yet.
    pub(super) actions: Vec<Action>,
    /// The shortcuts, in the order of their tables.
    bindings: Vec<Binding>,
    /// The platform whose ways the keys follow.
    pub(super) platform: Platform,
}

/// The input of a target that has none yet, which every reader of one sees.
static NO_INPUT: LazyLock<TargetInput> = LazyLock::new(TargetInput::default);

impl Interaction {
    /// What `target`'s input holds; nothing for a target that has had neither root nor event.
    pub(super) fn of(&self, target: TargetId) -> &TargetInput {
        self.targets
            .iter()
            .find(|(each, _)| *each == target)
            .map_or(&NO_INPUT, |(_, input)| input)
    }

    /// `target`'s input, to change, made if it had none.
    pub(super) fn of_mut(&mut self, target: TargetId) -> &mut TargetInput {
        let at = match self.targets.iter().position(|(each, _)| *each == target) {
            Some(at) => at,
            None => {
                self.targets.push((target, TargetInput::default()));
                self.targets.len() - 1
            }
        };
        &mut self.targets[at].1
    }

    /// Each target's input, to change.
    pub(super) fn each_mut(&mut self) -> impl Iterator<Item = (TargetId, &mut TargetInput)> {
        self.targets
            .iter_mut()
            .map(|(target, input)| (*target, input))
    }

    /// Whether the interface knows `target`: [`TargetId::MAIN`], or a target that has had a root.
    pub(super) fn knows(&self, target: TargetId) -> bool {
        target == TargetId::MAIN || self.targets.iter().any(|(each, _)| *each == target)
    }

    /// Whether a modal root is open on any target.
    fn any_modal(&self) -> bool {
        self.targets
            .iter()
            .any(|(_, input)| !input.modals.is_empty())
    }

    /// The `index`th target that has an input, in the order they came: a walk by index lets each
    /// one's input change on the way, without a list of them made for the walk.
    pub(super) fn target_at(&self, index: usize) -> Option<TargetId> {
        self.targets.get(index).map(|(target, _)| *target)
    }

    /// The targets that have an input.
    pub(super) fn targets(&self) -> impl Iterator<Item = TargetId> + '_ {
        self.targets.iter().map(|(target, _)| *target)
    }
}

/// What the interface keeps of a person's input on one target between two events.
#[derive(Default)]
pub(super) struct TargetInput {
    /// Where the pointer is, in pixels of the target; none once it left.
    pub(super) pointer: Option<[f32; 2]>,
    /// The button or the field under the pointer.
    hovered: Option<ElementId>,
    /// The button or the field that the pointer pressed, which holds it until it is let go.
    pub(super) captured: Option<ElementId>,
    /// The mouse's buttons whose press the interface took, whose release it takes too.
    held: Vec<MouseButton>,
    /// The element that holds the focus.
    pub(super) focus: Option<ElementId>,
    /// The modal roots open on the target, each with the focus that it took, given back when it
    /// closes.
    modals: Vec<(ElementId, Option<ElementId>)>,
    /// The modifiers held, as the host last gave them for the target.
    pub(super) modifiers: Modifiers,
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
    /// The draggable element that the pointer pressed, where it pressed it, and whether it moved
    /// far enough since to drag it.
    drag: Option<(ElementId, [f32; 2], bool)>,
    /// The share that the splitter that the pointer holds gave the program last, which its release
    /// commits; none while it has not moved it.
    pub(super) splitting: Option<f32>,
    /// The surface that holds the pointer, with the buttons that went down over it and are held.
    surface: Option<(ElementId, Vec<MouseButton>)>,
    /// Where the pointer last was on the surface that holds it, which a release after the pointer
    /// left gives.
    surface_at: [f32; 2],
    /// What the host was last told of the target's outputs ([`Ui::outputs`]).
    pub(super) told: super::host::Told,
}

impl Ui {
    /// What [`Input::PointerMoved`](crate::host::Input::PointerMoved) does.
    pub(super) fn pointer_moved_input(&mut self, t: TargetId, x: f32, y: f32) {
        self.interaction.of_mut(t).pointer = Some([x, y]);
        if let Some((surface, _)) = self.interaction.of(t).surface {
            let at = self.on_surface(surface, [x, y]);
            self.interaction.of_mut(t).surface_at = at;
            self.interaction.actions.push(Action {
                element: Some(surface),
                kind: ActionKind::Surface(SurfaceInput::Moved { at }),
            });
        }
        let hovered = self.interaction.of(t).hovered;
        if let Some(held) = self.interaction.of(t).bar {
            self.drag_bar(held, x, y);
        }
        if let Some(grab) = self.interaction.of(t).panel {
            self.drag_panel(grab, x, y);
        }
        // A draggable element pressed and moved past a few pixels is dragged, no longer pressed.
        if let Some((source, [from_x, from_y], false)) = self.interaction.of(t).drag
            && (x - from_x).hypot(y - from_y) > DRAG_THRESHOLD
        {
            self.interaction.of_mut(t).drag = Some((source, [from_x, from_y], true));
            self.change_state(source, |state| state.pressed = false);
        }
        if let Some(captured) = self.interaction.of(t).captured {
            match self.node(captured).map(|node| &node.element.kind) {
                Some(Kind::TextField { .. } | Kind::NumberField { .. }) => {
                    self.field_drag(captured, x, y);
                }
                Some(Kind::Slider { .. }) => self.slider_drag(captured, x),
                Some(Kind::ColorPart(_)) => self.part_drag(captured, x, y),
                Some(Kind::Split { .. }) => self.split_drag(captured, x, y),
                Some(Kind::Splitter { .. }) => self.splitter_drag(captured, x, y),
                _ => {}
            }
        }
        // One walk of the tree finds what both the hover and the rest follow.
        let on = self.element_at(t, x, y);
        self.hover_over(t, on);
        self.rest_on(t, on);
        // The item of a menu that the pointer comes over takes the focus, which the keys move on
        // from, as a press would give it; one of a bar of menus opens its menu while another's is
        // open.
        if let Some(item) = self.interaction.of(t).hovered
            && Some(item) != hovered
            && self.is_menu_item(item)
        {
            if self.in_menu_bar(item) {
                self.hover_bar_item(t, item);
            } else if self.hover_palette_row(item) {
                // The palette's rows are chosen as the pointer passes, its field keeping the focus.
            } else {
                self.interaction.of_mut(t).by_pointer = true;
                self.focus_on(t, Some(item));
            }
        }
    }

    /// What [`Input::PointerLeft`](crate::host::Input::PointerLeft) does.
    pub(super) fn pointer_left_input(&mut self, t: TargetId) {
        self.interaction.of_mut(t).pointer = None;
        self.hover(t);
        self.rest_on(t, None);
    }

    /// What [`Input::PointerButton`](crate::host::Input::PointerButton) does; gives whether the
    /// interface took it.
    pub(super) fn pointer_button_input(
        &mut self,
        t: TargetId,
        button: MouseButton,
        state: ButtonState,
    ) -> bool {
        let primary = button == MouseButton::Left;
        if state == ButtonState::Pressed {
            self.interaction.of_mut(t).by_pointer = true;
            self.interaction.of_mut(t).alt_alone = false;
            self.interaction.of_mut(t).bar_before = None;
            self.input_hides_tooltip(t);
            // A press outside a layer that closes as the user leaves it closes it; one on a
            // floating panel raises it.
            if let Some([x, y]) = self.interaction.of(t).pointer {
                self.dismiss_outside(t, x, y);
                if let Some(hit) = self.element_at(t, x, y) {
                    self.raise_panel(hit);
                }
            }
        }
        if self.surface_button(t, button, state) {
            return true;
        }
        match state {
            // A press on a scroll bar holds its thumb, or pages along its track.
            ButtonState::Pressed
                if primary
                    && let Some([x, y]) = self.interaction.of(t).pointer
                    && let Some((id, bar)) = self.bar_at(t, x, y) =>
            {
                self.interaction
                    .of_mut(t)
                    .held
                    .retain(|&held| held != button);
                self.interaction.of_mut(t).held.push(button);
                self.press_bar(id, bar, x, y);
                true
            }
            ButtonState::Released if primary && self.interaction.of_mut(t).bar.take().is_some() => {
                self.interaction
                    .of_mut(t)
                    .held
                    .retain(|&held| held != button);
                true
            }
            // The second button on what the interface draws asks for a context menu.
            ButtonState::Pressed
                if button == MouseButton::Right
                    && let Some([x, y]) = self.interaction.of(t).pointer
                    && (self.holds_pointer(t)
                        || self.pointer_target(t, Kind::takes_pointer).is_some()) =>
            {
                self.interaction
                    .of_mut(t)
                    .held
                    .retain(|&held| held != button);
                self.interaction.of_mut(t).held.push(button);
                let element = self.element_at(t, x, y);
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
                // A press without the release of the one before lets go of what that one held,
                // and of its drag.
                if primary {
                    self.interaction.of_mut(t).drag = None;
                    self.interaction.of_mut(t).splitting = None;
                }
                if primary && let Some(before) = self.interaction.of_mut(t).captured.take() {
                    self.change_state(before, |state| state.pressed = false);
                    self.field_press_lost(before);
                }
                let target = if primary {
                    self.pointer_target(t, Kind::takes_pointer)
                } else {
                    None
                };
                let taken = target.is_some() || self.holds_pointer(t);
                self.interaction
                    .of_mut(t)
                    .held
                    .retain(|&held| held != button);
                if taken {
                    self.interaction.of_mut(t).held.push(button);
                }
                let Some(target) = target else {
                    // A press that reaches no button and no field takes the focus away, a field
                    // committing; a modal root keeps it among its own elements, and a press that
                    // the interface cannot place, the pointer gone, leaves it alone.
                    if primary
                        && self.interaction.of(t).pointer.is_some()
                        && self.interaction.of(t).modals.is_empty()
                    {
                        self.focus_on(t, None);
                    }
                    return taken;
                };
                self.interaction.of_mut(t).captured = Some(target);
                let [x, y] = self.interaction.of(t).pointer.unwrap_or_default();
                // A floating panel's bar or corner, held, moves or sizes it.
                if let Some(grab) = self.panel_grab(target, x, y) {
                    self.interaction.of_mut(t).panel = Some(grab);
                    return true;
                }
                // The mark of a field of search empties it, the field holding the focus.
                if self
                    .search_clear(target)
                    .is_some_and(|clear| inside(clear, x, y))
                {
                    self.interaction.of_mut(t).captured = None;
                    self.focus_on(t, Some(target));
                    self.set_value(target, "");
                    return true;
                }
                if self.node(target).is_some_and(|node| node.field.is_some()) {
                    let shift = self.interaction.of(t).modifiers.shift;
                    self.field_press(target, x, y, shift);
                } else {
                    self.change_state(target, |state| state.pressed = true);
                    if self.node(target).is_some_and(|node| node.element.draggable) {
                        self.interaction.of_mut(t).drag = Some((target, [x, y], false));
                    }
                    // An item of a bar of menus takes the focus from the keys alone.
                    if !self.in_menu_bar(target) {
                        self.focus_on(t, Some(target));
                    }
                    // A slider's thumb comes under the pointer, wherever it pressed the track; a
                    // part of a color's picker takes the color under it.
                    self.slider_drag(target, x);
                    self.part_drag(target, x, y);
                }
                true
            }
            ButtonState::Released => {
                let Some(at) = self
                    .interaction
                    .of(t)
                    .held
                    .iter()
                    .position(|&held| held == button)
                else {
                    return false;
                };
                self.interaction.of_mut(t).held.swap_remove(at);
                if !primary {
                    return true;
                }
                if let Some(grab) = self.interaction.of_mut(t).panel.take() {
                    self.interaction.of_mut(t).captured = None;
                    self.panel_placed(grab);
                    return true;
                }
                let captured = self.interaction.of_mut(t).captured.take();
                // A drag drops on the target under the pointer, or nowhere, even once the element
                // dragged was built away, which let go of the pointer.
                if let Some(dragging) = self.dragging_on(t) {
                    self.interaction.of_mut(t).drag = None;
                    if let Some(target) = dragging.target {
                        self.interaction.actions.push(Action {
                            element: Some(target),
                            kind: ActionKind::Dropped {
                                source: dragging.source,
                                at: dragging.at,
                            },
                        });
                    }
                    return true;
                }
                self.interaction.of_mut(t).drag = None;
                let Some(captured) = captured else {
                    return true;
                };
                if self.node(captured).is_some_and(|node| node.field.is_some()) {
                    let [x, y] = self.interaction.of(t).pointer.unwrap_or_default();
                    self.field_release(captured, x, y);
                    return true;
                }
                self.change_state(captured, |state| state.pressed = false);
                // A slider's drag ends where it is, committed, and so does a picker's.
                if let Some(value) = self.slider_value(captured) {
                    self.set_slider(captured, value, true);
                    return true;
                }
                if self.part_release(captured) || self.splitter_release(captured) {
                    return true;
                }
                let over = self.pointer_target(t, Kind::takes_pointer);
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

    /// What [`Input::Wheel`](crate::host::Input::Wheel) does; gives whether the interface took it.
    pub(super) fn wheel_input(&mut self, t: TargetId, delta: ScrollDelta) -> bool {
        self.input_hides_tooltip(t);
        let surface = match self.interaction.of(t).surface {
            Some((surface, _)) => Some(surface),
            None => self.surface_under_pointer(t),
        };
        if let (Some(surface), Some(pointer)) = (surface, self.interaction.of(t).pointer) {
            let at = self.on_surface(surface, pointer);
            self.interaction.actions.push(Action {
                element: Some(surface),
                kind: ActionKind::Surface(SurfaceInput::Wheel { delta, at }),
            });
            return true;
        }
        let Some(target) =
            self.pointer_target(t, |kind| matches!(kind, Kind::Scroll | Kind::List { .. }))
        else {
            return self.holds_pointer(t);
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

    /// The drag under way, once the pointer moved a draggable element past a few pixels: what it
    /// drags, the target of drops under the pointer, the nearest of its ancestors that takes drops
    /// included, and where in it, from its top, 0, to its bottom, 1, which the program shows
    /// (`docs/adr/0168-rearranging-the-hierarchy.md`). None while nothing is dragged. A drag
    /// is its target's, whose press started it: the active target's first, then any other's.
    pub fn dragging(&self) -> Option<Dragging> {
        let active = self.interaction.active;
        self.dragging_on(active).or_else(|| {
            self.interaction
                .targets()
                .filter(|&t| t != active)
                .find_map(|t| self.dragging_on(t))
        })
    }

    /// The drag under way on `t`, as [`Ui::dragging`] says.
    fn dragging_on(&self, t: TargetId) -> Option<Dragging> {
        let (source, _, true) = self.interaction.of(t).drag? else {
            return None;
        };
        let [x, y] = self.interaction.of(t).pointer?;
        let mut at = self.element_at(t, x, y);
        while let Some(id) = at {
            let node = self.node(id)?;
            if node.element.drop_target && self.in_scope(id) {
                let rect = node.rect;
                let height = rect.height.max(f32::EPSILON);
                return Some(Dragging {
                    source,
                    target: Some(id),
                    at: ((y - rect.y) / height).clamp(0.0, 1.0),
                });
            }
            at = node.parent;
        }
        Some(Dragging {
            source,
            target: None,
            at: 0.0,
        })
    }

    /// Drops `source`, a draggable element, on `target`, a target of drops, `at` its height from
    /// its top, 0, to its bottom, 1, as a person who dragged it there: the target's
    /// [`ActionKind::Dropped`]. Gives whether it dropped; nothing happens for an element that is no
    /// such, or that a modal root above its own keeps from input.
    pub fn drop(&mut self, source: ElementId, target: ElementId, at: f32) -> bool {
        let draggable = self.node(source).is_some_and(|node| node.element.draggable);
        let takes = self
            .node(target)
            .is_some_and(|node| node.element.drop_target);
        if !draggable || !takes || !self.in_scope(source) || !self.in_scope(target) {
            return false;
        }
        self.interaction.actions.push(Action {
            element: Some(target),
            kind: ActionKind::Dropped {
                source,
                at: at.clamp(0.0, 1.0),
            },
        });
        true
    }

    /// The modifiers held, as the host last gave them
    /// ([`Input::Modifiers`](crate::host::Input::Modifiers)): what a press of a row selects by,
    /// with Ctrl or Shift (`docs/adr/0167-the-hierarchy.md`); those of the active target
    /// ([`Ui::active_target`]).
    pub fn held_modifiers(&self) -> Modifiers {
        self.interaction.of(self.interaction.active).modifiers
    }

    /// What [`Input::Key`](crate::host::Input::Key) does; gives whether the interface took it.
    pub(super) fn key_input(&mut self, t: TargetId, event: &KeyEvent) -> bool {
        let alt = event.logical == Key::Named(NamedKey::Alt);
        if event.state != ButtonState::Pressed {
            // Alt let go alone reaches the bar of menus.
            let alone = std::mem::take(&mut self.interaction.of_mut(t).alt_alone);
            return alt && alone && self.toggle_menu_bar(t);
        }
        // Alt pressed while a surface holds the pointer is a gesture's, not the bar of menus'.
        if alt {
            self.interaction.of_mut(t).alt_alone = self.interaction.of(t).surface.is_none();
            return false;
        }
        self.interaction.of_mut(t).alt_alone = false;
        // Escape gives a drag up, which then drops nowhere, wherever the pointer is.
        if event.logical == Key::Named(NamedKey::Escape)
            && self
                .interaction
                .of(t)
                .drag
                .is_some_and(|(_, _, started)| started)
        {
            self.interaction.of_mut(t).drag = None;
            self.interaction.of_mut(t).captured = None;
            return true;
        }
        // While a surface holds the pointer, the keys that fly through it reach the program.
        let modifiers = self.interaction.of(t).modifiers;
        if self.interaction.of(t).surface.is_some()
            && !(modifiers.control || modifiers.alt || modifiers.super_key)
        {
            return false;
        }
        self.interaction.of_mut(t).by_pointer = false;
        // A key hides the tooltip that shows; Escape does nothing more.
        if self.input_hides_tooltip(t) && event.logical == Key::Named(NamedKey::Escape) {
            return true;
        }
        let before = self.interaction.of(t).focus;
        let taken = self.press_key(t, event);
        // The element that the keys gave the focus shows its tooltip.
        if let Some(focus) = self.interaction.of(t).focus
            && Some(focus) != before
            && self
                .node(focus)
                .is_some_and(|node| node.element.tooltip.is_some())
        {
            self.show_tooltip(t, focus);
        }
        taken
    }

    /// Gives a button of the mouse to the surface that holds the pointer, or that it goes down
    /// over, as [`Kind::Surface`] says; gives whether a surface took it.
    fn surface_button(&mut self, t: TargetId, button: MouseButton, state: ButtonState) -> bool {
        // A release after the pointer left still lets the surface go, where the pointer was last.
        let pointer = self.interaction.of(t).pointer;
        let input = match state {
            ButtonState::Pressed => {
                let Some(pointer) = pointer else {
                    return false;
                };
                let surface = match self.interaction.of(t).surface {
                    Some((surface, _)) => surface,
                    None => match self.surface_under_pointer(t) {
                        Some(surface) => {
                            self.interaction.of_mut(t).surface = Some((surface, Vec::new()));
                            // The keys that fly through it type in no field.
                            if self.interaction.of(t).modals.is_empty() {
                                self.focus_on(t, None);
                            }
                            surface
                        }
                        None => return false,
                    },
                };
                if let Some((_, buttons)) = &mut self.interaction.of_mut(t).surface
                    && !buttons.contains(&button)
                {
                    buttons.push(button);
                }
                let at = self.on_surface(surface, pointer);
                self.interaction.of_mut(t).surface_at = at;
                (
                    surface,
                    SurfaceInput::Pressed {
                        button,
                        at,
                        modifiers: self.interaction.of(t).modifiers,
                    },
                )
            }
            ButtonState::Released => {
                let Some((surface, buttons)) = &mut self.interaction.of_mut(t).surface else {
                    return false;
                };
                let Some(place) = buttons.iter().position(|&held| held == button) else {
                    return false;
                };
                buttons.swap_remove(place);
                let surface = *surface;
                if buttons.is_empty() {
                    self.interaction.of_mut(t).surface = None;
                }
                let at = pointer.map_or(self.interaction.of(t).surface_at, |pointer| {
                    self.on_surface(surface, pointer)
                });
                (surface, SurfaceInput::Released { button, at })
            }
        };
        self.interaction.actions.push(Action {
            element: Some(input.0),
            kind: ActionKind::Surface(input.1),
        });
        true
    }

    /// The surface under the pointer that takes it, if any.
    fn surface_under_pointer(&self, t: TargetId) -> Option<ElementId> {
        self.pointer_target(t, |kind| matches!(kind, Kind::Surface { .. }))
            .filter(|&surface| self.in_scope(surface))
    }

    /// `pointer`, in pixels of the screen, in logical pixels from the top left corner of
    /// `surface`'s box.
    fn on_surface(&self, surface: ElementId, [x, y]: [f32; 2]) -> [f32; 2] {
        let scale = self.scale_of(surface);
        let rect = self.node(surface).map(|node| node.rect).unwrap_or_default();
        [(x - rect.x) / scale, (y - rect.y) / scale]
    }

    /// What a key's press does, as [`Input::Key`](crate::host::Input::Key) says, once the tooltip
    /// is hidden.
    fn press_key(&mut self, t: TargetId, event: &KeyEvent) -> bool {
        // Escape closes the top layer that closes as the user leaves it.
        if event.logical == Key::Named(NamedKey::Escape) && self.dismiss_top(t) {
            return true;
        }
        // The key of context menus, or Shift+F10, asks for one at the element that holds the
        // focus.
        let context = event.logical == Key::Named(NamedKey::ContextMenu)
            || (event.logical == Key::Named(NamedKey::F10)
                && self.interaction.of(t).modifiers.shift);
        if context && let Some(focus) = self.interaction.of(t).focus {
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
            && !self.interaction.of(t).modifiers.shift
            && self.toggle_menu_bar(t)
        {
            return true;
        }
        // The palette's field chooses among the commands found by the arrows and Enter.
        if self.palette_key(t, event) {
            return true;
        }
        let modal = !self.interaction.of(t).modals.is_empty();
        if event.logical == Key::Named(NamedKey::F6)
            && !modal
            && self.next_root(t, self.interaction.of(t).modifiers.shift)
        {
            return true;
        }
        if let Some(field) = self.focused_field(t)
            && self.field_key(field, event)
        {
            return true;
        }
        // A focus that a press gave leaves the keys to the program, as none does.
        let focus = self
            .interaction
            .of(t)
            .focus
            .filter(|&focus| self.takes_keys(focus));
        if let Some(focus) = focus
            && (self.slider_key(focus, event)
                || self.part_key(focus, event)
                || self.tab_key(focus, event)
                || self.split_key(focus, event)
                || self.splitter_key(focus, event)
                || self.menu_key(focus, event)
                || self.choice_key(focus, event))
        {
            return true;
        }
        if let Some(command) = self.shortcut(t, event) {
            // A held key gives its command once: a chord that shows and hides a panel does not
            // make it flicker.
            if event.repeat {
                return true;
            }
            let element = self.interaction.of(t).focus;
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
            NamedKey::Tab if focus.is_none() && !modal && !self.tab_enters => return false,
            NamedKey::Tab if self.interaction.of(t).modifiers.shift => Intent::Previous,
            NamedKey::Tab => Intent::Next,
            NamedKey::ArrowUp => Intent::Up,
            NamedKey::ArrowDown => Intent::Down,
            NamedKey::ArrowLeft => Intent::Left,
            NamedKey::ArrowRight => Intent::Right,
            NamedKey::Enter | NamedKey::Space => Intent::Accept,
            NamedKey::Escape => Intent::Cancel,
            _ => return modal,
        };
        let quiet = self.interaction.of(t).focus.is_some() && focus.is_none();
        match intent {
            // Tab goes on from the element that a press gave the focus.
            Intent::Next | Intent::Previous => self.move_focus(t, intent),
            // The other keys reach the program as with no focus: the element is not accepted, nor
            // the focus moved.
            _ if quiet => {
                let queued = match intent {
                    Intent::Accept => true,
                    Intent::Cancel => !self.dismiss_top(t),
                    _ => false,
                };
                if queued {
                    self.interaction.actions.push(Action {
                        element: None,
                        kind: ActionKind::Intent(intent),
                    });
                }
                return modal;
            }
            _ => self.act(Action {
                element: focus,
                kind: ActionKind::Intent(intent),
            }),
        }
        focus.is_some() || self.interaction.of(t).focus.is_some() || modal
    }

    /// Whether the focus that `focus` holds takes the keys that move it and its widget's own: one
    /// that the keys, an agent or the program gave, a field, one in a layer that closes as the user
    /// leaves it or in a modal root, and one whose widget's layer is open. A focus that a press
    /// gave to a button, a check box, a slider, a segment, a tab, a color well or a floating
    /// panel takes none (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).
    fn takes_keys(&self, focus: ElementId) -> bool {
        let Some(node) = self.node(focus) else {
            return false;
        };
        if node.state.focus_visible || node.field.is_some() {
            return true;
        }
        let root = self.root_of(focus).and_then(|root| self.node(root));
        let held = root.and_then(|root| root.layer.as_ref());
        root.and_then(|root| root.root)
            .is_some_and(|root| root.modal)
            || held.is_some_and(|held| held.layer.dismiss || held.layer.modal)
            || self.opened_layer(focus)
    }

    /// Gives the focus to the first element of the place after the one that holds it, or before it
    /// when `back`, then to nothing, as F6 and Shift+F6 do, from a focus that a press gave as from
    /// none: the places are the panes of each root, in the order of its tree, or the root itself
    /// where it has none (`docs/adr/0172-regions-of-tabs.md`), the top root's first, of the roots
    /// that hold an element that can take the focus, but the layers that close as the user leaves
    /// them, which close first, and the tooltip. Gives whether such a place is open, the key taken
    /// then.
    fn next_root(&mut self, t: TargetId, back: bool) -> bool {
        while self.dismiss_top(t) {}
        let tooltip = self.interaction.of(t).tooltip;
        let places: Vec<(ElementId, ElementId)> = self
            .roots_on(t)
            .into_iter()
            .rev()
            .filter(|&root| Some(root) != tooltip)
            .filter(|&root| {
                self.node(root)
                    .and_then(|node| node.layer.as_ref())
                    .is_none_or(|held| !held.layer.dismiss)
            })
            .flat_map(|root| self.places(root))
            .collect();
        if places.is_empty() {
            return false;
        }
        // A focus that a press gave counts as none.
        let focus = self
            .interaction
            .of(t)
            .focus
            .filter(|&focus| self.takes_keys(focus));
        let at = focus.and_then(|focus| {
            let place = self.place_of(focus)?;
            places.iter().position(|&(each, _)| each == place)
        });
        let next = match (at, back) {
            (None, false) => Some(0),
            (None, true) => Some(places.len() - 1),
            (Some(at), false) => Some(at + 1).filter(|&next| next < places.len()),
            (Some(at), true) => at.checked_sub(1),
        };
        self.focus_on(t, next.map(|next| places[next].1));
        true
    }

    /// The places of `root` that F6 moves the focus between, in the order of its tree, each with
    /// its first element that takes the focus: its panes that hold one, or the root itself where
    /// none does.
    fn places(&self, root: ElementId) -> Vec<(ElementId, ElementId)> {
        let focusables = self.focusables(root);
        let mut panes = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else {
                continue;
            };
            if node.style.layout.display == Display::None {
                continue;
            }
            if node.element.pane {
                panes.push(id);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        let found: Vec<(ElementId, ElementId)> = panes
            .into_iter()
            .filter_map(|pane| {
                let first = focusables
                    .iter()
                    .find(|&&focusable| self.place_of(focusable) == Some(pane))?;
                Some((pane, *first))
            })
            .collect();
        if found.is_empty() {
            focusables
                .first()
                .map(|&first| (root, first))
                .into_iter()
                .collect()
        } else {
            found
        }
    }

    /// The place that F6 counts `id` in: the nearest pane that holds it, itself included, or its
    /// root.
    fn place_of(&self, id: ElementId) -> Option<ElementId> {
        let mut at = id;
        loop {
            let node = self.node(at)?;
            if node.element.pane {
                return Some(at);
            }
            match node.parent {
                Some(parent) => at = parent,
                None => return Some(at),
            }
        }
    }

    /// Whether Tab and Shift+Tab, while no focus takes the keys, give the focus to the interface:
    /// none holds it, or one that a press gave. They do by default, as a program whose window is
    /// its interface wants, Tab going on from the element that a press gave the focus. A game turns
    /// it off, so that Tab reaches it until its interface holds a focus that takes the keys, which
    /// F6, an agent or the game gives ([`Ui::set_focus`]); a modal root takes Tab whatever it is
    /// (`docs/adr/0153-tab-and-f6-in-a-games-interface.md`).
    pub fn set_tab_enters(&mut self, enters: bool) {
        self.tab_enters = enters;
    }

    /// Does `action`, as a person's input does, for an agent or the program: a press of a button
    /// that a person could press, not disabled, drawn, and no modal root above its own, reaches the
    /// queue; the intents of navigation move the focus; `Accept` commits the field that holds the
    /// focus, which queues what it commits, if anything; it presses the button that holds it, and
    /// reaches the queue as an intent where none can be pressed; `Cancel`, commands, and a field's
    /// changes and commits reach the queue. A press of what is not such a button, or of what
    /// names nothing, is left out. An action acts on the target of its element, or on the active
    /// target without one ([`Ui::active_target`]): an intent moves that target's focus.
    pub fn act(&mut self, action: Action) {
        let t = self.action_target(&action);
        self.interaction.of_mut(t).by_pointer = false;
        self.perform(action);
    }

    /// The target that `action` acts on: its element's, or the active one without one.
    fn action_target(&self, action: &Action) -> TargetId {
        action
            .element
            .and_then(|id| self.target_of(id))
            .unwrap_or(self.interaction.active)
    }

    /// Does `action`, as [`Ui::act`] says, for any input: the pointer's, which keeps whether it was
    /// the last.
    pub(super) fn perform(&mut self, action: Action) {
        let t = self.action_target(&action);
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
                    Some(Kind::ColorWell { .. }) => self.press_well(id),
                    _ => self.interaction.actions.push(action),
                }
            }
            ActionKind::Intent(Intent::Accept) => {
                let focus = self.interaction.of(t).focus;
                if let Some(field) = self.focused_field(t) {
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
            ActionKind::Intent(Intent::Cancel) if self.dismiss_top(t) => {}
            ActionKind::Intent(Intent::Cancel)
            | ActionKind::Command(_)
            | ActionKind::Changed(_)
            | ActionKind::Committed(_)
            | ActionKind::ContextMenu { .. }
            | ActionKind::Dismissed
            | ActionKind::Dropped { .. }
            | ActionKind::Surface(_)
            | ActionKind::ClipboardText { .. } => {
                self.interaction.actions.push(action);
            }
            // An agent places a floating layer as the pointer would, at a place of finite numbers
            // and of a size above nothing.
            ActionKind::Placed { origin, size } => {
                let valid = origin.iter().all(|at| at.is_finite())
                    && size.iter().all(|side| side.is_finite() && *side > 0.0);
                let Some(root) = action.element.filter(|_| valid) else {
                    return;
                };
                let Some(Place::Floating { over, .. }) = self.layer_place(root) else {
                    return;
                };
                self.set_layer_place(root, Place::Floating { over, origin, size });
                self.interaction.actions.push(action);
            }
            ActionKind::Intent(intent) => self.move_focus(t, intent),
        }
    }

    /// The actions that the program has not taken yet, in the order they came, taken.
    pub fn take_actions(&mut self) -> Vec<Action> {
        self.drop_palette_actions();
        self.settle_picker_actions();
        std::mem::take(&mut self.interaction.actions)
    }

    /// The element that holds the focus of the active target ([`Ui::active_target`]), which the
    /// keys reach.
    pub fn focus(&self) -> Option<ElementId> {
        self.focus_in(self.interaction.active)
    }

    /// The element that holds the focus of `target`: each target keeps its own, which takes the
    /// keys given on it.
    pub fn focus_in(&self, target: TargetId) -> Option<ElementId> {
        self.interaction.of(target).focus
    }

    /// The target of the last key or button that the interface was given, [`TargetId::MAIN`]
    /// until then. What names no element follows it: the focus that [`Ui::focus`] reads and
    /// [`Ui::set_focus`] takes; the modifiers held ([`Ui::held_modifiers`]); the palette of
    /// commands and the sheet of shortcuts, which open over its first root; the program's clipboard
    /// ([`Ui::write_clipboard`], [`Ui::read_clipboard`]); an action without an element
    /// ([`Ui::act`]). A focus that the program gives does not change it
    /// (`docs/adr/0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md`).
    pub fn active_target(&self) -> TargetId {
        self.interaction.active
    }

    /// Gives the focus to `id`, a button, a field or a widget that is not disabled and is drawn, or
    /// takes it from what holds it, for `None`; a field that loses it commits. Nothing happens for an
    /// element that cannot hold it, or that a modal root above its own keeps it from. The focus
    /// that the program or an agent gives shows and takes the keys, as one that the keys gave
    /// does. An element takes the focus of its own target, which does not become active; `None`
    /// takes the active target's ([`Ui::active_target`]).
    pub fn set_focus(&mut self, id: Option<ElementId>) {
        let t = match id {
            Some(id) => match self.target_of(id) {
                Some(t) => t,
                None => return,
            },
            None => self.interaction.active,
        };
        self.interaction.of_mut(t).by_pointer = false;
        self.focus_on(t, id);
    }

    /// Gives `t`'s focus to `id`, an element of `t`, as [`Ui::set_focus`] says, for the last input:
    /// the focus that a press of the pointer gave neither shows nor takes the keys. Nothing happens
    /// for an element of another target.
    pub(super) fn focus_on(&mut self, t: TargetId, id: Option<ElementId>) {
        if let Some(id) = id
            && (!self.focusable(id) || !self.in_scope(id) || self.target_of(id) != Some(t))
        {
            return;
        }
        if let Some(before) = self.interaction.of_mut(t).focus.take() {
            // A field that the focus leaves commits what the user typed; its caret goes, and its
            // input ends.
            if Some(before) != id && self.node(before).is_some_and(|node| node.field.is_some()) {
                self.commit(before);
                self.mark_paint(before);
                self.interaction.of_mut(t).told.field_left();
                self.drop_field_reads(t);
            }
            self.change_state(before, |state| {
                state.focused = false;
                state.focus_visible = false;
            });
        }
        if let Some(id) = id {
            // The focus that the keyboard, an agent or the program gave shows; the pointer's does
            // not, as browsers show it.
            let visible = !self.interaction.of(t).by_pointer;
            self.interaction.of_mut(t).focus = Some(id);
            self.change_state(id, |state| {
                state.focused = true;
                state.focus_visible = visible;
            });
            self.mark_paint(id);
        }
    }

    /// Reads the shortcuts of `tables`, in their order, those of a later table winning over an
    /// earlier one's for one chord in one scope, as the platform resolves it ([`Ui::platform`]).
    ///
    /// # Errors
    ///
    /// [`DocumentErrors`] if a table is not a document of scopes, of chords and the names of
    /// commands, a chord names no key or modifier, or two chords of a scope are the same keys on
    /// some platform ([`Shortcuts`]): the interface keeps the shortcuts it had.
    pub fn set_shortcuts(&mut self, tables: &[Shortcuts]) -> Result<(), DocumentErrors> {
        self.interaction.bindings = shortcuts::bindings(tables)?;
        Ok(())
    }

    /// The platform whose ways the keys follow: which modifier `Primary` is, which moves a field's
    /// caret by words, and how a shortcut is written for a person ([`Ui::command_chord`]). The one
    /// that the program is built for, [`Platform::BUILT`], until [`Ui::set_platform`].
    #[must_use]
    pub fn platform(&self) -> Platform {
        self.interaction.platform
    }

    /// Follows the ways of `platform` with keys: a host that knows the system better than the
    /// build does, a program on the web, or a test of another platform's ways.
    pub fn set_platform(&mut self, platform: Platform) {
        self.interaction.platform = platform;
    }

    /// The chord of a key that gives `command`, as the platform writes it for a person, `Ctrl+S` on
    /// Linux and Windows, `⌘S` on macOS: of the scope `global` first, then of the first scope that
    /// gives it, in the order of [`Ui::shortcuts`]; a chord that a later table gives another
    /// command in its scope gives this one no more. None for a command that no key's shortcut
    /// gives: a button's chord gives none of its own ([`Ui::button_command`]).
    pub fn command_chord(&self, command: &str) -> Option<String> {
        let platform = self.interaction.platform;
        let scopes = self.shortcuts();
        let buttons: Vec<String> = self
            .interaction
            .bindings
            .iter()
            .filter(|binding| binding.chord.is_button())
            .map(|binding| binding.chord.shown(platform))
            .collect();
        let chord = |(_, chords): &(String, Vec<(String, String)>)| {
            chords
                .iter()
                .find(|(chord, each)| each == command && !buttons.contains(chord))
                .map(|(chord, _)| chord.clone())
        };
        scopes
            .iter()
            .find(|(scope, _)| scope == "global")
            .and_then(chord)
            .or_else(|| scopes.iter().find_map(chord))
    }

    /// The command that a press of `button` with `modifiers` held gives in `scope`, which the
    /// program reads itself, as an editor's viewport reads which button orbits: of the chords of
    /// the button whose modifiers are held, whatever others are, the one that names the most, of
    /// two that name as many the later in the tables. None if no chord of the button holds
    /// ([`Shortcuts`]).
    pub fn button_command(
        &self,
        scope: &str,
        button: MouseButton,
        modifiers: Modifiers,
    ) -> Option<&str> {
        self.interaction
            .bindings
            .iter()
            .rev()
            .filter(|binding| binding.scope == scope)
            .filter_map(|binding| {
                let named = binding
                    .chord
                    .holds(button, modifiers, self.interaction.platform)?;
                Some((named, binding))
            })
            .fold(
                None,
                |best: Option<(usize, &Binding)>, (named, binding)| match best {
                    Some((most, _)) if most >= named => best,
                    _ => Some((named, binding)),
                },
            )
            .map(|(_, binding)| binding.command.as_str())
    }

    /// The shortcuts that the tables give, by scope, each scope's in the order of the tables:
    /// its name, then each chord, as the platform writes it for a person ([`Ui::platform`]), and
    /// its command. A chord that a later table gives again in a scope, as the platform resolves
    /// it, is given once, with the later command.
    pub fn shortcuts(&self) -> Vec<(String, Vec<(String, String)>)> {
        let platform = self.interaction.platform;
        let mut scopes: Vec<(String, Vec<&Binding>)> = Vec::new();
        for binding in &self.interaction.bindings {
            let at = match scopes.iter().position(|(scope, _)| *scope == binding.scope) {
                Some(at) => at,
                None => {
                    scopes.push((binding.scope.clone(), Vec::new()));
                    scopes.len() - 1
                }
            };
            let chords = &mut scopes[at].1;
            chords.retain(|held| !held.chord.same(&binding.chord, platform));
            chords.push(binding);
        }
        scopes
            .into_iter()
            .map(|(scope, chords)| {
                let chords = chords
                    .into_iter()
                    .map(|binding| (binding.chord.shown(platform), binding.command.clone()))
                    .collect();
                (scope, chords)
            })
            .collect()
    }

    /// The element that the pointer would reach at `x` and `y` of `target`, in pixels of the
    /// target: the deepest drawn there, of the top root of that target that holds the point, its
    /// clip holding it too, a tooltip letting it through; none below a modal root of that target.
    /// Roots on other targets are not reached.
    pub fn element_at(&self, target: TargetId, x: f32, y: f32) -> Option<ElementId> {
        let tooltip = self.interaction.of(target).tooltip;
        for &root in self.roots.iter().rev() {
            if self.root_target(root) != target {
                continue;
            }
            // The pointer passes through a tooltip.
            if tooltip == Some(root) {
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
    pub(super) fn pointer_target(
        &self,
        t: TargetId,
        takes: impl Fn(&Kind) -> bool,
    ) -> Option<ElementId> {
        let pointer @ [x, y] = self.interaction.of(t).pointer?;
        self.target_from(self.element_at(t, x, y), pointer, takes)
    }

    /// `at`, the element under the point `x` and `y`, or its nearest ancestor, of a kind that
    /// `takes`, as [`Ui::pointer_target`] says.
    fn target_from(
        &self,
        mut at: Option<ElementId>,
        [x, y]: [f32; 2],
        takes: impl Fn(&Kind) -> bool,
    ) -> Option<ElementId> {
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
    /// leaves: a modal root is open, or the pointer lies on what the interface draws
    /// ([`Ui::draws_at`]).
    fn holds_pointer(&self, t: TargetId) -> bool {
        if !self.interaction.of(t).modals.is_empty() {
            return true;
        }
        self.interaction
            .of(t)
            .pointer
            .is_some_and(|[x, y]| self.draws_at(t, x, y))
    }

    /// Whether a press at `x` and `y` would be the interface's, a modal root open or not: the
    /// interface draws there, or a button, a field or a surface takes the pointer there.
    pub(super) fn takes_press_at(&self, t: TargetId, x: f32, y: f32) -> bool {
        if !self.interaction.of(t).modals.is_empty() || self.draws_at(t, x, y) {
            return true;
        }
        let at = self.element_at(t, x, y);
        self.target_from(at, [x, y], |kind| {
            kind.takes_pointer() || matches!(kind, Kind::Surface { .. })
        })
        .is_some()
    }

    /// Whether the point `x` and `y` lies on what the interface draws, as its last frame drew it:
    /// a scroll bar, a box with a fill, a border or a shadow, a text, where its lines lie, an image
    /// or a field. A box that only lays others out draws nothing, nor does an empty text.
    fn draws_at(&self, t: TargetId, x: f32, y: f32) -> bool {
        if self.bar_at(t, x, y).is_some() {
            return true;
        }
        let mut at = self.element_at(t, x, y);
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
    fn hover(&mut self, t: TargetId) {
        let on = self
            .interaction
            .of(t)
            .pointer
            .and_then(|[x, y]| self.element_at(t, x, y));
        self.hover_over(t, on);
    }

    /// Hovers what [`Ui::hover`] does, `on` being the element under the pointer.
    fn hover_over(&mut self, t: TargetId, on: Option<ElementId>) {
        let over = self
            .interaction
            .of(t)
            .pointer
            .and_then(|pointer| self.target_from(on, pointer, Kind::takes_pointer));
        let hovered = match self.interaction.of(t).captured {
            Some(captured) => (over == Some(captured)).then_some(captured),
            None => over,
        };
        if hovered != self.interaction.of(t).hovered {
            if let Some(before) = self.interaction.of(t).hovered {
                self.change_state(before, |state| state.hovered = false);
            }
            if let Some(now) = hovered {
                self.change_state(now, |state| state.hovered = true);
            }
            self.interaction.of_mut(t).hovered = hovered;
        }
    }

    /// The command of the shortcut whose chord `event` is, with the modifiers held.
    fn shortcut(&self, t: TargetId, event: &KeyEvent) -> Option<String> {
        let modifiers = self.interaction.of(t).modifiers;
        let mut scopes: Vec<&str> = Vec::new();
        let mut at = self.interaction.of(t).focus;
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
                .find(|binding| {
                    binding.scope == *scope
                        && binding
                            .chord
                            .matches(event, modifiers, self.interaction.platform)
                })
                .map(|binding| binding.command.clone())
        })
    }

    /// Moves `t`'s focus as `intent` says, among the elements of its root, or of the top modal
    /// root. Without a focus, among those of the top root of `t` that has any: Tab gives it to the
    /// first, Shift+Tab to the last, and the arrows, which a game moves with, give it to none.
    fn move_focus(&mut self, t: TargetId, intent: Intent) {
        let focus = self.interaction.of(t).focus;
        let scopes: Vec<ElementId> = match self.interaction.of(t).modals.last() {
            // Within the root of the focus, a layer above the modal root, or else the modal root.
            Some(&(modal, _)) => match focus.filter(|&focus| self.in_scope(focus)) {
                Some(focus) => self.root_of(focus).into_iter().collect(),
                None => vec![modal],
            },
            None => match focus.and_then(|focus| self.root_of(focus)) {
                Some(root) => vec![root],
                None => self.roots_on(t).into_iter().rev().collect(),
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
            self.focus_on(t, Some(first));
            self.scroll_into_view(first);
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
            self.focus_on(t, Some(next));
            self.scroll_into_view(next);
        }
    }

    /// Whether `id` lies in a root that no modal root above keeps the focus from: the top modal
    /// root of its target, or a root above it.
    pub(super) fn in_scope(&self, id: ElementId) -> bool {
        // Most interfaces hold no modal root: no walk to the root then.
        if !self.interaction.any_modal() {
            return true;
        }
        let Some(t) = self.target_of(id) else {
            return true;
        };
        match self.interaction.of(t).modals.last() {
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
                        | Kind::ColorWell { .. }
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

    /// Takes the focus from the roots below when `root`, a modal one, opens: on its own target,
    /// whose roots below it take no input while it is there.
    pub(super) fn opened_modal(&mut self, root: ElementId) {
        let Some(t) = self.target_of(root) else {
            return;
        };
        let before = self.interaction.of(t).focus;
        self.focus_on(t, None);
        self.interaction.of_mut(t).modals.push((root, before));
        // What every element of the roots below takes changed.
        self.meaning_whole_on(t);
    }

    /// Forgets what `removed`, an element taken out of the tree with what it held, leaves behind
    /// on every target, since its root can no longer be found: the hover, the pointer held, the
    /// focus; a modal root closed gives back the focus it took.
    pub(super) fn forget(&mut self, removed: ElementId) {
        let mut index = 0;
        while let Some(t) = self.interaction.target_at(index) {
            index += 1;
            self.forget_on(t, removed);
        }
    }

    /// What [`Ui::forget`] does on `t`.
    fn forget_on(&mut self, t: TargetId, removed: ElementId) {
        self.interaction.of_mut(t).moved = true;
        // A field that the focus leaves with its element drops the reads it asked, as one that
        // the focus leaves otherwise does.
        if self
            .interaction
            .of(t)
            .focus
            .is_some_and(|id| !alive(&self.slots, id))
        {
            self.drop_field_reads(t);
        }
        let input = self.interaction.of_mut(t);
        for id in [&mut input.hovered, &mut input.captured, &mut input.focus] {
            if id.is_some_and(|id| !alive(&self.slots, id)) {
                *id = None;
            }
        }
        // A press that had not yet become a drag ends with its element; a drag goes on.
        if input
            .drag
            .is_some_and(|(id, _, started)| !started && !alive(&self.slots, id))
        {
            input.drag = None;
        }
        self.modal_gone(t, removed);
    }

    /// The modal root `root` left `t`, removed or moved to another target: it leaves the focus
    /// that it took to the modal root above it, when what that one took went, or gives it back
    /// when it was the top one and nothing holds the focus. Gives whether `root` was one of `t`'s
    /// modal roots.
    fn modal_gone(&mut self, t: TargetId, root: ElementId) -> bool {
        let input = self.interaction.of_mut(t);
        let Some(at) = input.modals.iter().position(|&(each, _)| each == root) else {
            return false;
        };
        let (_, before) = input.modals.remove(at);
        self.meaning_whole_on(t);
        let input = self.interaction.of_mut(t);
        match input.modals.get_mut(at) {
            Some((_, above)) => {
                if above.is_none_or(|id| !alive(&self.slots, id)) {
                    *above = before;
                }
            }
            None => {
                if input.focus.is_none() {
                    self.focus_on(t, before);
                }
            }
        }
        true
    }

    /// Moves `group`, a root and the layers that stand over what it holds ([`Ui::with_layers_over`]),
    /// from the target `from` to `to`, at once. What `from`'s input held in them lets go first
    /// ([`Ui::leave_target`]); then their target changes; then each modal root among them leaves
    /// `from`'s stack, giving back the focus that it took, which no element that left can take,
    /// and joins `to`'s. A group without a modal root stands below `to`'s lowest modal root, so
    /// that a dialog keeps its window.
    pub(super) fn move_to_target(&mut self, group: &[ElementId], from: TargetId, to: TargetId) {
        self.leave_target(from, group);
        self.interaction.of_mut(to);
        // The old target draws them no more.
        self.changed_on(from);
        for &root in group {
            if let Some(settings) = self.node_mut(root).and_then(|node| node.root.as_mut()) {
                settings.target = to;
            }
            self.draw_on(root, to);
            self.mark_layout(root);
            // It leaves the old target's tree of meaning and joins the new one's whole.
            self.mark_meaning_below(root);
        }
        self.top_changed_on(from);
        self.top_changed_on(to);
        let modals: Vec<ElementId> = group
            .iter()
            .copied()
            .filter(|&root| self.modal_gone(from, root))
            .collect();
        if modals.is_empty() {
            self.stand_below_modal(group, to);
        }
        for modal in modals {
            self.opened_modal(modal);
        }
    }

    /// Places `group`'s roots, in their order, right below the lowest modal root of `to`, when
    /// they stood above it.
    fn stand_below_modal(&mut self, group: &[ElementId], to: TargetId) {
        let place = |ui: &Self, root: ElementId| ui.roots.iter().position(|&each| each == root);
        let Some(lowest) = self
            .interaction
            .of(to)
            .modals
            .iter()
            .filter_map(|&(modal, _)| place(self, modal))
            .min()
        else {
            return;
        };
        if group.first().and_then(|&root| place(self, root)) < Some(lowest) {
            return;
        }
        self.roots.retain(|root| !group.contains(root));
        let at = self
            .interaction
            .of(to)
            .modals
            .iter()
            .filter_map(|&(modal, _)| place(self, modal))
            .min()
            .unwrap_or(self.roots.len());
        self.roots.splice(at..at, group.iter().copied());
        self.top_changed_on(to);
    }

    /// `going`, a root and the layers that stand over what it holds, leave the target `from`
    /// for another: what `from`'s input held in them lets go, as if that input had left them. A
    /// held control commits, a surface is told its buttons were let go, a field that held the
    /// focus commits and its input ends, and the hover and the rest leave them.
    fn leave_target(&mut self, from: TargetId, going: &[ElementId]) {
        let inside =
            |ui: &Self, id: ElementId| ui.root_of(id).is_some_and(|root| going.contains(&root));
        let input = self.interaction.of(from);
        let (captured, drag, bar, panel, surface, hovered, focus, tooltip) = (
            input.captured,
            input.drag,
            input.bar,
            input.panel,
            input.surface.clone(),
            input.hovered,
            input.focus,
            input.tooltip,
        );
        let rest = self.resting_on(from);
        if let Some(captured) = captured.filter(|&id| inside(self, id)) {
            self.interaction.of_mut(from).captured = None;
            self.change_state(captured, |state| state.pressed = false);
            self.field_press_lost(captured);
            if let Some(value) = self.slider_value(captured) {
                self.set_slider(captured, value, true);
            }
            let _ = self.part_release(captured) || self.splitter_release(captured);
        }
        if drag.is_some_and(|(id, _, _)| inside(self, id)) {
            self.interaction.of_mut(from).drag = None;
        }
        if bar.is_some_and(|held| inside(self, held.element)) {
            self.interaction.of_mut(from).bar = None;
        }
        if let Some(grab) = panel.filter(|grab| going.contains(&grab.root)) {
            self.interaction.of_mut(from).panel = None;
            self.panel_placed(grab);
        }
        if let Some((element, buttons)) = surface.filter(|(id, _)| inside(self, *id)) {
            let at = self.interaction.of(from).surface_at;
            self.interaction.of_mut(from).surface = None;
            for button in buttons {
                self.interaction.actions.push(Action {
                    element: Some(element),
                    kind: ActionKind::Surface(SurfaceInput::Released { button, at }),
                });
            }
        }
        if let Some(hovered) = hovered.filter(|&id| inside(self, id)) {
            self.interaction.of_mut(from).hovered = None;
            self.change_state(hovered, |state| state.hovered = false);
        }
        if focus.is_some_and(|id| inside(self, id)) {
            self.focus_on(from, None);
        }
        if tooltip.is_some_and(|tip| going.contains(&tip)) {
            self.hide_tooltip(from);
        }
        if rest.is_some_and(|id| inside(self, id)) {
            self.interaction.of_mut(from).rest = super::tooltip::Rest::default();
        }
    }

    /// Keeps `t`'s input's states true to the tree, as its frame begins and its styles are made:
    /// a focus that its element can no longer hold is dropped, the top modal root takes the focus
    /// when nothing of its own holds it, and the hover follows what moved under the pointer.
    pub(super) fn settle_input(&mut self, t: TargetId) {
        if !self.interaction.knows(t) {
            return;
        }
        if let Some(focus) = self.interaction.of(t).focus
            && !self.focusable(focus)
        {
            self.focus_on(t, None);
        }
        if let Some(&(modal, _)) = self.interaction.of(t).modals.last() {
            let inside = self
                .interaction
                .of(t)
                .focus
                .is_some_and(|focus| self.root_of(focus) == Some(modal));
            if !inside && let Some(&first) = self.focusables(modal).first() {
                self.focus_on(t, Some(first));
            }
        }
        if std::mem::take(&mut self.interaction.of_mut(t).moved) {
            self.hover(t);
        }
    }

    /// Marks what lies under `t`'s pointer to be found again as its next frame begins: its roots
    /// were laid out or scrolled.
    pub(super) fn moved(&mut self, t: TargetId) {
        if self.interaction.knows(t) {
            self.interaction.of_mut(t).moved = true;
        }
    }
}

/// Whether `id` names an element of the tree that `slots` hold.
fn alive(slots: &[super::Slot], id: ElementId) -> bool {
    slots
        .get(id.index as usize)
        .is_some_and(|slot| slot.generation == id.generation && slot.node.is_some())
}

/// Whether `node`'s box holds `x` and `y`, in pixels of the screen, its clip holding them too.
fn holds(node: &Node, x: f32, y: f32) -> bool {
    inside(node.rect, x, y) && node.clip.is_none_or(|clip| inside(clip, x, y))
}

/// Whether `rect` holds `x` and `y`.
pub(super) fn inside(rect: crate::Rect, x: f32, y: f32) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}
