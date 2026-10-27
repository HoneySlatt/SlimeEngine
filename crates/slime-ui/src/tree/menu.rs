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
            // The pointer closes the menu of a bar's item.
            if self.interaction.by_pointer && self.in_menu_bar(id) {
                self.close_layer(submenu, true);
            } else if !self.interaction.by_pointer
                && let Some(first) = self.first_focusable(submenu)
            {
                self.set_focus(Some(first));
            }
            return;
        }
        self.close_other_submenus(id);
        self.interaction.actions.push(Action {
            element: Some(id),
            kind: ActionKind::Press,
        });
    }

    /// Closes the submenus that the other items of the menu of the item `id` opened.
    pub(super) fn close_other_submenus(&mut self, id: ElementId) {
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
        if self.in_menu_bar(id) {
            return self.bar_key(id, named, forward, backward);
        }
        // In a menu of a bar, the arrows towards a line's start and end move to the bar's other
        // menus, but on an item that opens a submenu.
        if let Some(bar_item) = self.root_of(id).and_then(|root| self.bar_item_of(root)) {
            if named == backward {
                self.bar_step(bar_item, -1, true);
                return true;
            }
            if named == forward && !submenu {
                self.bar_step(bar_item, 1, true);
                return true;
            }
        }
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
            let from_bar = self.bar_item_of(first).is_some();
            self.close_layer(first, true);
            // A command of a bar's menu gives the focus back to what held it before the keys
            // reached the bar.
            if from_bar && let Some(back) = self.interaction.bar_before.take() {
                self.set_focus(Some(back));
            }
        }
    }

    /// A key reached the item of a bar of menus `id`, which holds the focus: the arrows towards a
    /// line's end and start move to the next item and the one before, around, opening its menu
    /// while one is open; Home and End to the first and the last; the arrows up and down open its
    /// menu; Escape gives the focus back to what held it before the keys reached the bar. Gives
    /// whether it took the key.
    fn bar_key(
        &mut self,
        id: ElementId,
        named: NamedKey,
        forward: NamedKey,
        backward: NamedKey,
    ) -> bool {
        let open = self
            .bar_items(id)
            .iter()
            .any(|&item| self.submenu_of(item).is_some());
        match named {
            _ if named == forward => self.bar_step(id, 1, open),
            _ if named == backward => self.bar_step(id, -1, open),
            NamedKey::Home | NamedKey::End => {
                let items = self.bar_items(id);
                let item = if named == NamedKey::Home {
                    items.first()
                } else {
                    items.last()
                };
                self.set_focus(item.copied());
            }
            NamedKey::ArrowDown | NamedKey::ArrowUp => self.perform(Action {
                element: Some(id),
                kind: ActionKind::Press,
            }),
            NamedKey::Escape => {
                let back = self.interaction.bar_before.take();
                self.set_focus(back);
            }
            _ => return false,
        }
        true
    }

    /// Moves the focus from the item of a bar of menus `item` to the one `step` places along,
    /// around, closing the menu that `item` opened and opening the other's, for `open`.
    pub(super) fn bar_step(&mut self, item: ElementId, step: isize, open: bool) {
        let items = self.bar_items(item);
        let Some(at) = items.iter().position(|&each| each == item) else {
            return;
        };
        let next = items[(at as isize + step).rem_euclid(items.len() as isize) as usize];
        if let Some(menu) = self.submenu_of(item) {
            self.close_layer(menu, true);
        }
        self.set_focus(Some(next));
        if open {
            self.perform(Action {
                element: Some(next),
                kind: ActionKind::Press,
            });
        }
    }

    /// The pointer came over the item of a bar of menus `id`: while another item's menu is open,
    /// its own opens.
    pub(super) fn hover_bar_item(&mut self, id: ElementId) {
        let open = self
            .bar_items(id)
            .into_iter()
            .any(|item| item != id && self.submenu_of(item).is_some());
        if open && self.submenu_of(id).is_none() {
            self.interaction.by_pointer = true;
            self.perform(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
        }
    }

    /// Gives the first item of the first bar of menus drawn, from the top root, the layers left
    /// out, the focus, keeping what held it; or, where an item of a bar holds it or one of its
    /// menus is open, closes that menu and gives the focus back. Gives whether there was a bar.
    pub(super) fn toggle_menu_bar(&mut self) -> bool {
        // The keys reach the bar: its focus shows, whatever the last press was.
        self.interaction.by_pointer = false;
        // On the bar, or in one of its menus: the menus close, and the focus goes back.
        let open = self
            .layers()
            .into_iter()
            .find(|&layer| self.bar_item_of(layer).is_some());
        let on_bar = self
            .interaction
            .focus
            .is_some_and(|focus| self.in_menu_bar(focus));
        if on_bar || open.is_some() {
            if let Some(menu) = open {
                self.close_layer(menu, true);
            }
            // What held the focus before the keys reached the bar takes it back; a menu that the
            // pointer opened gave it back as it closed.
            let back = self.interaction.bar_before.take();
            if back.is_some() || on_bar {
                self.set_focus(back);
            }
            return true;
        }
        let bar = self.roots.iter().rev().find_map(|&root| {
            if self.layer_place(root).is_some() {
                return None;
            }
            let mut stack = vec![root];
            while let Some(id) = stack.pop() {
                let node = self.node(id)?;
                if node.style.layout.display == crate::style::Display::None {
                    continue;
                }
                if matches!(node.element.kind, Kind::MenuBar) {
                    return Some(id);
                }
                stack.extend(node.children.iter().rev().copied());
            }
            None
        });
        let Some(first) = bar.and_then(|bar| {
            self.children(bar)
                .iter()
                .copied()
                .find(|&item| self.focusable(item) && self.in_scope(item))
        }) else {
            return false;
        };
        self.interaction.bar_before = self.interaction.focus;
        self.set_focus(Some(first));
        true
    }

    /// Whether `id` is an item of a bar of menus.
    pub(crate) fn in_menu_bar(&self, id: ElementId) -> bool {
        self.is_menu_item(id)
            && self
                .node(id)
                .and_then(|node| node.parent)
                .and_then(|parent| self.node(parent))
                .is_some_and(|parent| matches!(parent.element.kind, Kind::MenuBar))
    }

    /// The item of a bar of menus that the layer `root` stands beside: the bar's menu that it is.
    fn bar_item_of(&self, root: ElementId) -> Option<ElementId> {
        match self.layer_place(root)? {
            Place::Beside { element, .. } if self.in_menu_bar(element) => Some(element),
            _ => None,
        }
    }

    /// The items of the bar of menus that holds the item `item` that can hold the focus.
    fn bar_items(&self, item: ElementId) -> Vec<ElementId> {
        let Some(bar) = self.node(item).and_then(|node| node.parent) else {
            return Vec::new();
        };
        self.children(bar)
            .iter()
            .copied()
            .filter(|&each| self.focusable(each))
            .collect()
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
