#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct InSockIpv6 {
    _hop_limit: u8,
    _checksum: i32,
    _interface_index: u16,
    _hops: i16,
}
