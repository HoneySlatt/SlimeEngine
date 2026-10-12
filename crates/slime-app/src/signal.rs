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
/// If the process already has a handler installed through the `ctrlc` crate, for example by the
/// game, that handler is kept and the engine logs a warning.
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
        if let Err(error) = installed {
            tracing::warn!(
                %error,
                "cannot handle termination signals: they will end the process without a clean \
                 shutdown"
            );
        }
    });
}

/// Whether a termination signal was received since the handler was installed.
pub(crate) fn received() -> bool {
    RECEIVED.load(Ordering::SeqCst)
}
