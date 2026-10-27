//! Lists to choose from (`docs/adr/0150-layers.md`): the list of options that one opens below it,
//! in a layer that it owns, a menu whose items are its options; the keys that choose while it is
//! closed; the choice, which the program reads as actions.

use slime_input::{Key, KeyEvent, NamedKey};

use super::control::Control;
use super::{Along, Element, ElementId, ItemMark, Kind, Layer, Place, Role, Side, Ui};
use crate::action::{Action, ActionKind, FieldValue};
use crate::style::{Dimension, Style};

impl Ui {
    /// A press of the list to choose from `id`: its list opens below it, or closes unchanged when
    /// it is open.
    pub(super) fn press_choice(&mut self, id: ElementId) {
        if let Some(list) = self.owned_layer(id) {
            self.close_layer(list, true);
            return;
        }
        let Some(Kind::Choice { options, .. }) = self.node(id).map(|node| &node.element.kind)
        else {
            return;
        };
        let options = options.clone();
        let chosen = self.chosen(id).unwrap_or(0);
        let width = self.node(id).map_or(0.0, |node| node.rect.width) / self.scale_of(id);
        let by_pointer = self.interaction.by_pointer;
        let place = Place::Beside {
            element: id,
            side: Side::Below,
            along: Along::Start,
        };
        let layer = Layer {
            place,
            modal: false,
            dismiss: true,
        };
        let root = self.open_owned_layer(id, layer, Style::default());
        // As wide as the list to choose from, at least; read as a list of options.
        let mut style = Style::default();
        style.layout.min_size[0] = Dimension::Pixels(width);
        let mut items = Vec::new();
        self.build(root, 0, |children| {
            let menu = Element::menu(style).role(Role::ListBox);
            children.add("options", menu, |menu| {
                for (place, option) in options.iter().enumerate() {
                    let mark = ItemMark::Check(place == chosen);
                    let item = Element::menu_item(option.as_str(), "", mark, Style::default())
                        .role(Role::ListBoxOption);
                    items.push(menu.add(place, item, |_| {}));
                }
            });
        });
        for (place, &item) in items.iter().enumerate() {
            self.change_state(item, |state| state.selected = place == chosen);
        }
        self.change_state(id, |state| state.expanded = true);
        // The keyboard finds the chosen option under the focus.
        if !by_pointer && let Some(&item) = items.get(chosen) {
            self.set_focus(Some(item));
        }
    }

    /// A press of the item of a menu `item` in the list that the list to choose from `owner` opened:
    /// the option is chosen, and the list closes.
    pub(super) fn press_option(&mut self, owner: ElementId, item: ElementId) {
        let place = self
            .node(item)
            .and_then(|node| node.parent)
            .map(|menu| self.children(menu))
            .and_then(|items| items.iter().position(|&each| each == item));
        if let Some(root) = self.root_of(item) {
            self.close_layer(root, false);
        }
        if let Some(place) = place {
            self.choose(owner, place);
        }
    }

    /// A key reached the list to choose from `id`, which holds the focus: closed, the arrows up and
    /// down choose the option before or after the chosen one, Home and End the first and the last,
    /// and Alt+Down or F4 open its list; open, the arrows up and down give the chosen option the
    /// focus. Gives whether it took the key.
    pub(super) fn choice_key(&mut self, id: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = event.logical else {
            return false;
        };
        let Some(Kind::Choice { options, .. }) = self.node(id).map(|node| &node.element.kind)
        else {
            return false;
        };
        let last = options.len().saturating_sub(1);
        let chosen = self.chosen(id).unwrap_or(0);
        // Open, the arrows go into its list, on the chosen option.
        if let Some(list) = self.owned_layer(id) {
            if !matches!(named, NamedKey::ArrowDown | NamedKey::ArrowUp) {
                return false;
            }
            let options = self.children(list).first().map(|&menu| self.children(menu));
            if let Some(&item) = options.and_then(|items| items.get(chosen)) {
                self.set_focus(Some(item));
            }
            return true;
        }
        let alt = self.interaction.modifiers.alt;
        let place = match named {
            NamedKey::ArrowDown if alt => {
                self.press_choice(id);
                return true;
            }
            NamedKey::F4 => {
                self.press_choice(id);
                return true;
            }
            NamedKey::ArrowDown => (chosen + 1).min(last),
            NamedKey::ArrowUp => chosen.saturating_sub(1),
            NamedKey::Home => 0,
            NamedKey::End => last,
            _ => return false,
        };
        self.choose(id, place);
        true
    }

    /// Chooses the option at `place` of the list to choose from `id`, as the user does: the program
    /// reads [`ActionKind::Changed`] then [`ActionKind::Committed`] when the choice changed. Gives
    /// whether `id` is a list to choose from and `place` one of its options.
    pub(crate) fn choose(&mut self, id: ElementId, place: usize) -> bool {
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let Kind::Choice { options, .. } = &node.element.kind else {
            return false;
        };
        if place >= options.len() {
            return false;
        }
        let Some(Control::Choice { chosen, .. }) = node.control.as_mut() else {
            return false;
        };
        if *chosen == place {
            return true;
        }
        *chosen = place;
        self.mark_paint(id);
        self.mark_meaning(id);
        for kind in [ActionKind::Changed, ActionKind::Committed] {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: kind(FieldValue::Chosen(place)),
            });
        }
        true
    }

    /// The list to choose from that opened the layer that holds `item`, if one did.
    pub(super) fn option_owner(&self, item: ElementId) -> Option<ElementId> {
        let owner = self.layer_owner(self.root_of(item)?)?;
        self.node(owner)
            .is_some_and(|node| matches!(node.element.kind, Kind::Choice { .. }))
            .then_some(owner)
    }
}
