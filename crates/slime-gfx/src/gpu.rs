//! Opening the GPU: the adapter, the device and its queue.

use std::sync::{Arc, OnceLock};

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
    /// Why the device was lost, once it has been: set by wgpu's device-lost callback.
    pub(crate) loss: Arc<OnceLock<String>>,
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
    /// wgpu passes debug labels to the driver, so that capture tools such as RenderDoc show them:
    /// on Vulkan in debug builds (or with `WGPU_DEBUG=1`), on Direct3D 12 in every build (unless
    /// `WGPU_DISCARD_HAL_LABELS=1` is set). The graphics API's own validation layers stay off
    /// unless `WGPU_VALIDATION=1` is set; wgpu always validates how the engine uses it, and a use
    /// it rejects is a bug that panics, except once the device is lost ([`Gpu::check`]). The
    /// device has the default limits, and the optional features the adapter offers among those the
    /// engine uses: timestamp queries ([`Gpu::can_time`]).
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

        // Optional features are asked for only when the adapter offers them, and everything works
        // without them (ADR-0031): timestamp queries, which time the GPU's work.
        let required_features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SlimeEngine device"),
            required_features,
            ..Default::default()
        }))
        .map_err(|error| GfxError::RequestDevice(Box::new(error)))?;
        let loss = Arc::new(OnceLock::new());
        device.set_device_lost_callback({
            let loss = Arc::clone(&loss);
            move |reason, message| {
                match reason {
                    wgpu::DeviceLostReason::Destroyed => tracing::debug!("GPU device destroyed"),
                    _ => tracing::error!(%message, "GPU device lost"),
                }
                // Only the first loss counts.
                let _ = loss.set(message);
            }
        });
        device.on_uncaptured_error(Arc::new({
            let loss = Arc::clone(&loss);
            move |error: wgpu::Error| {
                // Every use of a lost device fails; the loss itself is what matters, and the run
                // stops on it (ADR-0022). Before the loss, an error is a bug in the engine or the
                // application, as with wgpu's default handler.
                if loss.get().is_some() {
                    tracing::debug!(%error, "GPU error after the device was lost");
                } else {
                    panic!("wgpu error: {error}");
                }
            }
        }));

        let gpu = Self {
            device,
            queue,
            adapter_name: info.name,
            loss,
        };
        Ok((gpu, adapter))
    }

    /// Whether the device can still be used. It cannot once it is lost: the driver crashed or was
    /// reset, the GPU was removed, or it ran out of memory. The methods of this crate that return a
    /// `Result` then return [`GfxError::DeviceLost`]; the others do nothing, since wgpu ignores
    /// every use of a lost device, and the objects they return cannot be used.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost, with the reason the driver gave.
    pub fn check(&self) -> Result<(), GfxError> {
        match self.loss.get() {
            Some(message) => Err(GfxError::DeviceLost {
                message: message.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Runs `create` inside wgpu error scopes and returns what it created, with the error wgpu
    /// reported if there is one: a validation error, or an internal one, which wgpu raises when it
    /// cannot translate a shader for the graphics API, and Direct3D 12's shader compiler or driver
    /// when it rejects a shader or pipeline that wgpu's own validation accepted. Errors of other
    /// kinds reach the error handler.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] if the device is lost once `create` returns. On Vulkan, a driver
    /// that fails to create a shader or pipeline (which a conformant one does only when it runs
    /// out of memory) makes wgpu lose the device, and no scope sees an error.
    pub(crate) fn catch_errors<T>(
        &self,
        create: impl FnOnce() -> T,
    ) -> Result<(T, Option<wgpu::Error>), GfxError> {
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = self.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let created = create();
        // Scopes are popped in the reverse order of their pushes.
        let internal = pollster::block_on(internal.pop());
        let validation = pollster::block_on(validation.pop());
        self.check()?;
        Ok((created, internal.or(validation)))
    }

    /// Creates a buffer of `usage` holding `contents`, which must not be empty. On a lost device,
    /// the buffer cannot be used, like every object created then.
    pub(crate) fn create_buffer_with(
        &self,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            // A buffer mapped at creation has a size in multiples of 4 bytes.
            size: (contents.len() as u64).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
            usage,
            mapped_at_creation: true,
        });
        // wgpu's own `create_buffer_init` panics here on a lost device: the buffer it then creates
        // is invalid and cannot be mapped. Such a buffer is returned as it is, unusable (ADR-0022).
        if let Ok(mut mapped) = buffer.get_mapped_range_mut(..) {
            mapped.slice(..contents.len()).copy_from_slice(contents);
            drop(mapped);
            buffer.unmap();
        }
        buffer
    }

    /// The name of the adapter in use, as its driver reports it: for example
    /// `"AMD Radeon RX 6900 XT (RADV NAVI21)"`, or `"llvmpipe (LLVM 21.1.8, 256 bits)"` for
    /// lavapipe.
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// The wgpu device behind the engine's, for the engine's tooling only (ADR-0031): it draws
    /// with libraries built on wgpu, such as egui's renderer. Games must not use it.
    #[cfg(feature = "wgpu")]
    pub fn wgpu_device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The wgpu queue behind the engine's, for the engine's tooling only (ADR-0031).
    #[cfg(feature = "wgpu")]
    pub fn wgpu_queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The alignment, in bytes, of the offsets given to
    /// [`RenderPass::set_bind_group`](crate::RenderPass::set_bind_group) for bindings with a
    /// dynamic offset (256 with the default limits): a uniform buffer that holds the values of
    /// many draws lays them out at multiples of it.
    pub fn uniform_offset_alignment(&self) -> usize {
        self.device.limits().min_uniform_buffer_offset_alignment as usize
    }

    /// The largest width or height, in pixels, of a texture, a render target, a depth target or a
    /// window's image (8192 with the default limits). A caller that creates textures from files
    /// checks their size against it: [`Gpu::create_texture`] panics on a larger one.
    pub fn max_texture_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Submits an empty piece of work, then blocks until the GPU has finished everything submitted
    /// so far.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]).
    ///
    /// # Panics
    ///
    /// If the wait itself is what finds the device lost: wgpu treats that as fatal. Also if wgpu
    /// reports an error that its indefinite wait cannot produce.
    pub fn wait_idle(&self) -> Result<(), GfxError> {
        self.check()?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SlimeEngine wait_idle"),
            });
        self.queue.submit([encoder.finish()]);
        // wgpu reports a wait that timed out or that named a submission not made yet; an indefinite
        // wait without a submission index does neither.
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("an indefinite wait neither times out nor names a submission");
        Ok(())
    }
}

/// The wgpu instance, limited to the platform's graphics API unless `WGPU_BACKEND` chooses another.
pub(crate) fn create_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::from_env().unwrap_or(DEFAULT_BACKENDS);
    descriptor.flags = instance_flags();
    wgpu::Instance::new(descriptor)
}

/// Debug labels in debug builds, which Vulkan needs to receive them (Direct3D 12 receives them in
/// every build, since only `DISCARD_HAL_LABELS` keeps them back); validation layers of the
/// graphics API only on request (`WGPU_VALIDATION=1`), because they are slow and not always
/// installed. Indirect draw arguments are always validated, as wgpu recommends.
fn instance_flags() -> wgpu::InstanceFlags {
    let mut flags = wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL;
    if cfg!(debug_assertions) {
        flags |= wgpu::InstanceFlags::DEBUG;
    }
    flags.with_env()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, RenderPassDescriptor, TextureFormat};

    #[test]
    fn a_lost_device_is_reported_and_using_it_does_not_panic() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let target = gpu.create_render_target("lost target", 4, 4, TextureFormat::Rgba8UnormSrgb);
        assert!(gpu.check().is_ok());

        gpu.device.destroy();
        // wgpu reports a destroyed device when it is next polled and its queue is empty.
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        assert!(
            matches!(gpu.check(), Err(GfxError::DeviceLost { .. })),
            "the loss is reported"
        );

        let black = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(black),
            depth: None,
            timer: None,
        };
        gpu.render(&pass, |_pass| {});
        // wgpu cannot map a buffer created on a lost device to fill it.
        gpu.create_vertex_buffer("lost vertices", &[0; 6]);
        gpu.create_index_buffer("lost indices", &[0]);
        gpu.create_uniform_buffer("lost uniforms", &[0; 4]);
        for result in [
            gpu.read_render_target(&target).map(|_| ()),
            gpu.wait_idle(),
            gpu.create_shader_module("lost shader", "").map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(GfxError::DeviceLost { .. })),
                "a lost device is reported by every method that returns a result: {result:?}"
            );
        }
    }

    #[test]
    fn a_device_lost_while_creating_is_reported() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        // On Vulkan, a driver that fails to create a pipeline makes wgpu lose the device without an
        // error that a scope catches; destroying the device does the same.
        let result = gpu.catch_errors(|| {
            gpu.device.destroy();
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        });
        assert!(
            matches!(result, Err(GfxError::DeviceLost { .. })),
            "the loss is reported: {result:?}"
        );
    }
}
