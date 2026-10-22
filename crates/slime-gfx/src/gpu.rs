//! Opening the GPU: the adapter, the device and its queue.

use std::sync::{Arc, OnceLock, PoisonError, RwLock};
use std::time::Duration;

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
/// the tasks of `slime-render`'s loads do. Each clone sees the loss of the device ([`Gpu::check`]).
/// The work that a clone submits waits while the device's window surface is configured, which wgpu
/// would otherwise fail: a resize of the window during a load does not lose the surface.
///
/// Every object a `Gpu` creates belongs to its device: using one with a `Gpu` that another call to
/// [`Gpu::new`] or [`Gpu::new_for_window`] opened is a bug that neither the engine nor wgpu
/// reliably catches. Each of those calls opens its own wgpu instance, whose objects are numbered
/// from zero, so the call may panic inside wgpu, or act on the object of the other `Gpu` that has
/// the same number: a render target read back with another `Gpu` can return the pixels of that
/// `Gpu`'s own target. A device opened to replace a lost one ([`Gpu::restore`],
/// [`Surface::restore`](crate::Surface::restore)) shares its instance, where wgpu refuses an object
/// of the lost device: that use panics as the bug it is.
#[derive(Clone, Debug)]
pub struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// The wgpu instance that opened the device, which opens the device that replaces it.
    instance: wgpu::Instance,
    adapter_name: String,
    /// Whether the adapter renders on the CPU (lavapipe, WARP).
    software: bool,
    /// Why the device was lost, once it has been: set by wgpu's device-lost callback, by the error
    /// handler when memory runs out where no caller can be told, or by [`Gpu::lose`].
    pub(crate) loss: Arc<OnceLock<Loss>>,
    /// Taken, shared, by each submission of work that the clones make ([`submit`]), and alone by
    /// the configuration of the device's window surface: wgpu fails to configure a surface when
    /// another thread submits while it waits for the GPU, as threads that load a scene do
    /// (ADR-0074).
    pub(crate) submissions: Arc<RwLock<()>>,
}

/// Why a GPU device was lost ([`GfxError::DeviceLost`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LossKind {
    /// The driver lost it: it crashed or was reset, as Windows resets a GPU that takes too long to
    /// answer, or the GPU was removed.
    Driver,
    /// The program lost it on purpose ([`Gpu::lose`]), or destroyed it.
    Destroyed,
    /// Memory ran out where no caller could be told (ADR-0044), and the engine gave the device up.
    /// A device that replaced it would run out the same way, so none does
    /// ([`Gpu::restore`]).
    OutOfMemory,
}

/// A device's loss: why, and what the driver or the engine said of it.
#[derive(Clone, Debug)]
pub(crate) struct Loss {
    pub(crate) kind: LossKind,
    pub(crate) message: String,
}

/// What opening a device to replace a lost one needs to know of it, once the lost `Gpu` is
/// released: whether it rendered on the CPU, and its adapter's name.
pub(crate) struct Lost {
    software: bool,
    adapter_name: String,
}

impl Lost {
    /// The description of `gpu`, which is released, or why no device should replace it: it was
    /// lost because memory ran out.
    ///
    /// # Panics
    ///
    /// If `gpu` is not lost.
    pub(crate) fn of(gpu: Gpu) -> Result<Self, GfxError> {
        let loss = gpu
            .loss
            .get()
            .expect("only a lost device is restored: Gpu::check says whether it is");
        if loss.kind == LossKind::OutOfMemory {
            return Err(GfxError::CannotRestore {
                reason: format!(
                    "it was lost because memory ran out, and a new one would run out the same way \
                     ({})",
                    loss.message
                ),
            });
        }
        Ok(Self {
            software: gpu.software,
            adapter_name: gpu.adapter_name.clone(),
        })
    }
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
    /// that runs out is not a bug: see [`GfxError::OutOfMemory`]. The device has every limit that
    /// its adapter offers (ADR-0114), which must reach the engine's floor, WebGPU's default limits
    /// (wgpu's `Limits::default`): among them, 4 bind groups per pipeline, 16 sampled textures and
    /// 8 storage buffers per shader stage, textures of 8192 texels a side, buffers of 256 MiB and
    /// storage bindings of 128 MiB, offsets aligned on 256 bytes. The engine draws everything it
    /// draws within them; every desktop GPU of the last ten years offers more, as lavapipe and WARP
    /// do: textures of 16 384 texels a side on all three of the drivers the engine is checked on,
    /// storage bindings of 2 GiB on the RX 6900 XT and WARP, of 128 MiB on lavapipe. It has the
    /// optional features that the adapter offers among those the engine uses: timestamp queries
    /// ([`Gpu::can_time`]) and BC texture compression ([`Gpu::can_sample_bc`]).
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
    /// [`GfxError::NoAdapter`] if no adapter of the graphics API is available;
    /// [`GfxError::BelowFloor`] if the adapter offers less than the floor of one of its limits,
    /// which it names; and [`GfxError::RequestDevice`] if the adapter refuses to open a device, as
    /// when the memory budget is too small for it.
    pub fn new() -> Result<Self, GfxError> {
        Self::open(&create_instance(), None, None).map(|(gpu, _adapter)| gpu)
    }

    /// Opens a device to replace this one, which was lost, without a window: the most capable
    /// adapter of the same wgpu instance, as [`Gpu::new`] chooses it, which is the lost device's
    /// adapter again after a driver reset. It takes the lost `Gpu`, and releases it before the new
    /// device opens: every other clone of it, and every object it created, must be dropped first,
    /// since Direct3D 12 opens no device on an adapter while a removed one still exists in the
    /// process, and would offer only another adapter. Every object must then be created again with
    /// the new device; using one of the lost device's with it panics. A program that draws into a
    /// window replaces its device with [`Surface::restore`](crate::Surface::restore) instead, which
    /// opens one that can draw into the window. Applications built on `slime-app` choose with
    /// `AppConfig` whether the run does it for them (ADR-0075).
    ///
    /// # Errors
    ///
    /// [`GfxError::CannotRestore`] when no device should replace this one: it was lost because
    /// memory ran out ([`LossKind::OutOfMemory`]), or the only adapter left renders on the CPU
    /// where the lost one did not, which would draw a frame in seconds rather than milliseconds.
    /// As [`Gpu::new`] when no device opens.
    ///
    /// # Panics
    ///
    /// If the device is not lost ([`Gpu::check`]).
    pub fn restore(self) -> Result<Gpu, GfxError> {
        let instance = self.instance.clone();
        let lost = Lost::of(self)?;
        Self::open(&instance, None, Some(&lost)).map(|(gpu, _adapter)| gpu)
    }

    /// Loses the device on purpose, as a driver reset would: every object it created can no longer
    /// be used, and [`Gpu::check`] reports the loss at once, as [`LossKind::Destroyed`] unless it
    /// was already lost otherwise. For the tests and tools that check how a program goes on after a
    /// lost device.
    pub fn lose(&self) {
        let _ = self.loss.set(Loss {
            kind: LossKind::Destroyed,
            message: "the program lost it on purpose".to_owned(),
        });
        self.device.destroy();
    }

    /// Opens the most capable adapter of `instance`, among those that can present to
    /// `compatible_surface` if there is one, and its device; to replace the device that `lost`
    /// describes if it is given, refusing a CPU adapter after a hardware one.
    pub(crate) fn open(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
        lost: Option<&Lost>,
    ) -> Result<(Self, wgpu::Adapter), GfxError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            ..Default::default()
        }))
        .map_err(|error| GfxError::NoAdapter(Box::new(error)))?;
        let info = adapter.get_info();
        let software = info.device_type == wgpu::DeviceType::Cpu;
        if let Some(lost) = lost
            && software
            && !lost.software
        {
            return Err(GfxError::CannotRestore {
                reason: format!(
                    "the only adapter left, {}, renders on the CPU, where the lost device was {}",
                    info.name, lost.adapter_name
                ),
            });
        }
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
        // Every limit the adapter offers, so that what the engine sizes by the device's limits,
        // the instances a frame draws among them, grows with the GPU (ADR-0114).
        let required_limits = adapter.limits();
        check_floor(&info.name, &required_limits, &floor())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SlimeEngine device"),
            required_features,
            required_limits,
            ..Default::default()
        }))
        .map_err(|error| GfxError::RequestDevice(Box::new(error)))?;
        let loss = Arc::new(OnceLock::new());
        device.set_device_lost_callback({
            let loss = Arc::clone(&loss);
            move |reason, message: String| {
                let kind = match reason {
                    wgpu::DeviceLostReason::Destroyed => LossKind::Destroyed,
                    _ => LossKind::Driver,
                };
                // Recorded before it is logged, which can take a while, so that other threads see
                // the loss as soon as possible: wgpu holds the device invalid already. Only the
                // first loss counts: one that `lose` or `lost` recorded is not overwritten.
                let _ = loss.set(Loss {
                    kind,
                    message: message.clone(),
                });
                match kind {
                    LossKind::Driver => tracing::error!(%message, "GPU device lost"),
                    _ => tracing::debug!("GPU device destroyed"),
                }
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
                    let _ = loss.set(Loss {
                        kind: LossKind::OutOfMemory,
                        message: format!("out of GPU memory where no caller can be told: {error}"),
                    });
                } else {
                    panic!("wgpu error: {error}");
                }
            }
        }));

        let gpu = Self {
            device,
            queue,
            instance: instance.clone(),
            adapter_name: info.name,
            software,
            loss,
            submissions: Arc::default(),
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
    /// [`GfxError::DeviceLost`] once the device is lost, with why ([`LossKind`]) and the reason
    /// the driver gave.
    pub fn check(&self) -> Result<(), GfxError> {
        match self.loss.get() {
            Some(loss) => Err(GfxError::DeviceLost {
                kind: loss.kind,
                message: loss.message.clone(),
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
    /// [`GfxError::DeviceLost`] if the device is lost once `create` returns, recorded or not yet
    /// ([`Gpu::still_valid`]). On Vulkan, a driver that fails to create a shader or pipeline (which
    /// a conformant one does only when it runs out of memory) makes wgpu lose the device, and no
    /// scope sees an error.
    pub(crate) fn catch_errors<T>(
        &self,
        create: impl FnOnce() -> T,
    ) -> Result<(T, Option<wgpu::Error>), GfxError> {
        let (created, error) = within_error_scopes(&self.device, create);
        self.check()?;
        if !self.still_valid() {
            return Err(self.lost("creating a shader or a pipeline"));
        }
        Ok((created, error))
    }

    /// Runs `create`, which creates the GPU objects of `label`, inside an out-of-memory error
    /// scope, and returns what it created, which can be used: wgpu held the device valid once
    /// `create` returned ([`Gpu::still_valid`]). On a lost device, `create` does not run: nothing
    /// could be created. Errors of other kinds reach the error handler.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if wgpu reports that the GPU's memory cannot hold what `create`
    /// asked for, and [`GfxError::DeviceLost`] if the device is lost before `create` runs or once
    /// it returns, recorded or not yet.
    pub(crate) fn create<T>(&self, label: &str, create: impl FnOnce() -> T) -> Result<T, GfxError> {
        self.check()?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let created = create();
        let valid = self.still_valid();
        let error = pollster::block_on(scope.pop());
        self.check()?;
        match error {
            Some(_) => Err(GfxError::OutOfMemory {
                label: label.to_owned(),
            }),
            None if !valid => Err(self.lost(&format!("creating `{label}`"))),
            None => Ok(created),
        }
    }

    /// Whether wgpu still holds the device valid. wgpu marks a device lost before it reports the
    /// loss, and does not say so otherwise: every creation in between makes objects that cannot be
    /// used, which wgpu then reports as a misuse, an error that panics (ADR-0022). It asks: a
    /// buffer of 4 bytes created mapped can be written only on a valid device, or one whose memory
    /// is full, which wgpu reports, where it says nothing of a lost device. It costs that buffer
    /// and an error scope: in a release build on lavapipe, the question alone took 1.2 to 1.6 µs,
    /// and creating a vertex buffer of 1 KiB 6.8 to 9.5 µs with it, 5.2 to 7.2 µs without, from one
    /// run to the next.
    pub(crate) fn still_valid(&self) -> bool {
        // Its own scope, so that memory full is neither taken for the creation of a caller's
        // object nor reported where no caller hears of it, which gives the device up (ADR-0044).
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let probe = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SlimeEngine probe of the device"),
            size: wgpu::COPY_BUFFER_ALIGNMENT,
            usage: wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: true,
        });
        let mapped = probe.get_mapped_range_mut(..).is_ok();
        let out_of_memory = pollster::block_on(scope.pop()).is_some();
        mapped || out_of_memory
    }

    /// The loss of the device that `what` found, an operation that fails only on a lost device:
    /// recorded, as the driver's, if wgpu has not reported it yet ([`Gpu::still_valid`]), so that
    /// [`Gpu::check`] reports it from then on.
    pub(crate) fn lost(&self, what: &str) -> GfxError {
        let recorded = self.loss.set(Loss {
            kind: LossKind::Driver,
            message: format!("{what} failed, as it does once the device is lost"),
        });
        if recorded.is_ok() {
            tracing::error!("the GPU device is lost: {what} failed before wgpu reported the loss");
        }
        self.check()
            .expect_err("the loss is recorded, by wgpu or above")
    }

    /// Blocks until `ready` gives what it waits for, which a callback of wgpu's gives once the GPU
    /// has finished the work it waits for, and returns it.
    ///
    /// wgpu runs such callbacks when the device is polled or work is submitted. It treats a poll
    /// that finds the device lost as fatal and panics, where a submission only reports the loss
    /// (ADR-0022): the wait submits empty work, sleeping in between, rather than poll, as
    /// [`Gpu::wait_idle`] says.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] if the device is lost before `ready` gives anything.
    pub(crate) fn wait_for<T>(&self, mut ready: impl FnMut() -> Option<T>) -> Result<T, GfxError> {
        let mut pause = Duration::from_micros(50);
        loop {
            if let Some(ready) = ready() {
                return Ok(ready);
            }
            self.check()?;
            if !self.still_valid() {
                return Err(self.lost("waiting for the GPU"));
            }
            submit(self, []);
            if let Some(ready) = ready() {
                return Ok(ready);
            }
            std::thread::sleep(pause);
            pause = (pause * 2).min(Duration::from_millis(1));
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
        // returned as it is, and `create`, which this runs in, reports why (ADR-0022, ADR-0044).
        if let Ok(mut mapped) = buffer.get_mapped_range_mut(..) {
            mapped.slice(..contents.len()).copy_from_slice(contents);
            drop(mapped);
            buffer.unmap();
        }
        buffer
    }

    /// Creates a buffer of `usage` and of `size` bytes, all zero, inside [`create`](Self::create),
    /// which reports a buffer that could not be created: wgpu zeroes what is not written.
    pub(crate) fn create_zeroed_buffer(
        &self,
        label: &str,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
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
    /// dynamic offset (256 at most: the engine's floor, [`Gpu::new`]): a uniform buffer that holds
    /// the values of many draws lays them out at multiples of it.
    pub fn uniform_offset_alignment(&self) -> usize {
        self.device.limits().min_uniform_buffer_offset_alignment as usize
    }

    /// The largest width or height, in pixels, of a texture, a render target, a depth target or a
    /// window's image: the adapter's, 8192 at least, the engine's floor. A caller that creates
    /// textures from files checks their size against it: [`Gpu::create_texture`] panics on a larger
    /// one.
    pub fn max_texture_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// The largest size, in bytes, of a buffer: the adapter's, 256 MiB at least, the engine's
    /// floor. A caller that
    /// creates buffers from files checks their size against it: [`Gpu::create_vertex_buffer`] and
    /// [`Gpu::create_index_buffer`] panic on a larger one.
    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size
    }

    /// The largest storage buffer that a shader reads at once, in bytes: the size of the largest
    /// buffer the device creates or of the largest range it binds for storage, whichever is
    /// smaller: the adapter's, 128 MiB at least, the engine's floor.
    pub fn max_storage_buffer_size(&self) -> u64 {
        let limits = self.device.limits();
        limits
            .max_buffer_size
            .min(limits.max_storage_buffer_binding_size)
    }

    /// The bytes of GPU memory that the device's buffers, textures and other objects take, as the
    /// allocator of wgpu counts them, without what the driver keeps for itself; `None` where wgpu
    /// does not count them, which it does on Vulkan and Direct3D 12. An object dropped gives its
    /// memory back once the GPU has finished the work submitted with it: after
    /// [`Gpu::wait_idle`], at once.
    pub fn memory_in_use(&self) -> Option<u64> {
        self.device
            .generate_allocator_report()
            .map(|report| report.total_allocated_bytes)
    }

    /// Submits an empty piece of work, then blocks until the GPU has finished everything submitted
    /// so far, or the device is lost. It does not poll the device, on which wgpu panics when the
    /// poll finds the device lost (ADR-0103): it submits empty work until the GPU has finished,
    /// sleeping 50 µs at first, then twice as long each time, up to 1 ms, so that it wakes up to
    /// 1 ms after the GPU has finished.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]), before or while it waits.
    pub fn wait_idle(&self) -> Result<(), GfxError> {
        self.check()?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SlimeEngine wait_idle"),
            });
        submit(self, [encoder.finish()]);
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.queue.on_submitted_work_done({
            let done = Arc::clone(&done);
            move || done.store(true, std::sync::atomic::Ordering::Release)
        });
        self.wait_for(|| {
            done.load(std::sync::atomic::Ordering::Acquire)
                .then_some(())
        })
    }
}

/// Submits `commands` to the queue of `gpu`, unless a surface of its device is being configured,
/// which it waits for ([`Gpu::submissions`]).
pub(crate) fn submit(gpu: &Gpu, commands: impl IntoIterator<Item = wgpu::CommandBuffer>) {
    // Nothing is guarded but the order of the calls: a lock poisoned is as good as whole.
    let _shared = gpu
        .submissions
        .read()
        .unwrap_or_else(PoisonError::into_inner);
    gpu.queue.submit(commands);
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

/// The least that the engine needs of a GPU adapter's limits, the *floor*, which [`Gpu::new`]
/// checks and describes (ADR-0114).
fn floor() -> wgpu::Limits {
    wgpu::Limits::default()
}

/// The largest buffer through which a texture is written or an image is read back at once: the
/// largest buffer at the floor, so that these cost as much memory on every GPU, however large the
/// buffers that its adapter offers.
pub(crate) const LARGEST_STAGING: u64 = 256 << 20;

/// Refuses `offered`, the limits of the adapter named `adapter`, if one is below `floor`, naming
/// the first such limit.
fn check_floor(
    adapter: &str,
    offered: &wgpu::Limits,
    floor: &wgpu::Limits,
) -> Result<(), GfxError> {
    let mut below = None;
    floor.check_limits_with_fail_fn(offered, true, |limit, needed, offered| {
        below = Some((limit, needed, offered));
    });
    match below {
        Some((limit, needed, offered)) => Err(GfxError::BelowFloor {
            adapter: adapter.to_owned(),
            limit,
            needed,
            offered,
        }),
        None => Ok(()),
    }
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
#[path = "../tests/common/memory_budget.rs"]
mod memory_budget;

#[cfg(test)]
mod tests {
    use super::memory_budget::{CHILD_ENV, PROBE, run_out_of_memory_in_a_child};
    use super::*;
    use crate::{RenderPassDescriptor, RenderTarget, TextureFormat};
    use slime_color::LinearColor;

    #[test]
    fn an_adapter_below_the_floor_is_refused_with_the_limit_it_lacks() {
        let floor = floor();
        let offered = wgpu::Limits {
            max_bind_groups: 3,
            ..floor.clone()
        };
        let error = check_floor("an old GPU", &offered, &floor).expect_err("refused");
        assert!(
            matches!(
                &error,
                GfxError::BelowFloor { adapter, limit: "max_bind_groups", needed: 4, offered: 3 }
                    if adapter == "an old GPU"
            ),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "the GPU adapter an old GPU offers 3 for its limit max_bind_groups, where the engine \
             needs 4"
        );
        // An alignment is better the smaller it is.
        let finer = wgpu::Limits {
            min_uniform_buffer_offset_alignment: 64,
            ..floor.clone()
        };
        assert!(check_floor("a GPU", &finer, &floor).is_ok());
        assert_eq!(
            LARGEST_STAGING, floor.max_buffer_size,
            "the floor's largest buffer"
        );
    }

    /// Prints the limits of the device, which CI's log shows for the drivers it runs on.
    #[test]
    fn the_device_opens_above_the_floor_and_prints_its_limits() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let limits = gpu.device.limits();
        assert!(floor().check_limits(&limits), "{limits:#?}");
        eprintln!("the limits of {}: {limits:#?}", gpu.adapter_name);
    }

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
    fn a_device_invalid_before_its_loss_is_recorded_gives_errors_not_panics() {
        // As a driver reset comes: wgpu holds the device invalid before it reports the loss, which,
        // for a device destroyed, it does only when the device is next polled. Creating a texture
        // panicked in wgpu's unmap of its uploads, and reading a target back in its mapping.
        type Call = fn(&Gpu, &RenderTarget) -> Result<(), GfxError>;
        let calls: [(&str, Call); 4] = [
            ("a texture", |gpu, _| {
                gpu.create_texture("lost", 4, 4, TextureFormat::Rgba8UnormSrgb, &[0; 64])
                    .map(|_| ())
            }),
            ("a vertex buffer", |gpu, _| {
                gpu.create_vertex_buffer("lost", &[0; 16]).map(|_| ())
            }),
            ("a readback", |gpu, target| {
                gpu.read_render_target(target).map(|_| ())
            }),
            ("a wait", |gpu, _| gpu.wait_idle()),
        ];
        for (what, call) in calls {
            let gpu = Gpu::new()
                .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
            let target = gpu
                .create_render_target("before", 4, 4, TextureFormat::Rgba8UnormSrgb)
                .expect("the GPU has memory for it");
            gpu.device.destroy();
            assert!(gpu.check().is_ok(), "{what}: the loss is not recorded yet");
            let result = call(&gpu, &target);
            assert!(
                matches!(result, Err(GfxError::DeviceLost { .. })),
                "{what}: {result:?}"
            );
            assert!(gpu.check().is_err(), "{what}: the loss is recorded");
        }
    }

    #[test]
    fn a_device_lost_on_hardware_is_not_replaced_by_one_that_renders_on_the_cpu() {
        let mut lost =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let cpu_only = lost.software;
        lost.lose();
        // As if the lost device had rendered on a GPU, whatever this machine's adapter is.
        lost.software = false;
        let restored = lost.restore();
        if cpu_only {
            assert!(
                matches!(restored, Err(GfxError::CannotRestore { .. })),
                "{restored:?}"
            );
        } else {
            assert!(restored.is_ok(), "{restored:?}");
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

    /// The most render targets of [`filler`] that a child creates: 4 GiB.
    const FILLERS: usize = 8;

    /// A render target of 8192 × 8192 pixels of 8 bytes: 512 MiB.
    fn filler(gpu: &Gpu) -> Result<RenderTarget, GfxError> {
        gpu.create_render_target("filler", 8192, 8192, TextureFormat::Rgba16Float)
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
        let Ok(mode) = std::env::var(CHILD_ENV) else {
            let Some(stdout) = run_out_of_memory_in_a_child(
                "gpu::tests::a_creation_beyond_the_memory_fails_and_the_device_goes_on",
            ) else {
                eprintln!("skipped: no memory budget makes the memory of this driver run out");
                return;
            };
            assert!(stdout.contains("the device goes on"), "{stdout}");
            return;
        };
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
        if mode == PROBE {
            return;
        }
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
        let Ok(mode) = std::env::var(CHILD_ENV) else {
            let Some(stdout) = run_out_of_memory_in_a_child(
                "gpu::tests::memory_that_runs_out_where_no_caller_is_told_gives_the_device_up",
            ) else {
                eprintln!("skipped: no memory budget makes the memory of this driver run out");
                return;
            };
            assert!(stdout.contains("given up"), "{stdout}");
            return;
        };
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
        if mode == PROBE {
            return;
        }
        let Err(GfxError::DeviceLost { kind, message }) = gpu.check() else {
            unreachable!("the loop ends on a loss");
        };
        assert_eq!(kind, LossKind::OutOfMemory);
        assert!(
            message.starts_with("out of GPU memory where no caller can be told"),
            "{message}"
        );
        // Like a lost device, a given-up one creates nothing more, and no device replaces it.
        assert!(matches!(filler(&gpu), Err(GfxError::DeviceLost { .. })));
        assert!(matches!(gpu.restore(), Err(GfxError::CannotRestore { .. })));
        println!("given up");
    }
}
