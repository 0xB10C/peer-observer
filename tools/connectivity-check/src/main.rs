#![cfg_attr(feature = "strict", deny(warnings))]

use crossbeam;
use crossbeam::channel::{unbounded, Receiver, Sender};
use rand::Rng;
use shared::bitcoin::consensus::{encode, Decodable};
use shared::bitcoin::p2p::message::NetworkMessage;
use shared::bitcoin::p2p::message_network::VersionMessage;
use shared::bitcoin::p2p::{address, message, message_network, ServiceFlags};
use shared::bitcoin::Network;
use shared::clap;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::net_msg::message::Msg;
use shared::net_msg::Message as NetMessage;
use shared::nng::options::protocol::pubsub::Subscribe;
use shared::nng::options::Options;
use shared::nng::{Protocol, Socket};
use shared::primitive::address::Address as AddressType;
use shared::primitive::Address;
use shared::prost::Message as ProstMessage;
use shared::simple_logger;
use shared::util;
use std::collections::HashMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod metrics;
mod metricserver;

const WORKERS: usize = 50;

const NETWORK: Network = Network::Bitcoin;
const USER_AGENT: &str = "/bitnodes.io:0.3/";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const RECENT_CONNECTION_DURATION: Duration = Duration::from_secs(60 * 60);

/// Simple peer-observer tool that checks the connectivity of received addr message entries
/// and offers stats as prometheus metrics
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // The extractor address the tool should connect to.
    #[arg(short, long, default_value = "tcp://127.0.0.1:8883")]
    address: String,
    // The metrics server address the tool should listen on.
    #[arg(short, long, default_value = "127.0.0.1:18282")]
    metrics_address: String,

    // The log level the tool should run with.
    // Valid log levels are "trace", "debug", "info", "warn",
    // "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

#[derive(Clone, Debug)]
enum AddrMessageVersion {
    Addr,
    Addrv2,
}

#[derive(Clone, Debug)]
enum NetworkType {
    IPv4,
    IPv6,
}

impl fmt::Display for AddrMessageVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            AddrMessageVersion::Addr => write!(f, "addr"),
            AddrMessageVersion::Addrv2 => write!(f, "addrv2"),
        }
    }
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            NetworkType::IPv4 => write!(f, "IPv4"),
            NetworkType::IPv6 => write!(f, "IPv6"),
        }
    }
}

#[derive(Debug, Clone)]
struct Input {
    pub address: Address,
    pub source_id: u64,
    pub timestamp: u64,
    pub source_ip: String,
    pub version: AddrMessageVersion,
}

#[derive(Debug)]
struct Output {
    pub input: Input,
    pub result: bool,
    pub cached: bool,
    pub network: NetworkType,
    pub version: Option<VersionMessage>,
}

#[derive(serde::Serialize)]
struct Row {
    result_timestamp: u64,
    addr_address: String,
    addr_port: u32,
    addr_services: u64,
    addr_timestamp: u64,
    addr_network_type: String,
    addr_version: String,
    source_address: String,
    source_id: u64,
    source_tor_exit_node: bool,
    result_success: bool,
    result_cached: bool,
    version: bool,
    version_useragent: String,
    version_relay: bool,
    version_version: u32,
    version_services: u64,
    version_start_height: i32,
    version_nonce: u64,
}

fn worker(
    output_sender: &Sender<Output>,
    input_receiver: &Receiver<Input>,
    recent_succesful_connections_cache: Arc<Mutex<HashMap<String, Instant>>>,
) {
    for input in input_receiver.iter() {
        let address = input.clone().address.address.unwrap();
        let key = format!(
            "{}--{}",
            input.clone().address.address.unwrap(),
            input.clone().address.port
        );
        let recent_succesful_connection = match recent_succesful_connections_cache
            .lock()
            .expect("could not lock cache for lookup")
            .get(&key)
        {
            Some(last_succesful_connection_time) => {
                last_succesful_connection_time.elapsed() < RECENT_CONNECTION_DURATION
            }
            None => false,
        };

        let network_type_opt: Option<NetworkType> = match address.clone() {
            AddressType::Ipv4(_) => Some(NetworkType::IPv4),
            AddressType::Ipv6(_) => Some(NetworkType::IPv6),
            _ => None, // TODO: only IPv4 and IPv6 supported for now
        };
        if let Some(network_type) = network_type_opt {
            let ip_addr_opt: Option<IpAddr> = match address.clone() {
                AddressType::Ipv4(ipv4) => match ipv4.parse() {
                    Ok(ipv4_addr) => Some(IpAddr::V4(ipv4_addr)),
                    Err(_) => None,
                },
                AddressType::Ipv6(ipv6) => match ipv6.parse() {
                    Ok(ipv6_addr) => Some(IpAddr::V6(ipv6_addr)),
                    Err(_) => None,
                },
                _ => None,
            };

            if let Some(ip_addr) = ip_addr_opt {
                let mut version: Option<VersionMessage> = None;
                let result: bool = match recent_succesful_connection {
                    true => true,
                    false => {
                        version = try_connect(SocketAddr::new(ip_addr, input.address.port as u16));
                        if version.is_some() {
                            recent_succesful_connections_cache
                                .lock()
                                .expect("could not lock cache for insert")
                                .insert(key, Instant::now());
                            true
                        } else {
                            false
                        }
                    }
                };

                output_sender
                    .send(Output {
                        input,
                        result,
                        cached: recent_succesful_connection,
                        network: network_type,
                        version,
                    })
                    .unwrap();
            }
        }
    }
}

fn handle_event(event: Event, timestamp: u64, input_sender: Sender<Input>) {
    match event {
        Event::Msg(msg) => {
            if msg.meta.inbound {
                handle_inbound_message(msg, timestamp, input_sender);
            }
        }
        _ => (),
    }
}

fn handle_inbound_message(msg: NetMessage, timestamp: u64, input_sender: Sender<Input>) {
    if let Some(inbound_msg) = msg.msg {
        match inbound_msg {
            Msg::Addr(addr) => {
                if addr.addresses.len() == 1000 {
                    log::info!("Received an addr message with 1000 addresses from {}. Likely a getaddr response. Ignoring.", msg.meta.addr.clone());
                    return;
                }

                for addr in addr.addresses {
                    let input = Input {
                        address: addr,
                        timestamp,
                        source_id: msg.meta.peer_id,
                        source_ip: msg.meta.addr.clone(),
                        version: AddrMessageVersion::Addr,
                    };
                    input_sender.send(input).unwrap();
                }
            }
            Msg::Addrv2(addrv2) => {
                if addrv2.addresses.len() == 1000 {
                    log::info!("Received an addrv2 message with 1000 addresses from {}. Likely a getaddr response. Ignoring.", msg.meta.addr.clone());
                    return;
                }

                for addr in addrv2.addresses {
                    let input = Input {
                        address: addr,
                        timestamp,
                        source_id: msg.meta.peer_id,
                        source_ip: msg.meta.addr.clone(),
                        version: AddrMessageVersion::Addrv2,
                    };
                    input_sender.send(input).unwrap();
                }
            }
            _ => (),
        }
    }
}

fn build_raw_network_message(payload: message::NetworkMessage) -> message::RawNetworkMessage {
    message::RawNetworkMessage::new(NETWORK.magic(), payload)
}

fn build_version_message() -> message::NetworkMessage {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time error")
        .as_secs();

    message::NetworkMessage::Version(message_network::VersionMessage::new(
        ServiceFlags::NONE,
        timestamp as i64,
        address::Address::new(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            ServiceFlags::NONE,
        ), // addr from
        address::Address::new(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            ServiceFlags::NONE,
        ), // addr from
        rand::thread_rng().gen(), // nonce
        String::from(USER_AGENT), // user-agent
        0,                        // start height
    ))
}

fn try_connect(address: SocketAddr) -> Option<VersionMessage> {
    if let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
        let _ = stream.write_all(
            encode::serialize(&build_raw_network_message(build_version_message())).as_slice(),
        );

        stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();

        if let Ok(read_stream) = stream.try_clone() {
            let mut stream_reader = BufReader::new(read_stream);
            if let Ok(msg) = message::RawNetworkMessage::consensus_decode(&mut stream_reader) {
                log::debug!("msg: {:?}", msg);
                match msg.payload() {
                    NetworkMessage::Version(version) => {
                        let _ = stream.shutdown(Shutdown::Both);
                        return Some(version.clone());
                    }
                    _ => {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                }
            }
        }
    }
    return None;
}

// TODO:
// - general clean up
// - error handling

fn main() {
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level).unwrap();

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(&args.address).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let (input_sender, input_receiver) = unbounded();
    let (output_sender, output_receiver) = unbounded();

    metricserver::start(&args.metrics_address).unwrap();
    log::info!("metrics-server started on {}", &args.metrics_address);

    crossbeam::scope(|s| {
        s.spawn(|_| loop {
            let msg = sub.recv().unwrap();
            let wrapped = event_msg::EventMsg::decode(msg.as_slice()).unwrap();
            let unwrapped = wrapped.event;
            if let Some(event) = unwrapped {
                handle_event(event, wrapped.timestamp, input_sender.clone());
            }
        });

        let recent_succesful_connections_cache: Arc<Mutex<HashMap<String, Instant>>> =
            Arc::new(Mutex::new(HashMap::new()));
        for _ in 0..WORKERS {
            let (sender, receiver) = (output_sender.clone(), input_receiver.clone());
            let cache = recent_succesful_connections_cache.clone();
            s.spawn(move |_| worker(&sender, &receiver, cache));
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("addr-connectivity.csv")
            .unwrap();
        let mut wtr = csv::Writer::from_writer(file);

        for output in output_receiver.iter() {
            log::debug!("Sink received {:?}", output);

            let network = output.network.to_string();
            let version = output.input.version.to_string();
            let source_ip = util::ip_from_ipport(output.input.source_ip);

            metrics::ADDR_TRIED
                .with_label_values(&[&network, &version])
                .inc();

            if output.result {
                metrics::ADDR_SUCCESSFUL_CONNECTION
                    .with_label_values(&[&network, &version, &source_ip])
                    .inc();
            } else {
                metrics::ADDR_UNSUCCESSFUL_CONNECTION
                    .with_label_values(&[&network, &version, &source_ip])
                    .inc();
            }
            if output.cached {
                metrics::ADDR_CACHED
                    .with_label_values(&[&network, &version])
                    .inc();
            }

            // We substract the timestamp in the address from the time we received the
            // message. If the remaining offset is larger than or equal to zero, the address
            // timestamp lies in the past. If the offset is smaller than zero, the address
            // timestamp lies in the future.
            let offset = output.input.timestamp as i64 - output.input.address.timestamp as i64;
            let offset_direction = if offset >= 0 { "past" } else { "future" };
            let successful = if output.result { "yes" } else { "no" };

            metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                .with_label_values(&[&network, &version, &offset_direction, &successful])
                .observe(offset.abs() as f64);

            if util::is_tor_exit_node(&source_ip.clone()) {
                metrics::ADDR_TRIED_FROM_TOR_EXIT
                    .with_label_values(&[&network, &version])
                    .inc();

                if output.result {
                    metrics::ADDR_SUCCESSFUL_CONNECTION_TOR_EXIT
                        .with_label_values(&[&network, &version])
                        .inc();
                }
            }

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time error")
                .as_secs();

            let version_msg = output.version;

            wtr.serialize(Row {
                result_timestamp: timestamp,
                addr_address: output
                    .input
                    .address
                    .address
                    .expect("should be some address")
                    .to_string(),
                addr_port: output.input.address.port,
                addr_services: output.input.address.services,
                addr_timestamp: output.input.timestamp,
                addr_network_type: network,
                addr_version: version,
                source_address: source_ip.clone(),
                source_id: output.input.source_id,
                source_tor_exit_node: util::is_tor_exit_node(&source_ip),
                result_success: output.result,
                result_cached: output.cached,
                version: version_msg.is_some(),
                version_useragent: version_msg
                    .as_ref()
                    .map_or(String::default(), |v| v.user_agent.clone()),
                version_relay: version_msg.as_ref().map_or(false, |v| v.relay),
                version_version: version_msg.as_ref().map_or(0, |v| v.version),
                version_services: version_msg.as_ref().map_or(0, |v| v.services.to_u64()),
                version_start_height: version_msg.as_ref().map_or(-1, |v| v.start_height),
                version_nonce: version_msg.as_ref().map_or(0, |v| v.nonce),
            })
            .unwrap();
        }
    })
    .unwrap();
}
