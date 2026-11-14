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
use uwui_accesskit::accesskit::{Node, Rect, Role};

/// What screen readers read of a window: its tree, and what the window's node holds.
pub(crate) struct ScreenReaders {
    /// The tree of the target that the window shows.
    tree: ScreenReaderTree,
    /// The window's title, its node's name.
    title: String,
    /// The window's content area at the last update, its node's bounds.
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
        let size = window.inner_size();
        let resized = std::mem::replace(&mut self.size, size) != size;
        let title = &self.title;
        if let Some(update) = self.tree.update(ui, resized, || window_node(title, size)) {
            window.update_accessibility(update);
        }
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
