#![cfg_attr(feature = "strict", deny(warnings))]
// Allow for more metric macros in metrics.rs
#![recursion_limit = "256"]

use shared::addrman::addrman_event;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::mempool::mempool_event;
use shared::metricserver;
use shared::net_conn::connection_event;
use shared::net_msg;
use shared::net_msg::{message::Msg, reject::RejectReason};
use shared::prost::Message;
use shared::rpc::rpc_event;
use shared::simple_logger;
use shared::util;
use shared::validation::validation_event;
use shared::{clap, nats};
use std::collections::HashMap;
use std::convert::TryFrom;

mod metrics;

const LOG_TARGET: &str = "main";

/// A peer-observer tool that produces Prometheus metrics for received events
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The NATS server address the tool should connect and subscribe to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    nats_address: String,
    /// The metrics server address the tool should listen on.
    #[arg(short, long, default_value = "127.0.0.1:8282")]
    metrics_address: String,
    /// The log level the tool should run with. Valid log levels
    /// are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

fn main() {
    let args = Args::parse();

    simple_logger::init_with_level(args.log_level).unwrap();

    log::info!(target: LOG_TARGET, "Starting metrics-server...",);

    metrics::RUNTIME_START_TIMESTAMP.set(util::current_timestamp() as i64);

    metricserver::start(&args.metrics_address).unwrap();
    log::info!(target: LOG_TARGET, "metrics-server listening on: {}", args.metrics_address);

    let nc = nats::connect(args.nats_address).expect("should be able to connect to NATS server");
    let sub = nc.subscribe("*").expect("could not subscribe to topic '*'");
    for msg in sub.messages() {
        let unwrapped = event_msg::EventMsg::decode(msg.data.as_slice()).unwrap();

        if let Some(event) = unwrapped.event {
            match event {
                Event::Msg(msg) => {
                    handle_p2p_message(&msg, unwrapped.timestamp);
                }
                Event::Conn(c) => {
                    handle_connection_event(c.event.unwrap(), unwrapped.timestamp);
                }
                Event::Addrman(a) => {
                    handle_addrman_event(&a.event.unwrap());
                }
                Event::Mempool(m) => {
                    handle_mempool_event(&m.event.unwrap());
                }
                Event::Validation(v) => handle_validation_event(&v.event.unwrap()),
                Event::Rpc(r) => handle_rpc_event(&r.event.unwrap()),
            }
        }
    }

    fn handle_rpc_event(e: &rpc_event::Event) {
        match e {
            rpc_event::Event::PeerInfos(info) => {
                let mut on_gmax_banlist = 0;
                let mut on_monero_banlist = 0;
                let mut on_tor_exit_list = 0;
                let mut on_linkinglion_list = 0;
                let mut addr_rate_limited_peers = 0; // number of peers that had at least one address rate limited.
                let mut addr_rate_limited_total: u64 = 0; // total number of rate-limited addresses
                let mut addr_processed_total: u64 = 0; // total number of processed addresses
                for peer in info.infos.iter() {
                    let ip = util::ip_from_ipport(peer.address.clone());
                    if util::is_on_gmax_banlist(&ip) {
                        on_gmax_banlist += 1;
                    }
                    if util::is_on_monero_banlist(&ip) {
                        on_monero_banlist += 1;
                    }
                    if util::is_tor_exit_node(&ip) {
                        on_tor_exit_list += 1;
                    }
                    if util::is_on_linkinglion_banlist(&ip) {
                        on_linkinglion_list += 1;
                    }

                    if peer.addr_rate_limited > 0 {
                        addr_rate_limited_peers += 1;
                    }

                    addr_rate_limited_total += peer.addr_rate_limited;
                    addr_processed_total += peer.addr_processed;
                }

                metrics::PEER_INFO_LIST_CONNECTIONS_GMAX_BAN.set(on_gmax_banlist);
                metrics::PEER_INFO_LIST_CONNECTIONS_MONERO_BAN.set(on_monero_banlist);
                metrics::PEER_INFO_LIST_CONNECTIONS_TOR_EXIT.set(on_tor_exit_list);
                metrics::PEER_INFO_LIST_CONNECTIONS_LINKINGLION.set(on_linkinglion_list);
                metrics::PEER_INFO_ADDR_RATELIMITED_PEERS.set(addr_rate_limited_peers);
                metrics::PEER_INFO_ADDR_RATELIMITED_TOTAL.set(addr_rate_limited_total as i64);
                metrics::PEER_INFO_ADDR_PROCESSED_TOTAL.set(addr_processed_total as i64);
            }
        }
    }

    fn handle_mempool_event(e: &mempool_event::Event) {
        match e {
            mempool_event::Event::Added(a) => {
                metrics::MEMPOOL_ADDED.inc();
                metrics::MEMPOOL_ADDED_VBYTES.inc_by(a.vsize as u64);
            }
            mempool_event::Event::Removed(r) => {
                metrics::MEMPOOL_REMOVED
                    .with_label_values(&[&r.reason])
                    .inc();
            }
            mempool_event::Event::Replaced(r) => {
                metrics::MEMPOOL_REPLACED.inc();
                metrics::MEMPOOL_REPLACED_VBYTES.inc_by(r.replaced_vsize as u64);
            }
            mempool_event::Event::Rejected(r) => {
                metrics::MEMPOOL_REJECTED
                    .with_label_values(&[&r.reason])
                    .inc();
            }
        }
    }

    fn handle_validation_event(e: &validation_event::Event) {
        match e {
            validation_event::Event::BlockConnected(v) => {
                // Due to an undetected API break, 23.x and 24.x used microseconds, while
                // 25.x, 26.x, and 27.x use nanoseconds.
                // See https://github.com/bitcoin/bitcoin/pull/29877
                // Assume the tracepoint passed nanoseconds here, but we don't need the
                // nanosecond precision and already have metrics recorded as microseconds.
                let duration_microseconds = (v.connection_time / 1000) as u64;
                metrics::VALIDATION_BLOCK_CONNECTED_LATEST_HEIGHT.set(v.height as i64);
                metrics::VALIDATION_BLOCK_CONNECTED_LATEST_TIME.set(duration_microseconds as i64);
                metrics::VALIDATION_BLOCK_CONNECTED_DURATION.inc_by(duration_microseconds);
                metrics::VALIDATION_BLOCK_CONNECTED_LATEST_SIGOPS.set(v.sigops);
                metrics::VALIDATION_BLOCK_CONNECTED_LATEST_INPUTS.set(v.inputs.into());
                metrics::VALIDATION_BLOCK_CONNECTED_LATEST_TRANSACTIONS.set(v.transactions);
            }
        }
    }

    fn handle_connection_event(cevent: connection_event::Event, timestamp: u64) {
        match cevent {
            connection_event::Event::Inbound(i) => {
                let ip = util::ip_from_ipport(i.conn.addr);
                metrics::CONN_INBOUND.inc();
                if util::is_tor_exit_node(&ip) {
                    metrics::CONN_INBOUND_TOR_EXIT.inc();
                }
                if util::is_on_gmax_banlist(&ip) {
                    metrics::CONN_INBOUND_BANLIST_GMAX
                        .with_label_values(&[&ip])
                        .inc();
                }
                if util::is_on_monero_banlist(&ip) {
                    metrics::CONN_INBOUND_BANLIST_MONERO
                        .with_label_values(&[&ip])
                        .inc();
                }
                metrics::CONN_INBOUND_SUBNET
                    .with_label_values(&[&util::subnet(ip)])
                    .inc();
                metrics::CONN_INBOUND_NETWORK
                    .with_label_values(&[&i.conn.network.to_string()])
                    .inc();
                metrics::CONN_INBOUND_CURRENT.set(i.existing_connections as i64);
            }
            connection_event::Event::Outbound(o) => {
                let ip = util::ip_from_ipport(o.conn.addr);
                metrics::CONN_OUTBOUND.inc();
                metrics::CONN_OUTBOUND_NETWORK
                    .with_label_values(&[&o.conn.network.to_string()])
                    .inc();
                metrics::CONN_OUTBOUND_SUBNET
                    .with_label_values(&[&util::subnet(ip)])
                    .inc();
                metrics::CONN_OUTBOUND_CURRENT.set(o.existing_connections as i64);
            }
            connection_event::Event::Closed(c) => {
                let ip = util::ip_from_ipport(c.conn.addr);
                metrics::CONN_CLOSED.inc();
                metrics::CONN_CLOSED_AGE.inc_by(timestamp - c.time_established);
                metrics::CONN_CLOSED_NETWORK
                    .with_label_values(&[&c.conn.network.to_string()])
                    .inc();
                metrics::CONN_CLOSED_SUBNET
                    .with_label_values(&[&util::subnet(ip)])
                    .inc();
            }
            connection_event::Event::InboundEvicted(e) => {
                metrics::CONN_EVICTED.inc();
                metrics::CONN_EVICTED_WITHINFO
                    .with_label_values(&[
                        &util::ip_from_ipport(e.conn.addr),
                        &e.conn.network.to_string(),
                    ])
                    .inc();
            }
            connection_event::Event::Misbehaving(m) => {
                metrics::CONN_MISBEHAVING
                    .with_label_values(&[
                        &m.id.to_string(),
                        &m.score_increase.to_string(),
                        &m.xmessage,
                    ])
                    .inc();
                metrics::CONN_MISBEHAVING_REASON
                    .with_label_values(&[&m.xmessage])
                    .inc();
                metrics::CONN_MISBEHAVING_SCORE_INC
                    .with_label_values(&[&m.id.to_string(), &m.xmessage])
                    .inc_by(m.score_increase as u64);
            }
        }
    }

    fn handle_addrman_event(aevent: &addrman_event::Event) {
        match aevent {
            addrman_event::Event::New(new) => {
                metrics::ADDRMAN_NEW_INSERT
                    .with_label_values(&[&new.inserted.to_string()])
                    .inc();
            }
            addrman_event::Event::Tried(_) => {
                metrics::ADDRMAN_TRIED_INSERT.inc();
            }
        }
    }

    fn handle_p2p_message(msg: &net_msg::Message, timestamp: u64) {
        let conn_type = msg.meta.conn_type.to_string();
        let direction = if msg.meta.inbound {
            "inbound".to_string()
        } else {
            "outbound".to_string()
        };
        let ip = util::ip_from_ipport(msg.meta.addr.clone());
        let subnet = util::subnet(ip.clone());
        let mut labels = HashMap::<&str, &str>::new();
        labels.insert(metrics::LABEL_P2P_MSG_TYPE, &msg.meta.command);
        labels.insert(metrics::LABEL_P2P_CONNECTION_TYPE, &conn_type);
        labels.insert(metrics::LABEL_P2P_DIRECTION, &direction);

        metrics::P2P_MESSAGE_COUNT.with(&labels).inc();
        metrics::P2P_MESSAGE_BYTES
            .with(&labels)
            .inc_by(msg.meta.size);

        metrics::P2P_MESSAGE_BYTES_BY_SUBNET
            .with_label_values(&[&direction, &subnet])
            .inc_by(msg.meta.size);
        metrics::P2P_MESSAGE_COUNT_BY_SUBNET
            .with_label_values(&[&direction, &subnet])
            .inc();

        match msg.msg.as_ref().unwrap() {
            Msg::Addr(addr) => {
                metrics::P2P_ADDR_ADDRESS_HISTOGRAM
                    .with_label_values(&[&direction])
                    .observe(addr.addresses.len() as f64);
                let future_offset = metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, &"future".to_string()]);
                let past_offset = metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, &"past".to_string()]);
                for address in addr.addresses.iter() {
                    // We substract the timestamp in the address from the time we received the
                    // message. If the remaining offset is larger than or equal to zero, the address
                    // timestamp lies in the past. If the offset is smaller than zero, the address
                    // timestamp lies in the future.
                    let offset = timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64);
                    } else {
                        future_offset.observe((offset * -1) as f64);
                    }
                    for i in 0..32 {
                        if (1 << i) & address.services > 0 {
                            metrics::P2P_ADDR_SERVICES_HISTOGRAM
                                .with_label_values(&[&direction])
                                .observe(i as f64)
                        }
                    }
                    metrics::P2P_ADDR_SERVICES
                        .with_label_values(&[&direction, &address.services.to_string()])
                        .inc();
                }
            }
            Msg::Addrv2(addrv2) => {
                metrics::P2P_ADDRV2_ADDRESS_HISTOGRAM
                    .with_label_values(&[&direction])
                    .observe(addrv2.addresses.len() as f64);
                let future_offset = metrics::P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, &"future".to_string()]);
                let past_offset = metrics::P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, &"past".to_string()]);
                for address in addrv2.addresses.iter() {
                    // We substract the timestamp in the address from the time we received the
                    // message. If the remaining offset is larger than or equal to zero, the address
                    // timestamp lies in the past. If the offset is smaller than zero, the address
                    // timestamp lies in the future.
                    let offset = timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64);
                    } else {
                        future_offset.observe((offset * -1) as f64);
                    }

                    for i in 0..32 {
                        if (1 << i) & address.services > 0 {
                            metrics::P2P_ADDRV2_SERVICES_HISTOGRAM
                                .with_label_values(&[&direction])
                                .observe(i as f64)
                        }
                    }
                    metrics::P2P_ADDRV2_SERVICES
                        .with_label_values(&[&direction, &address.services.to_string()])
                        .inc();
                }
            }
            Msg::Emptyaddrv2(_) => {
                metrics::P2P_EMPTYADDRV2
                    .with_label_values(&[&direction, &ip])
                    .inc();
            }
            Msg::Inv(inv) => {
                let mut count_by_invtype: HashMap<String, u64> = HashMap::new();
                for item in inv.items.iter() {
                    let count = count_by_invtype
                        .entry(String::from((*item).inv_type()))
                        .or_insert(0);
                    *count += 1;
                }
                for (inv_type, count) in &count_by_invtype {
                    metrics::P2P_INV_ENTRIES
                        .with_label_values(&[&direction, inv_type])
                        .inc_by(*count);
                }
                metrics::P2P_INV_ENTRIES_HISTOGRAM
                    .with_label_values(&[&direction])
                    .observe(inv.items.len() as f64);
                if count_by_invtype.len() == 1 {
                    metrics::P2P_INV_ENTRIES_HOMOGENOUS
                        .with_label_values(&[&direction])
                        .inc();
                } else {
                    metrics::P2P_INV_ENTRIES_HETEROGENEOUS
                        .with_label_values(&[&direction])
                        .inc();
                }
            }
            Msg::Ping(_) => {
                if msg.meta.inbound {
                    metrics::P2P_PING_SUBNET.with_label_values(&[&subnet]).inc();
                }
            }
            Msg::Oldping(_) => {
                if msg.meta.inbound {
                    metrics::P2P_OLDPING_SUBNET
                        .with_label_values(&[&subnet])
                        .inc();
                }
            }
            Msg::Version(v) => {
                if msg.meta.inbound {
                    metrics::P2P_VERSION_SUBNET
                        .with_label_values(&[&subnet])
                        .inc();
                    metrics::P2P_VERSION_USERAGENT
                        .with_label_values(&[&v.user_agent])
                        .inc();
                }
            }
            Msg::Feefilter(f) => {
                metrics::P2P_FEEFILTER_FEERATE
                    .with_label_values(&[&direction, &f.fee.to_string()])
                    .inc();
            }
            Msg::Reject(r) => {
                if msg.meta.inbound {
                    metrics::P2P_REJECT_MESSAGE
                        .with_label_values(&[
                            &r.rejected_command,
                            &RejectReason::try_from(r.reason).unwrap().to_string(),
                        ])
                        .inc();
                }
            }
            _ => (),
        }
    }
}
