#![cfg_attr(feature = "strict", deny(warnings))]

use std::io::{BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shared::bitcoin;
use shared::bitcoin::consensus::{encode, Decodable};
use shared::bitcoin::network::message::NetworkMessage;
use shared::bitcoin::network::message_network::VersionMessage;
use shared::bitcoin::network::{address, constants, message, message_network};
use shared::net_msg::message::Msg;
use shared::net_msg::Message as NetMessage;
use shared::primitive::address::Address::Ipv4;
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

const NETWORK: constants::Network = constants::Network::Bitcoin;
const USER_AGENT: &str = "/bitnodes.io:0.3/";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct Input {
    pub address: Address,
    pub peer_id: u64,
    pub source: String,
    pub addrv2: bool,
}

#[derive(Debug)]
struct Output {
    pub input: Input,
    pub result: bool,
}

fn worker(output_sender: &Sender<Output>, input_receiver: &Receiver<Input>) {
    for input in input_receiver.iter() {
        let mut shaked_hands: bool = false;

        match input.clone().address.address.unwrap() {
            Ipv4(ipv4) => {
                if let Ok(ipv4_addr) = ipv4.parse() {
                    let socket = SocketAddr::new(IpAddr::V4(ipv4_addr), input.address.port as u16);
                    let shaked_hands = handshake(socket);
                    let output = Output {
                        input,
                        result: shaked_hands,
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
                for addr in addr.addresses {
                    let input = Input {
                        address: addr,
                        peer_id: msg.meta.peer_id,
                        source: msg.meta.addr.clone(),
                        addrv2: false,
                    };
                    input_sender.send(input).unwrap();
                }
            }
            Msg::Addrv2(addrv2) => {
                for addr in addrv2.addresses {
                    let input = Input {
                        address: addr,
                        peer_id: msg.meta.peer_id,
                        source: msg.meta.addr.clone(),
                        addrv2: true,
                    };
                    input_sender.send(input).unwrap();
                }
            }
            _ => (),
        }
    }
}

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let (input_sender, input_receiver) = unbounded();
    let (output_sender, output_receiver) = unbounded();
    let n_workers = 4;

    metricserver::start(&METRICS_ADDRESS).unwrap();

    crossbeam::scope(|s| {
        s.spawn(|_| {
            loop {
                let msg = sub.recv().unwrap();
                let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap().wrap;

                if let Some(event) = unwrapped {
                    handle_event(event, input_sender.clone());
                }
            }

            drop(input_sender);
        });

        for _ in 0..n_workers {
            let (sender, receiver) = (output_sender.clone(), input_receiver.clone());
            s.spawn(move |_| worker(&sender, &receiver));
        }
        drop(output_sender);

        for output in output_receiver.iter() {
            metrics::ADDR_PROCESSED.inc();
            if output.result {
                metrics::ADDR_SUCCESSFUL_HANDSHAKES.inc();
            }
            println!("Sink received {:?}", output);
        }
    })
    .unwrap();
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

fn handshake(address: SocketAddr) -> bool {
    if let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
        let _ = stream.write_all(
            encode::serialize(&build_raw_network_message(build_version_message())).as_slice(),
        );

        if let Ok(read_stream) = stream.try_clone() {
            let mut stream_reader = BufReader::new(read_stream);
            loop {
                if let Ok(reply) = message::RawNetworkMessage::consensus_decode(&mut stream_reader)
                {
                    match reply.payload {
                        message::NetworkMessage::Version(_) => {
                            let second_message = message::RawNetworkMessage {
                                magic: NETWORK.magic(),
                                payload: message::NetworkMessage::Verack,
                            };

                            let _ = stream.write_all(encode::serialize(&second_message).as_slice());
                        }
                        message::NetworkMessage::Verack => {
                            return true;
                            break;
                        }
                        _ => {
                            break;
                        }
                    }
                }
            }
        }
        let _ = stream.shutdown(Shutdown::Both);
    } else {
        return false;
    }
    return true;
}
