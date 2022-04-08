use base32;
use bitcoin::hashes::hex::ToHex;
use bitcoin::hashes::Hash;
use bitcoin::network;
use bitcoin::util::bip152;

use std::fmt;
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
            tx: prefilled_tx.tx.into(),
        }
    }
}

impl From<bip152::HeaderAndShortIds> for CompactBlock {
    fn from(cmpct_block: bip152::HeaderAndShortIds) -> Self {
        CompactBlock {
            header: BlockHeader::from(cmpct_block.header),
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

impl From<network::address::Address> for address::Address {
    fn from(addr: network::address::Address) -> Self {
        match addr.socket_addr() {
            Ok(x) => {
                // is either an IPv4 or IPv6 address
                match x {
                    SocketAddr::V4(a) => address::Address::Ipv4(a.ip().to_string()),
                    SocketAddr::V6(a) => address::Address::Ipv6(a.ip().to_string()),
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
                address::Address::Torv2(format!(
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
            address: Some(addr_entry.1.clone().into()),
        }
    }
}

impl From<network::address::AddrV2Message> for Address {
    fn from(addrv2: network::address::AddrV2Message) -> Self {
        Address {
            timestamp: addrv2.time,
            services: addrv2.services.as_u64(),
            port: addrv2.port as u32,
            address: Some(addrv2.addr.into()),
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
    fn from(block: bitcoin::Block) -> Self {
        Block {
            header: block.header.into(),
            transactions: block.txdata.iter().map(|tx| tx.clone().into()).collect(),
        }
    }
}

impl From<bitcoin::Transaction> for Transaction {
    fn from(tx: bitcoin::Transaction) -> Self {
        Transaction {
            txid: tx.txid().to_vec(),
            wtxid: tx.wtxid().to_vec(),
            raw: Some(bitcoin::consensus::serialize(&tx)),
        }
    }
}

impl From<network::message_filter::GetCFCheckpt> for GetCfCheckpt {
    fn from(getcfcheckpt: network::message_filter::GetCFCheckpt) -> Self {
        GetCfCheckpt {
            filter_type: getcfcheckpt.filter_type as u32,
            stop_hash: getcfcheckpt.stop_hash.to_vec(),
        }
    }
}

impl From<network::message_filter::CFCheckpt> for CfCheckpt {
    fn from(cfcheckpt: network::message_filter::CFCheckpt) -> Self {
        CfCheckpt {
            filter_type: cfcheckpt.filter_type as u32,
            stop_hash: cfcheckpt.stop_hash.to_vec(),
            filter_headers: cfcheckpt
                .filter_headers
                .iter()
                .map(|h| h.to_vec())
                .collect(),
        }
    }
}

impl From<network::message_filter::CFHeaders> for CfHeaders {
    fn from(cfheaders: network::message_filter::CFHeaders) -> Self {
        CfHeaders {
            filter_type: cfheaders.filter_type as u32,
            stop_hash: cfheaders.stop_hash.to_vec(),
            previous_filter_header: cfheaders.previous_filter_header.to_vec(),
            filter_hashes: cfheaders.filter_hashes.iter().map(|h| h.to_vec()).collect(),
        }
    }
}

impl From<network::message_filter::GetCFilters> for GetCFilter {
    fn from(getcfilter: network::message_filter::GetCFilters) -> Self {
        GetCFilter {
            filter_type: getcfilter.filter_type as u32,
            start_height: getcfilter.start_height,
            stop_hash: getcfilter.stop_hash.to_vec(),
        }
    }
}

impl From<network::message_filter::GetCFHeaders> for GetCfHeaders {
    fn from(getcfheaders: network::message_filter::GetCFHeaders) -> Self {
        GetCfHeaders {
            filter_type: getcfheaders.filter_type as u32,
            start_height: getcfheaders.start_height as u32,
            stop_hash: getcfheaders.stop_hash.to_vec(),
        }
    }
}

impl From<network::message_filter::CFilter> for CFilter {
    fn from(cfilter: network::message_filter::CFilter) -> Self {
        CFilter {
            filter_type: cfilter.filter_type as u32,
            block_hash: cfilter.block_hash.to_vec(),
            filter: cfilter.filter.to_vec(),
        }
    }
}

impl From<bitcoin::util::merkleblock::MerkleBlock> for MerkleBlock {
    fn from(merkle_block: bitcoin::util::merkleblock::MerkleBlock) -> Self {
        MerkleBlock {
            header: merkle_block.header.into(),
            num_transactions: merkle_block.txn.num_transactions,
            bits: merkle_block.txn.bits,
            hashes: merkle_block.txn.hashes.iter().map(|h| h.to_vec()).collect(),
        }
    }
}

impl From<bitcoin::Transaction> for Tx {
    fn from(tx: bitcoin::Transaction) -> Self {
        Tx { tx: tx.into() }
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

impl fmt::Display for InventoryItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use inventory_item::Item;
        if let Some(item) = &self.item {
            match item {
                Item::Transaction(txid) => {
                    write!(f, "Tx({})", bitcoin::Txid::from_slice(&txid).unwrap())
                }
                Item::Block(hash) => write!(
                    f,
                    "Block({})",
                    bitcoin::BlockHash::from_slice(&hash).unwrap()
                ),
                Item::Wtx(wtxid) => {
                    write!(f, "WTx({})", bitcoin::Wtxid::from_slice(&wtxid).unwrap())
                }
                Item::WitnessTransaction(wtxid) => {
                    write!(f, "WitnessTx({})", bitcoin::Wtxid::from_slice(&wtxid).unwrap())
                }
                Item::WitnessBlock(hash) => {
                    write!(
                        f,
                        "WitnessBlock({})",
                        bitcoin::BlockHash::from_slice(&hash).unwrap()
                    )
                }
                Item::CompactBlock(hash) => {
                    write!(
                        f,
                        "CompactBlock({})",
                        bitcoin::BlockHash::from_slice(&hash).unwrap()
                    )
                }
                Item::Unknown(uitem) => write!(
                    f,
                    "Unknown(type={}, hash={})",
                    uitem.inv_type,
                    bitcoin::hashes::sha256::Hash::from_slice(&uitem.hash).unwrap()
                ),
                Item::Error(_) => write!(f, "Error"),
            }
        } else {
            write!(f, "None")
        }
    }
}

impl From<network::address::AddrV2> for address::Address {
    fn from(addr: network::address::AddrV2) -> Self {
        match addr {
            network::address::AddrV2::Ipv4(a) => address::Address::Ipv4(a.to_string()),
            network::address::AddrV2::Ipv6(a) => address::Address::Ipv6(a.to_string()),
            network::address::AddrV2::TorV2(a) => address::Address::Torv2(format!(
                "{}.onion",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::TorV3(a) => address::Address::Torv3(format!(
                "{}.onion",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::I2p(a) => address::Address::I2p(format!(
                "{}.b32.i2p",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &a).to_lowercase()
            )),
            network::address::AddrV2::Cjdns(a) => address::Address::Cjdns(a.to_string()),
            network::address::AddrV2::Unknown(id, a) => address::Address::Unknown(UnknownAddress {
                id: id as u32,
                address: a.to_vec(),
            }),
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
            receiver: Address {
                timestamp: 0,
                port: version_msg.receiver.port as u32,
                services: version_msg.receiver.services.as_u64(),
                address: Some(version_msg.receiver.into()),
            },
            sender: Address {
                timestamp: 0,
                port: version_msg.sender.port as u32,
                services: version_msg.sender.services.as_u64(),
                address: Some(version_msg.sender.into()),
            },
            nonce: version_msg.nonce,
            user_agent: version_msg.user_agent.clone(),
            start_height: version_msg.start_height,
            relay: version_msg.relay,
        }
    }
}

impl fmt::Display for PrefilledTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PrefilledTransaction(diff_index={}, tx={})",
            self.diff_index, self.tx,
        )
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction(txid={}, wtxid={})",
            bitcoin::Txid::from_slice(&self.txid).unwrap(),
            bitcoin::Wtxid::from_slice(&self.wtxid).unwrap()
        )
    }
}

impl fmt::Display for BlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BlockHeader(hash={}, version={}, prev_blockhash={}, merkle_root={}, time={}, bits={}, nonce={})",
            bitcoin::BlockHash::from_slice(&self.hash).unwrap(),
            self.version,
            bitcoin::BlockHash::from_slice(&self.prev_blockhash).unwrap(),
            bitcoin::TxMerkleNode::from_slice(&self.merkle_root).unwrap(),
            self.time,
            self.bits,
            self.nonce,
        )
    }
}

impl fmt::Display for address::Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            address::Address::Ipv4(a) => write!(f, "IPv4({})", a),
            address::Address::Ipv6(a) => write!(f, "IPv6({})", a),
            address::Address::Torv2(a) => write!(f, "TorV2({})", a),
            address::Address::Torv3(a) => write!(f, "TorV3({})", a),
            address::Address::I2p(a) => write!(f, "I2p({})", a),
            address::Address::Cjdns(a) => write!(f, "Cjdns({})", a),
            address::Address::Unknown(u) => write!(f, "Unknown({})", u),
        }
    }
}

impl fmt::Display for UnknownAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "id={}, bytes={}", self.id, self.address.to_hex())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Address(timestamp={}, address={}, port={}, services={})",
            self.timestamp,
            self.address.as_ref().unwrap(),
            self.port,
            self.services,
        )
    }
}

impl fmt::Display for Ping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Ping({})", self.value)
    }
}

impl fmt::Display for Pong {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pong({})", self.value)
    }
}

impl fmt::Display for Inv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let item_strs: Vec<String> = self.items.iter().map(|i| i.to_string()).collect();
        write!(f, "Inv([{}])", item_strs.join(", "))
    }
}

impl fmt::Display for GetData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let item_strs: Vec<String> = self.items.iter().map(|i| i.to_string()).collect();
        write!(f, "GetData([{}])", item_strs.join(", "))
    }
}

impl fmt::Display for NotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let item_strs: Vec<String> = self.items.iter().map(|i| i.to_string()).collect();
        write!(f, "NotFound([{}])", item_strs.join(", "))
    }
}

impl fmt::Display for Tx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tx({})", self.tx)
    }
}

impl fmt::Display for Headers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let header_strings: Vec<String> = self.headers.iter().map(|i| i.to_string()).collect();
        write!(f, "Headers([{}])", header_strings.join(", "))
    }
}

impl fmt::Display for GetHeaders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hashes_strs: Vec<String> = self
            .locator_hashes
            .iter()
            .map(|h| bitcoin::BlockHash::from_slice(&h).unwrap().to_string())
            .collect();
        write!(
            f,
            "GetHeaders(version={}, locator_hashes=[{}], stop_hash={})",
            self.version,
            hashes_strs.join(", "),
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap()
        )
    }
}

impl fmt::Display for GetBlocks {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hashes_strs: Vec<String> = self
            .locator_hashes
            .iter()
            .map(|h| bitcoin::BlockHash::from_slice(&h).unwrap().to_string())
            .collect();
        write!(
            f,
            "GetBlocks(version={}, locator_hashes=[{}], stop_hash={})",
            self.version,
            hashes_strs.join(", "),
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap()
        )
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let addr_strings: Vec<String> = self.addresses.iter().map(|i| i.to_string()).collect();
        write!(f, "Addr([{}])", addr_strings.join(", "))
    }
}

impl fmt::Display for AddrV2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let addr_strings: Vec<String> = self.addresses.iter().map(|i| i.to_string()).collect();
        write!(f, "AddrV2([{}])", addr_strings.join(", "))
    }
}

impl fmt::Display for FeeFilter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FeeFilter({})", self.fee)
    }
}

impl fmt::Display for reject::RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            reject::RejectReason::Malformed => write!(f, "Malformed"),
            reject::RejectReason::Invalid => write!(f, "Invalid"),
            reject::RejectReason::Obsolete => write!(f, "Obsolete"),
            reject::RejectReason::Duplicate => write!(f, "Duplicate"),
            reject::RejectReason::Nonstandard => write!(f, "NonStandard"),
            reject::RejectReason::Fee => write!(f, "Fee"),
            reject::RejectReason::Dust => write!(f, "Dust"),
            reject::RejectReason::Checkpoint => write!(f, "Checkpoint"),
        }
    }
}

impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Reject(command={}, reason={}, details={}, hash={})",
            self.rejected_command,
            self.reason,
            self.reason_details,
            bitcoin::BlockHash::from_slice(&self.hash)
                .unwrap()
                .to_string()
        )
    }
}

impl fmt::Display for SendCompact {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SendCompact(send_compact={}, version={})",
            self.send_compact, self.version
        )
    }
}

impl fmt::Display for CompactBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let short_id_strs: Vec<String> = self.short_ids.iter().map(|id| id.to_hex()).collect();
        let ptx_strs: Vec<String> = self.transactions.iter().map(|pt| pt.to_string()).collect();
        write!(
            f,
            "CmpctBlock(header={}, nonce={}, short_ids=[{}], prefilled_transactions=[{}])",
            self.header,
            self.nonce,
            short_id_strs.join(", "),
            ptx_strs.join(", ")
        )
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let tx_strs: Vec<String> = self.transactions.iter().map(|tx| tx.to_string()).collect();
        write!(
            f,
            "Block(header={}, transactions={})",
            self.header,
            tx_strs.join(", ")
        )
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Version(version={}, services={}, timestamp={}, receiver={}, sender={}, nonce={}, user_agent={}, start_height={}, relay={})", self.version, self.services, self.timestamp, self.receiver, self.sender, self.nonce, self.user_agent, self.start_height, self.relay)
    }
}

impl fmt::Display for GetBlockTxn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let index_strs: Vec<String> = self.tx_indexes.iter().map(|i| i.to_string()).collect();
        write!(
            f,
            "GetBlockTxn(hash={}, tx_indexes={})",
            bitcoin::BlockHash::from_slice(&self.block_hash).unwrap(),
            index_strs.join(", ")
        )
    }
}

impl fmt::Display for BlockTxn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let tx_strs: Vec<String> = self.transactions.iter().map(|tx| tx.to_string()).collect();
        write!(
            f,
            "BlockTxn(hash={}, transactions={})",
            bitcoin::BlockHash::from_slice(&self.block_hash).unwrap(),
            tx_strs.join(", ")
        )
    }
}

impl fmt::Display for Alert {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Alert(alert={})", self.alert.to_hex(),)
    }
}

impl fmt::Display for FilterAdd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FilterAdd(filter={})", self.filter.to_hex(),)
    }
}

impl fmt::Display for filter_load::BloomFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            filter_load::BloomFlags::None => write!(f, "None"),
            filter_load::BloomFlags::All => write!(f, "All"),
            filter_load::BloomFlags::PubkeyOnly => write!(f, "PubkeyOnly"),
        }
    }
}

impl fmt::Display for GetCfCheckpt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "GetCFCheckpt(filter_type={}, stop_hash={})",
            self.filter_type,
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap(),
        )
    }
}

impl fmt::Display for GetCfHeaders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "GetCFHeaders(filter_type={}, start_height={}, stop_hash={})",
            self.filter_type,
            self.start_height,
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap(),
        )
    }
}

impl fmt::Display for GetCFilter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "GetCFilter(filter_type={}, start_height={}, stop_hash={})",
            self.filter_type,
            self.start_height,
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap(),
        )
    }
}

impl fmt::Display for CFilter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CFilter(filter_type={}, block_hash={}, filter={})",
            self.filter_type,
            bitcoin::BlockHash::from_slice(&self.block_hash).unwrap(),
            self.filter.to_hex(),
        )
    }
}

impl fmt::Display for CfHeaders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let filter_hash_strs: Vec<String> = self
            .filter_hashes
            .iter()
            .map(|h| {
                bitcoin::hash_types::FilterHash::from_slice(&h)
                    .unwrap()
                    .to_string()
            })
            .collect();
        write!(
            f,
            "CFHeaders(filter_type={}, stop_hash={}, prev_filter_header={}, filter_hashes=[{}])",
            self.filter_type,
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap(),
            bitcoin::hash_types::FilterHeader::from_slice(&self.previous_filter_header).unwrap(),
            filter_hash_strs.join(", "),
        )
    }
}

impl fmt::Display for CfCheckpt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let filter_header_strs: Vec<String> = self
            .filter_headers
            .iter()
            .map(|h| {
                bitcoin::hash_types::FilterHeader::from_slice(&h)
                    .unwrap()
                    .to_string()
            })
            .collect();
        write!(
            f,
            "CFCheckpt(filter_type={}, stop_hash={}, filter_headers=[{}])",
            self.filter_type,
            bitcoin::BlockHash::from_slice(&self.stop_hash).unwrap(),
            filter_header_strs.join(", "),
        )
    }
}

impl fmt::Display for FilterLoad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FilterLoad(filter={}, hash_funcs={}, tweak={}, flags={})",
            self.filter.to_hex(),
            self.hash_funcs,
            self.tweak,
            self.flags
        )
    }
}

impl fmt::Display for Unknown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FilterLoad(command={}, payload={})",
            self.command,
            self.payload.to_hex(),
        )
    }
}

impl fmt::Display for MerkleBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bits_strs: Vec<String> = self.bits.iter().map(|b| b.to_string()).collect();
        let hash_strs: Vec<String> = self
            .hashes
            .iter()
            .map(|h| {
                bitcoin::hash_types::TxMerkleNode::from_slice(&h)
                    .unwrap()
                    .to_string()
            })
            .collect();
        write!(
            f,
            "MerkleBlock(header={}, num_transactions={}, bits=[{}], hashes=[{}])",
            self.header,
            self.num_transactions,
            bits_strs.join(","),
            hash_strs.join(", "),
        )
    }
}

impl fmt::Display for message::Msg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            message::Msg::Ping(ping) => write!(f, "{}", ping),
            message::Msg::Pong(pong) => write!(f, "{}", pong),
            message::Msg::Inv(inv) => write!(f, "{}", inv),
            message::Msg::Getdata(getdata) => write!(f, "{}", getdata),
            message::Msg::Tx(tx) => write!(f, "{}", tx),
            message::Msg::Headers(headers) => write!(f, "{}", headers),
            message::Msg::Addr(addr) => write!(f, "{}", addr),
            message::Msg::Addrv2(addr) => write!(f, "{}", addr),
            message::Msg::Feefilter(feefilter) => write!(f, "{}", feefilter),
            message::Msg::Getheaders(getheaders) => write!(f, "{}", getheaders),
            message::Msg::Getblocks(getblocks) => write!(f, "{}", getblocks),
            message::Msg::Version(version) => write!(f, "{}", version),
            message::Msg::Notfound(notfound) => write!(f, "{}", notfound),
            message::Msg::Reject(reject) => write!(f, "{}", reject),
            message::Msg::Compactblock(compactblock) => write!(f, "{}", compactblock),
            message::Msg::Sendcompact(sendcompact) => write!(f, "{}", sendcompact),
            message::Msg::Block(block) => write!(f, "{}", block),
            message::Msg::Getblocktxn(getblocktxn) => write!(f, "{}", getblocktxn),
            message::Msg::Blocktxn(blocktxn) => write!(f, "{}", blocktxn),
            message::Msg::Alert(blocktxn) => write!(f, "{}", blocktxn),
            message::Msg::Filteradd(filteradd) => write!(f, "{}", filteradd),
            message::Msg::Filterload(filterload) => write!(f, "{}", filterload),
            message::Msg::Getcfcheckpt(getcfcheckpt) => write!(f, "{}", getcfcheckpt),
            message::Msg::Cfcheckpt(cfcheckpt) => write!(f, "{}", cfcheckpt),
            message::Msg::Cfheaders(cfheaders) => write!(f, "{}", cfheaders),
            message::Msg::Getcfheaders(getcfheaders) => write!(f, "{}", getcfheaders),
            message::Msg::Getcfilter(getcfilter) => write!(f, "{}", getcfilter),
            message::Msg::Cfilter(cfilter) => write!(f, "{}", cfilter),
            message::Msg::Merkleblock(merkleblock) => write!(f, "{}", merkleblock),
            message::Msg::Unknown(unknown) => write!(f, "{}", unknown),
            message::Msg::Filterclear(_) => write!(f, "FilterClear()"),
            message::Msg::Verack(_) => write!(f, "Verack()"),
            message::Msg::Sendheaders(_) => write!(f, "SendHeaders()"),
            message::Msg::Getaddr(_) => write!(f, "Getaddr()"),
            message::Msg::Mempool(_) => write!(f, "Mempool()"),
            message::Msg::Wtxidrelay(_) => write!(f, "Wtxidrelay()"),
            message::Msg::Sendaddrv2(_) => write!(f, "Sendaddrv2()"),
        }
    }
}

impl From<&network::message::NetworkMessage> for message::Msg {
    fn from(msg: &network::message::NetworkMessage) -> Self {
        use bitcoin::network::message::NetworkMessage;
        use message::Msg;

        match msg {
            NetworkMessage::Ping(x) => Msg::Ping(Ping { value: *x }),
            NetworkMessage::Pong(x) => Msg::Pong(Pong { value: *x }),
            NetworkMessage::Inv(invs) => Msg::Inv(Inv {
                items: invs.iter().map(|inv| inv.clone().into()).collect(),
            }),
            NetworkMessage::NotFound(invs) => Msg::Notfound(NotFound {
                items: invs.iter().map(|inv| inv.clone().into()).collect(),
            }),
            NetworkMessage::Tx(tx) => Msg::Tx(tx.clone().into()),
            NetworkMessage::GetData(gets) => Msg::Getdata(GetData {
                items: gets.iter().map(|get| get.clone().into()).collect(),
            }),
            NetworkMessage::Headers(headers) => Msg::Headers(Headers {
                headers: headers.iter().map(|h| h.clone().into()).collect(),
            }),
            NetworkMessage::Addr(addrs) => Msg::Addr(Addr {
                addresses: addrs
                    .iter()
                    .map(|addr_entry| addr_entry.clone().into())
                    .collect(),
            }),
            NetworkMessage::AddrV2(addrs) => Msg::Addrv2(AddrV2 {
                addresses: addrs.iter().map(|addrv2| addrv2.clone().into()).collect(),
            }),
            NetworkMessage::FeeFilter(fee) => Msg::Feefilter(FeeFilter { fee: *fee }),
            NetworkMessage::GetHeaders(get_headers_msg) => Msg::Getheaders(GetHeaders {
                version: get_headers_msg.version,
                locator_hashes: get_headers_msg
                    .locator_hashes
                    .iter()
                    .map(|h| h.to_vec())
                    .collect(),
                stop_hash: get_headers_msg.stop_hash.to_vec(),
            }),
            NetworkMessage::GetBlocks(get_blocks_msg) => Msg::Getblocks(GetBlocks {
                version: get_blocks_msg.version,
                locator_hashes: get_blocks_msg
                    .locator_hashes
                    .iter()
                    .map(|h| h.to_vec())
                    .collect(),
                stop_hash: get_blocks_msg.stop_hash.to_vec(),
            }),
            NetworkMessage::WtxidRelay => Msg::Wtxidrelay(true),
            NetworkMessage::SendAddrV2 => Msg::Sendaddrv2(true),
            NetworkMessage::Verack => Msg::Verack(true),
            NetworkMessage::SendHeaders => Msg::Sendheaders(true),
            NetworkMessage::GetAddr => Msg::Getaddr(true),
            NetworkMessage::MemPool => Msg::Mempool(true),
            NetworkMessage::Reject(reject) => Msg::Reject(reject.clone().into()),
            NetworkMessage::Version(version) => Msg::Version(version.clone().into()),
            NetworkMessage::CmpctBlock(cmpct_block) => {
                Msg::Compactblock(cmpct_block.compact_block.clone().into())
            }
            NetworkMessage::SendCmpct(send_cmpct) => Msg::Sendcompact(send_cmpct.clone().into()),
            NetworkMessage::Block(block) => Msg::Block(block.clone().into()),
            NetworkMessage::GetBlockTxn(request) => {
                Msg::Getblocktxn(request.txs_request.clone().into())
            }
            NetworkMessage::BlockTxn(response) => {
                Msg::Blocktxn(response.transactions.clone().into())
            }
            NetworkMessage::Alert(alert) => Msg::Alert(Alert {
                alert: alert.clone(),
            }),
            NetworkMessage::FilterAdd(filteradd) => Msg::Filteradd(FilterAdd {
                filter: filteradd.data.clone(),
            }),
            NetworkMessage::FilterClear => Msg::Filterclear(true),
            NetworkMessage::FilterLoad(filterload) => Msg::Filterload(filterload.clone().into()),
            NetworkMessage::GetCFCheckpt(getcfcheckpt) => {
                Msg::Getcfcheckpt(getcfcheckpt.clone().into())
            }
            NetworkMessage::CFCheckpt(cfcheckpt) => Msg::Cfcheckpt(cfcheckpt.clone().into()),
            NetworkMessage::GetCFHeaders(getcfheaders) => {
                Msg::Getcfheaders(getcfheaders.clone().into())
            }
            NetworkMessage::CFHeaders(cfheaders) => Msg::Cfheaders(cfheaders.clone().into()),
            NetworkMessage::GetCFilters(getcfilter) => Msg::Getcfilter(getcfilter.clone().into()),
            NetworkMessage::CFilter(cfilter) => Msg::Cfilter(cfilter.clone().into()),
            NetworkMessage::MerkleBlock(merkle_block) => {
                Msg::Merkleblock(merkle_block.clone().into())
            }
            NetworkMessage::Unknown { command, payload } => Msg::Unknown(Unknown {
                command: command.to_string(),
                payload: payload.to_vec(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_address_into_addrtype_onion() {
        use bitcoin::network::address::Address;
        use bitcoin::network::constants::ServiceFlags;
        use shared::p2p;

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
            p2p::address::Address::Torv2(String::from("5wyqrzbvrdsumnok.onion"))
        );
    }
}
