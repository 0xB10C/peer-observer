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
        let proto = p2p::Message {
            meta: metadata.create_protobuf_metadata(),
            msg: Some((&network_msg).into()),
        };
        s.send(&proto.encode_to_vec()).unwrap();
    })
}
