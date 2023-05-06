#![cfg_attr(feature = "strict", deny(warnings))]

use std::env;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use libc;

use libbpf_rs::RingBufferBuilder;
use libbpf_rs::UprobeOpts;

use nng::{Protocol, Socket};

use prost::Message;

use shared::ctypes::{
    ClosedConnection, InboundConnection, MisbehavingConnection, OutboundConnection, P2PMessage,
    P2PMessageSize,
};
use shared::fn_timings::function_timings::Function;
use shared::fn_timings::FunctionTimings;
use shared::net_conn;
use shared::net_msg;
use shared::wrapper::wrapper::Wrap;
use shared::wrapper::Wrapper;

// readelf --syms --wide ./src/bitcoind
const UPROBE_SYMBOL_SENDMESSAGES: &str = "_ZN12_GLOBAL__N_115PeerManagerImpl12SendMessagesEP5CNode";
const UPROBE_SYMBOL_PROCESSMESSAGES: &str =
    "_Z18AcceptToMemoryPoolR10ChainstateRKSt10shared_ptrIK12CTransactionElbb";
const UPROBE_SYMBOL_ATMP: &str =
    "_Z18AcceptToMemoryPoolR10ChainstateRKSt10shared_ptrIK12CTransactionElbb";

const MAX_FNTIME_BUFFER_AGE: Duration = Duration::from_secs(3);
const MAX_FMTIME_BUFFER_SIZE: usize = 200;

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

fn hook_uprobe(
    tracing_fn: &mut libbpf_rs::Program,
    pid: i32,
    path: &str,
    retprobe: bool,
    func_offset: usize,
    func_name: String,
) -> Result<libbpf_rs::Link, libbpf_rs::Error> {
    match tracing_fn.attach_uprobe_with_opts(
        pid,
        &path,
        func_offset,
        UprobeOpts {
            retprobe,
            func_name: func_name.clone(),
            ..Default::default()
        },
    ) {
        Ok(link) => Ok(link),
        Err(e) => {
            println!(
                "Could not attach uprobe (ret: {}) to function {} in '{}'",
                retprobe, func_name, path
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

    #[cfg_attr(rustfmt, rustfmt_skip)]
    {
        // SendMessages uprobe and uretprobe
        if let Ok(link_uprobe) = hook_uprobe(fns.uprobe_netprocessing_sendmessages(), pid, path, false, 0, UPROBE_SYMBOL_SENDMESSAGES.to_string()) {
            if let Ok(link_uretprobe) = hook_uprobe(fns.uretprobe_netprocessing_sendmessages(), pid, path, true, 0, UPROBE_SYMBOL_SENDMESSAGES.to_string()) {
                links.push(link_uprobe);
                links.push(link_uretprobe);
                println!("Attached u(ret)probes to {}", UPROBE_SYMBOL_SENDMESSAGES);
            }
        }

        // ProcessMessages uprobe and uretprobe
        if let Ok(link_uprobe) = hook_uprobe(fns.uprobe_netprocessing_processmessages(), pid, path, false, 0, UPROBE_SYMBOL_PROCESSMESSAGES.to_string()) {
            if let Ok(link_uretprobe) = hook_uprobe(fns.uretprobe_netprocessing_processmessages(), pid, path, true, 0, UPROBE_SYMBOL_PROCESSMESSAGES.to_string()) {
                links.push(link_uprobe);
                links.push(link_uretprobe);
                println!("Attached u(ret)probes to {}", UPROBE_SYMBOL_PROCESSMESSAGES);
            }
        }

        // ATMP uprobe and uretprobe
        if let Ok(link_uprobe) = hook_uprobe(fns.uprobe_validation_atmp(), pid, path, false, 0, UPROBE_SYMBOL_ATMP.to_string()) {
            if let Ok(link_uretprobe) = hook_uprobe(fns.uprobe_validation_atmp(), pid, path, true, 0, UPROBE_SYMBOL_ATMP.to_string()) {
                links.push(link_uprobe);
                links.push(link_uretprobe);
                println!("Attached u(ret)probes to {}", UPROBE_SYMBOL_ATMP);
            }
        }
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

    let (chan_timing_sendmessages_send, chan_timing_sendmessages_recv) = channel();
    let (chan_timing_processmessages_send, chan_timing_processmessages_recv) = channel();

    handle_batched_fntime_netprocessing_processmessages(
        socket.clone(),
        chan_timing_processmessages_recv,
    );
    handle_batched_fntime_netprocessing_sendmessages(socket.clone(), chan_timing_sendmessages_recv);

    let maps = skel.maps();
    let mut ringbuff_builder = RingBufferBuilder::new();

    #[cfg_attr(rustfmt, rustfmt_skip)]
    ringbuff_builder
        .add(maps.net_msg_small(), |data| { handle_net_message::<{ P2PMessageSize::Small as usize }>(data, socket.clone()) })?
        .add(maps.net_msg_medium(), |data| { handle_net_message::<{ P2PMessageSize::Medium as usize }>(data, socket.clone()) })?
        .add(maps.net_msg_large(), |data| { handle_net_message::<{ P2PMessageSize::Large as usize }>(data, socket.clone()) })?
        .add(maps.net_msg_huge(), |data| { handle_net_message::<{ P2PMessageSize::Huge as usize }>(data, socket.clone()) })?
        .add(maps.net_conn_inbound(), |data| { handle_net_conn_inbound(data, socket.clone()) })?
        .add(maps.net_conn_outbound(), |data| { handle_net_conn_outbound(data, socket.clone()) })?
        .add(maps.net_conn_closed(), |data| { handle_net_conn_closed(data, socket.clone()) })?
        .add(maps.net_conn_evicted(), |data| { handle_net_conn_evicted(data, socket.clone()) })?
        .add(maps.net_conn_misbehaving(), |data| { handle_net_conn_misbehaving(data, socket.clone()) })?
        .add(maps.fntime_validation_atmp(), |data| { handle_fntime_validation_atmp(data, socket.clone()) })?
        .add(maps.fntime_netprocessing_sendmessages(), |data| { handle_fntime_netprocessing_messages(data, chan_timing_sendmessages_send.clone()) })?
        .add(maps.fntime_netprocessing_processmessages(), |data| { handle_fntime_netprocessing_messages(data, chan_timing_processmessages_send.clone()) })?;
    let ring_buffers = ringbuff_builder.build()?;

    loop {
        match ring_buffers.poll(Duration::from_millis(1)) {
            Ok(_) => (),
            Err(e) => println!("Failed to poll on ring buffers {}", e),
        };
    }
}

fn construct_wrapped_proto_message(wrap: Wrap) -> Wrapper {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(wrap),
    }
}

fn handle_batched_fntime_netprocessing_sendmessages(s: Socket, receiver: mpsc::Receiver<u64>) {
    thread::spawn(move || {
        let mut last_send = Instant::now();
        let mut buffer_send_messages: Vec<u64> = Vec::new();
        loop {
            let timing = receiver.recv().unwrap();
            buffer_send_messages.push(timing.clone());
            if buffer_send_messages.len() >= MAX_FMTIME_BUFFER_SIZE
                || last_send.elapsed() > MAX_FNTIME_BUFFER_AGE
            {
                let wrap = Wrap::Fntime(FunctionTimings {
                    function: Some(Function::Sendmsgs(
                        shared::fn_timings::NetProcessingSendMessages {
                            times: buffer_send_messages.clone(),
                        },
                    )),
                });
                let protobuf_msg = construct_wrapped_proto_message(wrap);
                s.send(&protobuf_msg.encode_to_vec()).unwrap();
                buffer_send_messages.clear();
                last_send = Instant::now();
            }
        }
    });
}

fn handle_batched_fntime_netprocessing_processmessages(s: Socket, receiver: mpsc::Receiver<u64>) {
    thread::spawn(move || {
        let mut buffer_process_messages: Vec<u64> = Vec::new();
        let mut last_send = Instant::now();
        loop {
            let timing = receiver.recv().unwrap();
            buffer_process_messages.push(timing.clone());
            if buffer_process_messages.len() >= MAX_FMTIME_BUFFER_SIZE
                || last_send.elapsed() > MAX_FNTIME_BUFFER_AGE
            {
                let wrap = Wrap::Fntime(FunctionTimings {
                    function: Some(Function::Processmsgs(
                        shared::fn_timings::NetProcessingProcessMessages {
                            times: buffer_process_messages.clone(),
                        },
                    )),
                });
                let protobuf_msg = construct_wrapped_proto_message(wrap);
                s.send(&protobuf_msg.encode_to_vec()).unwrap();
                buffer_process_messages.clear();
                last_send = Instant::now();
            }
        }
    });
}

fn handle_fntime_netprocessing_messages(data: &[u8], tx: mpsc::Sender<u64>) -> i32 {
    let delta = u64::from_le_bytes(data.try_into().expect("slice with incorrect length"));
    tx.send(delta / 1000).unwrap(); // as µs
    0
}

fn handle_fntime_validation_atmp(data: &[u8], s: Socket) -> i32 {
    let delta = u64::from_le_bytes(data.try_into().expect("slice with incorrect length"));
    let wrap = Wrap::Fntime(FunctionTimings {
        function: Some(Function::Atmp(shared::fn_timings::ValidationAtmp {
            times: vec![delta / 1000].into(), // as µs
        })),
    });
    let protobuf_msg = construct_wrapped_proto_message(wrap);
    s.send(&protobuf_msg.encode_to_vec()).unwrap();
    0
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

fn handle_net_message<const SIZE: usize>(data: &[u8], s: Socket) -> i32 {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let timestamp = now.as_secs();
    let timestamp_subsec_millis = now.subsec_micros();
    let message = P2PMessage::<SIZE>::from_bytes(data);
    let protobuf_message = match message.decode_to_protobuf_network_message() {
        Ok(msg) => msg.into(),
        Err(e) => {
            // TODO: warn
            println!("could not handle msg with size={}: {}", SIZE, e);
            return -1;
        }
    };
    let proto = Wrapper {
        timestamp: timestamp,
        timestamp_subsec_micros: timestamp_subsec_millis,
        wrap: Some(Wrap::Msg(net_msg::Message {
            meta: message.meta.create_protobuf_metadata(),
            msg: Some(protobuf_message),
        })),
    };
    s.send(&proto.encode_to_vec()).unwrap();
    0
}
