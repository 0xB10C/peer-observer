use std::{fmt, ptr};

use bitcoin::consensus;
use bitcoin::consensus::encode::Decodable;
use bitcoin::hashes::{hex::ToHex, sha256d, Hash};
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

const TXID_LENGTH: usize = 32;
const REMOVAL_REASON_LENGTH: usize = 9;
const REJECTION_REASON_LENGTH: usize = 118;

#[repr(usize)]
pub enum P2PMessageSize {
    Small = MAX_SMALL_MSG_LENGTH,
    Medium = MAX_MEDIUM_MSG_LENGTH,
    Large = MAX_LARGE_MSG_LENGTH,
    Huge = MAX_HUGE_MSG_LENGTH,
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
            "{}-msg ({} bytes) {} peer (id={}, addr={}, conntype={})",
            self.msg_type(),
            self.msg_size,
            if self.msg_inbound { "from" } else { "to" },
            self.peer_id,
            self.peer_addr(),
            self.peer_conn_type(),
        )
    }
}

#[repr(C)]
pub struct P2PMessage<const N: usize> {
    pub meta: P2PMessageMetadata,
    pub payload: [u8; N],
}

impl<const N: usize> P2PMessage<N> {
    pub fn from_bytes(x: &[u8]) -> P2PMessage<N> {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const P2PMessage<N>) }
    }

    // The msg.payload is MAX_SMALL_MSG_LENGTH bytes long, however the acctual
    // message size in meta.msg_size is likely smaller. Returns a slice
    // with the acctual message payload.
    pub fn trimmed_payload(&self) -> &[u8] {
        return &self.payload[..self.meta.msg_size as usize];
    }

    pub fn decode_to_protobuf_network_message(
        &self,
    ) -> Result<net_msg::message::Msg, P2PMessageDecodeError> {
        decode_network_message(&self.meta, self.trimmed_payload())
    }
}

#[repr(C)]
pub struct Connection {
    pub id: u64,
    pub addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub conn_type: [u8; MAX_PEER_CONN_TYPE_LENGTH],
    pub network: u32,
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
            "Connection(peer={}, addr={}, type={}, network={})",
            self.id,
            self.addr(),
            self.conn_type(),
            self.network,
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

#[repr(C)]
pub struct MempoolAdded {
    /// Txid of the added transaction
    pub txid: [u8; TXID_LENGTH],
    /// Vsize of the added transaction
    pub vsize: i32,
    /// Fee of the added transaction
    pub fee: i64,
}

impl fmt::Display for MempoolAdded {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MempoolAdded(txid={}, vsize={}, fee={})",
            bitcoin::Txid::from_slice(&self.txid).unwrap(),
            self.vsize,
            self.fee,
        )
    }
}

impl MempoolAdded {
    pub fn from_bytes(x: &[u8]) -> MempoolAdded {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MempoolAdded) }
    }
}

#[repr(C)]
pub struct MempoolRemoved {
    /// Txid of the removed transaction
    pub txid: [u8; TXID_LENGTH],
    /// Removal reason of the transaction
    pub reason: [u8; REMOVAL_REASON_LENGTH],
    /// Virtual size of the removed transaction
    pub vsize: i32,
    /// Fee of the removed transaction
    pub fee: i64,
    /// Mempool entry time of the removed transaction.
    pub entry_time: u64,
}

impl fmt::Display for MempoolRemoved {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MempoolRemoved(txid={}, reason={}, vsize={}, fee={}, entry_time={})",
            bitcoin::Txid::from_slice(&self.txid).unwrap(),
            self.reason(),
            self.vsize,
            self.fee,
            self.entry_time,
        )
    }
}

impl MempoolRemoved {
    /// Returns the removal reason as String
    pub fn reason(&self) -> String {
        String::from_utf8_lossy(&self.reason.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    pub fn from_bytes(x: &[u8]) -> MempoolRemoved {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MempoolRemoved) }
    }
}

#[repr(C)]
pub struct MempoolReplaced {
    /// Txid of the replaced transaction
    pub replaced_txid: [u8; TXID_LENGTH],
    /// Virtual size of the replaced transaction
    pub replaced_vsize: i32,
    /// Fee of the replaced transaction
    pub replaced_fee: i64,
    /// Mempool entry time of the replaced transaction.
    pub replaced_entry_time: u64,
    /// Txid of the replacement transaction
    pub replacement_txid: [u8; TXID_LENGTH],
    /// Virtual size of the replacement transaction
    pub replacement_vsize: i32,
    /// Fee of the replaced transaction
    pub replacement_fee: i64,
}

impl MempoolReplaced {
    pub fn from_bytes(x: &[u8]) -> MempoolReplaced {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MempoolReplaced) }
    }
}

impl fmt::Display for MempoolReplaced {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MempoolReplaced(old(txid={}, vsize={}, fee={}, entry_time={}), new(txid={}, vsize={}, fee={}))",
            bitcoin::Txid::from_slice(&self.replaced_txid).unwrap(),
            self.replaced_vsize,
            self.replaced_fee,
            self.replaced_entry_time,
            bitcoin::Txid::from_slice(&self.replacement_txid).unwrap(),
            self.replacement_vsize,
            self.replacement_fee,
        )
    }
}

#[repr(C)]
pub struct MempoolRejected {
    /// Txid of the added transaction
    pub txid: [u8; TXID_LENGTH],
    /// Reason why the transaction was rejected
    pub reason: [u8; REJECTION_REASON_LENGTH],
}

impl fmt::Display for MempoolRejected {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MempoolRejected(txid={}, reason={})",
            bitcoin::Txid::from_slice(&self.txid).unwrap(),
            self.reason(),
        )
    }
}

impl MempoolRejected {
    /// Returns the rejection reason as String
    pub fn reason(&self) -> String {
        String::from_utf8_lossy(&self.reason.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    pub fn from_bytes(x: &[u8]) -> MempoolRejected {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const MempoolRejected) }
    }
}

fn decode_network_message(
    meta: &P2PMessageMetadata,
    payload: &[u8],
) -> Result<net_msg::message::Msg, P2PMessageDecodeError> {
    // We first try to decode the network message with rust-bitcoin.
    // If that fails, we try to handle a few known, weird messages.
    match decode_rust_bitcoin_network_message(meta, payload) {
        Ok(rust_bitcoin_network_message) => Ok((&rust_bitcoin_network_message).into()),
        Err(e) => {
            if let Some(message) = decode_weird_network_message(meta, payload) {
                return Ok(message);
            }
            return Err(e);
        }
    }
}

fn decode_rust_bitcoin_network_message(
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
        Err(e) => Err(P2PMessageDecodeError::new(meta.clone(), e)),
    }
}

// There might be cases where rust-bitcoin can't deserialize a message.
// This happens, for example, when a message has no elements but rust-bitcoin
// expects at least one element. We try to handle known cases here on a best
// effort basis.
fn decode_weird_network_message(
    meta: &P2PMessageMetadata,
    payload: &[u8],
) -> Option<net_msg::message::Msg> {
    match meta.msg_type().as_str() {
        "addrv2" => {
            if meta.msg_size == 0 {
                // case: empty addrv2 message.
                println!("emtpy addrv2: {}", meta);
                return Some(net_msg::message::Msg::Emptyaddrv2(true));
            }
        }
        "ping" => {
            if meta.msg_size == 0 {
                // case: old ping message with no nonce.
                println!("no-value ping: {}", meta);
                return Some(net_msg::message::Msg::Oldping(true));
            }
        }
        "tx" => {
            println!(
                "invalid (?) tx with {} byte: {}",
                meta.msg_size,
                payload.to_hex()
            );
        }
        _ => (),
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

#[repr(C)]
pub struct AddrmanInsertNew {
    pub inserted: bool,
    pub bucket: i32,
    pub bucket_pos: i32,
    pub addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub addr_as: u32,
    pub source: [u8; MAX_PEER_ADDR_LENGTH],
    pub source_as: u32,
}

impl AddrmanInsertNew {
    // TODO: comment
    pub fn addr(&self) -> String {
        String::from_utf8_lossy(&self.addr.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    // TODO: comment
    pub fn source(&self) -> String {
        String::from_utf8_lossy(&self.source.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    pub fn from_bytes(x: &[u8]) -> AddrmanInsertNew {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const AddrmanInsertNew) }
    }
}

impl fmt::Display for AddrmanInsertNew {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AddrmanInsertNew(inserted={}, bucket={}, bucket_pos={}, addr={}, addr_AS={}, source={}, source_AS={})",
            self.inserted,
            self.bucket,
            self.bucket_pos,
            self.addr(),
            self.addr_as,
            self.source(),
            self.source_as,
        )
    }
}

#[repr(C)]
pub struct AddrmanInsertTried {
    pub bucket: i32,
    pub bucket_pos: i32,
    pub addr: [u8; MAX_PEER_ADDR_LENGTH],
    pub addr_as: u32,
    pub source: [u8; MAX_PEER_ADDR_LENGTH],
    pub source_as: u32,
}

impl AddrmanInsertTried {
    // TODO: comment
    pub fn addr(&self) -> String {
        String::from_utf8_lossy(&self.addr.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    // TODO: comment
    pub fn source(&self) -> String {
        String::from_utf8_lossy(&self.source.split(|c| *c == 0x00u8).next().unwrap()).into_owned()
    }

    pub fn from_bytes(x: &[u8]) -> AddrmanInsertTried {
        unsafe { ptr::read_unaligned(x.as_ptr() as *const AddrmanInsertTried) }
    }
}

impl fmt::Display for AddrmanInsertTried {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AddrmanInsertTried(bucket={}, bucket_pos={}, addr={}, addr_AS={}, source={}, source_AS={})",
            self.bucket,
            self.bucket_pos,
            self.addr(),
            self.addr_as,
            self.source(),
            self.source_as,
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn p2p_message_from_bytes_1() {
        const PAYLOAD_SIZE: usize = 8;

        // The acctual message ends after the "92e4200d3021c21b" payload. It's a few bytes larger
        // on purpose to test that it's still parsed correctly.
        let data_hex = "c79e9300000000003230392e3232322e3235322e34303a36343830390000000069746e6573732076657273696f6e20726573657276656420666f7220736f66742d666f726b20757067726164696e626f756e64005583899738227ad1576a13fc70696e6700000000f5d60e67005930cb080000000000000092e4200d3021c21b649b92000000000033";
        let data = hex::decode(data_hex).unwrap();

        let expected_peer_id = 9674439u64;
        let expected_peer_addr = "209.222.252.40:64809";
        let expected_peer_conn_type = "inbound";
        let expected_msg_type = "ping";
        let expected_msg_inbound = false;
        let expected_msg_size = 8u64;
        let expected_payload = hex::decode("92e4200d3021c21b").unwrap();

        let message = P2PMessage::<PAYLOAD_SIZE>::from_bytes(&data);

        assert_eq!(expected_peer_id, message.meta.peer_id);
        assert_eq!(expected_peer_addr, message.meta.peer_addr());
        assert_eq!(expected_peer_conn_type, message.meta.peer_conn_type());
        assert_eq!(expected_msg_type, message.meta.msg_type());
        assert_eq!(expected_msg_inbound, message.meta.msg_inbound);
        assert_eq!(expected_msg_size, message.meta.msg_size);
        assert_eq!(expected_payload, message.payload);

        message.decode_to_protobuf_network_message().unwrap();
    }
}
