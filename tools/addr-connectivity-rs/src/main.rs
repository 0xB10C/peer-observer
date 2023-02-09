#![cfg_attr(feature = "strict", deny(warnings))]

use std::fmt;
use std::io::{BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shared::bitcoin::consensus::{encode, Decodable};
use shared::bitcoin::network::{address, constants, message, message_network};
use shared::net_msg::message::Msg;
use shared::net_msg::Message as NetMessage;
use shared::primitive::address::Address as AddressType;
use shared::primitive::Address;
use shared::wrapper;
use shared::wrapper::wrapper::Wrap;

use crossbeam;
use crossbeam::channel::{unbounded, Receiver, Sender};
use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use prost::Message as ProstMessage;
use rand::Rng;

mod metrics;
mod metricserver;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";
const METRICS_ADDRESS: &'static str = "127.0.0.1:36437";
const WORKERS: usize = 50;

const NETWORK: constants::Network = constants::Network::Bitcoin;
const USER_AGENT: &str = "/bitnodes.io:0.3/";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(5);

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
    pub _source_id: u64,
    pub source_ip: String,
    pub version: AddrMessageVersion,
}

#[derive(Debug)]
struct Output {
    pub input: Input,
    pub result: bool,
    pub network: NetworkType,
}

fn worker(output_sender: &Sender<Output>, input_receiver: &Receiver<Input>) {
    for input in input_receiver.iter() {
        match input.clone().address.address.unwrap() {
            AddressType::Ipv4(ipv4) => {
                if let Ok(ipv4_addr) = ipv4.parse() {
                    let socket = SocketAddr::new(IpAddr::V4(ipv4_addr), input.address.port as u16);
                    let output = Output {
                        input,
                        result: try_connect(socket),
                        network: NetworkType::IPv4,
                    };
                    output_sender.send(output).unwrap();
                }
            }
            AddressType::Ipv6(ipv6) => {
                if let Ok(ipv6_addr) = ipv6.parse() {
                    let socket = SocketAddr::new(IpAddr::V6(ipv6_addr), input.address.port as u16);
                    let output = Output {
                        input,
                        result: try_connect(socket),
                        network: NetworkType::IPv6,
                    };
                    output_sender.send(output).unwrap();
                }
            }
            _ => (),
        };
    }
}

fn handle_event(event: Wrap, input_sender: Sender<Input>) {
    match event {
        Wrap::Msg(msg) => {
            if msg.meta.inbound {
                handle_inbound_message(msg, input_sender);
            }
        }
        _ => (),
    }
}

fn handle_inbound_message(msg: NetMessage, input_sender: Sender<Input>) {
    if let Some(inbound_msg) = msg.msg {
        match inbound_msg {
            Msg::Addr(addr) => {
                if addr.addresses.len() == 1000 {
                    println!("Received an addr message with 1000 addresses from {}. Likely a getaddr response. Ignoring.", msg.meta.addr.clone());
                    return;
                }

                for addr in addr.addresses {
                    let input = Input {
                        address: addr,
                        _source_id: msg.meta.peer_id,
                        source_ip: msg.meta.addr.clone(),
                        version: AddrMessageVersion::Addr,
                    };
                    input_sender.send(input).unwrap();
                }
            }
            Msg::Addrv2(addrv2) => {
                if addrv2.addresses.len() == 1000 {
                    println!("Received an addrv2 message with 1000 addresses from {}. Likely a getaddr response. Ignoring.", msg.meta.addr.clone());
                    return;
                }
                for addr in addrv2.addresses {
                    let input = Input {
                        address: addr,
                        _source_id: msg.meta.peer_id,
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
    message::RawNetworkMessage {
        magic: NETWORK.magic(),
        payload: payload,
    }
}

fn build_version_message() -> message::NetworkMessage {
    // "standard UNIX timestamp in seconds"
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time error")
        .as_secs();

    // Construct the message
    message::NetworkMessage::Version(message_network::VersionMessage::new(
        constants::ServiceFlags::NONE,
        timestamp as i64,
        address::Address::new(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            constants::ServiceFlags::NONE,
        ), // addr from
        address::Address::new(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            constants::ServiceFlags::NONE,
        ), // addr from
        rand::thread_rng().gen(), // nonce
        String::from(USER_AGENT), // user-agent
        0,                        // start height
    ))
}

fn try_connect(address: SocketAddr) -> bool {
    if let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
        let _ = stream.write_all(
            encode::serialize(&build_raw_network_message(build_version_message())).as_slice(),
        );

        stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();

        if let Ok(read_stream) = stream.try_clone() {
            let mut stream_reader = BufReader::new(read_stream);
            if let Ok(_) = message::RawNetworkMessage::consensus_decode(&mut stream_reader) {
                let _ = stream.shutdown(Shutdown::Both);
                return true;
            }
        }
    }
    return false;
}

/// Split and return the IP from an ip:port combination.
pub fn ip(addr: String) -> String {
    match addr.rsplit_once(":") {
        Some((ip, _)) => ip.replace("[", "").replace("]", "").to_string(),
        None => addr,
    }
}

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let (input_sender, input_receiver) = unbounded();
    let (output_sender, output_receiver) = unbounded();

    metricserver::start(&METRICS_ADDRESS).unwrap();

    crossbeam::scope(|s| {
        s.spawn(|_| loop {
            let msg = sub.recv().unwrap();
            let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap().wrap;
            if let Some(event) = unwrapped {
                handle_event(event, input_sender.clone());
            }
        });

        for _ in 0..WORKERS {
            let (sender, receiver) = (output_sender.clone(), input_receiver.clone());
            s.spawn(move |_| worker(&sender, &receiver));
        }

        for output in output_receiver.iter() {
            println!("Sink received {:?}", output);

            let network = output.network.to_string();
            let version = output.input.version.to_string();
            let ip = ip(output.input.source_ip);

            metrics::ADDR_TRIED
                .with_label_values(&[&network, &version])
                .inc();

            if output.result {
                metrics::ADDR_SUCCESSFUL_CONNECTION
                    .with_label_values(&[&network, &version, &ip])
                    .inc();
            } else {
                metrics::ADDR_UNSUCCESSFUL_CONNECTION
                    .with_label_values(&[&network, &version, &ip])
                    .inc();
            }
        }
    })
    .unwrap();
}
