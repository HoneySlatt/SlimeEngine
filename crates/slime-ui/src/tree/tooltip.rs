//! Tooltips, and what the pointer resting does (`docs/adr/0150-layers.md`): an element's tooltip
//! shows below it once the pointer has rested on it for half a second of the interface's clock, or
//! once the keys gave it the focus, in a layer that the pointer passes through, and goes as the
//! pointer leaves it, presses, or a key goes down; an item of a menu that the pointer rests on opens
//! its submenu, or closes those of the other items.

use std::time::Duration;

use super::{Along, Element, ElementId, ItemMark, Kind, Layer, Place, Role, Side, Ui};
use crate::action::{Action, ActionKind};
use crate::style::{FlexDirection, Style};

/// How long the pointer rests on an element before its tooltip shows.
const TOOLTIP_REST: Duration = Duration::from_millis(500);

/// How long the pointer rests on an item of a menu before its submenu opens, or the others' close.
const SUBMENU_REST: Duration = Duration::from_millis(250);

/// Where the pointer rests: the deepest element under it, and since when, on the interface's clock.
#[derive(Default)]
pub(super) struct Rest {
    on: Option<ElementId>,
    since: Duration,
    /// Whether resting there showed its tooltip, or the input left it shown no more.
    tip_done: bool,
    /// Whether resting there opened a submenu, or closed the others'.
    menu_done: bool,
}

impl Ui {
    /// The pointer moved over `on`, the deepest element under it, if any: resting starts again
    /// where it changed, and the tooltip of an element that it left goes.
    pub(super) fn rest_on(&mut self, on: Option<ElementId>) {
        if on == self.interaction.rest.on {
            return;
        }
        self.interaction.rest = Rest {
            on,
            since: self.now,
            tip_done: false,
            menu_done: false,
        };
        let owner = self
            .interaction
            .tooltip
            .and_then(|tip| self.layer_place(tip))
            .map(|place| match place {
                Place::Beside { element, .. } => element,
                Place::At { over, .. } | Place::Floating { over, .. } => over,
            });
        if let Some(owner) = owner
            && on.is_none_or(|on| !self.within(on, owner))
        {
            self.hide_tooltip();
        }
    }

    /// The element that the pointer rests on, the deepest under it.
    pub(super) fn resting_on(&self) -> Option<ElementId> {
        self.interaction.rest.on
    }

    /// The pointer pressed, or a key went down: the tooltip goes, and resting where the pointer is
    /// shows it no more. Gives whether one showed.
    pub(super) fn input_hides_tooltip(&mut self) -> bool {
        self.interaction.rest.tip_done = true;
        self.hide_tooltip()
    }

    /// Whether the pointer rests where a tooltip will show or a submenu open, which the clock must
    /// move on for.
    pub(crate) fn resting(&self) -> bool {
        let rest = &self.interaction.rest;
        let Some(on) = rest.on else {
            return false;
        };
        (!rest.tip_done && self.tooltip_of(on).is_some())
            || (!rest.menu_done && self.is_menu_item(on))
    }

    /// What the pointer resting does once it has rested long enough on the interface's clock: an
    /// item of a menu opens its submenu, or closes the others' in its menu; the tooltip of the
    /// element, or of its nearest ancestor that has one, shows. Run before a frame lays out.
    pub(crate) fn settle_rest(&mut self) {
        let Some(on) = self.interaction.rest.on else {
            return;
        };
        let rested = self.now.saturating_sub(self.interaction.rest.since);
        if !self.interaction.rest.menu_done && rested >= SUBMENU_REST {
            self.interaction.rest.menu_done = true;
            self.rest_on_item(on);
        }
        if !self.interaction.rest.tip_done && rested >= TOOLTIP_REST {
            self.interaction.rest.tip_done = true;
            if let Some(owner) = self.tooltip_of(on) {
                self.show_tooltip(owner);
            }
        }
    }

    /// The pointer rested on `id`: an item of a menu, not disabled, that opens a submenu presses
    /// it, as a press of the pointer would; another closes the submenus of its menu's other items.
    fn rest_on_item(&mut self, id: ElementId) {
        let Some(node) = self.node(id) else {
            return;
        };
        let Kind::MenuItem { mark, .. } = node.element.kind else {
            return;
        };
        // A bar of menus opens its menus as the pointer presses, not as it rests.
        if node.state.disabled || !self.in_scope(id) || self.in_menu_bar(id) {
            return;
        }
        if mark == ItemMark::Submenu {
            self.interaction.by_pointer = true;
            self.perform(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
        } else {
            self.close_other_submenus(id);
        }
    }

    /// Shows the tooltip of `owner` below it, the one that showed going.
    pub(super) fn show_tooltip(&mut self, owner: ElementId) {
        self.hide_tooltip();
        let Some(tooltip) = self
            .node(owner)
            .and_then(|node| node.element.tooltip.clone())
        else {
            return;
        };
        let layer = Layer {
            place: Place::Beside {
                element: owner,
                side: Side::Below,
                along: Along::Center,
            },
            modal: false,
            dismiss: false,
        };
        let root = self.add_quiet_layer(layer, Style::default());
        self.build(root, 0, |children| {
            let mut column = Style::default();
            column.layout.direction = FlexDirection::Column;
            let tip = Element::container(column).class("tooltip");
            children.add("tooltip", tip, |tip| {
                // Screen readers read it as its element's description, not twice.
                if !tooltip.title.is_empty() {
                    let title = Element::label(tooltip.title, Style::default())
                        .class("tooltip-title")
                        .role(Role::GenericContainer);
                    tip.add("title", title, |_| {});
                }
                let text =
                    Element::label(tooltip.text, Style::default()).role(Role::GenericContainer);
                tip.add("text", text, |_| {});
            });
        });
        self.interaction.tooltip = Some(root);
    }

    /// Hides the tooltip that shows: whether one did.
    pub(super) fn hide_tooltip(&mut self) -> bool {
        let Some(root) = self.interaction.tooltip else {
            return false;
        };
        self.close_layer(root, false);
        self.interaction.tooltip = None;
        true
    }

    /// The element that holds a tooltip nearest to `id`: itself or an ancestor.
    fn tooltip_of(&self, id: ElementId) -> Option<ElementId> {
        let mut at = Some(id);
        while let Some(each) = at {
            let node = self.node(each)?;
            if node.element.tooltip.is_some() {
                return Some(each);
            }
            at = node.parent;
        }
        None
    }
}
