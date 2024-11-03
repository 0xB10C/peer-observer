use dashmap::DashMap;
use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use shared::clap;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::net_msg;
use shared::prost::Message;
use simple_logger::SimpleLogger;
use std::sync::atomic;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

const ADDRESS: &str = "tcp://127.0.0.1:8883";
const STATS_INTERVAL: Duration = Duration::from_secs(10); // Duration(seconds) to print stats of peers

const LOG_TARGET: &str = "main";

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// set the threshold for spy detection (default value is 5)
    #[arg(short, long, default_value = "5")]
    threshold: u32,
}

#[derive(Debug, Default)]
struct PeerStats {
    inv_tx_received: AtomicU32,            // TX(INV) received by the peer
    inv_tx_sent: AtomicU32,                // TX(INV) sent by the peer
    inv_wtx_sent: AtomicU32,               // WTX(INV) received by the peer
    inv_wtx_received: AtomicU32,           // WTX(INV) received by the peer
    inv_witnesstx_received: AtomicU32,     // WitnessTX(INV) received by the peer
    inv_witnesstx_sent: AtomicU32,         // WitnessTX(INV) sent by the peer
    getdata_witnesstx_sent: AtomicU32,     // WitnessTX(GETDATA) sent by the peer
    getdata_witnesstx_received: AtomicU32, // WitnessTX(GETDATA) received by the peer
    tx_sent: AtomicU32,                    // TX sent by the peer
    tx_received: AtomicU32,                // TX received by the peer
    last_activity: Option<Instant>,        // Last activity time of the peer
}

type PeerMap = Arc<DashMap<String, PeerStats>>;

fn main() {
    let args = Args::parse();

    let threshold = &args.threshold;

    SimpleLogger::new()
        .init()
        .expect("Cannot setup Simple Logger.");

    log::info!(target: LOG_TARGET, "Starting spy-detector...",);

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let peer_map: PeerMap = Arc::new(DashMap::new());

    // Spawn a thread for periodic stats display - 2 mins
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

    log::info!(target: LOG_TARGET, "Spy-detector started",);

    loop {
        let msg = sub.recv().unwrap();
        let message = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;

        if let Some(event) = message {
            match event {
                Event::Msg(msg) => {
                    let msg_type: u32;

                    if msg.meta.inbound {
                        msg_type = 0;
                    } else {
                        msg_type = 1;
                    };

                    let peer_id = msg.meta.peer_id;

                    if let Some(p2p_msg) = msg.msg {
                        match p2p_msg {
                            net_msg::message::Msg::Inv(_) => {
                                process_inv_msg(&peer_map, &p2p_msg, msg_type, peer_id);
                            }

                            net_msg::message::Msg::Getdata(_) => {
                                process_getdata_msg(&peer_map, &p2p_msg, msg_type, peer_id);
                            }

                            net_msg::message::Msg::Tx(_) => {
                                process_tx_msg(&peer_map, &p2p_msg, msg_type, peer_id);
                            }
                            _ => {}
                        }
                    }
                }
                Event::Conn(c) => {
                    if let Some(event) = c.event {
                        process_connection_event(&peer_map, &event.to_string());
                        //println!("{}", event);
                    }
                }
                _ => {}
            }
        }
    }
}

fn process_inv_msg(peer_map: &PeerMap, msg: &net_msg::message::Msg, msg_type: u32, peer_id: u64) {
    let stats = peer_map
        .entry(peer_id.to_string())
        .or_insert_with(PeerStats::default);

    if let net_msg::message::Msg::Inv(inv) = msg {
        for inv_item in &inv.items {
            match inv_item.inv_type() {
                "Tx" => {
                    if msg_type == 0 {
                        stats
                            .inv_tx_received
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    } else {
                        stats.inv_tx_sent.fetch_add(1, atomic::Ordering::Relaxed);
                    }
                }
                "WTx" => {
                    if msg_type == 0 {
                        stats
                            .inv_wtx_received
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    } else {
                        stats.inv_wtx_sent.fetch_add(1, atomic::Ordering::Relaxed);
                    }
                }
                "WitnessTx" => {
                    if msg_type == 0 {
                        stats
                            .inv_witnesstx_received
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    } else {
                        stats
                            .inv_witnesstx_sent
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    }
                }
                _ => {} // Ignore other types
            }
        }
    }
}

fn process_getdata_msg(
    peer_map: &PeerMap,
    msg: &net_msg::message::Msg,
    msg_type: u32,
    peer_id: u64,
) {
    let stats = peer_map
        .entry(peer_id.to_string())
        .or_insert_with(PeerStats::default);

    if let net_msg::message::Msg::Inv(inv) = msg {
        for inv_item in &inv.items {
            match inv_item.inv_type() {
                "WitnessTx" => {
                    if msg_type == 0 {
                        println!("WitnessTx recieved");
                        stats
                            .getdata_witnesstx_received
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    } else {
                        println!("WitnessTx sent");
                        stats
                            .getdata_witnesstx_sent
                            .fetch_add(1, atomic::Ordering::Relaxed);
                    }
                }
                _ => {} // Ignore other types
            }
        }
    }
}

fn process_tx_msg(peer_map: &PeerMap, msg: &net_msg::message::Msg, msg_type: u32, peer_id: u64) {
    let stats = peer_map
        .entry(peer_id.to_string())
        .or_insert_with(PeerStats::default);

    if msg_type == 0 {
        stats.tx_received.fetch_add(1, atomic::Ordering::Relaxed);
    } else {
        stats.tx_sent.fetch_add(1, atomic::Ordering::Relaxed);
    }
}

fn process_connection_event(peer_map: &PeerMap, event: &str) {
    if event.starts_with("closed") {
        let peer_id = event.split(' ').nth(1).unwrap_or("");
        if let Some(stats) = peer_map.remove(peer_id) {
            println!("Connection closed for peer: {}", peer_id);
            print_peer_stats(peer_id, &stats.1);
        }
    }
}

fn print_peer_stats(peer_addr: &str, stats: &PeerStats) {
    println!(
        "[{}] Peer ID {} stats:\n  INV TX sent: {:?}\n  INV TX received: {:?}\n  INV WTX received: {:?}\n INV WitnessTX received: {:?}\n GetDATA WitnessTX received: {:?}\n  GETDATA WitnessTX received: {:?}\n  Tx sent: {:?}\n  Tx received: {:?}\n Last Activity: {:?}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        peer_addr,
        stats.inv_tx_sent,
        stats.inv_tx_received,
        stats.inv_wtx_received,
        stats.inv_witnesstx_received,
        stats.getdata_witnesstx_sent,
        stats.getdata_witnesstx_received,
        stats.tx_sent,
        stats.tx_received,
        stats.last_activity
    );
}

fn display_all_stats(peer_map: &PeerMap) {
    let map = peer_map;
    println!(
        "\n===== Stats for all peers ({}): =====",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    for item in map.iter() {
        let peer_id = item.key();
        let stats = item.value();

        //println!("Key: {}\n", peer_id);
        print_peer_stats(&peer_id, &stats);
        println!("----------------------------------------");
    }
    println!("=====================================\n");
}
