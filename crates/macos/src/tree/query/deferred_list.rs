use super::{child_read::ChildRead, traversal::LocatorTraversal};
use agent_desktop_core::{AdapterError, ObservedSubtree, Rect};

/// A list container whose children a walk reads after the rest of the tree.
///
/// A file browser or table can hold thousands of rows, and a depth-first walk
/// that enters it first spends its whole budget there before it reaches the
/// controls beside it. Reading rows last changes only the order of reads: the
/// rows are attached at their place in the tree, so a walk that finishes
/// builds exactly the tree it built before.
pub(crate) struct DeferredList {
    pub(crate) path: Vec<usize>,
    pub(crate) children: ChildRead,
    pub(crate) depths: (u8, u8),
    pub(crate) viewport: Option<Rect>,
    pub(crate) ancestors: Vec<usize>,
}

const DEFERRED_ROLES: [&str; 4] = ["AXOutline", "AXTable", "AXBrowser", "AXList"];

/// Whether a node with this native role holds rows that are read last.
pub(crate) fn defers_children(role: Option<&str>) -> bool {
    role.is_some_and(|role| DEFERRED_ROLES.contains(&role))
}

impl LocatorTraversal {
    /// Reads the rows of every deferred list, in the order the lists were met,
    /// and attaches them at their place. Rows of a list nested in those rows are
    /// queued again, so they too come after everything met before them. A list
    /// the budget no longer reaches stays incomplete and reports its row count.
    /// A request that leaves rows unread gets every list that way, without a
    /// read and without marking the rest of the walk incomplete.
    pub(super) fn read_deferred_lists(
        &mut self,
        mut root: Option<ObservedSubtree>,
    ) -> Result<Option<ObservedSubtree>, AdapterError> {
        while let Some(list) = self.deferred.pop_front() {
            let Some(tree) = root.as_mut() else {
                self.arena.drop_handles(list.children.elements.len());
                continue;
            };
            let path = list.path.clone();
            let (children, complete, children_count) = self.read_rows(list)?;
            if !tree.attach_deferred_children(&path, children, complete, children_count) {
                return Err(AdapterError::internal(
                    "deferred list lost its place in the observed tree",
                ));
            }
        }
        Ok(root)
    }

    fn read_rows(
        &mut self,
        list: DeferredList,
    ) -> Result<(Vec<ObservedSubtree>, bool, Option<u32>), AdapterError> {
        let leave_unread = self.request.observation_mode.leave_rows_unread;
        if leave_unread || self.remaining_budget().is_none() {
            if !leave_unread {
                self.note_deadline_exhausted();
            }
            let empty = list.children.total_count == 0 && list.children.complete;
            self.arena.drop_handles(list.children.elements.len());
            let total = u32::try_from(list.children.total_count)
                .ok()
                .filter(|count| *count > 0);
            return Ok((Vec::new(), empty, total));
        }
        let saved_ancestors = std::mem::replace(
            &mut self.arena.ancestors,
            list.ancestors.iter().copied().collect(),
        );
        let saved_viewport = std::mem::replace(&mut self.context.window_bounds, list.viewport);
        let result = self.visit_children(list.children, list.depths, &list.path);
        self.context.window_bounds = saved_viewport;
        self.arena.ancestors = saved_ancestors;
        let (children, complete) = result?;
        Ok((children, complete, None))
    }
}

#[cfg(test)]
#[path = "deferred_list_tests.rs"]
mod tests;
