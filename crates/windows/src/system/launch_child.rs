/// The process handle `CreateProcessW` returned for a launch, kept open so
/// every window poll can ask whether the launched program already exited.
#[cfg(target_os = "windows")]
pub(crate) struct LaunchedChild(windows_sys::Win32::Foundation::HANDLE);

#[cfg(target_os = "windows")]
impl LaunchedChild {
    pub(crate) fn new(handle: windows_sys::Win32::Foundation::HANDLE) -> Self {
        Self(handle)
    }

    /// The exit code once the handle has signaled, or `None` while the
    /// process runs. A wait or exit-code read that fails is `None`: liveness
    /// that cannot be judged keeps the launch waiting rather than reporting
    /// an exit that was never observed.
    pub(crate) fn exit_code(&self) -> Option<u32> {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

        if unsafe { WaitForSingleObject(self.0, 0) } != WAIT_OBJECT_0 {
            return None;
        }
        let mut code: u32 = 0;
        (unsafe { GetExitCodeProcess(self.0, &mut code) } != 0).then_some(code)
    }
}

#[cfg(target_os = "windows")]
impl Drop for LaunchedChild {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) struct LaunchedChild;

#[cfg(not(target_os = "windows"))]
impl LaunchedChild {
    pub(crate) fn exit_code(&self) -> Option<u32> {
        None
    }
}
