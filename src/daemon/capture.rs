//! Collects what one command writes to stdout and stderr. Both descriptors point at unlinked
//! files in the daemon directory while the command runs, so every writer is captured, a large
//! envelope never blocks on a pipe, and a helper process that inherits a descriptor can never
//! hold the capture open.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;

use super::endpoint::DaemonEndpoint;

/// The output of one command while it is being captured.
pub(crate) struct OutputCapture {
    saved_out: OwnedFd,
    saved_err: OwnedFd,
    out: File,
    err: File,
}

/// What a finished command wrote.
pub(crate) type CapturedOutput = (Vec<u8>, Vec<u8>);

impl OutputCapture {
    /// Points stdout and stderr at fresh capture files.
    pub(crate) fn begin(endpoint: &DaemonEndpoint) -> std::io::Result<Self> {
        std::io::stdout().flush()?;
        let out = capture_file(endpoint, "out")?;
        let err = capture_file(endpoint, "err")?;
        let saved_out = duplicate(libc::STDOUT_FILENO)?;
        let saved_err = duplicate(libc::STDERR_FILENO)?;
        redirect(out.as_raw_fd(), libc::STDOUT_FILENO)?;
        if let Err(error) = redirect(err.as_raw_fd(), libc::STDERR_FILENO) {
            let _ = redirect(saved_out.as_raw_fd(), libc::STDOUT_FILENO);
            return Err(error);
        }
        Ok(Self {
            saved_out,
            saved_err,
            out,
            err,
        })
    }

    /// Restores stdout and stderr and returns the bytes written to each.
    pub(crate) fn finish(self) -> std::io::Result<CapturedOutput> {
        let flushed = std::io::stdout().flush();
        let restored_out = redirect(self.saved_out.as_raw_fd(), libc::STDOUT_FILENO);
        let restored_err = redirect(self.saved_err.as_raw_fd(), libc::STDERR_FILENO);
        flushed?;
        restored_out?;
        restored_err?;
        Ok((read_all(self.out)?, read_all(self.err)?))
    }
}

fn capture_file(endpoint: &DaemonEndpoint, stream: &str) -> std::io::Result<File> {
    let path = endpoint.capture(stream);
    let _ = std::fs::remove_file(&path);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    std::fs::remove_file(&path)?;
    Ok(file)
}

fn duplicate(fd: RawFd) -> std::io::Result<OwnedFd> {
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if copy < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(copy) })
}

fn redirect(from: RawFd, to: RawFd) -> std::io::Result<()> {
    loop {
        if unsafe { libc::dup2(from, to) } >= 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn read_all(mut file: File) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
