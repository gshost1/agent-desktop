#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ProcFileInfo {
    _open_flags: u32,
    _status: u32,
    _offset: i64,
    _file_type: i32,
    _guard_flags: u32,
}
