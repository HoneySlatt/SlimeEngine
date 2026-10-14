//! Timing the GPU's work: how long render passes take on the GPU, when the adapter can tell.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::error::GfxError;
use crate::gpu::Gpu;

/// How many timed submissions can be in flight with their times not read yet; beyond that,
/// passes go untimed until one is read.
const SLOTS: usize = 4;
/// Two timestamps per pass: its start and its end.
const QUERIES_PER_PASS: u32 = 2;
/// The most passes one submission can time: wgpu's query sets hold at most 4096 queries, two per
/// pass in each of the four slots.
const MAX_PASSES: u32 = 512;

/// Numbers the timers, so that a submission tells apart the timers its passes name.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Measures how long render passes take on the GPU: a timestamp is written when a pass that
/// uses the timer ([`RenderPassDescriptor::timer`](crate::RenderPassDescriptor::timer),
/// [`DepthPassDescriptor::timer`](crate::DepthPassDescriptor::timer)) starts and ends, and read
/// back a few frames later, so that nothing waits for the GPU.
///
/// [`Gpu::create_timer`] gives one only when the adapter supports timestamp queries;
/// [`Gpu::can_time`] tells.
///
/// A timer times the passes of one submission ([`Gpu::submit`], or the single pass of
/// [`Gpu::render`]): the passes that name it are numbered from 0 in the order they are recorded,
/// up to the number it was created for, and [`latest`](Self::latest) gives each one's time. The
/// times a timer gives all come from the same submission. Timers are independent: the times of
/// two timers can come from different frames. On Vulkan, wgpu writes both timestamps once the
/// work before them is done, so a pass's time runs from the end of the previous work to the end of
/// the pass: the sum of a frame's pass times leaves out the gaps between submissions, and is not a
/// frame time.
///
/// At most four submissions can wait for their times: while four do, the passes of the next ones go
/// untimed.
pub struct GpuTimer {
    /// Tells this timer apart from the others in a submission.
    id: u64,
    query_set: wgpu::QuerySet,
    /// The most passes one submission times; the others go untimed.
    passes: u32,
    slots: Vec<Slot>,
    /// The slots whose readback is mapped, sent by wgpu's callbacks with how many passes each
    /// timed.
    mapped: mpsc::Receiver<(usize, u32)>,
    sender: mpsc::Sender<(usize, u32)>,
    /// Nanoseconds per timestamp tick.
    period: f32,
    /// The time of each pass of the latest submission whose times came back.
    latest: Box<[Cell<Option<Duration>>]>,
}

/// The buffers of one timed submission.
struct Slot {
    /// Where the timestamps are resolved.
    resolve: wgpu::Buffer,
    /// Where they are copied for the CPU to read.
    readback: wgpu::Buffer,
    /// Whether a submission has taken the slot and its times have not been read yet.
    in_flight: Cell<bool>,
}

impl Gpu {
    /// Whether the adapter can time its work: whether it supports timestamp queries. lavapipe,
    /// RADV and WARP do; `crates/slime-gfx/tests/timing.rs` prints the answer for the adapter it
    /// runs on.
    pub fn can_time(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Creates a timer for up to `passes` passes per submission, or `None` when the adapter
    /// cannot time its work ([`Gpu::can_time`]). `label` names its GPU objects in error messages
    /// and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold its queries and buffers, and
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `passes` is 0 or more than 512: wgpu's query sets hold at most 4096 queries, two per pass
    /// in each of the four submissions a timer can have in flight.
    pub fn create_timer(&self, label: &str, passes: u32) -> Result<Option<GpuTimer>, GfxError> {
        assert!(
            (1..=MAX_PASSES).contains(&passes),
            "timer `{label}` for {passes} passes per submission; a timer times 1 to {MAX_PASSES}"
        );
        if !self.can_time() {
            return Ok(None);
        }
        let (query_set, slots) = self.create(label, || self.create_timer_objects(label, passes))?;
        let (sender, mapped) = mpsc::channel();
        Ok(Some(GpuTimer {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            query_set,
            passes,
            slots,
            mapped,
            sender,
            period: self.queue.get_timestamp_period(),
            latest: (0..passes).map(|_| Cell::new(None)).collect(),
        }))
    }

    /// The query set and the slots of a timer for `passes` passes per submission.
    fn create_timer_objects(&self, label: &str, passes: u32) -> (wgpu::QuerySet, Vec<Slot>) {
        let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some(label),
            ty: wgpu::QueryType::Timestamp,
            count: QUERIES_PER_PASS * passes * SLOTS as u32,
        });
        let size = u64::from(QUERIES_PER_PASS * passes * wgpu::QUERY_SIZE);
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
        (query_set, slots)
    }
}

impl GpuTimer {
    /// The time the GPU took for pass `pass` (from 0, in the order the passes that name this timer
    /// were recorded) of the latest submission whose times have come back: `None` until the first
    /// one does, and for a pass that this submission did not time. A time comes back once wgpu has
    /// seen that the GPU finished the submission, which it checks whenever work is submitted (by
    /// the next frame's passes, for example) and in [`Gpu::wait_idle`]: a frame or more after it.
    /// Reading takes no time worth measuring and never waits for the GPU.
    ///
    /// # Panics
    ///
    /// If `pass` is not below the number of passes the timer was created for.
    pub fn latest(&self, pass: usize) -> Option<Duration> {
        // The callbacks of the readbacks run when work is submitted. Polling the device here would
        // run them sooner, but wgpu treats a poll that finds a lost Vulkan device as fatal, and
        // panics, where a submission only reports the loss (ADR-0022).
        while let Ok((index, timed)) = self.mapped.try_recv() {
            let slot = &self.slots[index];
            match slot.readback.get_mapped_range(..) {
                Ok(mapped) => {
                    let timestamp = |at: usize| {
                        let bytes: [u8; 8] = mapped[at..at + 8].try_into().unwrap_or([0; 8]);
                        u64::from_le_bytes(bytes)
                    };
                    for (pass, latest) in self.latest.iter().enumerate() {
                        latest.set((pass < timed as usize).then(|| {
                            let (start, end) = (timestamp(16 * pass), timestamp(16 * pass + 8));
                            let nanoseconds =
                                end.saturating_sub(start) as f64 * f64::from(self.period);
                            Duration::from_nanos(nanoseconds as u64)
                        }));
                    }
                }
                // wgpu reports the readback as mapped but refuses to give it: nothing to read.
                Err(_) => tracing::debug!("a timed submission could not be read back"),
            }
            slot.readback.unmap();
            slot.in_flight.set(false);
        }
        self.latest[pass].get()
    }

    /// Tells this timer apart from the others in a submission.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Takes a slot for the passes of a submission: what times them and reads their timestamps
    /// back once submitted; `None` when every slot is in flight.
    pub(crate) fn time_submission(&self) -> Option<TimedSubmission> {
        let index = self.slots.iter().position(|slot| !slot.in_flight.get())?;
        let slot = &self.slots[index];
        slot.in_flight.set(true);
        Some(TimedSubmission {
            timer: self.id,
            query_set: self.query_set.clone(),
            resolve: slot.resolve.clone(),
            readback: slot.readback.clone(),
            sender: self.sender.clone(),
            slot: index,
            first: QUERIES_PER_PASS * index as u32 * self.passes,
            passes: self.passes,
            timed: 0,
        })
    }
}

/// The passes of one submission that a timer times, recorded and not submitted yet.
pub(crate) struct TimedSubmission {
    /// The [`GpuTimer::id`] of the timer.
    pub(crate) timer: u64,
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    sender: mpsc::Sender<(usize, u32)>,
    slot: usize,
    /// The slot's first query.
    first: u32,
    /// The most passes it times.
    passes: u32,
    /// The passes timed so far.
    timed: u32,
}

impl TimedSubmission {
    /// The timestamps that the next pass naming the timer writes, or `None` once the timer's
    /// number of passes is timed: that pass goes untimed.
    pub(crate) fn next_pass(&mut self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if self.timed == self.passes {
            return None;
        }
        let first = self.first + QUERIES_PER_PASS * self.timed;
        self.timed += 1;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        })
    }

    /// Records, after the submission's passes, the copy of their timestamps into the readback
    /// buffer.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let queries = QUERIES_PER_PASS * self.timed;
        encoder.resolve_query_set(
            &self.query_set,
            self.first..self.first + queries,
            &self.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve,
            0,
            &self.readback,
            0,
            u64::from(queries * wgpu::QUERY_SIZE),
        );
    }

    /// Asks for the readback to be mapped once the submitted work is done; the callback reports it
    /// to [`GpuTimer::latest`].
    pub(crate) fn map(self) {
        let Self {
            readback,
            sender,
            slot,
            timed,
            ..
        } = self;
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            // The timer may be gone by then; a slot that nobody reads is no loss.
            if result.is_ok() {
                let _ = sender.send((slot, timed));
            }
        });
    }
}

impl std::fmt::Debug for GpuTimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuTimer")
            .field("period", &self.period)
            .field("passes", &self.passes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinearColor, RenderPassDescriptor, TextureFormat};

    #[test]
    fn passes_beyond_the_slots_go_untimed_until_reading_frees_them() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let Some(timer) = gpu
            .create_timer("slots", 1)
            .expect("the GPU has memory for it")
        else {
            eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
            return;
        };
        let target = gpu
            .create_render_target("timed", 4, 4, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: Some(&timer),
        };
        let in_flight = || {
            timer
                .slots
                .iter()
                .filter(|slot| slot.in_flight.get())
                .count()
        };

        // One pass more than there are slots, none of them read: the last finds every slot
        // waiting for its time, and goes untimed rather than map a readback whose map is pending,
        // which wgpu would reject.
        for _ in 0..=SLOTS {
            gpu.render(&pass, |_pass| {});
        }
        assert_eq!(in_flight(), SLOTS, "every slot waits for its time");

        gpu.wait_idle().expect("the GPU finishes the passes");
        assert!(timer.latest(0).is_some(), "the times have come back");
        assert_eq!(in_flight(), 0, "reading them frees every slot");

        gpu.render(&pass, |_pass| {});
        assert_eq!(in_flight(), 1, "a freed slot times the next pass");
        gpu.wait_idle().expect("the GPU finishes the pass");
        // `latest` still gives the time read before, so only the slot shows this one was read.
        timer.latest(0);
        assert_eq!(in_flight(), 0, "reading its time frees the slot again");
    }

    #[test]
    fn one_slot_times_the_passes_of_a_submission_up_to_the_timer_s_number() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let Some(timer) = gpu
            .create_timer("passes", 2)
            .expect("the GPU has memory for it")
        else {
            eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
            return;
        };
        let target = gpu
            .create_render_target("timed", 4, 4, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: Some(&timer),
        };
        let in_flight = || {
            timer
                .slots
                .iter()
                .filter(|slot| slot.in_flight.get())
                .count()
        };

        // Three passes for a timer of two: one slot, the third pass untimed.
        gpu.submit("three passes", |commands| {
            for _ in 0..3 {
                commands.render(&pass, |_pass| {});
            }
            assert_eq!(in_flight(), 1, "one slot for the whole submission");
        });
        gpu.wait_idle().expect("the GPU finishes the passes");
        assert!(timer.latest(0).is_some() && timer.latest(1).is_some());
        assert_eq!(in_flight(), 0, "reading the times frees the slot");

        // A submission of one pass: the second pass's time goes with the older submission.
        gpu.render(&pass, |_pass| {});
        gpu.wait_idle().expect("the GPU finishes the pass");
        assert!(timer.latest(0).is_some());
        assert_eq!(
            timer.latest(1),
            None,
            "the latest submission timed one pass"
        );
    }
}
