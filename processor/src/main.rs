use bcc::ring_buf::{RingBufBuilder, RingCallback};
use bcc::{BPFBuilder, USDTContext};
use std::env;

use bitcoin::network::message::NetworkMessage;
use nng::{Protocol, Socket};

use prost::Message;

mod types;

use types::{
    HugeP2PMessage, LargeP2PMessage, MediumP2PMessage, P2PMessageMetadata, P2PMessageSize,
    RustBitcoinNetworkMessage, SmallP2PMessage,
};

use shared::p2p;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let bitcoind_path = env::args().nth(1).expect("No bitcoind path provided.");
    let mut usdt_ctx = USDTContext::from_binary_path(bitcoind_path).unwrap();
    usdt_ctx
        .enable_probe("net:inbound_message", "trace_inbound_message_rb")
        .unwrap();
    usdt_ctx
        .enable_probe("net:outbound_message", "trace_outbound_message")
        .unwrap();
    let code = concat!(
        "#include <uapi/linux/ptrace.h>",
        "\n\n",
        include_str!("../bcc-programs/net_in_outbound.c"),
    );
    let bpf = BPFBuilder::new(code)
        .unwrap()
        .add_usdt_context(usdt_ctx)
        .unwrap()
        .build()
        .unwrap();

    let small_msgs_rb = bpf.table("messages_small").unwrap();
    let medium_msgs_rb = bpf.table("messages_medium").unwrap();
    let large_msgs_rb = bpf.table("messages_large").unwrap();
    let huge_msgs_rb = bpf.table("messages_huge").unwrap();

    let s: Socket = Socket::new(Protocol::Pub0).unwrap();
    s.listen(ADDRESS).unwrap();
    println!("listening on {}", ADDRESS);

    let small_msg_callback =
        RingCallback::new(callback_p2p_message(P2PMessageSize::Small, s.clone()));
    let medium_msg_callback =
        RingCallback::new(callback_p2p_message(P2PMessageSize::Medium, s.clone()));
    let large_msg_callback =
        RingCallback::new(callback_p2p_message(P2PMessageSize::Large, s.clone()));
    let huge_msg_callback =
        RingCallback::new(callback_p2p_message(P2PMessageSize::Huge, s.clone()));

    let mut p2p_messages = RingBufBuilder::new(small_msgs_rb, small_msg_callback)
        .add(medium_msgs_rb, medium_msg_callback)
        .add(large_msgs_rb, large_msg_callback)
        .add(huge_msgs_rb, huge_msg_callback)
        .build()
        .unwrap();

    loop {
        p2p_messages.poll(20);
    }
}

fn callback_p2p_message(bcc_msg_size: P2PMessageSize, s: Socket) -> Box<dyn FnMut(&[u8]) + Send> {
    Box::new(move |x| {
        let metadata: P2PMessageMetadata;
        let network_msg: NetworkMessage;

        match bcc_msg_size {
            P2PMessageSize::Small => {
                let msg = SmallP2PMessage::from_bytes(x);
                metadata = msg.meta.clone();
                network_msg = msg.rust_bitcoin_network_message();
            }
            P2PMessageSize::Medium => {
                let msg = MediumP2PMessage::from_bytes(x);
                metadata = msg.meta.clone();
                network_msg = msg.rust_bitcoin_network_message();
            }
            P2PMessageSize::Large => {
                let msg = LargeP2PMessage::from_bytes(x);
                metadata = msg.meta.clone();
                network_msg = msg.rust_bitcoin_network_message();
            }
            P2PMessageSize::Huge => {
                let msg = HugeP2PMessage::from_bytes(x);
                metadata = msg.meta.clone();
                network_msg = msg.rust_bitcoin_network_message();
            }
        };
        publish_p2p_message(&metadata, &network_msg, &s);
    })
}

fn publish_p2p_message(meta: &P2PMessageMetadata, msg: &NetworkMessage, s: &Socket) {
    match msg {
        NetworkMessage::Ping(x) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Ping(p2p::Ping { value: *x })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Pong(x) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Pong(p2p::Pong { value: *x })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Inv(invs) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Inv(p2p::Inv {
                    items: invs.iter().map(|inv| inv.clone().into()).collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::NotFound(invs) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Notfound(p2p::NotFound {
                    items: invs.iter().map(|inv| inv.clone().into()).collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Tx(tx) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Tx(tx.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetData(gets) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getdata(p2p::GetData {
                    items: gets.iter().map(|get| get.clone().into()).collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Headers(headers) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Headers(p2p::Headers {
                    headers: headers.iter().map(|h| h.clone().into()).collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Addr(addrs) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Addr(p2p::Addr {
                    addresses: addrs
                        .iter()
                        .map(|addr_entry| addr_entry.clone().into())
                        .collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::AddrV2(addrs) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Addrv2(p2p::AddrV2 {
                    addresses: addrs.iter().map(|addrv2| addrv2.clone().into()).collect(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::FeeFilter(fee) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Feefilter(p2p::FeeFilter { fee: *fee })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetHeaders(get_headers_msg) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getheaders(p2p::GetHeaders {
                    version: get_headers_msg.version,
                    hashes: get_headers_msg
                        .locator_hashes
                        .iter()
                        .map(|h| h.to_vec())
                        .collect(),
                    stop_hash: get_headers_msg.stop_hash.to_vec(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetBlocks(get_blocks_msg) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getblocks(p2p::GetBlocks {
                    version: get_blocks_msg.version,
                    hashes: get_blocks_msg
                        .locator_hashes
                        .iter()
                        .map(|h| h.to_vec())
                        .collect(),
                    stop_hash: get_blocks_msg.stop_hash.to_vec(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::WtxidRelay => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Wtxidrelay(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::SendAddrV2 => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Sendaddrv2(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Verack => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Verack(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::SendHeaders => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Sendheaders(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetAddr => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getaddr(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::MemPool => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Mempool(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Reject(reject) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Reject(reject.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Version(version) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Version(version.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::CmpctBlock(cmpct_block) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Compactblock(
                    cmpct_block.compact_block.clone().into(),
                )),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::SendCmpct(send_cmpct) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Sendcompact(send_cmpct.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Block(block) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Block(block.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetBlockTxn(request) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getblocktxn(
                    request.txs_request.clone().into(),
                )),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::BlockTxn(response) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Blocktxn(
                    response.transactions.clone().into(),
                )),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Alert(alert) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Alert(p2p::Alert {
                    alert: alert.clone(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::FilterAdd(filteradd) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Filteradd(p2p::FilterAdd {
                    filter: filteradd.data.clone(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::FilterClear => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Filterclear(true)),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::FilterLoad(filterload) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Filterload(filterload.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetCFCheckpt(getcfcheckpt) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getcfcheckpt(getcfcheckpt.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::CFCheckpt(cfcheckpt) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Cfcheckpt(cfcheckpt.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetCFHeaders(getcfheaders) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getcfheaders(getcfheaders.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::CFHeaders(cfheaders) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Cfheaders(cfheaders.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::GetCFilters(getcfilter) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Getcfilter(getcfilter.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::CFilter(cfilter) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Cfilter(cfilter.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::MerkleBlock(merkle_block) => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Merkleblock(merkle_block.clone().into())),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
        NetworkMessage::Unknown { command, payload } => {
            let proto_msg = p2p::Message {
                meta: Some(meta.create_protobuf_metadata()),
                msg: Some(p2p::message::Msg::Unknown(p2p::Unknown {
                    command: command.to_string(),
                    payload: payload.to_vec(),
                })),
            };
            s.send(&proto_msg.encode_to_vec()).unwrap();
        }
    }
}
