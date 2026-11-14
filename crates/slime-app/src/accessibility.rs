//! What a presentation's interface means, given to screen readers through AccessKit's adapter of the
//! window, and what they ask of it given back to the interface (`docs/adr/0146-screen-readers.md`).
//!
//! `uwui-accesskit` translates the interface's meaning into AccessKit's tree and does what screen
//! readers ask; this module is the window's side: the window's node, its title and size, and the
//! adapter's events and updates through `slime-platform`.

use slime_platform::{Accessibility, PhysicalSize, Window};
use uwui::host::TargetId;
use uwui::tree::Ui;
use uwui_accesskit::ScreenReaderTree;
use uwui_accesskit::accesskit::{Node, Rect, Role, TreeUpdate};

/// What screen readers read of a window: its tree, and what the window's node holds.
pub(crate) struct ScreenReaders {
    /// The tree of the target that the window shows.
    tree: ScreenReaderTree,
    /// The window's title, its node's name.
    title: String,
    /// The window's content area at the last update given, its node's bounds.
    size: PhysicalSize,
}

impl ScreenReaders {
    /// No screen reader reads a window of `title` yet.
    pub(crate) fn new(title: &str) -> Self {
        Self {
            tree: ScreenReaderTree::new(TargetId::MAIN),
            title: title.to_owned(),
            size: PhysicalSize {
                width: 0,
                height: 0,
            },
        }
    }

    /// A screen reader asked `asked` of `window`, whose presentation gives `ui`: the whole tree at
    /// once, an action given to the interface, or no update from then on.
    pub(crate) fn asked(&mut self, window: &Window, ui: Option<&mut Ui>, asked: &Accessibility) {
        match asked {
            Accessibility::TreeRequested => {
                self.tree.tree_requested();
                self.update(window, ui);
            }
            Accessibility::Deactivated => self.tree.deactivated(ui),
            Accessibility::Action(request) => {
                if let Some(ui) = ui {
                    self.tree.act(ui, request);
                }
            }
        }
    }

    /// Gives `window` what changed in what `ui`, the presentation's interface, means, while a
    /// screen reader reads it: after a frame. The window's node is given again when its size
    /// changed.
    pub(crate) fn update(&mut self, window: &Window, ui: Option<&mut Ui>) {
        if let Some(update) = self.next_update(ui, window.inner_size()) {
            window.update_accessibility(update);
        }
    }

    /// The update of what changed in what `ui` means, for a window's content area of `size`. The
    /// size is kept only once an update gave it: one that the tree did not give, the interface
    /// refusing a reader, is given by the next.
    fn next_update(&mut self, ui: Option<&mut Ui>, size: PhysicalSize) -> Option<TreeUpdate> {
        let title = &self.title;
        let update = self
            .tree
            .update(ui, size != self.size, || window_node(title, size))?;
        self.size = size;
        Some(update)
    }
}

/// The window's node, the root of its tree: named by its `title`, of its content area's `size`.
fn window_node(title: &str, size: PhysicalSize) -> Node {
    let mut window = Node::new(Role::Window);
    window.set_label(title);
    window.set_bounds(Rect {
        x0: 0.0,
        y0: 0.0,
        x1: f64::from(size.width),
        y1: f64::from(size.height),
    });
    window
}

#[cfg(test)]
mod tests {
    use uwui::text::Fonts;
    use uwui_accesskit::accesskit::NodeId;

    use super::*;

    /// A window's content area of `width` by 300 pixels.
    fn size(width: u32) -> PhysicalSize {
        PhysicalSize { width, height: 300 }
    }

    /// The width that `update` gives the window's node, none where it holds no node of the window.
    fn window_width(update: &TreeUpdate) -> Option<f64> {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(0))
            .and_then(|(_, node)| node.bounds())
            .map(|bounds| bounds.x1)
    }

    #[test]
    fn the_window_s_node_follows_its_size() {
        let mut ui = Ui::new(Fonts::new());
        let mut readers = ScreenReaders::new("Example");
        readers.tree.tree_requested();
        let first = readers
            .next_update(Some(&mut ui), size(400))
            .expect("an update");
        assert_eq!(window_width(&first), Some(400.0));
        let same = readers
            .next_update(Some(&mut ui), size(400))
            .expect("an update");
        assert_eq!(window_width(&same), None, "nothing changed");
        let larger = readers
            .next_update(Some(&mut ui), size(800))
            .expect("an update");
        assert_eq!(window_width(&larger), Some(800.0));
    }

    #[test]
    fn a_size_that_no_update_gave_is_given_by_the_next() {
        let mut ui = Ui::new(Fonts::new());
        let mut readers = ScreenReaders::new("Example");
        readers.tree.tree_requested();
        let first = readers
            .next_update(Some(&mut ui), size(400))
            .expect("an update");
        assert_eq!(window_width(&first), Some(400.0));
        // Another interface, which holds as many readers as it can, refuses the window one: no
        // update, though the window was resized meanwhile.
        let mut full = Ui::new(Fonts::new());
        while full.add_meaning_reader(TargetId::MAIN).is_some() {}
        assert!(readers.next_update(Some(&mut full), size(800)).is_none());
        // The first interface again, whose reader the window still holds: its node at the size
        // that no update gave.
        let back = readers
            .next_update(Some(&mut ui), size(800))
            .expect("an update");
        assert_eq!(window_width(&back), Some(800.0));
    }
}
