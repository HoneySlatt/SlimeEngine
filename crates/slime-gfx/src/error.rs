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
    /// The adapter offers less than the engine needs of one of its limits, the floor that
    /// [`Gpu::new`](crate::Gpu::new) gives: the engine does not open a device that it would fail
    /// on later.
    #[error(
        "the GPU adapter {adapter} offers {offered} for its limit {limit}, where the engine needs \
         {needed}"
    )]
    BelowFloor {
        /// The adapter's name, as its driver gives it.
        adapter: String,
        /// The limit, by its name in WebGPU and wgpu, such as `max_bind_groups`.
        limit: &'static str,
        /// What the engine needs: the floor's value.
        needed: u64,
        /// What the adapter offers.
        offered: u64,
    },
    /// The graphics API cannot draw into the window.
    #[error("cannot create a surface for the window")]
    CreateSurface(#[source] Source),
    /// The window offers no sRGB format that the engine supports.
    #[error("the window offers no sRGB format that the engine supports (it offers {offered})")]
    SurfaceFormat {
        /// The formats the window offers, as wgpu names them.
        offered: String,
    },
    /// The GPU device was lost: the driver crashed or was reset, the GPU was removed, it ran out of
    /// memory where wgpu cannot recover (see [`OutOfMemory`](Self::OutOfMemory)), or the program
    /// lost it on purpose ([`Gpu::lose`](crate::Gpu::lose)). Objects created from the device can no
    /// longer be used.
    #[error("the GPU device was lost: {message}")]
    DeviceLost {
        /// Why: the driver, the program, or memory that ran out.
        kind: crate::LossKind,
        /// The reason the driver or the engine gave.
        message: String,
    },
    /// No device replaces the one that was lost ([`Gpu::restore`](crate::Gpu::restore),
    /// [`Surface::restore`](crate::Surface::restore)): it was lost because memory ran out, or the
    /// only adapter left renders on the CPU where the lost one did not.
    #[error("no GPU device replaces the lost one: {reason}")]
    CannotRestore {
        /// Why.
        reason: String,
    },
    /// The GPU's memory cannot hold an object being created: a buffer, a texture, a render or
    /// depth target, a sampler, a timer's queries and buffers, or the buffer of a readback. Nothing
    /// was created and the device can still be used: the same creation can succeed once the
    /// caller has freed memory, by dropping objects the GPU no longer uses, or asks for less.
    ///
    /// wgpu reports these, and the engine returns them, for the memory of the object itself.
    /// When memory runs out elsewhere, wgpu loses the device instead, and the creation returns
    /// [`DeviceLost`](Self::DeviceLost): for the staging memory that carries an object's data to
    /// the GPU, and for the other kinds of objects (bind groups, shaders, pipelines). Memory that
    /// runs out in a creation the engine does not make, in a library built on wgpu, is handled as
    /// a device loss too. ADR-0044 records why.
    #[error("out of GPU memory: cannot create `{label}`")]
    OutOfMemory {
        /// The label given to the object.
        label: String,
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
    /// The window's surface was lost for good: after a loss, it could not be created again, or it
    /// was lost again each time it was. Nothing can be drawn into the window any more.
    #[error("the window's surface was lost for good: {message}")]
    SurfaceLost {
        /// Why: the error of creating it again, or the losses that followed; wgpu's message when
        /// the graphics API refused to configure it.
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
    /// The GPU cannot sample the texture's format: a block-compressed format
    /// ([`TextureFormat::is_block_compressed`](crate::TextureFormat::is_block_compressed)) on a GPU
    /// without BC texture compression, which every desktop GPU of Linux and Windows has, and which
    /// the asset pipeline's cooked textures use (`docs/adr/0069-the-asset-pipeline.md`).
    #[error("the GPU `{adapter}` cannot sample `{label}`, a texture compressed in BC blocks")]
    UnsupportedTexture {
        /// The GPU, as the driver names it.
        adapter: String,
        /// The label given to the texture.
        label: String,
    },
}
