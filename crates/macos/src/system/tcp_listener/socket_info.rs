use super::sock_buf_info::SockBufInfo;
use super::socket_metadata::SocketMetadata;
use super::socket_protocol_info::SocketProtocolInfo;

#[repr(C)]
pub(crate) struct SocketInfo {
    _stat: libc::vinfo_stat,
    _metadata: SocketMetadata,
    pub(super) _receive: SockBufInfo,
    _send: SockBufInfo,
    pub(crate) kind: i32,
    _reserved: u32,
    pub(crate) protocol_info: SocketProtocolInfo,
}
