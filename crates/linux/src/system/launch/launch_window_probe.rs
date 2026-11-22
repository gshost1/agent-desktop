use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use agent_desktop_core::{Deadline, ProcessIdentity};

use crate::system::SharedBus;
use crate::system::process::running_app::Running;
use crate::system::registry::Registry;
use crate::tree::{AtspiElement, ChildAddress};

/// How long a full window read of an application whose root children have
/// not changed answers for it, bounding how late a window that becomes
/// visible without a new root child is seen.
pub(super) const FULL_READ_REUSE: Duration = Duration::from_secs(1);

/// The longest the child list of one application root may take to read.
const CHILDREN_READ: Duration = Duration::from_millis(250);

/// The application roots of one registry read, by process instance.
pub(super) type Roots = RefCell<HashMap<ProcessIdentity, AtspiElement>>;

/// Whether an application shows a window, read once per process instance
/// when it does. An application root with no children has no window, which
/// one child list read shows. A full window read runs only when the root
/// has children, and its answer stands while the same children stay listed
/// for [`FULL_READ_REUSE`]. A child list that cannot be read proves nothing,
/// so the full read runs.
pub(super) struct WindowProbe<C, F> {
    children: C,
    full: F,
    windowed: HashSet<ProcessIdentity>,
    quiet: HashMap<ProcessIdentity, (Vec<ChildAddress>, Instant)>,
}

impl<C, F> WindowProbe<C, F>
where
    C: FnMut(&Running) -> Option<Vec<ChildAddress>>,
    F: FnMut(&Running) -> bool,
{
    pub(super) fn new(children: C, full: F) -> Self {
        Self {
            children,
            full,
            windowed: HashSet::new(),
            quiet: HashMap::new(),
        }
    }

    pub(super) fn has_window(&mut self, app: &Running) -> bool {
        let key = app.identity();
        if self.windowed.contains(&key) {
            return true;
        }
        let listed = (self.children)(app);
        if listed.as_ref().is_some_and(Vec::is_empty) {
            self.quiet.remove(&key);
            return false;
        }
        if let (Some(listed), Some((seen, at))) = (&listed, self.quiet.get(&key))
            && listed == seen
            && at.elapsed() < FULL_READ_REUSE
        {
            return false;
        }
        if (self.full)(app) {
            self.quiet.remove(&key);
            self.windowed.insert(key);
            return true;
        }
        match listed {
            Some(listed) => {
                self.quiet.insert(key, (listed, Instant::now()));
            }
            None => {
                self.quiet.remove(&key);
            }
        }
        false
    }
}

/// The application roots a registry read listed.
pub(super) fn roots_of(registry: &Registry) -> HashMap<ProcessIdentity, AtspiElement> {
    registry
        .apps
        .iter()
        .map(|app| {
            (
                ProcessIdentity::new(app.root.pid(), app.facts.instance.clone()),
                app.root.clone(),
            )
        })
        .collect()
}

/// The children of `app`'s root as one call lists them, absent when the root
/// is unknown or does not answer in time.
pub(super) fn root_children(
    bus: &SharedBus,
    roots: &Roots,
    app: &Running,
    deadline: Deadline,
) -> Option<Vec<ChildAddress>> {
    let root = roots.borrow().get(&app.identity()).cloned()?;
    let slice = Deadline::from_duration(deadline.remaining_slice(CHILDREN_READ).ok()?).ok()?;
    let shared = bus.get(slice).ok()?;
    crate::system::within(slice, crate::tree::list_children(shared.inner(), &root)).ok()
}

#[cfg(test)]
#[path = "launch_window_probe_tests.rs"]
mod tests;
