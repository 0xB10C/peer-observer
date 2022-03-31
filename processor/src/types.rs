use std::{fmt, ptr};
use std::time::SystemTime;

use bitcoin::consensus::encode::Decodable;
use bitcoin::hashes::{sha256d, Hash};
use bitcoin::network::message::NetworkMessage;
use bitcoin::network::message::RawNetworkMessage;

use shared::p2p;

pub enum P2PMessageSize {
    Small,
    Medium,
    Large,
    Huge,
}

// Tor v3 addresses are 62 chars + 6 chars for the port (':12345').
const MAX_PEER_ADDR_LENGTH: usize = 62 + 6;
const MAX_PEER_CONN_TYPE_LENGTH: usize = 20;
const MAX_MSG_TYPE_LENGTH: usize = 12;

/// The metadata for a P2P message.
#[repr(C)]
#[derive(Clone)]
pub struct P2PMessageMetadata {
    pub peer_id: u64,
    pub peer_addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub peer_conn_type: [u8; MAX_PEER_CONN_TYPE_LENGTH],
    pub msg_type: [u8; MAX_MSG_TYPE_LENGTH],
    pub msg_inbound: bool,
    pub msg_size: u64,
}

impl P2PMessageMetadata {
    // TODO: comment
    pub fn peer_addr(&self) -> String {
        String::from_utf8_lossy(&self.peer_addr.split(|c| *c == 0x00u8).next().unwrap())
            .into_owned()
    }

    // TODO: comment
    pub fn peer_conn_type(&self) -> String {
        String::from_utf8_lossy(&self.peer_conn_type.split(|c| *c == 0x00u8).next().unwrap())
            .into_owned()
    }

    // TODO: comment
    pub fn msg_type(&self) -> String {
        String::from_utf8_lossy(&self.msg_type.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    /// Returns `shared::p2p::Metadata` with a timestamp set to the current
    /// time.
    pub fn create_protobuf_metadata(&self) -> p2p::Metadata {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let timestamp = now.as_secs();
        let timestamp_subsec_millis = now.subsec_micros();

        p2p::Metadata{
            peer_id: self.peer_id,
            addr: self.peer_addr(),
            conn_type: self.peer_conn_type(),
            command: self.msg_type(),
            inbound: self.msg_inbound,
            size: self.msg_size,
            timestamp: timestamp,
            timestamp_subsec_micros: timestamp_subsec_millis,
        }
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
            self.peer_addr(),
            self.peer_conn_type(),
            self.msg_type(),
            self.msg_size,
        )
    }
}

pub trait RustBitcoinNetworkMessage {
    fn rust_bitcoin_network_message(&self) -> NetworkMessage;
}

fn build_raw_network_message(meta: &P2PMessageMetadata, payload: &[u8]) -> RawNetworkMessage {
    let mut raw_message: Vec<u8> = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    for (i, b) in meta.msg_type.iter().enumerate() {
        if *b == 0x00 {
            break;
        }
        raw_message[4 + i] = *b;
    }
    let payload_hash = sha256d::Hash::hash(payload);
    raw_message.append(&mut (meta.msg_size as u32).to_le_bytes().to_vec());
    raw_message.append(&mut payload_hash[..4].to_vec());
    raw_message.append(&mut payload.to_vec());

    let rnn = RawNetworkMessage::consensus_decode(raw_message.as_slice()).unwrap();

    match rnn.payload {
        NetworkMessage::Unknown { .. } => println!(
            "{} {:?}",
            if meta.msg_inbound {
                "inbound"
            } else {
                "outbound"
            },
            rnn
        ),
        _ => (),
    }

    return rnn;
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

    // The msg.payload is MAX_SMALL_MSG_LENGTH bytes long, however the acctual
    // message size in meta.msg_size is likely smaller. Returns a slice
    // with the acctual message payload.
    pub fn trimmed_payload(&self) -> &[u8] {
        return &self.payload[..self.meta.msg_size as usize];
    }
}

impl RustBitcoinNetworkMessage for SmallP2PMessage {
    fn rust_bitcoin_network_message(&self) -> NetworkMessage {
        return build_raw_network_message(&self.meta, self.trimmed_payload()).payload;
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

    // The msg.payload is MAX_MEDIUM_MSG_LENGTH bytes long, however the acctual
    // message size in meta.msg_size is likely smaller. Returns a slice
    // with the acctual message payload.
    pub fn trimmed_payload(&self) -> &[u8] {
        return &self.payload[..self.meta.msg_size as usize];
    }
}

impl RustBitcoinNetworkMessage for MediumP2PMessage {
    fn rust_bitcoin_network_message(&self) -> NetworkMessage {
        return build_raw_network_message(&self.meta, self.trimmed_payload()).payload;
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

    // The msg.payload is MAX_LARGE_MSG_LENGTH bytes long, however the acctual
    // message size in meta.msg_size is likely smaller. Returns a slice
    // with the acctual message payload.
    pub fn trimmed_payload(&self) -> &[u8] {
        return &self.payload[..self.meta.msg_size as usize];
    }
}

impl RustBitcoinNetworkMessage for LargeP2PMessage {
    fn rust_bitcoin_network_message(&self) -> NetworkMessage {
        return build_raw_network_message(&self.meta, self.trimmed_payload()).payload;
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

    // The msg.payload is MAX_HUGE_MSG_LENGTH bytes long, however the acctual
    // message size in meta.msg_size is likely smaller. Returns a slice
    // with the acctual message payload.
    pub fn trimmed_payload(&self) -> &[u8] {
        return &self.payload[..self.meta.msg_size as usize];
    }
}

impl RustBitcoinNetworkMessage for HugeP2PMessage {
    fn rust_bitcoin_network_message(&self) -> NetworkMessage {
        return build_raw_network_message(&self.meta, self.trimmed_payload()).payload;
    }
}
