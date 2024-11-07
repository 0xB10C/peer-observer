#![cfg_attr(feature = "strict", deny(warnings))]

use libbpf_rs::skel::{OpenSkel, Skel, SkelBuilder};
use libbpf_rs::{Map, MapCore, Object, ProgramMut, RingBufferBuilder};
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
use shared::log;
use shared::nng::{Protocol, Socket};
use shared::simple_logger;
use shared::{addrman, mempool, net_conn, net_msg, validation};
use std::mem::MaybeUninit;
use std::time::Duration;

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

const TRACEPOINTS_ADDRMAN: [Tracepoint; 2] = [
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

    // Default tracepoints
    /// Controls if the p2p message tracepoints should be hooked into.
    #[arg(long)]
    no_p2pmsg_tracepoints: bool,
    /// Controls if the connection tracepoints should be hooked into.
    #[arg(long)]
    no_connection_tracepoints: bool,
    /// Controls if the mempool tracepoints should be hooked into.
    #[arg(long)]
    no_mempool_tracepoints: bool,
    /// Controls if the validation tracepoints should be hooked into.
    #[arg(long)]
    no_validation_tracepoints: bool,

    // Custom tracepoints
    /// Controls if the addrman tracepoints should be hooked into.
    /// These may not have been PRed to Bitcoin Core yet.
    #[arg(long)]
    addrman_tracepoints: bool,

    /// The log level the extractor should run with. Valid log levels are "trace",
    /// "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

/// Find the BPF program with the given name, panic if it does not exist.
// inspired by https://github.com/libbpf/libbpf-rs/blob/18581bc5fb85bbbfd2c6ca6424a56f7b5d438470/libbpf-rs/tests/common/mod.rs#L70-L77
// TODO: this currently panics
// We can probably be a bit nicer with the error handling here.
pub fn find_prog_mut<'obj>(object: &'obj Object, name: &str) -> ProgramMut<'obj> {
    object
        .progs_mut()
        .find(|map| map.name() == name)
        .unwrap_or_else(|| panic!("failed to find BPF program `{name}`"))
}

/// Find the BPF map with the given name, panic if it does not exist.
// inspired by https://github.com/libbpf/libbpf-rs/blob/18581bc5fb85bbbfd2c6ca6424a56f7b5d438470/libbpf-rs/tests/common/mod.rs#L63-L68
// TODO: this currently panics
// We can probably be a bit nicer with the error handling here.
pub fn find_map<'obj>(object: &'obj Object, name: &str) -> Map<'obj> {
    object
        .maps()
        .find(|map| map.name() == name)
        .unwrap_or_else(|| panic!("failed to find BPF map `{name}`"))
}

fn main() -> Result<(), libbpf_rs::Error> {
    let args = Args::parse();

    simple_logger::init_with_level(args.log_level).unwrap();

    let mut skel_builder = tracing::TracingSkelBuilder::default();
    skel_builder.obj_builder.debug(true);

    let mut uninit = MaybeUninit::uninit();
    // We can probably be a bit nicer with the error handling here.
    let open_skel: tracing::OpenTracingSkel = skel_builder.open(&mut uninit).unwrap();
    let skel: tracing::TracingSkel = open_skel.load()?;
    let obj = skel.object();

    let socket: Socket = Socket::new(Protocol::Pub0).unwrap();
    match socket.listen(&args.address) {
        Ok(()) => {
            log::info!("listening on {}", args.address);
        }
        Err(e) => {
            panic!("Could not listen on {}: {}", args.address, e);
        }
    }

    let mut active_tracepoints = vec![];
    let mut ringbuff_builder = RingBufferBuilder::new();

    // P2P net msgs tracepoints
    let map_net_msg_small = find_map(&obj, "net_msg_small");
    let map_net_msg_medium = find_map(&obj, "net_msg_medium");
    let map_net_msg_large = find_map(&obj, "net_msg_large");
    let map_net_msg_huge = find_map(&obj, "net_msg_huge");
    if !args.no_p2pmsg_tracepoints {
        active_tracepoints.extend(&TRACEPOINTS_NET_MESSAGE);
        #[cfg_attr(rustfmt, rustfmt_skip)]
        ringbuff_builder
            .add(&map_net_msg_small,    |data| { handle_net_message(data, socket.clone()) })?
            .add(&map_net_msg_medium,   |data| { handle_net_message(data, socket.clone()) })?
            .add(&map_net_msg_large,    |data| { handle_net_message(data, socket.clone()) })?
            .add(&map_net_msg_huge,     |data| { handle_net_message(data, socket.clone()) })?;
    }

    // P2P connection tracepoints
    let map_net_conn_inbound = find_map(&obj, "net_conn_inbound");
    let map_net_conn_outbound = find_map(&obj, "net_conn_outbound");
    let map_net_conn_closed = find_map(&obj, "net_conn_closed");
    let map_net_conn_inbound_evicted = find_map(&obj, "net_conn_inbound_evicted");
    let map_net_conn_misbehaving = find_map(&obj, "net_conn_misbehaving");
    if !args.no_connection_tracepoints {
        active_tracepoints.extend(&TRACEPOINTS_NET_CONN);
        #[cfg_attr(rustfmt, rustfmt_skip)]
        ringbuff_builder
            .add(&map_net_conn_inbound,         |data| { handle_net_conn_inbound(data, socket.clone()) })?
            .add(&map_net_conn_outbound,        |data| { handle_net_conn_outbound(data, socket.clone()) })?
            .add(&map_net_conn_closed,          |data| { handle_net_conn_closed(data, socket.clone()) })?
            .add(&map_net_conn_inbound_evicted, |data| { handle_net_conn_inbound_evicted(data, socket.clone()) })?
            .add(&map_net_conn_misbehaving,     |data| { handle_net_conn_misbehaving(data, socket.clone()) })?;
    }

    // validation tracepoints
    let map_validation_block_connected = find_map(&obj, "validation_block_connected");
    if !args.no_validation_tracepoints {
        active_tracepoints.extend(&TRACEPOINTS_VALIDATION);
        ringbuff_builder.add(&map_validation_block_connected, |data| {
            handle_validation_block_connected(data, socket.clone())
        })?;
    }

    // mempool tracepoints
    let map_mempool_added = find_map(&obj, "mempool_added");
    let map_mempool_removed = find_map(&obj, "mempool_removed");
    let map_mempool_rejected = find_map(&obj, "mempool_rejected");
    let map_mempool_replaced = find_map(&obj, "mempool_replaced");
    if !args.no_mempool_tracepoints {
        active_tracepoints.extend(&TRACEPOINTS_MEMPOOL);
        #[cfg_attr(rustfmt, rustfmt_skip)]
        ringbuff_builder
            .add(&map_mempool_added,    |data| { handle_mempool_added(data, socket.clone()) })?
            .add(&map_mempool_removed,  |data| { handle_mempool_removed(data, socket.clone()) })?
            .add(&map_mempool_rejected, |data| { handle_mempool_rejected(data, socket.clone()) })?
            .add(&map_mempool_replaced, |data| { handle_mempool_replaced(data, socket.clone()) })?;
    }

    // addrman tracepoints
    let map_addrman_insert_new = find_map(&obj, "addrman_insert_new");
    let map_addrman_insert_tried = find_map(&obj, "addrman_insert_tried");
    if args.addrman_tracepoints {
        active_tracepoints.extend(&TRACEPOINTS_ADDRMAN);
        #[cfg_attr(rustfmt, rustfmt_skip)]
        ringbuff_builder
            .add(&map_addrman_insert_new, |data| { handle_addrman_new(data, socket.clone()) })?
            .add(&map_addrman_insert_tried, |data| { handle_addrman_tried(data, socket.clone()) })?;
    }

    if active_tracepoints.is_empty() {
        log::error!("No tracepoints enabled.");
        return Ok(());
    }

    // attach tracepoints
    let mut _links = Vec::new();
    for tracepoint in active_tracepoints {
        let prog = find_prog_mut(&obj, tracepoint.function);
        _links.push(prog.attach_usdt(
            -1,
            &args.bitcoind_path,
            tracepoint.context,
            tracepoint.name,
        )?)
    }

    let ring_buffers = ringbuff_builder.build()?;
    loop {
        match ring_buffers.poll(Duration::from_millis(1)) {
            Ok(_) => (),
            Err(e) => log::warn!("Failed to poll on ring buffers {}", e),
        };
    }
}

fn handle_net_conn_closed(data: &[u8], s: Socket) -> i32 {
    let closed = ClosedConnection::from_bytes(data);
    let proto = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Closed(closed.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_outbound(data: &[u8], s: Socket) -> i32 {
    let outbound = OutboundConnection::from_bytes(data);
    let proto = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Outbound(outbound.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_inbound(data: &[u8], s: Socket) -> i32 {
    let inbound = InboundConnection::from_bytes(data);
    let proto = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Inbound(inbound.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_inbound_evicted(data: &[u8], s: Socket) -> i32 {
    let evicted = ClosedConnection::from_bytes(data);
    let proto = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::InboundEvicted(
            evicted.into(),
        )),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_misbehaving(data: &[u8], s: Socket) -> i32 {
    let misbehaving = MisbehavingConnection::from_bytes(data);
    let proto = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Misbehaving(
            misbehaving.into(),
        )),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_message(data: &[u8], s: Socket) -> i32 {
    let message = P2PMessage::from_bytes(data);
    let protobuf_message = match message.decode_to_protobuf_network_message() {
        Ok(msg) => msg.into(),
        Err(e) => {
            log::warn!("could not handle msg with size={}: {}", data.len(), e);
            return -1;
        }
    };
    let proto = EventMsg::new(Event::Msg(net_msg::Message {
        meta: message.meta.create_protobuf_metadata(),
        msg: Some(protobuf_message),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_addrman_new(data: &[u8], s: Socket) -> i32 {
    let new = AddrmanInsertNew::from_bytes(data);
    let proto = EventMsg::new(Event::Addrman(addrman::AddrmanEvent {
        event: Some(addrman::addrman_event::Event::New(new.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_addrman_tried(data: &[u8], s: Socket) -> i32 {
    let tried = AddrmanInsertTried::from_bytes(data);
    let proto = EventMsg::new(Event::Addrman(addrman::AddrmanEvent {
        event: Some(addrman::addrman_event::Event::Tried(tried.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_added(data: &[u8], s: Socket) -> i32 {
    let added = MempoolAdded::from_bytes(data);
    let proto = EventMsg::new(Event::Mempool(mempool::MempoolEvent {
        event: Some(mempool::mempool_event::Event::Added(added.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_removed(data: &[u8], s: Socket) -> i32 {
    let removed = MempoolRemoved::from_bytes(data);
    let proto = EventMsg::new(Event::Mempool(mempool::MempoolEvent {
        event: Some(mempool::mempool_event::Event::Removed(removed.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_replaced(data: &[u8], s: Socket) -> i32 {
    let replaced = MempoolReplaced::from_bytes(data);
    let proto = EventMsg::new(Event::Mempool(mempool::MempoolEvent {
        event: Some(mempool::mempool_event::Event::Replaced(replaced.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_mempool_rejected(data: &[u8], s: Socket) -> i32 {
    let rejected = MempoolRejected::from_bytes(data);
    let proto = EventMsg::new(Event::Mempool(mempool::MempoolEvent {
        event: Some(mempool::mempool_event::Event::Rejected(rejected.into())),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_validation_block_connected(data: &[u8], s: Socket) -> i32 {
    let connected = ValidationBlockConnected::from_bytes(data);
    let proto = EventMsg::new(Event::Validation(validation::ValidationEvent {
        event: Some(validation::validation_event::Event::BlockConnected(
            connected.into(),
        )),
    }));
    s.send(&proto.encode_to_vec()).unwrap();
    0
}
