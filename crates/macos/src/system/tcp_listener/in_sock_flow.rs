#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct InSockFlow {
    _flags: u32,
    _flow: u32,
    pub(crate) address_flags: u8,
    _ttl: u8,
    _reserved: u32,
}
