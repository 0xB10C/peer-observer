use std::env;

use bcc::ring_buf::{RingBufBuilder, RingCallback};
use bcc::{BPFBuilder, USDTContext};

mod processing;
mod types;

use types::{HugeP2PMessage, LargeP2PMessage, MediumP2PMessage, SmallP2PMessage};

fn main() {
    println!("Hello, world!");
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

    let small_msg_callback = RingCallback::new(callback_small_message());
    let medium_msg_callback = RingCallback::new(callback_medium_message());
    let large_msg_callback = RingCallback::new(callback_large_message());
    let huge_msg_callback = RingCallback::new(callback_huge_message());

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

fn callback_small_message() -> Box<dyn FnMut(&[u8]) + Send> {
    Box::new(|x| {
        let msg = SmallP2PMessage::from_bytes(x);
        processing::process_p2p_message(
            &msg.meta,
            &msg.payload[..msg.meta.msg_size as usize].to_vec(),
        );
    })
}

fn callback_medium_message() -> Box<dyn FnMut(&[u8]) + Send> {
    Box::new(|x| {
        let msg = MediumP2PMessage::from_bytes(x);
        processing::process_p2p_message(
            &msg.meta,
            &msg.payload[..msg.meta.msg_size as usize].to_vec(),
        );
    })
}

fn callback_large_message() -> Box<dyn FnMut(&[u8]) + Send> {
    Box::new(|x| {
        let msg = LargeP2PMessage::from_bytes(x);
        processing::process_p2p_message(
            &msg.meta,
            &msg.payload[..msg.meta.msg_size as usize].to_vec(),
        );
    })
}

fn callback_huge_message() -> Box<dyn FnMut(&[u8]) + Send> {
    Box::new(|x| {
        let msg = HugeP2PMessage::from_bytes(x);
        processing::process_p2p_message(
            &msg.meta,
            &msg.payload[..msg.meta.msg_size as usize].to_vec(),
        );
    })
}
