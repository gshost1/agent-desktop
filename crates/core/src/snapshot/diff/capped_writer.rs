use std::io::{Error, ErrorKind, Write};

/// A writer that refuses to pass `limit` bytes to `inner`, so an oversized
/// baseline stops at the cap instead of being built in memory first.
pub(crate) struct CappedWriter<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> CappedWriter<W> {
    pub(crate) fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }

    pub(crate) fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for CappedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(Error::new(ErrorKind::FileTooLarge, "diff baseline over its cap"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
#[path = "capped_writer_tests.rs"]
mod tests;
