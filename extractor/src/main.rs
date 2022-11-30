#![cfg_attr(feature = "strict", deny(warnings))]

use std::env;
use std::time::Duration;
use std::time::SystemTime;

use libc;

use libbpf_rs::RingBufferBuilder;

use nng::{Protocol, Socket};

use prost::Message;

use shared::bitcoin::network::message::NetworkMessage;
use shared::ctypes::{
    ClosedConnection, HugeP2PMessage, InboundConnection, LargeP2PMessage, MediumP2PMessage,
    MisbehavingConnection, OutboundConnection, P2PMessageMetadata, P2PMessageSize,
    RustBitcoinNetworkMessage, SmallP2PMessage,
};
use shared::net_conn;
use shared::net_msg;
use shared::wrapper::wrapper::Wrap;
use shared::wrapper::Wrapper;

fn bump_memlock_rlimit() {
    let rlimit = libc::rlimit {
        rlim_cur: 128 << 20,
        rlim_max: 128 << 20,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        panic!("Failed to increase rlimit");
    }
}

#[path = "tracing.gen.rs"]
mod tracing;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn hook_usdt(
    tracing_fn: &mut libbpf_rs::Program,
    pid: i32,
    path: &str,
    context: &str,
    name: &str,
) -> Result<libbpf_rs::Link, libbpf_rs::Error> {
    match tracing_fn.attach_usdt(pid, &path, context, name) {
        Ok(link) => Ok(link),
        Err(e) => {
            println!(
                "Could not attach to USDT tracepoint {}:{} in '{}'",
                context, name, path
            );
            Err(e)
        }
    }
}

fn attach_usdt_tracepoints(
    fns: &mut tracing::TracingProgsMut,
    pid: i32,
    path: &str,
) -> Result<Vec<libbpf_rs::Link>, libbpf_rs::Error> {
    let mut links = Vec::new();
    // for readability of fn to -> context:name mappings, don't rustfmt this
    #[cfg_attr(rustfmt, rustfmt_skip)]
    {
        // net msg
        links.push(hook_usdt(fns.handle_net_msg_inbound(),      pid, path, "net", "inbound_message")?);
        links.push(hook_usdt(fns.handle_net_msg_outbound(),     pid, path, "net", "outbound_message")?);
        // net conn
        links.push(hook_usdt(fns.handle_net_conn_inbound(),     pid, path, "net", "inbound_connection")?);
        links.push(hook_usdt(fns.handle_net_conn_outbound(),    pid, path, "net", "outbound_connection")?);
        links.push(hook_usdt(fns.handle_net_conn_closed(),      pid, path, "net", "closed_connection")?);
        links.push(hook_usdt(fns.handle_net_conn_evicted(),     pid, path, "net", "evicted_connection")?);
        links.push(hook_usdt(fns.handle_net_conn_misbehaving(), pid, path, "net", "misbehaving_connection")?);
    }
    Ok(links)
}

fn main() -> Result<(), libbpf_rs::Error> {
    let bitcoind_path = env::args().nth(1).expect("No bitcoind path provided.");

    let mut skel_builder = tracing::TracingSkelBuilder::default();
    skel_builder.obj_builder.debug(true);

    bump_memlock_rlimit();

    let open_skel = skel_builder.open().unwrap();
    let mut skel = match open_skel.load() {
        Ok(skel) => skel,
        Err(e) => {
            panic!("Could not load skeleton file: {}", e);
        }
    };

    let _links = attach_usdt_tracepoints(&mut skel.progs_mut(), -1, &bitcoind_path)
        .expect("Could not attach to USDT tracepoints");

    let socket: Socket = Socket::new(Protocol::Pub0).unwrap();
    socket.listen(ADDRESS).unwrap();
    println!("listening on {}", ADDRESS);

    let maps = skel.maps();
    let mut ringbuff_builder = RingBufferBuilder::new();

    #[cfg_attr(rustfmt, rustfmt_skip)]
    ringbuff_builder
        .add(maps.net_msg_small(), |data| { handle_net_message(data, P2PMessageSize::Small, socket.clone()) })?
        .add(maps.net_msg_medium(), |data| { handle_net_message(data, P2PMessageSize::Medium, socket.clone()) })?
        .add(maps.net_msg_large(), |data| { handle_net_message(data, P2PMessageSize::Large, socket.clone()) })?
        .add(maps.net_msg_huge(), |data| { handle_net_message(data, P2PMessageSize::Huge, socket.clone()) })?
        .add(maps.net_conn_inbound(), |data| { handle_net_conn_inbound(data, socket.clone()) })?
        .add(maps.net_conn_outbound(), |data| { handle_net_conn_outbound(data, socket.clone()) })?
        .add(maps.net_conn_closed(), |data| { handle_net_conn_closed(data, socket.clone()) })?
        .add(maps.net_conn_evicted(), |data| { handle_net_conn_evicted(data, socket.clone()) })?
        .add(maps.net_conn_misbehaving(), |data| { handle_net_conn_misbehaving(data, socket.clone()) })?;
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
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Conn(net_conn::ConnectionEvent {
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
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Conn(net_conn::ConnectionEvent {
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
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Inbound(inbound.into())),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_conn_evicted(data: &[u8], s: Socket) -> i32 {
    let evicted = ClosedConnection::from_bytes(data);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Evicted(evicted.into())),
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
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Misbehaving(
                misbehaving.into(),
            )),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}

fn handle_net_message(data: &[u8], size: P2PMessageSize, s: Socket) -> i32 {
    let metadata: P2PMessageMetadata;
    let network_msg: NetworkMessage;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    match size {
        P2PMessageSize::Small => {
            let msg = SmallP2PMessage::from_bytes(data);
            metadata = msg.meta.clone();
            network_msg = match msg.rust_bitcoin_network_message() {
                Ok(msg) => msg,
                Err(e) => {
                    // TODO: warn
                    println!("could not handle small msg: {}", e);
                    return -1;
                }
            }
        }
        P2PMessageSize::Medium => {
            let msg = MediumP2PMessage::from_bytes(data);
            metadata = msg.meta.clone();
            network_msg = match msg.rust_bitcoin_network_message() {
                Ok(msg) => msg,
                Err(e) => {
                    // TODO: warn
                    println!("could not handle medium msg: {}", e);
                    return -1;
                }
            }
        }
        P2PMessageSize::Large => {
            let msg = LargeP2PMessage::from_bytes(data);
            metadata = msg.meta.clone();
            network_msg = match msg.rust_bitcoin_network_message() {
                Ok(msg) => msg,
                Err(e) => {
                    // TODO: warn
                    println!("could not handle large msg: {}", e);
                    return -1;
                }
            }
        }
        P2PMessageSize::Huge => {
            let msg = HugeP2PMessage::from_bytes(data);
            metadata = msg.meta.clone();
            network_msg = match msg.rust_bitcoin_network_message() {
                Ok(msg) => msg,
                Err(e) => {
                    // TODO: warn
                    println!("could not handle huge msg: {}", e);
                    return -1;
                }
            }
        }
    };
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Msg(net_msg::Message {
            meta: metadata.create_protobuf_metadata(),
            msg: Some((&network_msg).into()),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}
