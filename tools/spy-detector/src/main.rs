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
const STATS_INTERVAL: Duration = Duration::from_secs(120); // Duration(seconds) to print stats of peers

#[derive(Debug, Default, Clone, PartialEq)]
struct PeerStats {
    inv_sent: u32,
    inv_received: u32,
    getdata_sent: u32,
    getdata_received: u32,
    tx_sent: u32,
    tx_received: u32,
    last_activity: Option<Instant>,
}

type PeerMap = Arc<Mutex<HashMap<String, PeerStats>>>;
// const LOG_TARGET: &str = "main";

fn main() {
    SimpleLogger::new()
        .init()
        .expect("Cannot setup Simple Logger.");

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a thread for periodic stats display
    let display_peer_map = peer_map.clone();
    std::thread::spawn(move || {
        let mut last_display = Instant::now();
        loop {
            let now = Instant::now();
            if now.duration_since(last_display) >= STATS_INTERVAL {
                display_all_stats(&display_peer_map);
                last_display = now;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    loop {
        let msg = sub.recv().unwrap();
        let unwrapped = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;

        if let Some(event) = unwrapped {
            match event {
                Event::Msg(msg) => {
                    if let Some(p2p_msg) = msg.msg {
                        process_p2p_message(&peer_map, &msg.meta, &p2p_msg.to_string());
                    }
                }
                Event::Conn(c) => {
                    if let Some(event) = c.event {
                        process_connection_event(&peer_map, &event.to_string());
                    }
                }
                _ => {}
            }
        }
    }
}

fn process_p2p_message(peer_map: &PeerMap, meta: &net_msg::Metadata, p2p_msg: &str) {
    let mut map = peer_map.lock().unwrap();
    let peer_id = format!("{}", meta.peer_id);
    let stats = map.entry(peer_id.clone()).or_default();

    // let old_stats = stats.clone();

    stats.last_activity = Some(Instant::now());

    let msg_type = p2p_msg.split('(').next().unwrap_or("").trim();

    match msg_type {
        "Inv" => {
            if meta.inbound {
                stats.inv_received += 1;
            } else {
                stats.inv_sent += 1;
            }
        }
        "GetData" => {
            if meta.inbound {
                stats.getdata_received += 1;
            } else {
                stats.getdata_sent += 1;
            }
        }
        "Tx" => {
            if meta.inbound {
                stats.tx_received += 1;
            } else {
                stats.tx_sent += 1;
            }
        }
        _ => return,
    }

    // if *stats != old_stats {
    //     print_peer_stats(&peer_addr, stats);
    // }
}

fn process_connection_event(peer_map: &PeerMap, event: &str) {
    if event.starts_with("closed") {
        let peer_id = event.split(' ').nth(1).unwrap_or("");
        let mut map = peer_map.lock().unwrap();
        if let Some(stats) = map.remove(peer_id) {
            println!("Connection closed for peer: {}", peer_id);
            print_peer_stats(peer_id, &stats);
        }
    }
}

fn print_peer_stats(peer_addr: &str, stats: &PeerStats) {
    println!(
        "[{}] Peer ID {} stats:\n  INV sent: {}\n  INV received: {}\n  GETDATA sent: {}\n  GETDATA received: {}\n  Tx sent: {}\n  Tx received: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        peer_addr,
        stats.inv_sent,
        stats.inv_received,
        stats.getdata_sent,
        stats.getdata_received,
        stats.tx_sent,
        stats.tx_received
    );
}

fn display_all_stats(peer_map: &PeerMap) {
    let map = peer_map.lock().unwrap();
    println!(
        "\n===== Stats for all peers ({}): =====",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    for (peer_id, stats) in map.iter() {
        print_peer_stats(peer_id, stats);
        println!("----------------------------------------");
    }
    println!("=====================================\n");
}
