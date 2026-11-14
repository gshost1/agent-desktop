use std::fs::File;
use std::path::Path;

/// How far an atomic private write waits for the filesystem before it returns.
///
/// Both variants write a temporary and rename it over the destination, so a
/// reader never sees a torn file and a process that dies mid-write leaves the
/// previous contents in place. They differ only in what survives a power loss
/// or kernel crash.
#[derive(Clone, Copy)]
pub(crate) enum WriteDurability {
    /// Flushes the file and then its parent directory, so the replacement
    /// survives a power loss.
    Durable(fn(&Path) -> std::io::Result<()>),
    /// Leaves flushing to the kernel. A power loss can drop the replacement or
    /// leave it unreadable, which callers must treat as absent or invalid data.
    CacheOnly,
}

impl WriteDurability {
    pub(crate) fn sync_file(self, file: &File) -> std::io::Result<()> {
        match self {
            Self::Durable(_) => {
                count_sync();
                crate::storage::private_file::sync_to_device(file)
            }
            Self::CacheOnly => Ok(()),
        }
    }

    pub(crate) fn sync_parent(self, parent: &Path) -> std::io::Result<()> {
        match self {
            Self::Durable(sync_parent) => {
                count_sync();
                sync_parent(parent)
            }
            Self::CacheOnly => Ok(()),
        }
    }
}

#[cfg(test)]
thread_local! {
    static SYNCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn count_sync() {
    SYNCS.with(|syncs| syncs.set(syncs.get() + 1));
}

#[cfg(not(test))]
fn count_sync() {}

#[cfg(test)]
pub(crate) fn syncs_on_this_thread() -> usize {
    SYNCS.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "write_durability_tests.rs"]
mod tests;
