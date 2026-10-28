//! Copies to the GPU within a budget of bytes per frame, submitted by one thread (ADR-0107).

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::error::GfxError;
use crate::gpu::{Gpu, submit};
use crate::index_buffer::IndexBuffer;
use crate::render_target::TextureFormat;
use crate::storage_buffer::write_range;
use crate::texture::{Band, Texture};
use crate::vertex_buffer::VertexBuffer;

/// How long a budget lasts when no frame renews it: a frame at 60 frames a second, so that copies
/// go on when nothing is drawn, as while a program waits for a load.
const FRAME_WITHOUT_ONE: Duration = Duration::from_nanos(16_666_667);

/// How long without a frame the uploader waits before it submits copies whatever the budget, as it
/// does before the first frame: a program that draws nothing, as while it waits for a load, has no
/// frame for the copies to hold up, and its load would otherwise go at the budget's pace.
const IDLE: Duration = Duration::from_millis(100);

/// The smallest staging buffer made; larger ones are powers of two, so that a buffer given back
/// serves the next upload of its size.
const SMALLEST_STAGING: u64 = 64 << 10;

/// How many budgets' worth of staging buffers are kept for the next uploads once their copies are
/// done; those beyond are freed.
const KEPT_BUDGETS: u64 = 4;

/// The one owner of the copies to the GPU of what loads bring: textures and ranges of buffers that
/// loading threads write into staging buffers, and that one thread of its own submits, at most
/// `bytes_per_frame` bytes of copies for each renewal of its budget (ADR-0107, ADR-0118). Each call
/// of [`Uploader::frame`] renews it, as a renderer does once per frame that it draws, and so do
/// 16.7 ms without one; copies that come while some of the budget is left go at once. Before the
/// first frame, and once 100 ms have passed without one, as while a program waits for a load
/// without drawing, copies go whatever the budget: there is no frame for them to hold up. The
/// thread waits rather than computes: it hands over copies, which the GPU runs after the frames
/// submitted before them, so that a frame does not wait for what a load brings, and a load takes
/// longer when its bytes exceed the budget. A staging buffer holds at most a budget, its size a
/// power of two from 64 KiB; once the GPU has run its copies, it is kept for the next uploads, up
/// to four budgets of them, and named after the upload that made it in capture tools.
///
/// A texture or a range of a buffer written through it holds what was given once the upload's
/// callback says so; drawing from it before then draws what it held before, zeros for a new
/// texture. Clones share the uploader; the last one dropped submits the copies still waiting,
/// whatever the budget, and ends the thread, which then holds the `Gpu` no more.
///
/// Each upload has an [`UploadRank`]: of the copies that wait, those of the lowest rank go first,
/// and those of one rank in the order they came (ADR-0113), so that what a frame lacks goes before
/// what is fetched ahead.
#[derive(Clone)]
pub struct Uploader {
    owner: Arc<Owner>,
}

impl std::fmt::Debug for Uploader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Uploader")
            .field("bytes_per_frame", &self.owner.shared.budget)
            .finish_non_exhaustive()
    }
}

/// Where an upload's copies stand among those that wait (ADR-0113): the uploader submits those of
/// the lowest rank first, and those of one rank in the order they came. Clones share one rank,
/// which can change while their copies wait, as when what was fetched ahead comes into view. The
/// default is 0, before every other.
#[derive(Clone, Debug, Default)]
pub struct UploadRank(Arc<AtomicU64>);

impl UploadRank {
    /// A rank of `rank`.
    #[must_use]
    pub fn new(rank: u64) -> Self {
        Self(Arc::new(AtomicU64::new(rank)))
    }

    /// Changes the rank of every upload given it, the copies that wait included.
    pub fn set(&self, rank: u64) {
        self.0.store(rank, Ordering::Relaxed);
    }

    /// The rank.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Where a write of [`Uploader::buffers`] goes.
#[derive(Clone, Copy, Debug)]
pub enum UploadTarget<'a> {
    /// A vertex buffer, from a byte.
    Vertices(&'a VertexBuffer),
    /// An index buffer, from a byte: four for each index.
    Indices(&'a IndexBuffer),
}

/// What an upload's callback is told: its copies submitted, or why they could not be.
type Done = Box<dyn FnOnce(Result<(), GfxError>) + Send>;

/// The clones' share, which stops the thread when the last is dropped.
struct Owner {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.wake.notify_one();
        if let Some(thread) = self.thread.take() {
            // The thread submits what waits, then ends; its panic, a bug, is raised here.
            if let Err(panic) = thread.join() {
                std::panic::resume_unwind(panic);
            }
        }
    }
}

/// What the clones and the thread share.
struct Shared {
    gpu: Gpu,
    budget: u64,
    state: Mutex<State>,
    /// Told when copies wait, when a frame renews the budget, and when the uploader stops.
    wake: Condvar,
}

struct State {
    /// The copies waiting, one staging buffer each, in the order they were given.
    parts: Vec<Part>,
    /// The bytes that may still be submitted before the budget is renewed.
    allowance: u64,
    /// When the budget was last renewed.
    renewed: Instant,
    /// When [`Uploader::frame`] was last called, if it was.
    framed: Option<Instant>,
    /// Staging buffers whose copies are done, mapped for the next uploads.
    free: Vec<wgpu::Buffer>,
    /// Their bytes.
    free_bytes: u64,
    stop: bool,
}

/// One staging buffer and the copies from it, submitted together.
struct Part {
    /// The rank of its upload.
    rank: UploadRank,
    buffer: wgpu::Buffer,
    copies: Vec<StagedCopy>,
    /// The bytes that the copies bring.
    bytes: u64,
    /// The callback of the upload whose last part this is.
    done: Option<Done>,
}

/// A staging buffer, mapped and written, and the copies from it.
pub(crate) struct Staged {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) copies: Vec<StagedCopy>,
}

/// A copy from a staging buffer, once the bytes it brings are written there.
pub(crate) enum StagedCopy {
    /// A band of rows of a level of a texture.
    Band {
        texture: wgpu::Texture,
        band: Band,
        /// The width that the copy spans, in texels: the level's, rounded up to whole blocks.
        width: u32,
        /// The side of a block, in texels: 4 for a block-compressed format, otherwise 1.
        block: u32,
    },
    /// A range of a buffer.
    Range {
        to: wgpu::Buffer,
        /// Where the bytes are in the staging buffer.
        from: u64,
        /// Where they go in `to`.
        offset: u64,
        size: u64,
    },
}

impl StagedCopy {
    /// Records the copy from `buffer`, the staging buffer that holds what it brings.
    pub(crate) fn record(&self, encoder: &mut wgpu::CommandEncoder, buffer: &wgpu::Buffer) {
        match self {
            Self::Band {
                texture,
                band,
                width,
                block,
            } => encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: band.offset,
                        bytes_per_row: Some(band.pitch),
                        rows_per_image: Some(band.rows),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: band.level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: band.first_row * block,
                        z: band.layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: *width,
                    height: band.rows * block,
                    depth_or_array_layers: 1,
                },
            ),
            Self::Range {
                to,
                from,
                offset,
                size,
            } => encoder.copy_buffer_to_buffer(buffer, *from, to, *offset, *size),
        }
    }

    /// The bytes that the copy brings.
    fn bytes(&self) -> u64 {
        match self {
            Self::Band { band, .. } => u64::from(band.pitch) * u64::from(band.rows),
            Self::Range { size, .. } => *size,
        }
    }
}

impl Uploader {
    /// An uploader of `gpu`'s that submits at most `bytes_per_frame` bytes of copies per frame,
    /// and its thread, which waits for copies while there are none.
    ///
    /// # Panics
    ///
    /// If `bytes_per_frame` is below 64 KiB, the smallest staging buffer, or not a multiple of 4
    /// bytes, as copies ask, or if the operating system cannot start the thread.
    pub fn new(gpu: &Gpu, bytes_per_frame: u64) -> Self {
        assert!(
            bytes_per_frame >= SMALLEST_STAGING && bytes_per_frame.is_multiple_of(4),
            "an uploader submits at least {SMALLEST_STAGING} bytes per frame, in multiples of 4, \
             not {bytes_per_frame}"
        );
        let shared = Arc::new(Shared {
            gpu: gpu.clone(),
            budget: bytes_per_frame,
            state: Mutex::new(State {
                parts: Vec::new(),
                allowance: bytes_per_frame,
                renewed: Instant::now(),
                framed: None,
                free: Vec::new(),
                free_bytes: 0,
                stop: false,
            }),
            wake: Condvar::new(),
        });
        let thread = thread::Builder::new()
            .name("slime-gfx uploads".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                move || shared.submit_while_it_runs()
            })
            .expect("the operating system starts the uploader's thread");
        Self {
            owner: Arc::new(Owner {
                shared,
                thread: Some(thread),
            }),
        }
    }

    /// Says that a frame was submitted: renews the budget, so that the thread submits the next
    /// copies, which the GPU runs after that frame, and holds them to the budget for the next
    /// 100 ms. A renderer calls it once its frame is submitted.
    pub fn frame(&self) {
        let shared = &self.owner.shared;
        let mut state = shared.lock();
        let now = Instant::now();
        state.allowance = shared.budget;
        state.renewed = now;
        state.framed = Some(now);
        drop(state);
        shared.wake.notify_one();
    }

    /// Creates a texture of `width` × `height` pixels, at `rank`, whose levels `levels` gives, as
    /// [`Gpu::create_texture_with_levels`] takes them, writes them into staging buffers on the
    /// calling thread, and leaves their copies to the uploader's thread, which calls `done` once
    /// it has submitted the last, or with the error that kept them from the GPU. `done` runs on the
    /// uploader's thread, and holds up the next copies while it runs: it does little, as telling a
    /// load. The texture is returned at once, to be drawn once `done` says so. A panic of the
    /// uploader's submission, a bug, drops `done` uncalled.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_texture_with_levels`]; `done` is not called then.
    ///
    /// # Panics
    ///
    /// As [`Gpu::create_texture_with_levels`]; and if a row of a level, its bytes rounded up to a
    /// multiple of 256, is larger than the uploader's budget, which a texture of more than 8192
    /// half-float texels a side passes with the smallest budget, 64 KiB.
    pub fn texture(
        &self,
        label: &str,
        rank: &UploadRank,
        (width, height): (u32, u32),
        format: TextureFormat,
        levels: &[&[u8]],
        done: impl FnOnce(Result<(), GfxError>) + Send + 'static,
    ) -> Result<Texture, GfxError> {
        let shared = &self.owner.shared;
        let (texture, staged) = shared.gpu.stage_texture(
            label,
            (width, height, 1),
            format,
            levels,
            shared.budget,
            |size| shared.staging(label, size),
        )?;
        let parts = staged
            .into_iter()
            .map(|Staged { buffer, copies }| Part {
                rank: rank.clone(),
                bytes: copies.iter().map(StagedCopy::bytes).sum(),
                buffer,
                copies,
                done: None,
            })
            .collect();
        shared.give(parts, Box::new(done));
        Ok(texture)
    }

    /// Writes each of `writes`, a target, the byte where its bytes go and the bytes, into staging
    /// buffers on the calling thread, and leaves their copies to the uploader's thread, which
    /// calls `done` as [`Uploader::texture`] says; with no write, `done` is called at once, on the
    /// calling thread. `label` names the staging buffers in error messages and in GPU capture
    /// tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold a staging buffer, and
    /// [`GfxError::DeviceLost`] once the device is lost; `done` is not called then.
    ///
    /// # Panics
    ///
    /// If a write is empty, starts at a byte that is not a multiple of 4, holds a number of bytes
    /// that is not, or reaches beyond its target, as [`Gpu::write_vertex_buffer`] says.
    ///
    /// If a buffer written belongs to another `Gpu` than the uploader's.
    #[track_caller]
    pub fn buffers(
        &self,
        label: &str,
        rank: &UploadRank,
        writes: &[(UploadTarget<'_>, u64, &[u8])],
        done: impl FnOnce(Result<(), GfxError>) + Send + 'static,
    ) -> Result<(), GfxError> {
        let shared = &self.owner.shared;
        // Each write cut into pieces of at most a budget, the pieces packed into staging buffers
        // of at most a budget each: where each piece is, and the size of each buffer.
        let mut pieces = Vec::new();
        let mut sizes = vec![0_u64];
        let budget = usize::try_from(shared.budget).unwrap_or(usize::MAX);
        for &(target, offset, bytes) in writes {
            let (kind, name, to, size) = match target {
                UploadTarget::Vertices(buffer) => {
                    shared.gpu.id.owns(buffer.gpu, "a vertex buffer");
                    ("vertex", &buffer.label, &buffer.buffer, buffer.size)
                }
                UploadTarget::Indices(buffer) => {
                    shared.gpu.id.owns(buffer.gpu, "an index buffer");
                    (
                        "index",
                        &buffer.label,
                        &buffer.buffer,
                        u64::from(buffer.count()) * 4,
                    )
                }
            };
            write_range(kind, name, size, offset, bytes);
            for (index, piece) in bytes.chunks(budget).enumerate() {
                let length = piece.len() as u64;
                // A mapped range starts at a multiple of 8 bytes (`wgpu::MAP_ALIGNMENT`).
                let mut from = sizes.last().expect("there is a buffer").next_multiple_of(8);
                if from + length > shared.budget {
                    sizes.push(0);
                    from = 0;
                }
                let buffer = sizes.len() - 1;
                pieces.push((buffer, from, to, offset + (index * budget) as u64, piece));
                sizes[buffer] = from + length;
            }
        }
        let buffers = sizes
            .iter()
            .filter(|&&size| size > 0)
            .map(|&size| shared.staging(label, size))
            .collect::<Result<Vec<wgpu::Buffer>, GfxError>>()?;
        let mut copies: Vec<Vec<StagedCopy>> = buffers.iter().map(|_| Vec::new()).collect();
        for (buffer, from, to, offset, piece) in pieces {
            let length = piece.len() as u64;
            // As `Gpu::stage_texture` says of a buffer given mapped.
            let Ok(mut mapped) = buffers[buffer].get_mapped_range_mut(from..from + length) else {
                assert!(
                    shared.gpu.check().is_err() || !shared.gpu.still_valid(),
                    "the upload of `{label}` cannot be mapped, on a device that is not lost"
                );
                return Err(shared.gpu.lost(&format!("mapping the upload of `{label}`")));
            };
            mapped.copy_from_slice(piece);
            drop(mapped);
            copies[buffer].push(StagedCopy::Range {
                to: to.clone(),
                from,
                offset,
                size: length,
            });
        }
        let parts = buffers
            .into_iter()
            .zip(copies)
            .map(|(buffer, copies)| Part {
                rank: rank.clone(),
                bytes: copies.iter().map(StagedCopy::bytes).sum(),
                buffer,
                copies,
                done: None,
            })
            .collect();
        shared.give(parts, Box::new(done));
        Ok(())
    }
}

impl State {
    /// The part to submit next: the first given of those of the lowest rank.
    fn next(&self) -> Option<usize> {
        // The first index of the lowest rank: `min_by_key` keeps the first of equal keys.
        (0..self.parts.len()).min_by_key(|&index| self.parts[index].rank.get())
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // What runs under the lock does not panic: a lock poisoned is as good as whole.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A staging buffer of at least `size` bytes, mapped: one given back by an earlier upload, of
    /// the same power of two, or a new one.
    fn staging(&self, label: &str, size: u64) -> Result<wgpu::Buffer, GfxError> {
        let size = size.next_power_of_two().max(SMALLEST_STAGING);
        {
            let mut state = self.lock();
            if let Some(index) = state.free.iter().position(|buffer| buffer.size() == size) {
                state.free_bytes -= size;
                return Ok(state.free.swap_remove(index));
            }
        }
        self.gpu.create(label, || {
            self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: true,
            })
        })
    }

    /// Queues `parts`, an upload's, whose last calls `done`; an upload of no part calls it now.
    fn give(&self, mut parts: Vec<Part>, done: Done) {
        let Some(last) = parts.last_mut() else {
            done(Ok(()));
            return;
        };
        last.done = Some(done);
        self.lock().parts.extend(parts);
        self.wake.notify_one();
    }

    /// The uploader's thread: submits the parts waiting, within the budget while frames are drawn,
    /// until it stops, then those left whatever the budget.
    fn submit_while_it_runs(self: Arc<Self>) {
        loop {
            let (parts, stopping) = {
                let mut state = self.lock();
                let unbounded = loop {
                    let now = Instant::now();
                    let since = now.duration_since(state.renewed);
                    if since >= FRAME_WITHOUT_ONE {
                        state.allowance = self.budget;
                        state.renewed = now;
                    }
                    let until_idle = state
                        .framed
                        .map(|framed| IDLE.saturating_sub(now.duration_since(framed)))
                        .filter(|left| !left.is_zero());
                    let unbounded = state.stop || until_idle.is_none();
                    let fits = state.next().is_some_and(|next| {
                        unbounded || state.parts[next].bytes <= state.allowance
                    });
                    if state.stop || fits {
                        break unbounded;
                    }
                    state = if state.parts.is_empty() {
                        self.wake
                            .wait(state)
                            .unwrap_or_else(PoisonError::into_inner)
                    } else {
                        let renewal = FRAME_WITHOUT_ONE.saturating_sub(since);
                        self.wake
                            .wait_timeout(
                                state,
                                until_idle.map_or(renewal, |left| left.min(renewal)),
                            )
                            .unwrap_or_else(PoisonError::into_inner)
                            .0
                    };
                };
                let mut parts = Vec::new();
                while let Some(next) = state.next() {
                    let bytes = state.parts[next].bytes;
                    if !unbounded && bytes > state.allowance {
                        break;
                    }
                    state.allowance = state.allowance.saturating_sub(bytes);
                    parts.push(state.parts.remove(next));
                }
                (parts, state.stop)
            };
            // A panic of wgpu's, a bug, ends the submission, not the thread: the uploads of the
            // parts that it drops are told so ([`Uploader::texture`]), and the next ones go on.
            if panic::catch_unwind(AssertUnwindSafe(|| self.submit(parts))).is_err() {
                tracing::error!("the uploader's submission panicked; its uploads are dropped");
            }
            if stopping {
                return;
            }
        }
    }

    /// Submits the copies of `parts`, gives their staging buffers back once the GPU has run them,
    /// and tells the uploads that end with them. Called without the lock: a submission runs the
    /// callbacks of buffers mapped again, which take it.
    fn submit(self: &Arc<Self>, parts: Vec<Part>) {
        if parts.is_empty() {
            return;
        }
        if self.gpu.check().is_ok() {
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("uploads"),
                    });
            for part in &parts {
                part.buffer.unmap();
                for copy in &part.copies {
                    copy.record(&mut encoder, &part.buffer);
                }
            }
            submit(&self.gpu, [encoder.finish()]);
        }
        for part in parts {
            self.recycle(part.buffer);
            if let Some(done) = part.done {
                // A device lost before or while the copies were submitted drops them.
                done(self.gpu.check());
            }
        }
    }

    /// Maps `buffer` again, which happens once the GPU has run the copies from it, and keeps it for
    /// the next uploads, unless enough are kept.
    fn recycle(self: &Arc<Self>, buffer: wgpu::Buffer) {
        // The callback runs inside a later submission, on whichever thread submits.
        let shared: Weak<Self> = Arc::downgrade(self);
        let kept = buffer.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Write, move |mapped| {
                let Some(shared) = shared.upgrade() else {
                    return;
                };
                if mapped.is_ok() {
                    let mut state = shared.lock();
                    if state.free_bytes + kept.size() <= KEPT_BUDGETS * shared.budget {
                        state.free_bytes += kept.size();
                        state.free.push(kept);
                    }
                }
            });
    }
}
