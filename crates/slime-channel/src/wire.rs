//! How a message travels: its document, then the bytes that follow it.

use std::io::{self, Read, Write};

use crate::error::ChannelError;

/// The largest payload that follows a message: the pixels of an image of 16 384 × 16 384, the
/// largest that the GPUs of the engine's floor draw.
pub(crate) const MAX_PAYLOAD: u64 = 1 << 30;

/// Writes a message: the length of `document` in four bytes, little-endian, then `document`; the
/// length of `payload` in eight bytes, then `payload`. What comes before the payload goes in one
/// write, and the payload, if any, in a second: a message written in small pieces waits for the
/// acknowledgement of each on a socket that delays them, tens of milliseconds a message.
pub(crate) fn send(stream: &mut impl Write, document: &[u8], payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(document.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a document past 4 GiB"))?;
    let mut head = Vec::with_capacity(4 + document.len() + 8);
    head.extend_from_slice(&length.to_le_bytes());
    head.extend_from_slice(document);
    head.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    stream.write_all(&head)?;
    if !payload.is_empty() {
        stream.write_all(payload)?;
    }
    stream.flush()
}

/// Reads a message that [`send`] wrote: its document, of at most `max_document` bytes, and its
/// payload, of at most `max_payload`, which are refused larger before they are read.
pub(crate) fn receive(
    stream: &mut impl Read,
    max_document: usize,
    max_payload: u64,
) -> Result<(Vec<u8>, Vec<u8>), ChannelError> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > max_document {
        return Err(ChannelError::Malformed(format!(
            "a message of {length} bytes, past the {max_document} that it may hold"
        )));
    }
    let mut document = vec![0; length];
    stream.read_exact(&mut document)?;
    let mut payload_length = [0; 8];
    stream.read_exact(&mut payload_length)?;
    let payload_length = u64::from_le_bytes(payload_length);
    if payload_length > max_payload {
        return Err(ChannelError::Malformed(format!(
            "a payload of {payload_length} bytes, past the {max_payload} that it may hold"
        )));
    }
    let mut payload = Vec::new();
    stream.take(payload_length).read_to_end(&mut payload)?;
    if payload.len() as u64 != payload_length {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    Ok((document, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_comes_back_whole_and_one_too_large_is_refused_before_it_is_read() {
        let mut bytes = Vec::new();
        send(&mut bytes, b"document", b"pixels").expect("written");
        let read = receive(&mut &bytes[..], 64, 64).expect("read");
        assert_eq!(read, (b"document".to_vec(), b"pixels".to_vec()));
        assert!(matches!(
            receive(&mut &bytes[..], 4, 64),
            Err(ChannelError::Malformed(_))
        ));
        assert!(matches!(
            receive(&mut &bytes[..], 64, 2),
            Err(ChannelError::Malformed(_))
        ));
        assert!(matches!(
            receive(&mut &bytes[..bytes.len() - 1], 64, 64),
            Err(ChannelError::Io(_))
        ));
    }
}
