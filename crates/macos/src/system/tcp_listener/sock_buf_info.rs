#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct SockBufInfo {
    _values: [u32; 5],
    _flags: i16,
    _timeout: i16,
}
