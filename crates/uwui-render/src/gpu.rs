//! What `uwui-render` is given of its host's GPU, its errors, and the error scopes in which it
//! checks what it creates (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// What a draw records with: its host's wgpu device, its queue, and the encoder that the draw
/// records its pass into and that the host submits, as egui-wgpu takes them. The renderer keeps
/// none of them from one draw to the next.
///
/// What the queue writes reaches the GPU before the next submission: the host submits the encoder
/// before the renderer that recorded into it draws again, or the second draw overwrites what the
/// first one's pass reads.
#[derive(Debug)]
pub struct DrawContext<'a> {
    /// The device.
    pub device: &'a wgpu::Device,
    /// Its queue, which writes the items, the glyphs and the frame's size.
    pub queue: &'a wgpu::Queue,
    /// The encoder that the draw records into.
    pub encoder: &'a mut wgpu::CommandEncoder,
}

/// An error of `uwui-render`: memory that runs out, or a shader or a pipeline that the device
/// refuses. A device lost is not one of them: wgpu cannot be asked whether a device is lost, and
/// on a lost device every creation seems to succeed and draws nothing; the host, which knows of
/// the loss, reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The GPU's memory cannot hold an object being created, a buffer or a texture: nothing was
    /// created, the device goes on, and the same draw can succeed once memory is freed.
    OutOfMemory {
        /// The label of the object.
        label: String,
    },
    /// A shader module was refused, usually because its WGSL does not compile.
    Shader {
        /// The label of the shader.
        label: String,
        /// wgpu's message, with the line and column of the error in the source.
        message: String,
    },
    /// A render pipeline was refused, as Direct3D 12's shader compiler may refuse one whose
    /// shader wgpu accepted.
    Pipeline {
        /// The label of the pipeline.
        label: String,
        /// wgpu's message.
        message: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfMemory { label } => write!(f, "out of GPU memory: cannot create `{label}`"),
            Self::Shader { label, message } => {
                write!(f, "shader `{label}` was rejected:\n{message}")
            }
            Self::Pipeline { label, message } => {
                write!(f, "render pipeline `{label}` was rejected:\n{message}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Runs `create`, which creates the GPU objects of `label`, inside an error scope of memory, and
/// returns what it created, or [`Error::OutOfMemory`] if memory ran out. A scope of its own, so
/// that a host's scope does not take this error for one of its own.
pub(crate) fn within_memory<T>(
    device: &wgpu::Device,
    label: &str,
    create: impl FnOnce() -> T,
) -> Result<T, Error> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let created = create();
    match answer(scope.pop()) {
        None => Ok(created),
        Some(_) => Err(Error::OutOfMemory {
            label: label.to_owned(),
        }),
    }
}

/// Runs `create` inside error scopes of validation and of internal errors, and returns what it
/// created with the error that either caught: a shader that does not compile, or a pipeline that
/// wgpu or Direct3D 12's compiler refuses.
pub(crate) fn within_checks<T>(
    device: &wgpu::Device,
    create: impl FnOnce() -> T,
) -> (T, Option<wgpu::Error>) {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let created = create();
    // Scopes are popped in the reverse order of their pushes.
    let internal = answer(internal.pop());
    let validation = answer(validation.pop());
    (created, internal.or(validation))
}

/// What an error scope caught, read at once: on Vulkan, Direct3D 12 and Metal, wgpu answers
/// before the call returns. On the web, it answers later: the error is not known, and none is
/// returned (`docs/debt.md`).
fn answer(popped: impl Future<Output = Option<wgpu::Error>>) -> Option<wgpu::Error> {
    match pin!(popped).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(error) => error,
        Poll::Pending => None,
    }
}
