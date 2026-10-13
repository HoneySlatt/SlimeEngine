//! Timing the GPU's work: how long a render pass takes on the GPU, when the adapter can tell.

use std::cell::Cell;
use std::sync::mpsc;
use std::time::Duration;

use crate::gpu::Gpu;

/// How many timed passes can be in flight with their times not read yet; beyond that, passes
/// go untimed until one is read.
const SLOTS: usize = 4;
/// Two timestamps per slot: the start and the end of the pass.
const QUERIES_PER_SLOT: u32 = 2;

/// Measures how long render passes take on the GPU: a timestamp is written when a pass that
/// uses the timer ([`RenderPassDescriptor::timer`](crate::RenderPassDescriptor::timer),
/// [`DepthPassDescriptor::timer`](crate::DepthPassDescriptor::timer)) starts and ends, and read
/// back a few frames later, so that nothing waits for the GPU.
///
/// [`Gpu::create_timer`] gives one only when the adapter supports timestamp queries;
/// [`Gpu::can_time`] tells.
///
/// A timer measures one pass at a time. To time several passes of a frame, give each its own
/// timer and read each with [`latest`](Self::latest): its slots, query set and readbacks are its
/// own. The times of different timers come back independently, so they can come from different
/// frames. On Vulkan, wgpu writes both timestamps once the work before them is done, so a pass's
/// time runs from the end of the previous work to the end of the pass: the sum of a frame's pass
/// times leaves out the gaps between submissions, and is not a frame time. A timer named by
/// several passes of one frame reports whichever of them came back last.
pub struct GpuTimer {
    query_set: wgpu::QuerySet,
    slots: Vec<Slot>,
    /// Slot numbers whose readback is mapped, sent by wgpu's callbacks.
    mapped: mpsc::Receiver<usize>,
    sender: mpsc::Sender<usize>,
    /// Nanoseconds per timestamp tick.
    period: f32,
    latest: Cell<Option<Duration>>,
}

/// The buffers of one timed pass.
struct Slot {
    /// Where the two timestamps are resolved.
    resolve: wgpu::Buffer,
    /// Where they are copied for the CPU to read.
    readback: wgpu::Buffer,
    /// Whether the pass has been submitted and its time not read yet.
    in_flight: Cell<bool>,
}

impl Gpu {
    /// Whether the adapter can time its work: whether it supports timestamp queries. lavapipe
    /// and WARP are measured in `docs/PLAN.md`.
    pub fn can_time(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Creates a timer, or `None` when the adapter cannot time its work ([`Gpu::can_time`]).
    /// `label` names its GPU objects in error messages and in GPU capture tools.
    pub fn create_timer(&self, label: &str) -> Option<GpuTimer> {
        if !self.can_time() {
            return None;
        }
        let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some(label),
            ty: wgpu::QueryType::Timestamp,
            count: QUERIES_PER_SLOT * SLOTS as u32,
        });
        let size = u64::from(QUERIES_PER_SLOT * wgpu::QUERY_SIZE);
        let slots = (0..SLOTS)
            .map(|_| Slot {
                resolve: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{label} timestamps")),
                    size,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{label} timestamps readback")),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                in_flight: Cell::new(false),
            })
            .collect();
        let (sender, mapped) = mpsc::channel();
        Some(GpuTimer {
            query_set,
            slots,
            mapped,
            sender,
            period: self.queue.get_timestamp_period(),
            latest: Cell::new(None),
        })
    }
}

impl GpuTimer {
    /// The time the GPU took for the last timed pass whose time has come back: `None` until the
    /// first one does, a few frames after it was submitted. Reading takes no time worth measuring
    /// and never waits for the GPU.
    pub fn latest(&self, gpu: &Gpu) -> Option<Duration> {
        // Lets wgpu run the callbacks of the readbacks that have completed.
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        while let Ok(index) = self.mapped.try_recv() {
            let slot = &self.slots[index];
            let timestamps = slot.readback.get_mapped_range(..).ok().map(|mapped| {
                let timestamp = |at: usize| {
                    let bytes: [u8; 8] = mapped[at..at + 8].try_into().unwrap_or([0; 8]);
                    u64::from_le_bytes(bytes)
                };
                (timestamp(0), timestamp(8))
            });
            slot.readback.unmap();
            slot.in_flight.set(false);
            match timestamps {
                Some((start, end)) => {
                    let nanoseconds = end.saturating_sub(start) as f64 * f64::from(self.period);
                    self.latest
                        .set(Some(Duration::from_nanos(nanoseconds as u64)));
                }
                // wgpu reports the readback as mapped but refuses to give it: nothing to read.
                None => tracing::debug!("a timed pass could not be read back"),
            }
        }
        self.latest.get()
    }

    /// Reserves a slot for a pass, or `None` when every slot is in flight.
    pub(crate) fn reserve(&self) -> Option<usize> {
        let index = self.slots.iter().position(|slot| !slot.in_flight.get())?;
        self.slots[index].in_flight.set(true);
        Some(index)
    }

    /// The timestamps a pass writes into `slot`.
    pub(crate) fn writes(&self, slot: usize) -> wgpu::RenderPassTimestampWrites<'_> {
        let first = QUERIES_PER_SLOT * slot as u32;
        wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    /// Records, after the pass, the copy of `slot`'s timestamps into its readback buffer.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let first = QUERIES_PER_SLOT * slot as u32;
        let buffers = &self.slots[slot];
        encoder.resolve_query_set(
            &self.query_set,
            first..first + QUERIES_PER_SLOT,
            &buffers.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &buffers.resolve,
            0,
            &buffers.readback,
            0,
            u64::from(QUERIES_PER_SLOT * wgpu::QUERY_SIZE),
        );
    }

    /// Asks for `slot`'s readback to be mapped once the submitted work is done; the callback
    /// reports it to [`latest`](Self::latest).
    pub(crate) fn map(&self, slot: usize) {
        let sender = self.sender.clone();
        self.slots[slot]
            .readback
            .map_async(wgpu::MapMode::Read, .., move |result| {
                // The timer may be gone by then; a slot that nobody reads is no loss.
                if result.is_ok() {
                    let _ = sender.send(slot);
                }
            });
    }
}

impl std::fmt::Debug for GpuTimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuTimer")
            .field("period", &self.period)
            .field("latest", &self.latest.get())
            .finish_non_exhaustive()
    }
}
