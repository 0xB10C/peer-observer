#![cfg_attr(feature = "strict", deny(warnings))]

use libbpf_rs::skel::SkelBuilder;
use libbpf_rs::RingBufferBuilder;
use nng::{Protocol, Socket};
use prost::Message;
use shared::clap;
use shared::clap::Parser;
use shared::ctypes::{
    AddrmanInsertNew, AddrmanInsertTried, ClosedConnection, InboundConnection, MempoolAdded,
    MempoolRejected, MempoolRemoved, MempoolReplaced, MisbehavingConnection, OutboundConnection,
    P2PMessage, ValidationBlockConnected,
};
use shared::event_msg::event_msg::Event;
use shared::event_msg::EventMsg;
use shared::{addrman, mempool, net_conn, net_msg, validation};
use std::time::Duration;
use std::time::SystemTime;

#[path = "tracing.gen.rs"]
mod tracing;

struct Tracepoint<'a> {
    pub context: &'a str,
    pub name: &'a str,
    pub function: &'a str,
}

const TRACEPOINTS_NET_MESSAGE: [Tracepoint; 2] = [
    Tracepoint {
        context: "net",
        name: "inbound_message",
        function: "handle_net_msg_inbound",
    },
    Tracepoint {
        context: "net",
        name: "outbound_message",
        function: "handle_net_msg_outbound",
    },
];

const TRACEPOINTS_NET_CONN: [Tracepoint; 5] = [
    Tracepoint {
        context: "net",
        name: "inbound_connection",
        function: "handle_net_conn_inbound",
    },
    Tracepoint {
        context: "net",
        name: "outbound_connection",
        function: "handle_net_conn_outbound",
    },
    Tracepoint {
        context: "net",
        name: "closed_connection",
        function: "handle_net_conn_closed",
    },
    Tracepoint {
        context: "net",
        name: "evicted_inbound_connection",
        function: "handle_net_conn_inbound_evicted",
    },
    Tracepoint {
        context: "net",
        name: "misbehaving_connection",
        function: "handle_net_conn_misbehaving",
    },
];

const TRACEPOINTS_MEMPOOL: [Tracepoint; 4] = [
    Tracepoint {
        context: "mempool",
        name: "added",
        function: "handle_mempool_added",
    },
    Tracepoint {
        context: "mempool",
        name: "removed",
        function: "handle_mempool_removed",
    },
    Tracepoint {
        context: "mempool",
        name: "replaced",
        function: "handle_mempool_replaced",
    },
    Tracepoint {
        context: "mempool",
        name: "rejected",
        function: "handle_mempool_rejected",
    },
];

const _TRACEPOINTS_ADDRMAN: [Tracepoint; 2] = [
    Tracepoint {
        context: "addrman",
        name: "attempt_add",
        function: "handle_addrman_new",
    },
    Tracepoint {
        context: "addrman",
        name: "move_to_good",
        function: "handle_addrman_tried",
    },
];

const TRACEPOINTS_VALIDATION: [Tracepoint; 1] = [Tracepoint {
    context: "validation",
    name: "block_connected",
    function: "handle_validation_block_connected",
}];

/// The peer-observer extractor
/// Hooks into a Bitcoin Core binary with tracepoints and extracts events
/// from it into a nanomsg PUB-SUB queue.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP socket the nanomsg publisher binds on.
    #[arg(short, long, default_value = "tcp://127.0.0.1:8883")]
    address: String,

    /// Path to the Bitcoin Core (bitcoind) binary that should be hooked into.
    #[arg(short, long)]
    bitcoind_path: String,
}

fn main() -> Result<(), libbpf_rs::Error> {
    let args = Args::parse();

    let mut skel_builder = tracing::TracingSkelBuilder::default();
    skel_builder.obj_builder.debug(true);

    let open_skel: tracing::OpenTracingSkel = skel_builder.open().unwrap();
    let mut obj: libbpf_rs::Object = match open_skel.obj.load() {
        Ok(skel) => skel,
        Err(e) => {
            panic!("Could not load skeleton file: {}", e);
        }
    };

    let active_tracepoints = TRACEPOINTS_NET_MESSAGE
        .iter()
        .chain(TRACEPOINTS_NET_CONN.iter())
        .chain(TRACEPOINTS_VALIDATION.iter())
        .chain(TRACEPOINTS_MEMPOOL.iter());
    let mut links = Vec::new();
    for tracepoint in active_tracepoints {
        links.push(obj.prog_mut(tracepoint.function).unwrap().attach_usdt(
            -1,
            &args.bitcoind_path,
            tracepoint.context,
            tracepoint.name,
        )?)
    }

    let socket: Socket = Socket::new(Protocol::Pub0).unwrap();
    match socket.listen(&args.address) {
        Ok(()) => {
            println!("listening on {}", args.address);
        }
        Err(e) => {
            panic!("Could not listen on {}: {}", args.address, e);
        }
    }

    let mut ringbuff_builder = RingBufferBuilder::new();

    #[cfg_attr(rustfmt, rustfmt_skip)]
    ringbuff_builder
        .add(obj.map("net_msg_small").unwrap(), |data| { handle_net_message(data, socket.clone()) })?
        .add(obj.map("net_msg_medium").unwrap(), |data| { handle_net_message(data, socket.clone()) })?
        .add(obj.map("net_msg_large").unwrap(), |data| { handle_net_message(data, socket.clone()) })?
        .add(obj.map("net_msg_huge").unwrap(), |data| { handle_net_message(data, socket.clone()) })?
        .add(obj.map("net_conn_inbound").unwrap(), |data| { handle_net_conn_inbound(data, socket.clone()) })?
        .add(obj.map("net_conn_outbound").unwrap(), |data| { handle_net_conn_outbound(data, socket.clone()) })?
        .add(obj.map("net_conn_closed").unwrap(), |data| { handle_net_conn_closed(data, socket.clone()) })?
        .add(obj.map("net_conn_inbound_evicted").unwrap(), |data| { handle_net_conn_inbound_evicted(data, socket.clone()) })?
        .add(obj.map("net_conn_misbehaving").unwrap(), |data| { handle_net_conn_misbehaving(data, socket.clone()) })?
        .add(obj.map("addrman_insert_new").unwrap(), |data| { handle_addrman_new(data, socket.clone()) })?
        .add(obj.map("addrman_insert_tried").unwrap(), |data| { handle_addrman_tried(data, socket.clone()) })?
        .add(obj.map("mempool_added").unwrap(), |data| { handle_mempool_added(data, socket.clone()) })?
        .add(obj.map("mempool_removed").unwrap(), |data| { handle_mempool_removed(data, socket.clone()) })?
        .add(obj.map("mempool_rejected").unwrap(), |data| { handle_mempool_rejected(data, socket.clone()) })?
        .add(obj.map("mempool_replaced").unwrap(), |data| { handle_mempool_replaced(data, socket.clone()) })?
        .add(obj.map("validation_block_connected").unwrap(), |data| { handle_validation_block_connected(data, socket.clone()) })?;
    let ring_buffers = ringbuff_builder.build()?;

    loop {
        match ring_buffers.poll(Duration::from_millis(1)) {
            Ok(_) => (),
            Err(e) => println!("Failed to poll on ring buffers {}", e),
        };
    }
}

fn handle_net_conn_closed(data: &[u8], s: Socket) -> i32 {
    let closed = ClosedConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Closed(closed.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_outbound(data: &[u8], s: Socket) -> i32 {
    let outbound = OutboundConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Outbound(outbound.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_inbound(data: &[u8], s: Socket) -> i32 {
    let inbound = InboundConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Inbound(inbound.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_inbound_evicted(data: &[u8], s: Socket) -> i32 {
    let evicted = ClosedConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::InboundEvicted(
                evicted.into(),
            )),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_misbehaving(data: &[u8], s: Socket) -> i32 {
    let misbehaving = MisbehavingConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Misbehaving(
                misbehaving.into(),
            )),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_message(data: &[u8], s: Socket) -> i32 {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let message = P2PMessage::from_bytes(data);
    let protobuf_message = match message.decode_to_protobuf_network_message() {
        Ok(msg) => msg.into(),
        Err(e) => {
            // TODO: warn
            println!("could not handle msg with size={}: {}", data.len(), e);
            return -1;
        }
    };
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Msg(net_msg::Message {
            meta: message.meta.create_protobuf_metadata(),
            msg: Some(protobuf_message),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_addrman_new(data: &[u8], s: Socket) -> i32 {
    let new = AddrmanInsertNew::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Addrman(addrman::AddrmanEvent {
            event: Some(addrman::addrman_event::Event::New(new.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_addrman_tried(data: &[u8], s: Socket) -> i32 {
    let tried = AddrmanInsertTried::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Addrman(addrman::AddrmanEvent {
            event: Some(addrman::addrman_event::Event::Tried(tried.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_added(data: &[u8], s: Socket) -> i32 {
    let added = MempoolAdded::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Added(added.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_removed(data: &[u8], s: Socket) -> i32 {
    let removed = MempoolRemoved::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Removed(removed.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_replaced(data: &[u8], s: Socket) -> i32 {
    let replaced = MempoolReplaced::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Replaced(replaced.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_rejected(data: &[u8], s: Socket) -> i32 {
    let rejected = MempoolRejected::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Rejected(rejected.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_validation_block_connected(data: &[u8], s: Socket) -> i32 {
    let connected = ValidationBlockConnected::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = EventMsg {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        event: Some(Event::Validation(validation::ValidationEvent {
            event: Some(validation::validation_event::Event::BlockConnected(
                connected.into(),
            )),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}
