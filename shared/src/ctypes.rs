use std::{fmt, ptr};

use bitcoin::consensus;
use bitcoin::consensus::encode::Decodable;
use bitcoin::hashes::{sha256d, Hash};
use bitcoin::network::message::{NetworkMessage, RawNetworkMessage};

use crate::net_msg;
use crate::primitive::ConnType;

// Tor v3 addresses are 62 chars + 6 chars for the port (':12345').
const MAX_PEER_ADDR_LENGTH: usize = 62 + 6;
const MAX_PEER_CONN_TYPE_LENGTH: usize = 20;
const MAX_MSG_TYPE_LENGTH: usize = 12;
const MAX_MISBEHAVING_MESSAGE_LENGTH: usize = 128;
const MAX_SMALL_MSG_LENGTH: usize = 256;
const MAX_MEDIUM_MSG_LENGTH: usize = 4096;
const MAX_LARGE_MSG_LENGTH: usize = 65536;
const MAX_HUGE_MSG_LENGTH: usize = 4194304;

pub enum P2PMessageSize {
    Small,
    Medium,
    Large,
    Huge,
}

/// The metadata for a P2P message.
#[repr(C)]
#[derive(Clone, Debug)]
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

    pub fn from_bytes(x: &[u8]) -> Self {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const Self) }
    }

    /// Returns `p2p::Metadata`.
    pub fn create_protobuf_metadata(&self) -> net_msg::Metadata {
        let conn_type: ConnType = self.peer_conn_type().into();

        net_msg::Metadata {
            peer_id: self.peer_id,
            addr: self.peer_addr(),
            conn_type: conn_type as i32,
            command: self.msg_type(),
            inbound: self.msg_inbound,
            size: self.msg_size,
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
    fn rust_bitcoin_network_message(&self) -> Result<NetworkMessage, P2PMessageDecodeError> {
        return build_network_message(&self.meta, self.trimmed_payload());
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
    fn rust_bitcoin_network_message(&self) -> Result<NetworkMessage, P2PMessageDecodeError> {
        return build_network_message(&self.meta, self.trimmed_payload());
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
    fn rust_bitcoin_network_message(&self) -> Result<NetworkMessage, P2PMessageDecodeError> {
        return build_network_message(&self.meta, self.trimmed_payload());
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
    fn rust_bitcoin_network_message(&self) -> Result<NetworkMessage, P2PMessageDecodeError> {
        return build_network_message(&self.meta, self.trimmed_payload());
    }
}

#[repr(C)]
pub struct Connection {
    pub id: u64,
    pub addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub conn_type: [u8; MAX_PEER_CONN_TYPE_LENGTH],
    pub network: u32,
    pub net_group: u64,
}

impl Connection {
    // TODO: comment
    pub fn addr(&self) -> String {
        String::from_utf8_lossy(&self.addr.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    // TODO: comment
    pub fn conn_type(&self) -> String {
        String::from_utf8_lossy(&self.conn_type.split(|c| *c == 0x00u8).next().unwrap())
            .into_owned()
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Connection(peer={}, addr={}, type={}, network={}, netgroup={})",
            self.id,
            self.addr(),
            self.conn_type(),
            self.network,
            self.net_group,
        )
    }
}

#[repr(C)]
pub struct ClosedConnection {
    /// The connection being closed
    pub connection: Connection,
    /// Connection established UNIX epoch timestamp
    pub time_established: u64,
}

impl fmt::Display for ClosedConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClosedConnection(conn={}, time_established={})",
            self.connection, self.time_established,
        )
    }
}

impl ClosedConnection {
    pub fn from_bytes(x: &[u8]) -> ClosedConnection {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const ClosedConnection) }
    }
}

#[repr(C)]
pub struct InboundConnection {
    /// The inbound connection being opened
    pub connection: Connection,
    /// Number of inbound connections existing (not including this newly opened one)
    pub existing_connections: u64,
}

impl InboundConnection {
    pub fn from_bytes(x: &[u8]) -> InboundConnection {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const InboundConnection) }
    }
}

impl fmt::Display for InboundConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "InboundConnection(conn={}, existing_connections={})",
            self.connection, self.existing_connections,
        )
    }
}

#[repr(C)]
pub struct OutboundConnection {
    /// The outbound connection being opened
    pub connection: Connection,
    /// Number of outbound connections existing (not including this newly opened one)
    pub existing_connections: u64,
}

impl OutboundConnection {
    pub fn from_bytes(x: &[u8]) -> OutboundConnection {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const OutboundConnection) }
    }
}

impl fmt::Display for OutboundConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "OutboundConnection(conn={}, existing_connections={})",
            self.connection, self.existing_connections
        )
    }
}

#[repr(C)]
pub struct MisbehavingConnection {
    pub id: u64,
    pub score_before: i32,
    pub score_increase: i32,
    pub message: [u8; MAX_MISBEHAVING_MESSAGE_LENGTH],
    pub threshold_exceeded: bool,
}

impl MisbehavingConnection {
    pub fn from_bytes(x: &[u8]) -> MisbehavingConnection {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MisbehavingConnection) }
    }

    // TODO: comment
    pub fn message(&self) -> String {
        String::from_utf8_lossy(&self.message.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }
}

impl fmt::Display for MisbehavingConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MisbehavingConnection(id={}, score_before={}, score_increase={}, message={}, threshold_exceeded={})",
            self.id,
            self.score_before,
            self.score_increase,
            self.message(),
            self.threshold_exceeded,
        )
    }
}

pub trait RustBitcoinNetworkMessage {
    fn rust_bitcoin_network_message(&self) -> Result<NetworkMessage, P2PMessageDecodeError>;
}

fn build_network_message(
    meta: &P2PMessageMetadata,
    payload: &[u8],
) -> Result<NetworkMessage, P2PMessageDecodeError> {
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

    match RawNetworkMessage::consensus_decode(&mut raw_message.as_slice()) {
        Ok(rnm) => Ok(rnm.payload),
        Err(e) => {
            if let Some(nm) = build_alternative_network_message(meta, payload) {
                Ok(nm)
            } else {
                Err(P2PMessageDecodeError::new(meta.clone(), e))
            }
        }
    }
}

/// There might be cases where rust-bitcoin can't deserialize a message.
/// This happens, for example, when a message has no elements but rust-bitcoin
/// expects at least one element. We try to handle known cases here on a best
/// effort basis.
fn build_alternative_network_message(
    meta: &P2PMessageMetadata,
    _: &[u8],
) -> Option<NetworkMessage> {
    // case: empty addrv2 message.
    if meta.msg_type() == "addrv2" && meta.msg_size == 0 {
        println!("emtpy addrv2 {}", meta);
        return Some(NetworkMessage::AddrV2(vec![]));
    // case: old ping message with no nonce.
    } else if meta.msg_type() == "ping" && meta.msg_size == 0 {
        println!("no-value ping {}", meta);
        return Some(NetworkMessage::Ping(0));
    }
    None
}

#[derive(Debug)]
pub struct P2PMessageDecodeError {
    meta: P2PMessageMetadata,
    error: consensus::encode::Error,
}

impl P2PMessageDecodeError {
    fn new(meta: P2PMessageMetadata, error: consensus::encode::Error) -> Self {
        P2PMessageDecodeError { meta, error }
    }
}

impl fmt::Display for P2PMessageDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "P2P message decode error: {}: {}", self.meta, self.error)
    }
}

/*
impl Error for P2PMessageDecodeError {
    fn description(&self) -> &str {
        self.as_str()
    }
}
*/