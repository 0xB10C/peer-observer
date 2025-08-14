#![cfg_attr(feature = "strict", deny(warnings))]
// Allow for more metric macros in metrics.rs
#![recursion_limit = "256"]

use shared::addrman::addrman_event;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::futures::StreamExt;
use shared::log;
use shared::mempool::mempool_event;
use shared::metricserver;
use shared::net_conn::connection_event;
use shared::net_msg;
use shared::net_msg::{message::Msg, reject::RejectReason};
use shared::prost::Message;
use shared::rpc::rpc_event;
use shared::tokio::{
    sync::watch,
    time::{sleep, Duration},
};
use shared::util;
use shared::validation::validation_event;
use shared::{async_nats, clap};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::convert::TryFrom;

pub mod error;
mod metrics;
mod stat_util;

const LOG_TARGET: &str = "main";

/// A peer-observer tool that produces Prometheus metrics for received events
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The NATS server address the tool should connect and subscribe to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    nats_address: String,
    /// The metrics server address the tool should listen on.
    #[arg(short, long, default_value = "127.0.0.1:8282")]
    metrics_address: String,
    /// The log level the tool should run with. Valid log levels
    /// are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    pub log_level: log::Level,
}

impl Args {
    pub fn new(nats_address: String, metrics_address: String, log_level: log::Level) -> Self {
        Self {
            nats_address,
            metrics_address,
            log_level,
        }
    }
}

/// runs the metrics tool
/// Expects that a logger has been initialized already.
pub async fn run(
    args: Args,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), error::RuntimeError> {
    log::info!(target: LOG_TARGET, "Starting metrics-server...",);

    let metrics = metrics::Metrics::new();

    metricserver::start(&args.metrics_address, Some(metrics.registry.clone()))?;
    log::info!(target: LOG_TARGET, "metrics-server listening on: {}", args.metrics_address);

    let nc = async_nats::connect(args.nats_address).await?;
    let mut sub = nc.subscribe("*").await?;

    metrics
        .runtime_start_timestamp
        .set(util::current_timestamp() as i64);

    loop {
        shared::tokio::select! {
            maybe_msg = sub.next() => {
                if let Some(msg) = maybe_msg {
                    handle_event(msg, metrics.clone())?;
                    // directly process next event if available
                    continue
                } else {
                    break; // subscription ended
                }
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    log::info!("metrics tool received shutdown signal.");
                    break;
                }
            }
        }
        // Prevent busy loop if we don't have events
        sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

fn handle_event(
    msg: async_nats::Message,
    metrics: metrics::Metrics,
) -> Result<(), error::RuntimeError> {
    let unwrapped = event_msg::EventMsg::decode(msg.payload)?;
    if let Some(event) = unwrapped.event {
        match event {
            Event::Msg(msg) => {
                handle_p2p_message(&msg, unwrapped.timestamp, metrics);
            }
            Event::Conn(c) => {
                if let Some(e) = c.event {
                    handle_connection_event(&e, unwrapped.timestamp, metrics);
                }
            }
            Event::Addrman(a) => {
                if let Some(e) = a.event {
                    handle_addrman_event(&e, metrics);
                }
            }
            Event::Mempool(m) => {
                if let Some(e) = m.event {
                    handle_mempool_event(&e, metrics);
                }
            }
            Event::Validation(v) => {
                if let Some(e) = v.event {
                    handle_validation_event(&e, metrics);
                }
            }
            Event::Rpc(r) => {
                if let Some(e) = r.event {
                    handle_rpc_event(&e, metrics);
                }
            }
        }
    }

    Ok(())
}

fn handle_rpc_event(e: &rpc_event::Event, metrics: metrics::Metrics) {
    match e {
        rpc_event::Event::PeerInfos(info) => {
            let mut on_gmax_banlist = 0;
            let mut on_monero_banlist = 0;
            let mut on_tor_exit_list = 0;
            let mut on_linkinglion_list = 0;

            // track how many peers have a time offset > 10s and < -10s
            let mut timeoffset_plus10s = 0;
            let mut timeoffset_minus10s = 0;

            // track how many peers we consider high bandwidth compact block peers
            // and how many consider us to be a high bandwidth peer
            let mut bip152_highbandwidth_to = 0;
            let mut bip152_highbandwidth_from = 0;

            let mut addr_rate_limited_peers = 0; // number of peers that had at least one address rate limited.
            let mut addr_rate_limited_total: u64 = 0; // total number of rate-limited addresses
            let mut addr_processed_total: u64 = 0; // total number of processed addresses
            let mut addr_relay_enabled_peers = 0; // nmber of peers we participate in address relay with

            let mut pings = vec![];
            let mut min_pings = vec![];
            // Count the number of peers that have a ping_wait larger than 5 seconds.
            // These are good candidates for dropping connections due to network issues.
            let mut ping_wait_larger_5s = 0;

            let mut peers_by_transport_protocol_type: BTreeMap<&str, i64> = BTreeMap::new();
            let mut peers_by_network: BTreeMap<&str, i64> = BTreeMap::new();
            let mut peers_by_connection_type: BTreeMap<&str, i64> = BTreeMap::new();
            let mut peers_by_protocol_version: BTreeMap<u32, i64> = BTreeMap::new();
            let mut peers_by_asn: BTreeMap<u32, i64> = BTreeMap::new();

            // When we requested a block, but a peer hasn't yet sent us the block,
            // the block is considered inflight. If a peer doesn't send us a block at all,
            // it might be stalling us.
            let mut peers_with_inflight_blocks = 0;
            let mut distinct_inflight_block_heights = BTreeSet::new();

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

                if peer.addr_relay_enabled {
                    addr_relay_enabled_peers += 1;
                }

                if peer.time_offset < -10 {
                    timeoffset_minus10s += 1;
                } else if peer.time_offset > 10 {
                    timeoffset_plus10s += 1;
                }

                // Ping times are in seconds, but we want to have them as milliseconds.
                // Also, if the ping is 0, it means we don't have a ping. So don't report it.
                if peer.ping_time > 0.0 {
                    pings.push(peer.ping_time * 1000.0);
                }
                if peer.minimum_ping > 0.0 {
                    min_pings.push(peer.minimum_ping * 1000.0);
                }
                if peer.ping_wait > 5.0 {
                    ping_wait_larger_5s += 1;
                }

                if peer.bip152_hb_to {
                    bip152_highbandwidth_to += 1;
                }
                if peer.bip152_hb_from {
                    bip152_highbandwidth_from += 1;
                }

                if !peer.inflight.is_empty() {
                    peers_with_inflight_blocks += 1;
                    for inflight in peer.inflight.iter() {
                        distinct_inflight_block_heights.insert(*inflight);
                    }
                }

                peers_by_transport_protocol_type
                    .entry(&peer.transport_protocol_type)
                    .and_modify(|e| *e += 1)
                    .or_insert(1);

                peers_by_network
                    .entry(&peer.network)
                    .and_modify(|e| *e += 1)
                    .or_insert(1);

                peers_by_connection_type
                    .entry(&peer.connection_type)
                    .and_modify(|e| *e += 1)
                    .or_insert(1);

                peers_by_protocol_version
                    .entry(peer.version)
                    .and_modify(|e| *e += 1)
                    .or_insert(1);

                // Not all nodes use an ASMap file and we don't care about the number
                // of non-mapped peers here. So, ignore peers mapped as 0.
                if peer.mapped_as != 0 {
                    peers_by_asn
                        .entry(peer.mapped_as)
                        .and_modify(|e| *e += 1)
                        .or_insert(1);
                }
            }

            metrics
                .rpc_peer_info_list_peers_gmax_ban
                .set(on_gmax_banlist);
            metrics
                .rpc_peer_info_list_peers_monero_ban
                .set(on_monero_banlist);
            metrics
                .rpc_peer_info_list_peers_tor_exit
                .set(on_tor_exit_list);
            metrics
                .rpc_peer_info_list_peers_linkinglion
                .set(on_linkinglion_list);

            metrics
                .rpc_peer_info_addr_ratelimited_peers
                .set(addr_rate_limited_peers);
            metrics
                .rpc_peer_info_addr_ratelimited_total
                .set(addr_rate_limited_total as i64);
            metrics
                .rpc_peer_info_addr_processed_total
                .set(addr_processed_total as i64);
            metrics
                .rpc_peer_info_addr_relay_enabled_peers
                .set(addr_relay_enabled_peers);

            metrics
                .rpc_peer_info_ping_mean
                .set(stat_util::mean_f64(&pings));
            metrics
                .rpc_peer_info_ping_median
                .set(stat_util::median_f64(&pings));
            metrics
                .rpc_peer_info_minping_mean
                .set(stat_util::mean_f64(&min_pings));
            metrics
                .rpc_peer_info_minping_median
                .set(stat_util::median_f64(&min_pings));
            metrics
                .rpc_peer_info_ping_wait_larger_5_seconds_block_peers
                .set(ping_wait_larger_5s);

            metrics
                .rpc_peer_info_timeoffset_plus10s
                .set(timeoffset_plus10s);
            metrics
                .rpc_peer_info_timeoffset_minus10s
                .set(timeoffset_minus10s);

            metrics
                .rpc_peer_info_bip152_highbandwidth_to
                .set(bip152_highbandwidth_to);
            metrics
                .rpc_peer_info_bip152_highbandwidth_from
                .set(bip152_highbandwidth_from);

            metrics.rpc_peer_info_num_peers.set(info.infos.len() as i64);

            metrics
                .rpc_peer_info_inflight_block_peers
                .set(peers_with_inflight_blocks);
            metrics
                .rpc_peer_info_inflight_distinct_blocks_heights
                .set(distinct_inflight_block_heights.len() as i64);

            metrics.rpc_peer_info_transport_protocol_type_peers.reset();
            for (k, v) in peers_by_transport_protocol_type.iter() {
                metrics
                    .rpc_peer_info_transport_protocol_type_peers
                    .with_label_values(&[k])
                    .set(*v);
            }

            metrics.rpc_peer_info_network_peers.reset();
            for (k, v) in peers_by_network.iter() {
                metrics
                    .rpc_peer_info_network_peers
                    .with_label_values(&[k])
                    .set(*v);
            }

            metrics.rpc_peer_info_connection_type_peers.reset();
            for (k, v) in peers_by_connection_type.iter() {
                metrics
                    .rpc_peer_info_connection_type_peers
                    .with_label_values(&[k])
                    .set(*v);
            }

            metrics.rpc_peer_info_protocol_version_peers.reset();
            for (k, v) in peers_by_protocol_version.iter() {
                metrics
                    .rpc_peer_info_protocol_version_peers
                    .with_label_values(&[k.to_string()])
                    .set(*v);
            }

            metrics.rpc_peer_info_asn_peers.reset();
            for (k, v) in peers_by_asn.iter() {
                metrics
                    .rpc_peer_info_asn_peers
                    .with_label_values(&[k.to_string()])
                    .set(*v);
            }
        }
    }
}

fn handle_mempool_event(e: &mempool_event::Event, metrics: metrics::Metrics) {
    match e {
        mempool_event::Event::Added(a) => {
            metrics.mempool_added.inc();
            metrics.mempool_added_vbytes.inc_by(a.vsize as u64);
        }
        mempool_event::Event::Removed(r) => {
            metrics
                .mempool_removed
                .with_label_values(&[&r.reason])
                .inc();
        }
        mempool_event::Event::Replaced(r) => {
            metrics.mempool_replaced.inc();
            metrics
                .mempool_replaced_vbytes
                .inc_by(r.replaced_vsize as u64);
        }
        mempool_event::Event::Rejected(r) => {
            metrics
                .mempool_rejected
                .with_label_values(&[&r.reason])
                .inc();
        }
    }
}

fn handle_validation_event(e: &validation_event::Event, metrics: metrics::Metrics) {
    match e {
        validation_event::Event::BlockConnected(v) => {
            // Due to an undetected API break, 23.x and 24.x used microseconds, while
            // 25.x, 26.x, and 27.x use nanoseconds.
            // See https://github.com/bitcoin/bitcoin/pull/29877
            // Assume the tracepoint passed nanoseconds here, but we don't need the
            // nanosecond precision and already have metrics recorded as microseconds.
            let duration_microseconds = (v.connection_time / 1000) as u64;
            metrics
                .validation_block_connected_latest_height
                .set(v.height as i64);
            metrics
                .validation_block_connected_latest_connection_time
                .set(duration_microseconds as i64);
            metrics
                .validation_block_connected_connection_time
                .inc_by(duration_microseconds);
            metrics
                .validation_block_connected_latest_sigops
                .set(v.sigops);
            metrics
                .validation_block_connected_latest_inputs
                .set(v.inputs.into());
            metrics
                .validation_block_connected_latest_transactions
                .set(v.transactions);
        }
    }
}

fn handle_connection_event(
    cevent: &connection_event::Event,
    timestamp: u64,
    metrics: metrics::Metrics,
) {
    match cevent {
        connection_event::Event::Inbound(i) => {
            let ip = util::ip_from_ipport(i.conn.addr.clone());
            metrics.conn_inbound.inc();
            if util::is_tor_exit_node(&ip) {
                metrics.conn_inbound_tor_exit.inc();
            }
            if util::is_on_gmax_banlist(&ip) {
                metrics
                    .conn_inbound_banlist_gmax
                    .with_label_values(&[&ip])
                    .inc();
            }
            if util::is_on_monero_banlist(&ip) {
                metrics
                    .conn_inbound_banlist_monero
                    .with_label_values(&[&ip])
                    .inc();
            }
            metrics
                .conn_inbound_subnet
                .with_label_values(&[&util::subnet(ip)])
                .inc();
            metrics
                .conn_inbound_network
                .with_label_values(&[&i.conn.network.to_string()])
                .inc();
            metrics
                .conn_inbound_current
                .set(i.existing_connections as i64);
        }
        connection_event::Event::Outbound(o) => {
            let ip = util::ip_from_ipport(o.conn.addr.clone());
            metrics.conn_outbound.inc();
            metrics
                .conn_outbound_network
                .with_label_values(&[&o.conn.network.to_string()])
                .inc();
            metrics
                .conn_outbound_subnet
                .with_label_values(&[&util::subnet(ip)])
                .inc();
            metrics
                .conn_outbound_current
                .set(o.existing_connections as i64);
        }
        connection_event::Event::Closed(c) => {
            let ip = util::ip_from_ipport(c.conn.addr.clone());
            metrics.conn_closed.inc();
            metrics
                .conn_closed_age
                .inc_by(timestamp - c.time_established);
            metrics
                .conn_closed_network
                .with_label_values(&[&c.conn.network.to_string()])
                .inc();
            metrics
                .conn_closed_subnet
                .with_label_values(&[&util::subnet(ip)])
                .inc();
        }
        connection_event::Event::InboundEvicted(e) => {
            metrics.conn_evicted_inbound.inc();
            metrics
                .conn_evicted_inbound_withinfo
                .with_label_values(&[
                    &util::ip_from_ipport(e.conn.addr.clone()),
                    &e.conn.network.to_string(),
                ])
                .inc();
        }
        connection_event::Event::Misbehaving(m) => {
            metrics
                .conn_misbehaving
                .with_label_values(&[&m.id.to_string(), &m.message])
                .inc();
            metrics
                .conn_misbehaving_reason
                .with_label_values(&[&m.message])
                .inc();
        }
    }
}

fn handle_addrman_event(aevent: &addrman_event::Event, metrics: metrics::Metrics) {
    match aevent {
        addrman_event::Event::New(new) => {
            metrics
                .addrman_new_insert
                .with_label_values(&[&new.inserted.to_string()])
                .inc();
        }
        addrman_event::Event::Tried(_) => {
            metrics.addrman_tried_insert.inc();
        }
    }
}

fn handle_p2p_message(msg: &net_msg::Message, timestamp: u64, metrics: metrics::Metrics) {
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

    metrics.p2p_message_count.with(&labels).inc();
    metrics
        .p2p_message_bytes
        .with(&labels)
        .inc_by(msg.meta.size);

    if util::is_on_linkinglion_banlist(&ip) {
        let mut labels_ll = HashMap::<&str, &str>::new();
        labels_ll.insert(metrics::LABEL_P2P_MSG_TYPE, &msg.meta.command);
        labels_ll.insert(metrics::LABEL_P2P_DIRECTION, &direction);
        metrics.p2p_message_count_linkinglion.with(&labels_ll).inc();
        metrics
            .p2p_message_bytes_linkinglion
            .with(&labels_ll)
            .inc_by(msg.meta.size);
    }

    metrics
        .p2p_message_bytes_by_subnet
        .with_label_values(&[&direction, &subnet])
        .inc_by(msg.meta.size);
    metrics
        .p2p_message_count_by_subnet
        .with_label_values(&[&direction, &subnet])
        .inc();

    if let Some(msg_ref) = msg.msg.as_ref() {
        match msg_ref {
            Msg::Addr(addr) => {
                metrics
                    .p2p_addr_addresses
                    .with_label_values(&[&direction])
                    .observe(addr.addresses.len() as f64);
                let future_offset = metrics
                    .p2p_addr_timestamp_offset_seconds
                    .with_label_values(&[&direction, &"future".to_string()]);
                let past_offset = metrics
                    .p2p_addr_timestamp_offset_seconds
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
                            metrics
                                .p2p_addr_services_bits
                                .with_label_values(&[&direction])
                                .observe(i as f64)
                        }
                    }
                    metrics
                        .p2p_addr_services
                        .with_label_values(&[&direction, &address.services.to_string()])
                        .inc();
                }
            }
            Msg::Addrv2(addrv2) => {
                metrics
                    .p2p_addrv2_addresses
                    .with_label_values(&[&direction])
                    .observe(addrv2.addresses.len() as f64);
                let future_offset = metrics
                    .p2p_addrv2_timestamp_offset_seconds
                    .with_label_values(&[&direction, &"future".to_string()]);
                let past_offset = metrics
                    .p2p_addrv2_timestamp_offset_seconds
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
                            metrics
                                .p2p_addrv2_services_bits
                                .with_label_values(&[&direction])
                                .observe(i as f64)
                        }
                    }
                    metrics
                        .p2p_addrv2_services
                        .with_label_values(&[&direction, &address.services.to_string()])
                        .inc();
                }
            }
            Msg::Emptyaddrv2(_) => {
                metrics
                    .p2p_addrv2_empty
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
                    metrics
                        .p2p_inv_entries
                        .with_label_values(&[&direction, inv_type])
                        .inc_by(*count);
                }
                metrics
                    .p2p_inv_entries_histogram
                    .with_label_values(&[&direction])
                    .observe(inv.items.len() as f64);
                if count_by_invtype.len() == 1 {
                    metrics
                        .p2p_invs_homogeneous
                        .with_label_values(&[&direction])
                        .inc();
                } else {
                    metrics
                        .p2p_invs_heterogeneous
                        .with_label_values(&[&direction])
                        .inc();
                }
            }
            Msg::Ping(_) => {
                if msg.meta.inbound {
                    metrics.p2p_ping_subnet.with_label_values(&[&subnet]).inc();
                }
            }
            Msg::Oldping(_) => {
                if msg.meta.inbound {
                    metrics
                        .p2p_oldping_subnet
                        .with_label_values(&[&subnet])
                        .inc();
                }
            }
            Msg::Version(v) => {
                if msg.meta.inbound {
                    metrics
                        .p2p_version_subnet
                        .with_label_values(&[&subnet])
                        .inc();
                    metrics
                        .p2p_version_useragent
                        .with_label_values(&[&v.user_agent])
                        .inc();
                }
            }
            Msg::Feefilter(f) => {
                metrics
                    .p2p_feefilter_feerate
                    .with_label_values(&[&direction, &f.fee.to_string()])
                    .inc();
            }
            Msg::Reject(r) => {
                if msg.meta.inbound {
                    let reason = match RejectReason::try_from(r.reason) {
                        Ok(r) => r.to_string(),
                        Err(_) => "unknown".to_string(),
                    };
                    metrics
                        .p2p_reject_message
                        .with_label_values(&[&r.rejected_command, &reason])
                        .inc();
                }
            }
            _ => (),
        }
    }
}
