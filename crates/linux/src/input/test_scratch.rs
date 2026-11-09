use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn adopt(path: PathBuf) -> Self {
        let _ = remove_any(&path);
        Self(path)
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = remove_any(&self.0);
    }
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    fs::remove_dir_all(path).or_else(|_| fs::remove_file(path))
}

#[cfg(test)]
#[path = "test_scratch_tests.rs"]
mod tests;
