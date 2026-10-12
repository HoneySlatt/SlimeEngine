//! Clean shutdown on termination signals: Ctrl+C in a terminal (SIGINT), a service manager
//! stopping the process (SIGTERM), or the terminal closing (SIGHUP).

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set by the first termination signal and never cleared: a signal asks the whole process to stop.
static RECEIVED: AtomicBool = AtomicBool::new(false);

/// Installs the process-wide signal handler, once. The first signal only sets a flag, which the
/// drivers check between frames; a second one ends the process at once, with exit code 130, in
/// case the application no longer reaches the end of a frame.
///
/// The handler replaces whatever the process had set for these signals: another library's
/// handlers, or an inherited "ignore" (as under `nohup`, or for a command started in the
/// background by a script). Keeping an inherited "ignore" would mean, since `ctrlc` installs the
/// three signals together, not handling SIGTERM either. The only exception is a handler installed
/// earlier through the `ctrlc` crate, for example by the game: it is kept, and the engine logs a
/// warning.
pub(crate) fn install_handler() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let installed = ctrlc::set_handler(|| {
            if RECEIVED.swap(true, Ordering::SeqCst) {
                tracing::warn!("second termination signal, quitting immediately");
                std::process::exit(130);
            }
            tracing::info!(
                "termination signal received, stopping after the current frame (send it again \
                 to quit immediately)"
            );
        });
        match installed {
            Ok(()) => {}
            Err(ctrlc::Error::MultipleHandlers) => tracing::warn!(
                "termination signals are already handled through the ctrlc crate, probably by the \
                 game: the engine leaves them to that handler, so they do not stop runs cleanly"
            ),
            Err(error) => tracing::warn!(
                %error,
                "cannot handle termination signals: they will end the process without a clean \
                 shutdown"
            ),
        }
    });
}

/// Whether a termination signal was received since the handler was installed.
pub(crate) fn received() -> bool {
    RECEIVED.load(Ordering::SeqCst)
}
