use shared::clap;
use shared::clap::Parser;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::net_msg;
use shared::prost::Message;
use simple_logger::SimpleLogger;

const ADDRESS: &str = "tcp://127.0.0.1:8883";
//const STATS_INTERVAL: Duration = Duration::from_secs(120); // Duration(seconds) to print stats of peers

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// set the threshold for spy detection (default value is 5)
    #[arg(short, long, default_value = "5")]
    threshold: u32,
}

#[derive(Debug, Default, Clone, PartialEq)]
struct PeerStats {
    inv_tx_sent: u32,                // TX(INV) sent by the peer
    inv_wtx_received: u32,           // TX(INV) received by the peer
    inv_witnesstx_received: u32,     // WitnessTX(INV) received by the peer
    getdata_witnesstx_sent: u32,     // WitnessTX(GETDATA) sent by the peer
    getdata_witnesstx_received: u32, // WitnessTX(GETDATA) received by the peer
    tx_sent: u32,                    // TX sent by the peer
    tx_received: u32,                // TX received by the peer
    last_activity: Option<Instant>,  // Last activity time of the peer
}

type PeerMap = Arc<Mutex<HashMap<String, PeerStats>>>;
// const LOG_TARGET: &str = "main";

fn main() {
    let args = Args::parse();

    let threshold = &args.threshold;

    SimpleLogger::new()
        .init()
        .expect("Cannot setup Simple Logger.");

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a thread for periodic stats display
    // let display_peer_map = peer_map.clone();
    // std::thread::spawn(move || {
    //     let mut last_display = Instant::now();
    //     loop {
    //         let now = Instant::now();
    //         if now.duration_since(last_display) >= STATS_INTERVAL {
    //             display_all_stats(&display_peer_map);
    //             last_display = now;
    //         }
    //         std::thread::sleep(Duration::from_secs(1));
    //     }
    // });

    loop {
        let msg = sub.recv().unwrap();
        let message = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;

        // println!("{:?}", message);

        if let Some(event) = message {
            match event {
                Event::Msg(msg) => {
                    if let Some(p2p_msg) = msg.msg {
                        match p2p_msg {
                            net_msg::message::Msg::Inv(_) => {
                                process_inv_msg(&peer_map, &p2p_msg);
                            }

                            net_msg::message::Msg::Getdata(_) => {
                                process_getdata_msg(&peer_map, &p2p_msg);
                            }

                            net_msg::message::Msg::Tx(_) => {
                                process_tx_msg(&peer_map, &p2p_msg);
                            }

                            _ => {}
                        }
                    }
                }
                Event::Conn(c) => {
                    if let Some(event) = c.event {
                        // process_connection_event(&peer_map, &event.to_string());
                        println!("{}", event);
                    }
                }
                _ => {}
            }
        }
    }
}

fn process_inv_msg(peer_map: &PeerMap, msg: &net_msg::message::Msg) {
    let mut map = peer_map.lock().unwrap();
    let peer_id = format!("{}", msg);
    let stats = map.entry(peer_id.clone()).or_default();

    println!("{}", msg);
}

fn process_getdata_msg(peer_map: &PeerMap, msg: &net_msg::message::Msg) {
    let mut map = peer_map.lock().unwrap();
    let peer_id = format!("{}", msg);

    println!("{}", msg);
}

fn process_tx_msg(peer_map: &PeerMap, msg: &net_msg::message::Msg) {
    let mut map = peer_map.lock().unwrap();
    let peer_id = format!("{}", msg);

    println!("{}", msg);
}

fn process_connection_event(peer_map: &PeerMap, event: &str) {
    if event.starts_with("closed") {
        let peer_id = event.split(' ').nth(1).unwrap_or("");
        let mut map = peer_map.lock().unwrap();
        if let Some(stats) = map.remove(peer_id) {
            println!("Connection closed for peer: {}", peer_id);
            //print_peer_stats(peer_id, &stats);
        }
    }
}

// fn print_peer_stats(peer_addr: &str, stats: &PeerStats) {
//     println!(
//         "[{}] Peer ID {} stats:\n  INV sent: {}\n  INV received: {}\n  GETDATA sent: {}\n  GETDATA received: {}\n  Tx sent: {}\n  Tx received: {}",
//         chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//         peer_addr,
//         stats.inv_sent,
//         stats.inv_received,
//         stats.getdata_sent,
//         stats.getdata_received,
//         stats.tx_sent,
//         stats.tx_received
//     );
// }

// fn display_all_stats(peer_map: &PeerMap) {
//     let map = peer_map.lock().unwrap();
//     println!(
//         "\n===== Stats for all peers ({}): =====",
//         chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
//     );
//     for (peer_id, stats) in map.iter() {
//         print_peer_stats(peer_id, stats);
//         println!("----------------------------------------");
//     }
//     println!("=====================================\n");
// }
