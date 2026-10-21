//! Errors of a run.

use slime_gfx::GfxError;
use slime_platform::PlatformError;

/// The error that a game returns from a `start` method
/// ([`Simulation::start`](crate::Simulation::start),
/// [`Presentation::start`](crate::Presentation::start)) when it cannot start, or passes to
/// [`Context::exit_with_error`](crate::Context::exit_with_error) to stop a run: any error type,
/// boxed. The `?` operator converts other errors into it, and so does `.into()` for a message
/// (`Err("no save file".into())`).
pub type AppError = Box<dyn std::error::Error + Send + Sync>;

/// Why [`run`](crate::run) failed. The underlying error is available through
/// [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// The window could not be opened, or its event loop failed.
    #[error("the window or its event loop failed")]
    Platform(#[from] PlatformError),
    /// The GPU could not be opened to draw into the window, the GPU device was lost during the run
    /// (`GfxError::DeviceLost`) and no new one replaced it (`GfxError::CannotRestore`, when the
    /// game asked for one), or the window's surface was lost for good
    /// (`GfxError::SurfaceLost`).
    #[error("cannot draw into the window")]
    Gfx(#[from] GfxError),
    /// The game could not start: a `start` method returned this error.
    #[error("the application failed to start")]
    Init(#[source] AppError),
    /// The game stopped the run with this error, which it passed to
    /// [`Context::exit_with_error`](crate::Context::exit_with_error) or
    /// [`SimulationContext::exit_with_error`](crate::SimulationContext::exit_with_error), or that
    /// [`Presentation::gpu_restored`](crate::Presentation::gpu_restored) returned.
    #[error("the application stopped with an error")]
    App(#[source] AppError),
}
