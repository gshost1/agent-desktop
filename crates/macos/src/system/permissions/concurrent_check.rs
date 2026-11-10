//! Runs one permission check on its own thread while the caller runs another.
//!
//! Each permission check is a round trip to a system privacy service, about
//! 11-15 ms each on an M2, and the checks do not depend on each other. Asking
//! them at once costs the longest one instead of their sum. A check whose
//! thread cannot start, or whose thread ends without a result, runs in place.

pub(super) struct ConcurrentCheck<'scope, T, F> {
    handle: Option<std::thread::ScopedJoinHandle<'scope, T>>,
    check: F,
}

impl<'scope, T, F> ConcurrentCheck<'scope, T, F>
where
    T: Send + 'scope,
    F: Fn() -> T + Copy + Send + 'scope,
{
    pub(super) fn start<'env>(scope: &'scope std::thread::Scope<'scope, 'env>, check: F) -> Self {
        let handle = std::thread::Builder::new()
            .name("agent-desktop-permission-check".into())
            .spawn_scoped(scope, check)
            .ok();
        Self { handle, check }
    }

    pub(super) fn join(self) -> T {
        self.handle
            .and_then(|handle| handle.join().ok())
            .unwrap_or_else(|| (self.check)())
    }
}

#[cfg(test)]
#[path = "concurrent_check_tests.rs"]
mod tests;
