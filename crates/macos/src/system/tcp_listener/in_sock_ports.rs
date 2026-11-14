#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct InSockPorts {
    _foreign: i32,
    pub(crate) local: i32,
}
