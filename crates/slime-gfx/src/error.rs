//! Errors of the GPU layer.

/// A dependency's error, kept as the source of a [`GfxError`] without exposing its type.
type Source = Box<dyn std::error::Error + Send + Sync>;

/// An error from the GPU layer. wgpu's own error, when there is one, is available through
/// [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GfxError {
    /// No GPU adapter is available: no Vulkan driver on Linux, no Direct3D 12 adapter on Windows.
    #[error("no GPU adapter is available")]
    NoAdapter(#[source] Source),
    /// The adapter refused to open a device.
    #[error("cannot open the GPU device")]
    RequestDevice(#[source] Source),
    /// Waiting for the GPU to finish its work failed.
    #[error("waiting for the GPU failed")]
    Wait(#[source] Source),
}
