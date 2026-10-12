//! Errors of the platform layer.

/// A dependency's error, kept as the source of a [`PlatformError`] without exposing its type.
type Source = Box<dyn std::error::Error + Send + Sync>;

/// An error from the platform layer. The windowing library's own error, when there is one, is
/// available through [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlatformError {
    /// No windowing system could be reached: no Wayland compositor or X server is running, or
    /// the `WAYLAND_DISPLAY` and `DISPLAY` environment variables do not point to one.
    #[error("cannot connect to a windowing system (Wayland or X11)")]
    Connect(#[source] Source),
    /// An event loop was already requested in this process. The windowing library allows one
    /// attempt per process, even a failed one, so a process can run only one windowed
    /// application, and cannot retry after failing to connect.
    #[error("an event loop was already requested in this process; only one is allowed")]
    SecondEventLoop,
    /// The windowing system refused to create the window.
    #[error("cannot create the window")]
    CreateWindow(#[source] Source),
    /// The event loop stopped with an error.
    #[error("the event loop failed")]
    EventLoop(#[source] Source),
}
