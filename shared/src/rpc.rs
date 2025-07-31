use std::fmt;

use corepc_client::types::v26::{GetPeerInfo as RPCGetPeerInfo, PeerInfo as RPCPeerInfo};

// structs are generated via the rpc.proto file
include!(concat!(env!("OUT_DIR"), "/rpc.rs"));

impl fmt::Display for PeerInfos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let info_strs: Vec<String> = self.infos.iter().map(|i| i.to_string()).collect();
        write!(f, "PeerInfos([{}])", info_strs.join(", "))
    }
}

impl From<RPCGetPeerInfo> for PeerInfos {
    fn from(infos: RPCGetPeerInfo) -> Self {
        PeerInfos {
            infos: infos.0.iter().map(|i| i.clone().into()).collect(),
        }
    }
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PeerInfo(id={})", self.id,)
    }
}

impl fmt::Display for rpc_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            rpc_event::Event::PeerInfos(infos) => write!(f, "{}", infos),
        }
    }
}

impl From<RPCPeerInfo> for PeerInfo {
    fn from(info: RPCPeerInfo) -> Self {
        PeerInfo {
            address: info.address,
            address_bind: info.address_bind.unwrap_or_default(),
            address_local: info.address_local.unwrap_or_default(),
            addr_rate_limited: info.addresses_rate_limited.unwrap_or_default() as u64,
            addr_relay_enabled: info.addresses_relay_enabled.unwrap_or_default(),
            addr_processed: info.addresses_processed.unwrap_or_default() as u64,
            bip152_hb_from: info.bip152_hb_from,
            bip152_hb_to: info.bip152_hb_to,
            bytes_received: info.bytes_received,
            bytes_received_per_message: info.bytes_received_per_message.into_iter().collect(),
            bytes_sent: info.bytes_sent,
            bytes_sent_per_message: info.bytes_sent_per_message.into_iter().collect(),
            connection_time: info.connection_time,
            connection_type: info.connection_type.unwrap_or_default(),
            id: info.id,
            inbound: info.inbound,
            inflight: info.inflight.unwrap_or_default(),
            last_block: info.last_block,
            last_received: info.last_received,
            last_send: info.last_send,
            last_transaction: info.last_transaction,
            mapped_as: info.mapped_as.unwrap_or_default(),
            minfeefilter: info.minimum_fee_filter,
            minimum_ping: info.minimum_ping.unwrap_or_default(),
            network: info.network,
            ping_time: info.ping_time.unwrap_or_default(),
            ping_wait: info.ping_wait.unwrap_or_default(),
            permissions: info.permissions,
            relay_transactions: info.relay_transactions,
            services: info.services,
            starting_height: info.starting_height.unwrap_or_default(),
            subversion: info.subversion,
            synced_blocks: info.synced_blocks.unwrap_or_default(),
            synced_headers: info.synced_headers.unwrap_or_default(),
            time_offset: info.time_offset,
            transport_protocol_type: info.transport_protocol_type,
            version: info.version,
        }
    }
}
