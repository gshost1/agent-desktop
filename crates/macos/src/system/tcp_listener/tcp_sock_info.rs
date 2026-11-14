use super::in_sock_info::InSockInfo;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct TcpSockInfo {
    pub(crate) network: InSockInfo,
    pub(crate) state: i32,
    _timers: [i32; 4],
    _mss: i32,
    _flags: u32,
    _reserved: u32,
    _tcp_control_block: u64,
}
