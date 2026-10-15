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
///
/// Cloning a `Gpu` gives another handle to the same device and queue, cheaply: threads that share
/// the device each hold a clone, and an object that one clone creates is used with any other, as
/// `slime-render`'s loading threads do. Each clone sees the loss of the device ([`Gpu::check`]).
///
/// Every object a `Gpu` creates belongs to its device: using one with a `Gpu` that another call to
/// [`Gpu::new`] or [`Gpu::new_for_window`] opened is a bug that neither the engine nor wgpu
/// reliably catches. Each of those calls opens its own wgpu instance, whose objects are numbered
/// from zero, so the call may panic inside wgpu, or act on the object of the other `Gpu` that has
/// the same number: a render target read back with another `Gpu` can return the pixels of that
/// `Gpu`'s own target.
#[derive(Clone, Debug)]
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
    /// it rejects is a bug that panics, except once the device is lost ([`Gpu::check`]). Memory
    /// that runs out is not a bug: see [`GfxError::OutOfMemory`]. The device has the default
    /// limits, and the optional features the adapter offers among those the engine uses:
    /// timestamp queries ([`Gpu::can_time`]) and BC texture compression ([`Gpu::can_sample_bc`]).
    ///
    /// On Direct3D 12, shaders are compiled with DXC when its library, `dxcompiler.dll`, lies
    /// beside the program, and otherwise with FXC, which is part of Windows but whose code runs
    /// slower: `slime-render`'s scene pass takes about twice as long. Which one compiles is
    /// logged, at the `warn` level for FXC when no `dxcompiler.dll` was found (ADR-0066).
    /// `WGPU_DX12_COMPILER=fxc` asks for FXC, and `WGPU_DX12_COMPILER=dxc` for DXC wherever Windows
    /// finds `dxcompiler.dll`. A `dxcompiler.dll` that does not load, beside the program or where
    /// `dxc` asks for it, leaves no Direct3D 12 adapter, and so does, with DXC, a driver that offers
    /// no shader model 6.
    ///
    /// The `SLIME_GPU_MEMORY_BUDGET` environment variable ([`MEMORY_BUDGET_ENV_VAR`]), a percentage
    /// from 1 to 100, makes the creation of buffers, textures and timers fail with
    /// [`GfxError::OutOfMemory`] once the memory in use would pass that share of what the driver
    /// allows the process, to try a game with less memory and to test how it handles these errors.
    /// It works on Direct3D 12, and on Vulkan with drivers whose memory budget wgpu reads
    /// (`VK_EXT_memory_budget`): lavapipe's, not RADV's, which wgpu 30 ignores because of a driver
    /// bug; elsewhere it changes nothing. Drivers count the memory already in use differently:
    /// lavapipe counts the whole machine's, so that below a share that depends on what else runs,
    /// the device does not even open ([`GfxError::RequestDevice`]).
    ///
    /// # Errors
    ///
    /// [`GfxError::NoAdapter`] if no adapter of the graphics API is available, and
    /// [`GfxError::RequestDevice`] if the adapter refuses to open a device, as when the memory
    /// budget is too small for it.
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

        // Optional features are asked for only when the adapter offers them: timestamp queries,
        // which time the GPU's work and without which everything works (ADR-0031), and BC texture
        // compression, which only the asset pipeline's cooked textures need (ADR-0069).
        let required_features = adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TEXTURE_COMPRESSION_BC);
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
                // stops on it (ADR-0022). Memory that runs out here ran out in a creation that no
                // caller hears of, one made by a library built on wgpu such as egui's renderer:
                // the device is given up as if it were lost, so that the run stops cleanly
                // (ADR-0044). Before that, an error is a bug in the engine or the application, as
                // with wgpu's default handler.
                if loss.get().is_some() {
                    tracing::debug!(%error, "GPU error after the device was lost");
                } else if let wgpu::Error::OutOfMemory { .. } = error {
                    tracing::error!(%error, "out of GPU memory where no caller can be told");
                    let _ = loss.set(format!(
                        "out of GPU memory where no caller can be told: {error}"
                    ));
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
    /// every use of a lost device, and the objects they return cannot be used. The errors wgpu
    /// raises for those uses only repeat that the device is gone: they are logged at the `debug`
    /// level, where other wgpu errors panic.
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
        let (created, error) = within_error_scopes(&self.device, create);
        self.check()?;
        Ok((created, error))
    }

    /// Runs `create`, which creates the GPU objects of `label`, inside an out-of-memory error
    /// scope, and returns what it created. On a lost device, `create` does not run: nothing could
    /// be created. Errors of other kinds reach the error handler.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if wgpu reports that the GPU's memory cannot hold what `create`
    /// asked for, and [`GfxError::DeviceLost`] if the device is lost before `create` runs or once
    /// it returns.
    pub(crate) fn create<T>(&self, label: &str, create: impl FnOnce() -> T) -> Result<T, GfxError> {
        self.check()?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let created = create();
        let error = pollster::block_on(scope.pop());
        self.check()?;
        match error {
            None => Ok(created),
            Some(_) => Err(GfxError::OutOfMemory {
                label: label.to_owned(),
            }),
        }
    }

    /// Creates a buffer of `usage` holding `contents`, which must not be empty, inside
    /// [`create`](Self::create), which reports a buffer that could not be created.
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
        // wgpu's own `create_buffer_init` panics here when the buffer could not be created, on a
        // lost device or out of memory: it is invalid and cannot be mapped. Such a buffer is
        // returned as it is, and `create` reports why (ADR-0022, ADR-0044).
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

    /// The wgpu device behind the engine's, for the engine's tooling only, through the `wgpu`
    /// feature (ADR-0031): it draws with libraries built on wgpu, such as egui's renderer. Games
    /// must not use it.
    #[cfg(feature = "wgpu")]
    pub fn wgpu_device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The wgpu queue behind the engine's, for the engine's tooling only, through the `wgpu`
    /// feature (ADR-0031).
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

    /// The largest size, in bytes, of a buffer (256 MiB with the default limits). A caller that
    /// creates buffers from files checks their size against it: [`Gpu::create_vertex_buffer`] and
    /// [`Gpu::create_index_buffer`] panic on a larger one.
    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size
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

/// Runs `run` inside a validation and an internal error scope of `device`, and returns what it
/// gave, with the error wgpu reported in either scope, if any; errors of other kinds reach the
/// error handler.
pub(crate) fn within_error_scopes<T>(
    device: &wgpu::Device,
    run: impl FnOnce() -> T,
) -> (T, Option<wgpu::Error>) {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let result = run();
    // Scopes are popped in the reverse order of their pushes.
    let internal = pollster::block_on(internal.pop());
    let validation = pollster::block_on(validation.pop());
    (result, internal.or(validation))
}

/// The wgpu instance, limited to the platform's graphics API unless `WGPU_BACKEND` chooses another,
/// with the shader compiler of Direct3D 12 that [`dx12_compiler`] chooses, logged.
pub(crate) fn create_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::from_env().unwrap_or(DEFAULT_BACKENDS);
    descriptor.flags = instance_flags();
    if cfg!(windows) && descriptor.backends.contains(wgpu::Backends::DX12) {
        let requested = wgpu::Dx12Compiler::from_env();
        let compiler = dx12_compiler(requested.clone(), dxc_beside_program());
        match (&compiler, requested) {
            (wgpu::Dx12Compiler::DynamicDxc { dxc_path }, _) => {
                tracing::info!(path = %dxc_path, "Direct3D 12 compiles shaders with DXC");
            }
            (_, Some(wgpu::Dx12Compiler::Fxc)) => {
                tracing::info!("Direct3D 12 compiles shaders with FXC, as WGPU_DX12_COMPILER asks");
            }
            _ => tracing::warn!(
                "Direct3D 12 compiles shaders with FXC, whose code runs slower than DXC's: no \
                 {DXC_LIBRARY} lies beside the program (ADR-0066)"
            ),
        }
        descriptor.backend_options.dx12.shader_compiler = compiler;
    }
    descriptor.memory_budget_thresholds.for_resource_creation =
        memory_budget(std::env::var(MEMORY_BUDGET_ENV_VAR).ok().as_deref());
    wgpu::Instance::new(descriptor)
}

/// The environment variable that lowers the GPU memory that creations may use, to a percentage of
/// what the driver allows the process (ADR-0044).
pub const MEMORY_BUDGET_ENV_VAR: &str = "SLIME_GPU_MEMORY_BUDGET";

/// The percentage of the driver's memory budget past which wgpu refuses to create buffers,
/// textures and query sets, from the value of [`MEMORY_BUDGET_ENV_VAR`]: a whole number from 1 to
/// 100, or nothing. Another value is ignored, with a warning.
fn memory_budget(value: Option<&str>) -> Option<u8> {
    let value = value?;
    match value.trim().parse() {
        Ok(percent @ 1..=100) => Some(percent),
        _ => {
            tracing::warn!(
                "{MEMORY_BUDGET_ENV_VAR}={value} is ignored: it is a percentage, from 1 to 100"
            );
            None
        }
    }
}

/// The file name of DXC's library, which the engine looks for beside the program (ADR-0066).
const DXC_LIBRARY: &str = "dxcompiler.dll";

/// The shader compiler of Direct3D 12 (ADR-0066): what `WGPU_DX12_COMPILER` asks for, given here
/// as `requested`, if it is FXC (`fxc`) or DXC wherever Windows finds `dxcompiler.dll` (`dxc` or
/// `dynamicdxc`); otherwise DXC loaded from `beside`, the path of the `dxcompiler.dll` beside the
/// program if there is one, and FXC without it. wgpu's other choices are ignored: `auto` takes any
/// `dxcompiler.dll` that Windows finds, on `PATH` too, whatever its version, and `staticdxc` needs
/// a wgpu feature the engine does not build, on which wgpu panics.
fn dx12_compiler(
    requested: Option<wgpu::Dx12Compiler>,
    beside: Option<String>,
) -> wgpu::Dx12Compiler {
    match (requested, beside) {
        (Some(asked @ (wgpu::Dx12Compiler::Fxc | wgpu::Dx12Compiler::DynamicDxc { .. })), _) => {
            asked
        }
        (_, Some(dxc_path)) => wgpu::Dx12Compiler::DynamicDxc { dxc_path },
        (_, None) => wgpu::Dx12Compiler::Fxc,
    }
}

/// The path of the `dxcompiler.dll` that lies beside the running program, if there is one and its
/// path is valid Unicode, as wgpu needs: a full path, so that Windows loads that very file, never
/// another `dxcompiler.dll` on its search path.
fn dxc_beside_program() -> Option<String> {
    let path = std::env::current_exe().ok()?.with_file_name(DXC_LIBRARY);
    if !path.is_file() {
        return None;
    }
    path.into_os_string().into_string().ok()
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
    use crate::{LinearColor, RenderPassDescriptor, RenderTarget, TextureFormat};

    #[test]
    fn direct3d_12_compiles_shaders_with_the_dxc_beside_the_program_unless_asked_otherwise() {
        let beside = r"C:\game\dxcompiler.dll";
        for (requested, case) in [
            (None, "nothing asked"),
            (
                Some(wgpu::Dx12Compiler::StaticDxc),
                "a DXC the engine does not build",
            ),
            (Some(wgpu::Dx12Compiler::Auto), "wgpu's choice"),
        ] {
            assert!(
                matches!(
                    dx12_compiler(requested.clone(), Some(beside.to_owned())),
                    wgpu::Dx12Compiler::DynamicDxc { dxc_path } if dxc_path == beside
                ),
                "{case}, with DXC beside the program"
            );
            assert!(
                matches!(dx12_compiler(requested, None), wgpu::Dx12Compiler::Fxc),
                "{case}, without DXC beside the program"
            );
        }
        for found in [Some(beside.to_owned()), None] {
            assert!(
                matches!(
                    dx12_compiler(Some(wgpu::Dx12Compiler::Fxc), found.clone()),
                    wgpu::Dx12Compiler::Fxc
                ),
                "FXC asked for, {found:?} beside the program"
            );
            assert!(
                matches!(
                    dx12_compiler(Some(wgpu::Dx12Compiler::default_dynamic_dxc()), found.clone()),
                    wgpu::Dx12Compiler::DynamicDxc { dxc_path } if dxc_path == DXC_LIBRARY
                ),
                "DXC asked for wherever Windows finds it, {found:?} beside the program"
            );
        }
    }

    #[test]
    fn a_lost_device_is_reported_and_using_it_does_not_panic() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let target = gpu
            .create_render_target("lost target", 4, 4, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        assert!(gpu.check().is_ok());

        gpu.device.destroy();
        // wgpu reports a destroyed device when it is next polled and its queue is empty.
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        assert!(
            matches!(gpu.check(), Err(GfxError::DeviceLost { .. })),
            "the loss is reported"
        );

        let black = LinearColor::BLACK;
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(black),
            depth: None,
            timer: None,
            resolve: None,
        };
        gpu.render(&pass, |_pass| {});
        gpu.submit("lost frame", |commands| {
            commands.render(&pass, |_pass| {});
            commands.render(&pass, |_pass| {});
        });
        for result in [
            gpu.read_render_target(&target).map(|_| ()),
            gpu.wait_idle(),
            gpu.create_shader_module("lost shader", "").map(|_| ()),
            // wgpu cannot map a buffer created on a lost device to fill it: nothing is created.
            gpu.create_vertex_buffer("lost vertices", &[0; 6])
                .map(|_| ()),
            gpu.create_index_buffer("lost indices", &[0]).map(|_| ()),
            gpu.create_uniform_buffer("lost uniforms", &[0; 4])
                .map(|_| ()),
            gpu.create_render_target("lost target", 4, 4, TextureFormat::Rgba8UnormSrgb)
                .map(|_| ()),
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

    #[test]
    fn the_memory_budget_is_a_percentage_from_1_to_100() {
        assert_eq!(memory_budget(None), None);
        assert_eq!(memory_budget(Some("1")), Some(1));
        assert_eq!(memory_budget(Some(" 100 ")), Some(100));
        for ignored in ["0", "101", "-5", "half", ""] {
            assert_eq!(memory_budget(Some(ignored)), None, "{ignored:?}");
        }
    }

    /// Set in the processes that the out-of-memory tests start.
    const CHILD_ENV: &str = "SLIME_GFX_TEST_CHILD";

    /// The most render targets of [`filler`] that a child creates: 4 GiB.
    const FILLERS: usize = 8;

    /// A render target of 8192 × 8192 pixels of 8 bytes: 512 MiB.
    fn filler(gpu: &Gpu) -> Result<RenderTarget, GfxError> {
        gpu.create_render_target("filler", 8192, 8192, TextureFormat::Rgba16Float)
    }

    /// Runs the test `name` again in processes of its own, with [`MEMORY_BUDGET_ENV_VAR`] set,
    /// and returns what the first one that ran out of memory printed; `None` when nothing runs out
    /// even at a budget of 1 %: wgpu does not read the driver's budget, as with RADV, whose budget
    /// wgpu 30 ignores because of a driver bug, and no budget can make memory run out.
    ///
    /// The budget is a share of the memory that the driver says the process may use, but drivers
    /// count what is already used differently: lavapipe counts the whole machine's memory, which
    /// the creations of a test do not change, so that below a share that depends on what else
    /// runs, the device does not even open, and above it, nothing runs out. A binary search looks
    /// for a share in between, where the child prints neither `no device` nor `never ran out`. What
    /// else runs on the machine moves that share; the search starts again, up to three times, when
    /// it has moved too far to be found.
    fn run_out_of_memory_in_a_child(name: &str) -> Option<String> {
        if child(name, 1).contains("never ran out") {
            return None;
        }
        for _ in 0..3 {
            if let Some(stdout) = search_budget(name) {
                return Some(stdout);
            }
        }
        panic!("no budget lets the device open and then run out of memory within 4 GiB");
    }

    /// One binary search of [`run_out_of_memory_in_a_child`].
    fn search_budget(name: &str) -> Option<String> {
        let (mut low, mut high) = (1_u8, 100_u8);
        while low <= high {
            let budget = low + (high - low) / 2;
            let stdout = child(name, budget);
            if stdout.contains("no device") {
                low = budget + 1;
            } else if stdout.contains("never ran out") {
                high = budget - 1;
            } else {
                return Some(stdout);
            }
        }
        None
    }

    /// What the test `name` prints when it runs in a process of its own, with a memory budget of
    /// `budget` %.
    fn child(name: &str, budget: u8) -> String {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", name, "--nocapture"])
            .env(CHILD_ENV, "1")
            .env(MEMORY_BUDGET_ENV_VAR, budget.to_string())
            .output()
            .expect("child test process runs");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "the child failed with a budget of {budget} %: {}\n{stdout}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
    }

    /// The GPU of a child, or `None`, once it has printed `no device`, when the budget is too small
    /// for the device to open.
    fn child_gpu() -> Option<Gpu> {
        match Gpu::new() {
            Ok(gpu) => Some(gpu),
            Err(GfxError::RequestDevice(_)) => {
                println!("no device");
                None
            }
            Err(error) => panic!("a GPU adapter must be available: {error}"),
        }
    }

    #[test]
    fn a_creation_beyond_the_memory_fails_and_the_device_goes_on() {
        if std::env::var_os(CHILD_ENV).is_none() {
            let Some(stdout) = run_out_of_memory_in_a_child(
                "gpu::tests::a_creation_beyond_the_memory_fails_and_the_device_goes_on",
            ) else {
                eprintln!("skipped: no memory budget makes the memory of this driver run out");
                return;
            };
            assert!(stdout.contains("the device goes on"), "{stdout}");
            return;
        }
        let Some(gpu) = child_gpu() else { return };
        let mut fillers = Vec::new();
        let error = loop {
            if fillers.len() == FILLERS {
                println!("never ran out");
                return;
            }
            match filler(&gpu) {
                Ok(target) => fillers.push(target),
                Err(error) => break error,
            }
        };
        println!("out of memory after {} targets of 512 MiB", fillers.len());
        assert!(
            matches!(&error, GfxError::OutOfMemory { label } if label == "filler"),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "out of GPU memory: cannot create `filler`"
        );
        assert!(gpu.check().is_ok(), "the device is not lost");

        let small = gpu
            .create_render_target("small", 4, 4, TextureFormat::Rgba8UnormSrgb)
            .expect("a small target fits");
        let red = LinearColor::rgb(1.0, 0.0, 0.0);
        gpu.render(
            &RenderPassDescriptor {
                target: &small,
                clear: Some(red),
                depth: None,
                timer: None,
                resolve: None,
            },
            |_pass| {},
        );
        let image = gpu
            .read_render_target(&small)
            .expect("the target is read back");
        assert_eq!(image.pixels[..4], [255, 0, 0, 255]);
        println!("the device goes on");
    }

    #[test]
    fn memory_that_runs_out_where_no_caller_is_told_gives_the_device_up() {
        if std::env::var_os(CHILD_ENV).is_none() {
            let Some(stdout) = run_out_of_memory_in_a_child(
                "gpu::tests::memory_that_runs_out_where_no_caller_is_told_gives_the_device_up",
            ) else {
                eprintln!("skipped: no memory budget makes the memory of this driver run out");
                return;
            };
            assert!(stdout.contains("given up"), "{stdout}");
            return;
        }
        let Some(gpu) = child_gpu() else { return };
        // A texture that a library built on wgpu creates itself, outside the engine's scopes.
        let mut textures = Vec::new();
        while gpu.check().is_ok() {
            if textures.len() == FILLERS {
                println!("never ran out");
                return;
            }
            textures.push(gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("library texture"),
                size: wgpu::Extent3d {
                    width: 8192,
                    height: 8192,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            }));
        }
        let Err(GfxError::DeviceLost { message }) = gpu.check() else {
            unreachable!("the loop ends on a loss");
        };
        assert!(
            message.starts_with("out of GPU memory where no caller can be told"),
            "{message}"
        );
        // Like a lost device, a given-up one creates nothing more.
        assert!(matches!(filler(&gpu), Err(GfxError::DeviceLost { .. })));
        println!("given up");
    }
}
