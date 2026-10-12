//! Errors of a run.

use slime_gfx::GfxError;
use slime_platform::PlatformError;

/// The error an application returns from [`App::init`](crate::App::init) when it cannot start:
/// any error type, boxed. The `?` operator converts other errors into it, and so does `.into()`
/// for a message (`Err("no save file".into())`).
pub type AppError = Box<dyn std::error::Error + Send + Sync>;

/// Why [`run`](crate::run) failed. The underlying error is available through
/// [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// The window could not be opened, or its event loop failed.
    #[error("the window or its event loop failed")]
    Platform(#[from] PlatformError),
    /// The GPU could not be opened to draw into the window.
    #[error("cannot draw into the window")]
    Gfx(#[from] GfxError),
    /// The application could not start: [`App::init`](crate::App::init) returned this error.
    #[error("the application failed to start")]
    Init(#[source] AppError),
}
