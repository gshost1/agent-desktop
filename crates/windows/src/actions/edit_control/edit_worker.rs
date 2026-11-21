use std::ffi::c_void;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{CloseHandle, FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    FreeLibraryAndExitThread, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleHandleExW,
};
use windows_sys::Win32::System::Threading::CreateThread;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;

use super::edit_pending::PendingPermit;
use super::edit_replace::EditSendFailure;
use super::edit_send::{Completion, EM_REPLACESEL};

const WORKER_STACK_BYTES: usize = 64 * 1024;

struct EditWorker {
    handle: isize,
    text: Vec<u16>,
    permit: PendingPermit,
    done: Arc<Completion>,
    module: HMODULE,
}

pub(super) fn start(
    handle: isize,
    text: Vec<u16>,
    permit: PendingPermit,
    done: Arc<Completion>,
) -> Result<(), EditSendFailure> {
    let mut module = std::ptr::null_mut();
    let pinned = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            run as *const () as *const u16,
            &mut module,
        )
    };
    if pinned == 0 {
        return Err(EditSendFailure::NotSent(format!(
            "GetModuleHandleExW: {}",
            std::io::Error::last_os_error()
        )));
    }
    let worker = Box::into_raw(Box::new(EditWorker {
        handle,
        text,
        permit,
        done,
        module,
    }));
    let thread = unsafe {
        CreateThread(
            std::ptr::null(),
            WORKER_STACK_BYTES,
            Some(run),
            worker.cast(),
            0,
            std::ptr::null_mut(),
        )
    };
    if thread.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe {
            drop(Box::from_raw(worker));
            FreeLibrary(module);
        }
        return Err(EditSendFailure::NotSent(format!("CreateThread: {error}")));
    }
    unsafe { CloseHandle(thread) };
    Ok(())
}

unsafe extern "system" fn run(context: *mut c_void) -> u32 {
    let EditWorker {
        handle,
        text,
        permit,
        done,
        module,
    } = unsafe { *Box::from_raw(context.cast::<EditWorker>()) };
    unsafe {
        SendMessageW(
            handle as *mut c_void,
            EM_REPLACESEL,
            1,
            text.as_ptr() as isize,
        )
    };
    drop(text);
    drop(permit);
    done.set();
    drop(done);
    unsafe { FreeLibraryAndExitThread(module, 0) }
}
