//! Picking by the GPU (`docs/adr/0130-picking.md`): which instance is drawn at each pixel of an
//! area of a view, asked for at a frame and answered a frame or more later, without holding up a
//! frame.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use slime_gfx::{Gpu, Readback};

use crate::error::RenderError;
use crate::renderer::Viewport;

/// A picking asked of a view ([`View::pick`](crate::View::pick)): the next frame that draws the
/// view draws, in the area asked for, the number of each instance ([`Instance::pick`]) where it is
/// the nearest, then the GPU copies that image back to the CPU, and [`Pick::take`] gives it once it
/// has come back, a frame or more later. Asking for it never waits for the GPU.
///
/// Dropping it gives the picking up. A view draws one picking per frame, the oldest asked first; a
/// view dropped before it draws one never answers it.
///
/// [`Instance::pick`]: crate::Instance::pick
#[derive(Debug)]
pub struct Pick {
    pub(crate) state: Arc<Mutex<PickState>>,
}

/// Where a picking is, from its asking to its answer.
#[derive(Debug)]
pub(crate) enum PickState {
    /// Asked for this area of the view's rectangle, in its pixels.
    Asked(Viewport),
    /// Drawn, its image of `width` × `height` numbers on its way back.
    Reading {
        readback: Readback,
        width: u32,
        height: u32,
    },
    /// Its answer, which `take` gives once; `None` once given.
    Answered(Option<Result<Picked, RenderError>>),
}

/// The numbers that a picking found ([`Pick::take`]): for each pixel of the area asked for, the
/// [`Instance::pick`](crate::Instance::pick) of the instance drawn nearest there, 0 where none is.
/// A surface that a masked material cuts out lets the picking through to what lies behind; a
/// blended one is picked where its alpha reaches one half. Lines are not picked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picked {
    /// The width of the area, in pixels, cut to the view's rectangle.
    pub width: u32,
    /// Its height.
    pub height: u32,
    /// `width * height` numbers, rows from top to bottom.
    pub numbers: Vec<u64>,
}

impl Picked {
    /// The number at `(x, y)` of the area, `y` counted from the top; 0 beyond it.
    #[must_use]
    pub fn at(&self, x: u32, y: u32) -> u64 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.numbers[(y * self.width + x) as usize]
    }
}

impl Pick {
    pub(crate) fn new(area: Viewport) -> Self {
        Self {
            state: Arc::new(Mutex::new(PickState::Asked(area))),
        }
    }

    /// What the picking found, once it has come back: `None` until then, and once given.
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with `GfxError::DeviceLost` once the device is lost, whether the
    /// picking was drawn or not: nothing is trusted from a lost device; with another error if the
    /// GPU could not create what the picking needed, or copy its image back.
    pub fn take(&self, gpu: &Gpu) -> Option<Result<Picked, RenderError>> {
        let mut state = lock(&self.state);
        match &mut *state {
            PickState::Asked(_) => {
                let error = gpu.check().err()?;
                *state = PickState::Answered(None);
                Some(Err(error.into()))
            }
            PickState::Reading {
                readback,
                width,
                height,
            } => {
                let (width, height) = (*width, *height);
                let read = readback.try_read(gpu)?;
                *state = PickState::Answered(None);
                Some(read.map_err(RenderError::from).map(|bytes| {
                    Picked {
                        width,
                        height,
                        numbers: bytes
                            .as_chunks::<8>()
                            .0
                            .iter()
                            .map(|texel| u64::from_le_bytes(*texel))
                            .collect(),
                    }
                }))
            }
            PickState::Answered(answer) => answer.take(),
        }
    }
}

/// The state of a picking; a panic while it is held leaves it as it was, each change being whole.
pub(crate) fn lock(state: &Mutex<PickState>) -> MutexGuard<'_, PickState> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}
