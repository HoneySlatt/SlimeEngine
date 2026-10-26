//! What screen readers ask of a window, through AccessKit's adapter, and the updates of the tree
//! that they read (`docs/adr/0146-screen-readers.md`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use accesskit::{ActivationHandler, Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

/// What a screen reader asks of a window ([`Event::Accessibility`](crate::Event::Accessibility)),
/// through AccessKit's adapter, which the window holds from before it shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Accessibility {
    /// A screen reader began to read the window: the whole tree is due, in an update of
    /// [`Window::update_accessibility`](crate::Window::update_accessibility) that holds it, before
    /// the next frame; the adapter shows a tree of its own until then. Updates are due from then
    /// on, each with what changed.
    TreeRequested,
    /// A screen reader asked for an action on a node of the tree: a click, the focus, a value or a
    /// selection set, a scroll.
    Action(accesskit::ActionRequest),
    /// No screen reader reads the window any more: no update is due until the tree is requested
    /// again. Windows does not tell it.
    Deactivated,
}

/// AccessKit's adapter of the window, and whether a screen reader asked for the tree since the
/// handler last gave a whole one.
pub(crate) struct Adapter {
    adapter: accesskit_winit::Adapter,
    /// Set by the activation handler, under the adapter's lock, as the adapter begins to wait for
    /// the whole tree; cleared as a whole one is given.
    asked: Arc<AtomicBool>,
}

impl Adapter {
    /// The adapter of `window`, which must not have shown yet; what screen readers ask reaches the
    /// event loop through `proxy`.
    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        window: &winit::window::Window,
        proxy: EventLoopProxy<accesskit_winit::Event>,
    ) -> Self {
        let asked = Arc::new(AtomicBool::new(false));
        let activation = Activation {
            window: window.id(),
            proxy: proxy.clone(),
            asked: Arc::clone(&asked),
        };
        Self {
            adapter: accesskit_winit::Adapter::with_mixed_handlers(
                event_loop, window, activation, proxy,
            ),
            asked,
        }
    }

    /// Lets the adapter see `event` before the handler does.
    pub(crate) fn process_event(&mut self, window: &winit::window::Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }

    /// Gives the adapter `update`, if a screen reader reads the window. Once one asked for the
    /// tree, the adapter takes the next update as the whole tree, and AccessKit panics on one that
    /// is not: a screen reader may ask again while a frame makes an update of what changed, which
    /// is given before the handler hears that it asked. Such an update gives way to a tree of the
    /// window's node alone, until the whole tree comes.
    pub(crate) fn update(&mut self, update: TreeUpdate) {
        let asked = &self.asked;
        self.adapter.update_if_active(|| {
            if update.tree.is_some() {
                asked.store(false, Ordering::SeqCst);
                update
            } else if asked.load(Ordering::SeqCst) {
                lone_window()
            } else {
                update
            }
        });
    }
}

/// The adapter's activation handler: it tells the event loop that a screen reader asked for the
/// tree, which the handler gives later.
struct Activation {
    window: WindowId,
    proxy: EventLoopProxy<accesskit_winit::Event>,
    asked: Arc<AtomicBool>,
}

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.asked.store(true, Ordering::SeqCst);
        let event = accesskit_winit::Event {
            window_id: self.window,
            window_event: accesskit_winit::WindowEvent::InitialTreeRequested,
        };
        // A loop that has ended asks for nothing.
        let _ = self.proxy.send_event(event);
        None
    }
}

/// A tree of a window's node alone, which the whole tree that follows replaces.
fn lone_window() -> TreeUpdate {
    let window = NodeId(0);
    TreeUpdate {
        nodes: vec![(window, Node::new(Role::Window))],
        tree: Some(TreeInfo::new(window)),
        tree_id: TreeId::ROOT,
        focus: window,
    }
}
