//! The system's dialogs to choose a file, through rfd, each on a thread of its own, so that the
//! event loop goes on while one is open (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`).

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::event_loop::UserEvent;

/// A dialog of the system to choose a file to open, as [`Window::choose_file`](crate::Window::choose_file)
/// opens it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileChoice {
    /// The dialog's title.
    pub title: String,
    /// The kinds of files it offers, each a name and its extensions without their dot:
    /// `("Levels", ["scene"])`; none for any file.
    pub filters: Vec<(String, Vec<String>)>,
    /// The directory it opens in; none for the one that the system chooses.
    pub directory: Option<PathBuf>,
}

/// Names a dialog that [`Window::choose_file`](crate::Window::choose_file) opened, in its answer
/// ([`Event::FileChosen`](crate::Event::FileChosen)): a number that no other dialog of the process
/// is given, from 0 up. A test of a program gives the answers of dialogs that it did not open by
/// their numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialogId(pub u64);

/// Opens the dialog of `choice` on a thread of its own, its parent `window`, and gives its answer
/// to the event loop through `proxy`; the dialog's number.
pub(crate) fn choose_file(
    window: crate::Window,
    choice: FileChoice,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
) -> DialogId {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dialog = DialogId(NEXT.fetch_add(1, Ordering::Relaxed));
    let spawned = std::thread::Builder::new()
        .name("slime file dialog".to_owned())
        .spawn({
            let proxy = proxy.clone();
            move || {
                // rfd panics on some texts, a filter's name that holds a NUL among them: the
                // panic is logged, and the dialog answered, as cancelled, so that no one waits.
                let file = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let mut asked = rfd::FileDialog::new()
                        .set_title(choice.title.as_str())
                        .set_parent(&window);
                    for (name, extensions) in &choice.filters {
                        asked = asked.add_filter(name.as_str(), extensions);
                    }
                    if let Some(directory) = &choice.directory {
                        asked = asked.set_directory(directory);
                    }
                    asked.pick_file()
                }))
                .unwrap_or_default();
                // A loop that has ended has nothing to answer.
                let _ = proxy.send_event(UserEvent::FileChosen { dialog, file });
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread runs the system's dialog: it is answered as cancelled");
        let _ = proxy.send_event(UserEvent::FileChosen { dialog, file: None });
    }
    dialog
}
