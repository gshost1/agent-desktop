use super::in_sock_flow::InSockFlow;
use super::in_sock_ipv6::InSockIpv6;
use super::in_sock_ports::InSockPorts;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct InSockInfo {
    pub(crate) ports: InSockPorts,
    _generation: u64,
    pub(crate) flow: InSockFlow,
    pub(crate) addresses: [[u64; 2]; 2],
    _ipv4_tos: u8,
    _ipv6: InSockIpv6,
}
