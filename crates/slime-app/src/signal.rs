//! Clean shutdown on termination signals: Ctrl+C in a terminal (SIGINT), a service manager
//! stopping the process (SIGTERM), or the terminal closing (SIGHUP).
//!
//! On Windows, the same handler receives the console's events: Ctrl+C and Ctrl+Break stop the run
//! cleanly. Closing the console window, logging off or shutting down also reach it, but Windows
//! ends the process as soon as the handler returns, which it does at once, so these get no clean
//! shutdown. A program without a console (a Windows game) receives none of these events.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set by the first termination signal and never cleared: a signal asks the whole process to stop.
static RECEIVED: AtomicBool = AtomicBool::new(false);

/// Installs the process-wide signal handler, once. The first signal sets a flag, which the drivers
/// check between frames, and logs that it was received, from another thread: the line can come
/// after lines the run writes later, and is lost if the process ends first. A second signal ends
/// the process at once, with exit code 130 and without a log line, in case the application no
/// longer reaches the end of a frame.
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
        // `ctrlc` runs this on one thread, which waits for the next signal only once it returns.
        // Writing to standard error can block, when it is a pipe that nobody reads, so a log line
        // written here could keep the second signal from ever being handled: the second signal
        // writes nothing, and the first one's line is written from a thread of its own, which is
        // not waited for. The line can therefore come after the run's own lines, and is lost if
        // the process ends before that thread writes it, or if the thread cannot start.
        let installed = ctrlc::set_handler(|| {
            if RECEIVED.swap(true, Ordering::SeqCst) {
                std::process::exit(130);
            }
            let _ = std::thread::Builder::new()
                .name("signal log".to_owned())
                .spawn(|| {
                    tracing::info!(
                        "termination signal received, stopping after the current frame (send it \
                         again to quit immediately)"
                    );
                });
        });
        match installed {
            Ok(()) => {}
            Err(ctrlc::Error::MultipleHandlers) => tracing::warn!(
                "termination signals are already handled through the ctrlc crate, probably by the \
                 game: the engine leaves them to that handler, so they do not stop runs cleanly"
            ),
            // `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
            // When the thread cannot start (a limit on threads is reached), the handlers stay and
            // what they catch reaches nobody, so the signals are ignored; when the handlers cannot
            // be installed, the signals keep their previous effect. The error does not say which
            // step failed. Its `Display` only names its kind: the cause is the error it holds.
            Err(ctrlc::Error::System(cause)) => tracing::warn!(
                %cause,
                "cannot handle termination signals: Ctrl+C, SIGTERM and SIGHUP either keep their \
                 previous effect, usually ending the process without a clean shutdown, or are \
                 ignored from now on (SIGKILL, or closing the console on Windows, still ends the \
                 process)"
            ),
            Err(error) => tracing::warn!(%error, "cannot handle termination signals"),
        }
    });
}

/// Whether a termination signal was received since the handler was installed.
pub(crate) fn received() -> bool {
    RECEIVED.load(Ordering::SeqCst)
}
