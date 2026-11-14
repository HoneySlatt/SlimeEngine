//! Tooltips, and what the pointer resting does (`docs/adr/0150-layers.md`): an element's tooltip
//! shows below it once the pointer has rested on it for half a second of the host's clock, or
//! once the keys gave it the focus, in a layer that the pointer passes through, and goes as the
//! pointer leaves it, presses, or a key goes down; an item of a menu that the pointer rests on opens
//! its submenu, or closes those of the other items.

use std::time::Duration;

use super::{Along, Element, ElementId, ItemMark, Kind, Layer, Place, Role, Side, Ui};
use crate::action::{Action, ActionKind};
use crate::host::TargetId;
use crate::style::FlexDirection;
use crate::time::Instant;

/// How long the pointer rests on an element before its tooltip shows.
const TOOLTIP_REST: Duration = Duration::from_millis(500);

/// How long the pointer rests on an item of a menu before its submenu opens, or the others' close.
const SUBMENU_REST: Duration = Duration::from_millis(250);

/// Where the pointer rests: the deepest element under it, and since when, on the host's clock.
#[derive(Default)]
pub(super) struct Rest {
    on: Option<ElementId>,
    since: Instant,
    /// Whether resting there showed its tooltip, or the input left it shown no more.
    tip_done: bool,
    /// Whether resting there opened a submenu, or closed the others'.
    menu_done: bool,
}

impl Ui {
    /// `t`'s pointer moved over `on`, the deepest element under it, if any: resting starts again
    /// where it changed, and the tooltip of an element that it left goes.
    pub(super) fn rest_on(&mut self, t: TargetId, on: Option<ElementId>) {
        if on == self.interaction.of(t).rest.on {
            return;
        }
        self.interaction.of_mut(t).rest = Rest {
            on,
            since: self.now,
            tip_done: false,
            menu_done: false,
        };
        let owner = self
            .interaction
            .of(t)
            .tooltip
            .and_then(|tip| self.layer_place(tip))
            .map(|place| match place {
                Place::Beside { element, .. } => element,
                Place::At { over, .. } | Place::Floating { over, .. } => over,
            });
        if let Some(owner) = owner
            && on.is_none_or(|on| !self.within(on, owner))
        {
            self.hide_tooltip(t);
        }
    }

    /// The element that `t`'s pointer rests on, the deepest under it.
    pub(super) fn resting_on(&self, t: TargetId) -> Option<ElementId> {
        self.interaction.of(t).rest.on
    }

    /// The pointer pressed on `t`, or a key went down: its tooltip goes, and resting where the
    /// pointer is shows it no more. Gives whether one showed.
    pub(super) fn input_hides_tooltip(&mut self, t: TargetId) -> bool {
        self.interaction.of_mut(t).rest.tip_done = true;
        self.hide_tooltip(t)
    }

    /// How long a pointer must rest still before a tooltip shows or a submenu opens where it
    /// rests, on the host's clock, the soonest of every target's; none when each rests where
    /// neither will.
    pub(crate) fn rest_remaining(&self) -> Option<Duration> {
        self.interaction
            .targets()
            .filter_map(|t| self.rest_remaining_on(t))
            .min()
    }

    /// What [`Ui::rest_remaining`] says of `t`'s pointer.
    fn rest_remaining_on(&self, t: TargetId) -> Option<Duration> {
        let rest = &self.interaction.of(t).rest;
        let on = rest.on?;
        let rested = self.now - rest.since;
        let menu = (!rest.menu_done && self.is_menu_item(on)).then_some(SUBMENU_REST);
        let tip = (!rest.tip_done && self.tooltip_of(on).is_some()).then_some(TOOLTIP_REST);
        menu.into_iter()
            .chain(tip)
            .min()
            .map(|rest| rest.saturating_sub(rested))
    }

    /// What each target's pointer resting does once it has rested long enough on the host's
    /// clock: an item of a menu opens its submenu, or closes the others' in its menu; the tooltip
    /// of the element, or of its nearest ancestor that has one, shows. Run before a frame lays
    /// out.
    pub(crate) fn settle_rest(&mut self) {
        let mut index = 0;
        while let Some(t) = self.interaction.target_at(index) {
            index += 1;
            self.settle_rest_on(t);
        }
    }

    /// What [`Ui::settle_rest`] does on `t`.
    fn settle_rest_on(&mut self, t: TargetId) {
        let Some(on) = self.interaction.of(t).rest.on else {
            return;
        };
        let rested = self.now - self.interaction.of(t).rest.since;
        if !self.interaction.of(t).rest.menu_done && rested >= SUBMENU_REST {
            self.interaction.of_mut(t).rest.menu_done = true;
            self.rest_on_item(t, on);
        }
        if !self.interaction.of(t).rest.tip_done && rested >= TOOLTIP_REST {
            self.interaction.of_mut(t).rest.tip_done = true;
            if let Some(owner) = self.tooltip_of(on) {
                self.show_tooltip(t, owner);
            }
        }
    }

    /// The pointer rested on `id`: an item of a menu, not disabled, that opens a submenu presses
    /// it, as a press of the pointer would; another closes the submenus of its menu's other items.
    fn rest_on_item(&mut self, t: TargetId, id: ElementId) {
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
            self.interaction.of_mut(t).by_pointer = true;
            self.perform(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
        } else {
            self.close_other_submenus(id);
        }
    }

    /// Shows the tooltip of `owner`, an element of `t`, below it, the one that showed on `t`
    /// going.
    pub(super) fn show_tooltip(&mut self, t: TargetId, owner: ElementId) {
        self.hide_tooltip(t);
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
        let root = self.add_quiet_layer("tooltip", layer, |s| s);
        self.build(root, 0, |children| {
            let tip = Element::container()
                .style(|s| s.direction(FlexDirection::Column))
                .class("tooltip");
            children.add("tooltip", tip, |tip| {
                // Screen readers read it as its element's description, not twice.
                if !tooltip.title.is_empty() {
                    let title = Element::label(tooltip.title)
                        .class("tooltip-title")
                        .role(Role::GenericContainer);
                    tip.add("title", title, |_| {});
                }
                let text = Element::label(tooltip.text).role(Role::GenericContainer);
                tip.add("text", text, |_| {});
            });
        });
        self.interaction.of_mut(t).tooltip = Some(root);
    }

    /// Hides the tooltip that shows on `t`: whether one did.
    pub(super) fn hide_tooltip(&mut self, t: TargetId) -> bool {
        let Some(root) = self.interaction.of(t).tooltip else {
            return false;
        };
        self.close_layer(root, false);
        self.interaction.of_mut(t).tooltip = None;
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
