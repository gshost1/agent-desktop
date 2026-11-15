#[repr(C)]
pub(crate) struct SocketMetadata {
    _handles: [u64; 2],
    _protocol: [i32; 3],
    _options_and_queues: [i16; 7],
    _error: u16,
    _out_of_band_mark: u32,
}
