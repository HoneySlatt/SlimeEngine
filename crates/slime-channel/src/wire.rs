//! How a message travels on a socket: its line, then the bytes that it announces.

use std::io::{self, Read, Write};

use uwui_agent::{Frame, Reader};

use crate::error::ChannelError;

/// The most bytes of a game's answer's line: a list of a thousand entities or elements, with room
/// to spare.
pub(crate) const MAX_LINE: usize = 16 << 20;

/// The most bytes of a request's line after the hello, which the game refuses past it. A tool's
/// requests are small, and a line read whole costs its program about 17 times its size in memory
/// as it is parsed (`docs/measurements.md`, M6d.9a1a): 1 MiB keeps each
/// request a connection makes under some 17 MiB.
pub(crate) const MAX_REQUEST: usize = 1 << 20;

/// The most bytes after a message: the pixels of an image of 16 384 × 16 384, the largest that the
/// GPUs of the engine's floor draw.
pub(crate) const MAX_BYTES: u64 = 1 << 30;

/// Writes a message's `line`, then `bytes`, the line in one write and the bytes, if any, in a
/// second: a message written in small pieces waits for the acknowledgement of each on a socket that
/// delays them, tens of milliseconds a message.
pub(crate) fn send(stream: &mut impl Write, line: &[u8], bytes: &[u8]) -> io::Result<()> {
    stream.write_all(line)?;
    if !bytes.is_empty() {
        stream.write_all(bytes)?;
    }
    stream.flush()
}

/// Reads from `stream` until `reader` holds a whole message, and gives it.
///
/// # Errors
///
/// [`ChannelError::Io`] when the stream fails, or ends before a whole message,
/// `io::ErrorKind::UnexpectedEof`; [`ChannelError::Session`] when what came is not a message
/// within the reader's limits.
pub(crate) fn receive(stream: &mut impl Read, reader: &mut Reader) -> Result<Frame, ChannelError> {
    let mut piece = [0; 64 * 1024];
    loop {
        if let Some(frame) = reader.message()? {
            return Ok(frame);
        }
        let read = stream.read(&mut piece)?;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        reader.push(&piece[..read]);
    }
}
