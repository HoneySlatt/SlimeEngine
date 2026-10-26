//! Errors of the platform layer.

/// A dependency's error, kept as the source of a [`PlatformError`] without exposing its type.
type Source = Box<dyn std::error::Error + Send + Sync>;

/// An error from the platform layer. The windowing library's own error, when there is one, is
/// available through [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlatformError {
    /// No windowing system could be reached. On Linux: no Wayland compositor or X server is
    /// running, or the `WAYLAND_DISPLAY` and `DISPLAY` environment variables do not point to one.
    #[error("cannot connect to the windowing system")]
    Connect(#[source] Source),
    /// An event loop was already requested in this process. The windowing library allows one
    /// attempt per process, even a failed one, so a process can run only one windowed
    /// application, and cannot retry after failing to connect.
    #[error("an event loop was already requested in this process; only one is allowed")]
    SecondEventLoop,
    /// The windowing system refused to create the window.
    #[error("cannot create the window")]
    CreateWindow(#[source] Source),
    /// The event loop stopped with an error. On Wayland, this is what a compositor that dies or
    /// closes the connection produces: the loop ends cleanly first, so
    /// [`EventHandler::exiting`](crate::EventHandler::exiting) is still called, and the source is
    /// the windowing library's `Exit Failure: <code>`; the cause appears only in lines that the
    /// Wayland library prints to standard error itself.
    ///
    /// On X11, an X server that dies or closes the connection does not produce this error: the
    /// X11 library (Xlib) prints that the X connection is broken and ends the process with exit
    /// status 1, so neither `exiting` nor anything after it runs.
    #[error("the event loop failed")]
    EventLoop(#[source] Source),
    /// The system gives no clipboard: on Linux, neither a compositor that gives Wayland's protocol
    /// of the clipboard nor an X server.
    #[error("cannot open the system's clipboard")]
    Clipboard(#[source] Source),
}
