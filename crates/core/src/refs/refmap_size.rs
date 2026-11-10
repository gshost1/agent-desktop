//! Byte accounting for a refmap against `MAX_REFMAP_BYTES`, measured without
//! building the serialized string.

use crate::AppError;
use crate::refs::RefMap;
use std::collections::HashSet;
use std::collections::hash_map::HashMap;

/// An `io::Write` sink that only counts the bytes written to it.
#[derive(Default)]
struct ByteCount(usize);

impl std::io::Write for ByteCount {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn serialized_len<T: serde::Serialize + ?Sized>(value: &T) -> Result<usize, AppError> {
    let mut count = ByteCount::default();
    serde_json::to_writer(&mut count, value)?;
    Ok(count.0)
}

impl RefMap {
    /// The length of this map's stored form, the number the size cap compares.
    pub(crate) fn serialized_len(&self) -> Result<usize, AppError> {
        serialized_len(self)
    }

    /// The length of this map's stored form with no entries: the fixed cost
    /// every stored map pays before its first ref.
    pub(crate) fn serialized_len_without_entries(&self) -> Result<usize, AppError> {
        serialized_len(&RefMap {
            inner: HashMap::new(),
            counter: self.counter,
        })
    }

    /// What `ref_id` adds to the stored form: its quoted key, the colon and its
    /// entry. The comma between two entries is the caller's to count. `None`
    /// when the map holds no such ref.
    pub(crate) fn serialized_entry_len(&self, ref_id: &str) -> Result<Option<usize>, AppError> {
        let Some(entry) = self.inner.get(ref_id) else {
            return Ok(None);
        };
        Ok(Some(serialized_len(ref_id)? + 1 + serialized_len(entry)?))
    }

    /// Drops every ref not in `keep`. The counter is unchanged, so a kept ref
    /// keeps its id and an id that was dropped is never handed out again.
    pub(crate) fn retain_refs(&mut self, keep: &HashSet<String>) {
        self.inner.retain(|ref_id, _| keep.contains(ref_id));
    }
}

#[cfg(test)]
#[path = "refmap_size_tests.rs"]
mod tests;
