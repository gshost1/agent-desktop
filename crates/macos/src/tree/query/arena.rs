use crate::tree::AXElement;
use agent_desktop_core::{AdapterError, LocatorStats};
use rustc_hash::FxHashSet;

pub(crate) struct TraversalArena {
    pub(crate) stats: LocatorStats,
    ancestors: FxHashSet<usize>,
    ancestor_path: Vec<AXElement>,
    pub(crate) structurally_complete: bool,
    owned_handles: u64,
}

impl TraversalArena {
    pub(crate) fn new() -> Self {
        Self {
            stats: LocatorStats::default(),
            ancestors: FxHashSet::default(),
            ancestor_path: Vec::new(),
            structurally_complete: true,
            owned_handles: 0,
        }
    }

    pub(crate) fn add_handles(&mut self, count: usize) {
        self.owned_handles = self
            .owned_handles
            .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        self.stats.traversal.peak_handles_owned = self
            .stats
            .traversal
            .peak_handles_owned
            .max(self.owned_handles);
    }

    pub(crate) fn drop_handles(&mut self, count: usize) {
        self.owned_handles = self
            .owned_handles
            .saturating_sub(u64::try_from(count).unwrap_or(u64::MAX));
    }

    /// Enters an element on the current ancestor path, or returns false when
    /// it is already there. The cycle guard compares native pointers, so the
    /// path holds a retained handle to every element on it: a pointer the
    /// guard still compares against is never released and handed to an
    /// unrelated element read later.
    pub(crate) fn enter(&mut self, element: &AXElement) -> bool {
        if !self.ancestors.insert(element.0 as usize) {
            return false;
        }
        self.ancestor_path.push(element.clone());
        true
    }

    pub(crate) fn leave(&mut self, element: &AXElement) {
        if !self.ancestors.remove(&(element.0 as usize)) {
            return;
        }
        if let Some(position) = self
            .ancestor_path
            .iter()
            .rposition(|ancestor| ancestor.0 == element.0)
        {
            self.ancestor_path.remove(position);
        }
    }

    /// Retained handles to the current ancestor path, for a read that resumes
    /// below it after the walk has left it.
    pub(crate) fn ancestor_path(&self) -> Vec<AXElement> {
        self.ancestor_path.clone()
    }

    /// Makes `path` the current ancestor path and returns the one it replaces.
    pub(crate) fn replace_ancestor_path(&mut self, path: Vec<AXElement>) -> Vec<AXElement> {
        self.ancestors = path.iter().map(|element| element.0 as usize).collect();
        std::mem::replace(&mut self.ancestor_path, path)
    }

    pub(crate) fn mark_incomplete(&mut self) {
        self.structurally_complete = false;
    }

    pub(crate) fn finish(self) -> Result<LocatorStats, AdapterError> {
        if self.owned_handles != 0 {
            return Err(AdapterError::internal(format!(
                "observation retained {} native handles",
                self.owned_handles
            )));
        }
        Ok(self.stats)
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;
