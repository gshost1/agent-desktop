//! Length-prefixed frames: a big-endian `u32` byte count, then the bytes. Messages are JSON
//! frames; captured output travels as raw byte frames.

use std::io::{Error, ErrorKind, Read, Write};

/// The largest request a client may send; argv alone is capped near this by the OS.
pub(crate) const REQUEST_LIMIT: usize = 8 * 1024 * 1024;
/// The largest message or output frame a client accepts from the daemon.
pub(crate) const REPLY_LIMIT: usize = 512 * 1024 * 1024;

pub(crate) fn write_frame(out: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "frame too large"))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(bytes)
}

pub(crate) fn read_frame(input: &mut impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut len = [0_u8; 4];
    input.read_exact(&mut len)?;
    let len = usize::try_from(u32::from_be_bytes(len))
        .map_err(|_| Error::new(ErrorKind::InvalidData, "frame length"))?;
    if len > limit {
        return Err(Error::new(ErrorKind::InvalidData, "frame over its limit"));
    }
    let mut bytes = vec![0_u8; len];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn write_message(out: &mut impl Write, message: &impl serde::Serialize) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(message).map_err(Error::other)?;
    write_frame(out, &bytes)?;
    out.flush()
}

pub(crate) fn read_message<T: serde::de::DeserializeOwned>(
    input: &mut impl Read,
    limit: usize,
) -> std::io::Result<T> {
    let bytes = read_frame(input, limit)?;
    serde_json::from_slice(&bytes).map_err(|error| Error::new(ErrorKind::InvalidData, error))
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
