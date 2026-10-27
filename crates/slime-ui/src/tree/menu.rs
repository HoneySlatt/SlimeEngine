//! Menus and their items (`docs/adr/0150-layers.md`): the keys that move among a menu's items and
//! open and close its submenus, and what a press on an item closes.

use slime_input::{Key, KeyEvent, NamedKey};

use super::{ElementId, ItemMark, Kind, LayoutDirection, Place, Ui};
use crate::action::{Action, ActionKind};

impl Ui {
    /// A press of the item of a menu `id`: an option of a list to choose from is chosen; an item
    /// that opens a submenu reaches the program, which opens it, the submenus of the other items of
    /// its menu closing, or gives the submenu open the focus, to the keyboard; another reaches the
    /// program and closes the menus that led to it.
    pub(super) fn press_menu_item(&mut self, id: ElementId) {
        if let Some(owner) = self.option_owner(id) {
            self.press_option(owner, id);
            return;
        }
        let Some(Kind::MenuItem { mark, .. }) = self.node(id).map(|node| &node.element.kind) else {
            return;
        };
        if *mark != ItemMark::Submenu {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
            self.close_menus(id);
            return;
        }
        if let Some(submenu) = self.submenu_of(id) {
            if !self.interaction.by_pointer
                && let Some(first) = self.first_focusable(submenu)
            {
                self.set_focus(Some(first));
            }
            return;
        }
        let root = self.root_of(id);
        for layer in self.layers().into_iter().rev() {
            if let Some(Place::Beside { element, .. }) = self.layer_place(layer)
                && element != id
                && self.root_of(element) == root
                && self.is_menu_item(element)
            {
                self.close_layer(layer, true);
            }
        }
        self.interaction.actions.push(Action {
            element: Some(id),
            kind: ActionKind::Press,
        });
    }

    /// A key reached the item of a menu `id`, which holds the focus: the arrows up and down move
    /// the focus among the items of its menu, around, Home and End to the first and the last; the
    /// arrow towards the end of a line presses an item that opens a submenu, and the arrow towards
    /// its start closes the submenu that holds the item, which an item opened, the focus going back
    /// to that item. Gives whether it took the key.
    pub(super) fn menu_key(&mut self, id: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = event.logical else {
            return false;
        };
        let Some(Kind::MenuItem { mark, .. }) = self.node(id).map(|node| &node.element.kind) else {
            return false;
        };
        let submenu = *mark == ItemMark::Submenu;
        let (forward, backward) = match self.root_settings(id).map(|root| root.direction) {
            Some(LayoutDirection::RightToLeft) => (NamedKey::ArrowLeft, NamedKey::ArrowRight),
            _ => (NamedKey::ArrowRight, NamedKey::ArrowLeft),
        };
        if named == forward && submenu {
            self.perform(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
            return true;
        }
        if named == backward
            && let Some(root) = self.root_of(id)
            && self.opened_by_item(root)
        {
            self.close_layer(root, true);
            return true;
        }
        let Some(menu) = self.menu_of(id) else {
            return false;
        };
        let items: Vec<ElementId> = self
            .focusables(menu)
            .into_iter()
            .filter(|&item| self.is_menu_item(item))
            .collect();
        let Some(at) = items.iter().position(|&item| item == id) else {
            return false;
        };
        let count = items.len();
        let next = match named {
            NamedKey::ArrowDown => (at + 1) % count,
            NamedKey::ArrowUp => (at + count - 1) % count,
            NamedKey::Home => 0,
            NamedKey::End => count - 1,
            _ => return false,
        };
        self.set_focus(Some(items[next]));
        true
    }

    /// Closes the menus that led to the item `id`: the layer that holds it, and those that hold
    /// the items that opened it, down to the first, with the layers over them.
    fn close_menus(&mut self, id: ElementId) {
        let mut first = None;
        let mut at = self.root_of(id);
        while let Some(root) = at.filter(|&root| self.layer_place(root).is_some()) {
            first = Some(root);
            at = match self.layer_place(root) {
                Some(Place::Beside { element, .. }) if self.is_menu_item(element) => {
                    self.root_of(element)
                }
                _ => None,
            };
        }
        if let Some(first) = first {
            self.close_layer(first, true);
        }
    }

    /// The layer open beside the item `id`: its submenu.
    fn submenu_of(&self, id: ElementId) -> Option<ElementId> {
        self.layers().into_iter().find(|&layer| {
            matches!(self.layer_place(layer), Some(Place::Beside { element, .. }) if element == id)
        })
    }

    /// Whether the layer `root` stands beside an item of a menu: a submenu.
    fn opened_by_item(&self, root: ElementId) -> bool {
        matches!(self.layer_place(root), Some(Place::Beside { element, .. }) if self.is_menu_item(element))
    }

    /// The menu that holds `id`: its nearest ancestor that is one.
    fn menu_of(&self, id: ElementId) -> Option<ElementId> {
        let mut at = self.node(id)?.parent;
        while let Some(each) = at {
            let node = self.node(each)?;
            if matches!(node.element.kind, Kind::Menu) {
                return Some(each);
            }
            at = node.parent;
        }
        None
    }

    /// Whether `id` is an item of a menu.
    pub(super) fn is_menu_item(&self, id: ElementId) -> bool {
        self.node(id)
            .is_some_and(|node| matches!(node.element.kind, Kind::MenuItem { .. }))
    }
}
