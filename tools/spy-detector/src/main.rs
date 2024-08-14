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

const ADDRESS: &str = "tcp://127.0.0.1:8883";
const SPY_THRESHOLD: f64 = 0.5; // Adjust this value for spy detection

#[derive(Debug, Default)]
struct PeerStats {
    inv_sent: u32,
    getdata_received: u32,
    inv_received: u32,
    last_activity: Option<Instant>,
}

type PeerMap = Arc<Mutex<HashMap<String, PeerStats>>>;

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    let cleanup_peer_map = peer_map.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        cleanup_inactive_peers(&cleanup_peer_map);
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
    let peer_id = format!("{}:{}", meta.peer_id, meta.conn_type);
    let stats = map.entry(peer_id.clone()).or_default();

    stats.last_activity = Some(Instant::now());

    match p2p_msg {
        "inv" => {
            if meta.inbound {
                stats.inv_received += 1;
            } else {
                stats.inv_sent += 1;
            }
        }
        "getdata" => {
            if meta.inbound {
                stats.getdata_received += 1;
            }
        }
        "tx" => {}
        _ => return,
    }

    analyze_peer_behavior(&peer_id, stats);
}

fn process_connection_event(peer_map: &PeerMap, event: &str) {
    if event.starts_with("closed") {
        let peer_id = event.split(' ').nth(1).unwrap_or("");
        let mut map = peer_map.lock().unwrap();
        map.remove(peer_id);
        println!("Connection closed for peer: {}", peer_id);
    }
}

fn analyze_peer_behavior(peer_id: &str, stats: &PeerStats) {
    let inv_getdata_ratio = if stats.inv_sent > 0 {
        stats.getdata_received as f64 / stats.inv_sent as f64
    } else {
        1.0
    };

    let behavior = if inv_getdata_ratio < SPY_THRESHOLD || stats.inv_received == 0 {
        "SPY NODE"
    } else {
        "NORMAL NODE"
    };

    println!(
        "Peer {}: {} (inv_sent: {}, getdata_received: {}, inv_received: {}, ratio: {:.2})",
        peer_id,
        behavior,
        stats.inv_sent,
        stats.getdata_received,
        stats.inv_received,
        inv_getdata_ratio
    );
}

fn cleanup_inactive_peers(peer_map: &PeerMap) {
    let mut map = peer_map.lock().unwrap();
    let now = Instant::now();
    map.retain(|_, stats| {
        stats
            .last_activity
            .map(|last| now.duration_since(last) < Duration::from_secs(3600))
            .unwrap_or(false)
    });
}
