//! Opening the GPU: the adapter, the device and its queue.

use crate::error::GfxError;

/// The graphics API used when `WGPU_BACKEND` does not choose another one: Vulkan on Linux,
/// Direct3D 12 on Windows (ADR-0016).
#[cfg(windows)]
const DEFAULT_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(not(windows))]
const DEFAULT_BACKENDS: wgpu::Backends = wgpu::Backends::VULKAN;

/// The GPU device the engine works with, and the queue that receives its work.
///
/// [`Gpu::new`] opens it without a window, so that it can render offscreen and run in tests;
/// [`Gpu::new_for_window`] opens it together with the surface of a window.
#[derive(Debug)]
pub struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    adapter_name: String,
}

impl Gpu {
    /// Opens the most capable GPU of the machine.
    ///
    /// The graphics API is Vulkan on Linux and Direct3D 12 on Windows. The `WGPU_BACKEND`
    /// environment variable can choose another API that this build supports, for example
    /// `WGPU_BACKEND=vulkan` on Windows. Among the adapters of that API, a discrete GPU comes
    /// first, then an integrated one; a software adapter (lavapipe, WARP) is chosen only when no
    /// other is available, for example when `VK_DRIVER_FILES` restricts the Vulkan loader to
    /// lavapipe. The chosen adapter is logged at the `info` level.
    ///
    /// In debug builds, wgpu passes debug labels to the driver, so that capture tools such as
    /// RenderDoc show them. The graphics API's own validation layers stay off unless
    /// `WGPU_VALIDATION=1` is set; wgpu always validates how the engine uses it, and a use it
    /// rejects is a bug that panics.
    ///
    /// # Errors
    ///
    /// [`GfxError::NoAdapter`] if no adapter of the graphics API is available, and
    /// [`GfxError::RequestDevice`] if the adapter refuses to open a device.
    pub fn new() -> Result<Self, GfxError> {
        Self::open(&create_instance(), None).map(|(gpu, _adapter)| gpu)
    }

    /// Opens the most capable adapter of `instance`, among those that can present to
    /// `compatible_surface` if there is one, and its device.
    pub(crate) fn open(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<(Self, wgpu::Adapter), GfxError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            ..Default::default()
        }))
        .map_err(|error| GfxError::NoAdapter(Box::new(error)))?;
        let info = adapter.get_info();
        tracing::info!(
            name = %info.name,
            api = ?info.backend,
            kind = ?info.device_type,
            driver = %format!("{} {}", info.driver, info.driver_info).trim(),
            "GPU adapter"
        );

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SlimeEngine device"),
            ..Default::default()
        }))
        .map_err(|error| GfxError::RequestDevice(Box::new(error)))?;
        device.set_device_lost_callback(|reason, message| match reason {
            wgpu::DeviceLostReason::Destroyed => tracing::debug!("GPU device destroyed"),
            _ => tracing::error!(%message, "GPU device lost"),
        });

        let gpu = Self {
            device,
            queue,
            adapter_name: info.name,
        };
        Ok((gpu, adapter))
    }

    /// The name of the adapter in use, as its driver reports it: for example
    /// `"AMD Radeon RX 6900 XT (RADV NAVI21)"`, or `"llvmpipe (LLVM 21.1.8, 256 bits)"` for
    /// lavapipe.
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// Submits an empty piece of work, then blocks until the GPU has finished everything submitted
    /// so far.
    ///
    /// # Errors
    ///
    /// [`GfxError::Wait`] if waiting failed, for example because the device was lost.
    pub fn wait_idle(&self) -> Result<(), GfxError> {
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SlimeEngine wait_idle"),
            });
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|error| GfxError::Wait(Box::new(error)))
    }
}

/// The wgpu instance, limited to the platform's graphics API unless `WGPU_BACKEND` chooses another.
pub(crate) fn create_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::from_env().unwrap_or(DEFAULT_BACKENDS);
    descriptor.flags = instance_flags();
    wgpu::Instance::new(descriptor)
}

/// Debug labels in debug builds; validation layers of the graphics API only on request
/// (`WGPU_VALIDATION=1`), because they are slow and not always installed. Indirect draw arguments
/// are always validated, as wgpu recommends.
fn instance_flags() -> wgpu::InstanceFlags {
    let mut flags = wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL;
    if cfg!(debug_assertions) {
        flags |= wgpu::InstanceFlags::DEBUG;
    }
    flags.with_env()
}
