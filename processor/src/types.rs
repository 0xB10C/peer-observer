use std::{fmt, ptr};

// Tor v3 addresses are 62 chars + 6 chars for the port (':12345').
const MAX_PEER_ADDR_LENGTH: usize = 62 + 6;
const MAX_PEER_CONN_TYPE_LENGTH: usize = 20;
const MAX_MSG_TYPE_LENGTH: usize = 12;

/// The metadata for a P2P message.
#[repr(C)]
pub struct P2PMessageMetadata {
    pub peer_id: u64,
    pub peer_addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub peer_conn_type: [u8; MAX_PEER_CONN_TYPE_LENGTH],
    pub msg_type: [u8; MAX_MSG_TYPE_LENGTH],
    pub msg_inbound: bool,
    pub msg_size: u64,
}

impl P2PMessageMetadata {
    pub fn get_peer_addr(&self) -> String {
        String::from_utf8_lossy(&self.peer_addr.split(|c| *c == 0x00u8).next().unwrap())
            .into_owned()
    }

    pub fn get_peer_conn_type(&self) -> String {
        String::from_utf8_lossy(&self.peer_conn_type.split(|c| *c == 0x00u8).next().unwrap())
            .into_owned()
    }

    pub fn get_msg_type(&self) -> String {
        String::from_utf8_lossy(&self.msg_type.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }
}

impl fmt::Display for P2PMessageMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} msg from peer {} ({}, {}): {} with {} bytes",
            if self.msg_inbound {
                "inbound"
            } else {
                "outbound"
            },
            self.peer_id,
            self.get_peer_addr(),
            self.get_peer_conn_type(),
            self.get_msg_type(),
            self.msg_size,
        )
    }
}

const MAX_SMALL_MSG_LENGTH: usize = 256;
const MAX_MEDIUM_MSG_LENGTH: usize = 4096;
const MAX_LARGE_MSG_LENGTH: usize = 65536;
const MAX_HUGE_MSG_LENGTH: usize = 4194304;

#[repr(C)]
pub struct SmallP2PMessage {
    pub meta: P2PMessageMetadata,
    pub payload: [u8; MAX_SMALL_MSG_LENGTH],
}

impl SmallP2PMessage {
    pub fn from_bytes(x: &[u8]) -> SmallP2PMessage {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const SmallP2PMessage) }
    }
}

#[repr(C)]
pub struct MediumP2PMessage {
    pub meta: P2PMessageMetadata,
    pub payload: [u8; MAX_MEDIUM_MSG_LENGTH],
}

impl MediumP2PMessage {
    pub fn from_bytes(x: &[u8]) -> MediumP2PMessage {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MediumP2PMessage) }
    }
}

#[repr(C)]
pub struct LargeP2PMessage {
    pub meta: P2PMessageMetadata,
    pub payload: [u8; MAX_LARGE_MSG_LENGTH],
}

impl LargeP2PMessage {
    pub fn from_bytes(x: &[u8]) -> LargeP2PMessage {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const LargeP2PMessage) }
    }
}

#[repr(C)]
pub struct HugeP2PMessage {
    pub meta: P2PMessageMetadata,
    pub payload: [u8; MAX_HUGE_MSG_LENGTH],
}

impl HugeP2PMessage {
    pub fn from_bytes(x: &[u8]) -> HugeP2PMessage {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const HugeP2PMessage) }
    }
}
