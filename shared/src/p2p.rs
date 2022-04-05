use base32;
use bitcoin::network;
use bitcoin::util::bip152;
use std::net::SocketAddr;

// structs are generated via the p2p.proto file

include!(concat!(env!("OUT_DIR"), "/p2p.rs"));

impl From<bitcoin::BlockHeader> for BlockHeader {
    fn from(header: bitcoin::BlockHeader) -> Self {
        BlockHeader {
            hash: header.block_hash().to_vec(),
            version: header.version,
            prev_blockhash: header.prev_blockhash.to_vec(),
            merkle_root: header.merkle_root.to_vec(),
            time: header.time,
            bits: header.bits,
            nonce: header.nonce,
        }
    }
}

impl From<bip152::PrefilledTransaction> for PrefilledTransaction {
    fn from(prefilled_tx: bip152::PrefilledTransaction) -> Self {
        PrefilledTransaction {
            diff_index: prefilled_tx.idx as u32,
            tx: Some(prefilled_tx.tx.into()),
        }
    }
}

impl From<bip152::HeaderAndShortIds> for CompactBlock {
    fn from(cmpct_block: bip152::HeaderAndShortIds) -> Self {
        CompactBlock {
            header: Some(BlockHeader::from(cmpct_block.header)),
            nonce: cmpct_block.nonce,
            short_ids: cmpct_block
                .short_ids
                .iter()
                .map(|id| id.0.to_vec())
                .collect(),
            transactions: cmpct_block
                .prefilled_txs
                .iter()
                .map(|tx| PrefilledTransaction::from(tx.clone()))
                .collect(),
        }
    }
}

impl From<network::address::Address> for net_address::AddrType {
    fn from(addr: network::address::Address) -> Self {
        match addr.socket_addr() {
            Ok(x) => {
                // is either an IPv4 or IPv6 address
                match x {
                    SocketAddr::V4(a) => net_address::AddrType::Ipv4(a.ip().to_string()),
                    SocketAddr::V6(a) => net_address::AddrType::Ipv6(a.ip().to_string()),
                }
            }
            Err(_) => {
                let mut addr_vec = vec![];
                for (i, x) in addr.address.iter().enumerate() {
                    // The first three u16's, contain an onion prefix, are skipped here.
                    if i >= 3 {
                        addr_vec.push((*x >> 8) as u8);
                        addr_vec.push(*x as u8);
                    }
                }
                net_address::AddrType::Torv2(format!(
                    "{}.onion",
                    base32::encode(base32::Alphabet::RFC4648 { padding: false }, &addr_vec)
                        .to_lowercase()
                ))
            }
        }
    }
}

impl From<(u32, network::address::Address)> for Address {
    fn from(addr_entry: (u32, network::address::Address)) -> Self {
        Address {
            timestamp: addr_entry.0,
            services: addr_entry.1.services.as_u64(),
            port: addr_entry.1.port as u32,
            address: Some(NetAddress {
                addr_type: Some(addr_entry.1.clone().into()),
            }),
        }
    }
}

impl From<network::address::AddrV2Message> for Address {
    fn from(addrv2: network::address::AddrV2Message) -> Self {
        Address {
            timestamp: addrv2.time,
            services: addrv2.services.as_u64(),
            port: addrv2.port as u32,
            address: Some(NetAddress {
                addr_type: Some(addrv2.addr.into()),
            }),
        }
    }
}

impl From<network::message_compact_blocks::SendCmpct> for SendCompact {
    fn from(send_cmpct: network::message_compact_blocks::SendCmpct) -> Self {
        SendCompact {
            send_compact: send_cmpct.send_compact,
            version: send_cmpct.version,
        }
    }
}

impl From<bitcoin::Block> for Block {
    fn from(block: bitcoin::blockdata::block::Block) -> Self {
        Block {
            header: Some(block.header.into()),
            transactions: block.txdata.iter().map(|tx| tx.clone().into()).collect(),
        }
    }
}

impl From<bitcoin::Transaction> for Transaction {
    fn from(tx: bitcoin::blockdata::transaction::Transaction) -> Self {
        Transaction {
            txid: tx.txid().to_vec(),
            wtxid: tx.wtxid().to_vec(),
            raw: Some(bitcoin::consensus::serialize(&tx)),
        }
    }
}

impl From<bitcoin::Transaction> for Tx {
    fn from(tx: bitcoin::Transaction) -> Self {
        Tx {
            tx: Some(tx.into()),
        }
    }
}

impl From<bip152::BlockTransactionsRequest> for GetBlockTxn {
    fn from(request: bip152::BlockTransactionsRequest) -> Self {
        GetBlockTxn {
            block_hash: request.block_hash.to_vec(),
            tx_indexes: request.indexes,
        }
    }
}

impl From<bip152::BlockTransactions> for BlockTxn {
    fn from(blocktxn: bip152::BlockTransactions) -> Self {
        BlockTxn {
            block_hash: blocktxn.block_hash.to_vec(),
            transactions: blocktxn
                .transactions
                .iter()
                .map(|tx| tx.clone().into())
                .collect(),
        }
    }
}

impl From<network::message_bloom::FilterLoad> for FilterLoad {
    fn from(filterload: network::message_bloom::FilterLoad) -> Self {
        FilterLoad {
            filter: filterload.filter.to_vec(),
            hash_funcs: filterload.hash_funcs,
            tweak: filterload.tweak,
            flags: match filterload.flags {
                network::message_bloom::BloomFlags::None => filter_load::BloomFlags::None as i32,
                network::message_bloom::BloomFlags::All => filter_load::BloomFlags::All as i32,
                network::message_bloom::BloomFlags::PubkeyOnly => {
                    filter_load::BloomFlags::PubkeyOnly as i32
                }
            },
        }
    }
}

impl From<network::message_blockdata::Inventory> for InventoryItem {
    fn from(inv_item: network::message_blockdata::Inventory) -> Self {
        use network::message_blockdata::Inventory;
        match inv_item {
            Inventory::Error => InventoryItem {
                item: Some(inventory_item::Item::Error(true)),
            },
            Inventory::Transaction(txid) => InventoryItem {
                item: Some(inventory_item::Item::Transaction(txid.to_vec())),
            },
            Inventory::Block(hash) => InventoryItem {
                item: Some(inventory_item::Item::Block(hash.to_vec())),
            },
            Inventory::WTx(wtxid) => InventoryItem {
                item: Some(inventory_item::Item::Wtx(wtxid.to_vec())),
            },
            Inventory::WitnessTransaction(txid) => InventoryItem {
                item: Some(inventory_item::Item::WitnessTransaction(txid.to_vec())),
            },
            Inventory::WitnessBlock(hash) => InventoryItem {
                item: Some(inventory_item::Item::WitnessBlock(hash.to_vec())),
            },
            Inventory::CompactBlock(hash) => InventoryItem {
                item: Some(inventory_item::Item::CompactBlock(hash.to_vec())),
            },
            Inventory::Unknown { inv_type, hash } => InventoryItem {
                item: Some(inventory_item::Item::Unknown(UnknownItem {
                    inv_type: inv_type,
                    hash: hash.to_vec(),
                })),
            },
        }
    }
}

impl From<network::address::AddrV2> for net_address::AddrType {
    fn from(addr: network::address::AddrV2) -> Self {
        match addr {
            network::address::AddrV2::Ipv4(a) => net_address::AddrType::Ipv4(a.to_string()),
            network::address::AddrV2::Ipv6(a) => net_address::AddrType::Ipv6(a.to_string()),
            network::address::AddrV2::TorV2(a) => net_address::AddrType::Torv2(format!(
                "{}.onion",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::TorV3(a) => net_address::AddrType::Torv3(format!(
                "{}.onion",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::I2p(a) => net_address::AddrType::I2p(format!(
                "{}.b32.i2p",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::Cjdns(a) => net_address::AddrType::Cjdns(a.to_string()),
            network::address::AddrV2::Unknown(id, a) => {
                net_address::AddrType::Unknown(UnknownNetAddress {
                    id: id as u32,
                    address: a.to_vec(),
                })
            }
        }
    }
}

impl From<network::message_network::Reject> for Reject {
    fn from(reject: network::message_network::Reject) -> Self {
        Reject {
            rejected_command: reject.message.to_string(),
            reason: match reject.ccode {
                network::message_network::RejectReason::Malformed => {
                    reject::RejectReason::Malformed as i32
                }
                network::message_network::RejectReason::Invalid => {
                    reject::RejectReason::Invalid as i32
                }
                network::message_network::RejectReason::Obsolete => {
                    reject::RejectReason::Obsolete as i32
                }
                network::message_network::RejectReason::Duplicate => {
                    reject::RejectReason::Duplicate as i32
                }
                network::message_network::RejectReason::NonStandard => {
                    reject::RejectReason::Nonstandard as i32
                }
                network::message_network::RejectReason::Dust => reject::RejectReason::Dust as i32,
                network::message_network::RejectReason::Fee => reject::RejectReason::Fee as i32,
                network::message_network::RejectReason::Checkpoint => {
                    reject::RejectReason::Checkpoint as i32
                }
            },
            reason_details: reject.reason.clone().into(),
            hash: reject.hash.to_vec(),
        }
    }
}

impl From<network::message_network::VersionMessage> for Version {
    fn from(version_msg: network::message_network::VersionMessage) -> Self {
        Version {
            version: version_msg.version,
            services: version_msg.services.as_u64(),
            timestamp: version_msg.timestamp,
            receiver: Some(Address {
                timestamp: 0,
                port: version_msg.receiver.port as u32,
                services: version_msg.receiver.services.as_u64(),
                address: Some(NetAddress {
                    addr_type: Some(version_msg.receiver.into()),
                }),
            }),
            sender: Some(Address {
                timestamp: 0,
                port: version_msg.sender.port as u32,
                services: version_msg.sender.services.as_u64(),
                address: Some(NetAddress {
                    addr_type: Some(version_msg.sender.into()),
                }),
            }),
            nonce: version_msg.nonce,
            user_agent: version_msg.user_agent.clone(),
            start_height: version_msg.start_height,
            relay: version_msg.relay,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_address_into_addrtype_onion() {
        use bitcoin::network::address::Address;
        use bitcoin::network::constants::ServiceFlags;
        use shared::p2p::net_address::AddrType;

        let a = Address {
            services: ServiceFlags::from(0),
            // From
            // https://github.com/bitcoin/bitcoin/pull/1174/commits/70f7f0038592a28e846f02d084f0119fc34eb52f#diff-1d804b435c22c29ffc21022ad3a88a8b7d3bd44ae451c3d50e4700344ffef60dR93-R100
            address: [
                0xFD87, 0xD87E, 0xEB43, 0xedb1, 0x8e4, 0x3588, 0xe546, 0x35ca,
            ],
            port: 0,
        };
        assert_eq!(
            a.into(),
            AddrType::Torv2(String::from("5wyqrzbvrdsumnok.onion"))
        );
    }
}
