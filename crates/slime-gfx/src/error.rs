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
    /// The graphics API cannot draw into the window.
    #[error("cannot create a surface for the window")]
    CreateSurface(#[source] Source),
    /// The window offers no sRGB format that the engine supports.
    #[error("the window offers no sRGB format that the engine supports (it offers {offered})")]
    SurfaceFormat {
        /// The formats the window offers, as wgpu names them.
        offered: String,
    },
    /// The GPU device was lost: the driver crashed or was reset, the GPU was removed, or it ran out
    /// of memory. Objects created from the device can no longer be used.
    #[error("the GPU device was lost: {message}")]
    DeviceLost {
        /// The reason the driver gave.
        message: String,
    },
    /// A shader module was rejected, usually because its WGSL source does not compile.
    #[error("shader `{label}` was rejected:\n{message}")]
    Shader {
        /// The label given to the shader module.
        label: String,
        /// The compiler's message, with the line and column of the error in the source.
        message: String,
    },
    /// A render pipeline was rejected, for example because an entry point does not exist in its
    /// shader or does not match the vertex layout.
    #[error("render pipeline `{label}` was rejected:\n{message}")]
    Pipeline {
        /// The label given to the pipeline.
        label: String,
        /// wgpu's message.
        message: String,
    },
    /// Reading a render target back to the CPU failed.
    #[error("reading the render target `{label}` back failed")]
    Readback {
        /// The label of the render target.
        label: String,
        /// wgpu's error.
        #[source]
        source: Source,
    },
}
