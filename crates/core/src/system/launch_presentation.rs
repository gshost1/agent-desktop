/// How a launch presents an application that is already running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LaunchPresentation {
    /// Brings the application forward so it presents a window. A document-based
    /// application creates its first window in response to activation, so a
    /// caller that needs a window has to ask for one; waiting without asking
    /// waits for an event that never fires.
    pub activate: bool,
    /// Asks the platform to treat the launch as a reopen of an application that
    /// is already running, so an application with no window opens its default
    /// one. Without it a running application is only attached to or brought
    /// forward, never reopened.
    pub reopen: bool,
}
