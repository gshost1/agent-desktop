use std::ffi::{CStr, c_void};
use std::mem::MaybeUninit;
use std::path::PathBuf;

pub(crate) fn containing_image() -> Option<PathBuf> {
    let mut info = MaybeUninit::<libc::Dl_info>::zeroed();
    let address = containing_image as *const () as *const c_void;
    if unsafe { libc::dladdr(address, info.as_mut_ptr()) } == 0 {
        return None;
    }
    let filename = unsafe { info.assume_init().dli_fname };
    if filename.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(filename) };
    Some(PathBuf::from(bytes.to_string_lossy().as_ref()))
}
