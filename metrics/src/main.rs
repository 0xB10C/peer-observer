#![cfg_attr(feature = "strict", deny(warnings))]

use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use prost::Message;
use shared::addrman::addrman_event;
use shared::net_conn::connection_event;
use shared::net_msg;
use shared::net_msg::{message::Msg, reject::RejectReason};
use shared::wrapper;
use shared::wrapper::wrapper::Wrap;

use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time;

mod metrics;
mod metricserver;

const LOG_TARGET: &str = "main";
const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let metricserver_address = env::args()
        .nth(1)
        .expect("No metric server address to bind on provided (.e.g. 'localhost:8282').");

    log::info!(target: LOG_TARGET, "Starting metrics-server using...",);

    metrics::RUNTIME_START_TIMESTAMP.set(
        time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    );

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    metricserver::start(&metricserver_address).unwrap();

    log::info!(target: LOG_TARGET, "Started metrics-server.");

    loop {
        let msg = sub.recv().unwrap();
        let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap();

        if let Some(event) = unwrapped.wrap {
            match event {
                Wrap::Msg(msg) => {
                    handle_p2p_message(&msg, unwrapped.timestamp);
                }
                Wrap::Conn(c) => {
                    handle_connection_event(c.event.unwrap());
                }
                Wrap::Addrman(a) => {
                    handle_addrman_event(&a.event.unwrap());
                }
            }
        }
    }

    fn handle_connection_event(cevent: connection_event::Event) {
        match cevent {
            connection_event::Event::Inbound(i) => {
                let ip = ip(i.conn.addr);
                metrics::CONN_INBOUND.inc();
                if utils::torexitips::is_tor_exit_node(&ip) {
                    metrics::CONN_INBOUND_TOR_EXIT.inc();
                }
                metrics::CONN_INBOUND_ADDRESS
                    .with_label_values(&[&ip])
                    .inc();
                metrics::CONN_INBOUND_SUBNET
                    .with_label_values(&[&subnet_24_or_64_or_ip(ip)])
                    .inc();
                metrics::CONN_INBOUND_NETWORK
                    .with_label_values(&[&i.conn.network.to_string()])
                    .inc();
                metrics::CONN_INBOUND_NETGROUP
                    .with_label_values(&[&i.conn.net_group.to_string()])
                    .inc();
                metrics::CONN_INBOUND_CURRENT.set(i.existing_connections as i64 + 1);
            }
            connection_event::Event::Outbound(o) => {
                metrics::CONN_OUTBOUND.inc();
                metrics::CONN_OUTBOUND_NETWORK
                    .with_label_values(&[&o.conn.network.to_string()])
                    .inc();
                metrics::CONN_OUTBOUND_NETGROUP
                    .with_label_values(&[&o.conn.net_group.to_string()])
                    .inc();
                metrics::CONN_OUTBOUND_CURRENT.set(o.existing_connections as i64 + 1);
            }
            connection_event::Event::Closed(c) => {
                metrics::CONN_CLOSED.inc();
                metrics::CONN_CLOSED_ADDRESS
                    .with_label_values(&[&ip(c.conn.addr)])
                    .inc();
                metrics::CONN_CLOSED_NETWORK
                    .with_label_values(&[&c.conn.network.to_string()])
                    .inc();
                metrics::CONN_CLOSED_NETGROUP
                    .with_label_values(&[&c.conn.net_group.to_string()])
                    .inc();
            }
            connection_event::Event::Evicted(e) => {
                metrics::CONN_EVICTED.inc();
                metrics::CONN_EVICTED_WITHINFO
                    .with_label_values(&[&ip(e.conn.addr), &e.conn.network.to_string()])
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
            "inbound"
        } else {
            "outbound"
        };
        let ip = ip(msg.meta.addr.clone());
        let subnet = subnet_24_or_64_or_ip(ip.clone());
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
                    .with_label_values(&[&direction, "future"]);
                let past_offset = metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, "past"]);
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
                    .with_label_values(&[&direction, "future"]);
                let past_offset = metrics::P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[&direction, "past"]);
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
                    metrics::P2P_PING_ADDRESS.with_label_values(&[&ip]).inc();
                    metrics::P2P_PING_SUBNET
                        .with_label_values(&[&subnet_24_or_64_or_ip(ip)])
                        .inc();
                }
            }
            Msg::Oldping(_) => {
                println!("old ping");
                if msg.meta.inbound {
                    println!("inbound old ping");
                    metrics::P2P_OLDPING_ADDRESS.with_label_values(&[&ip]).inc();
                }
            }
            Msg::Version(v) => {
                if msg.meta.inbound {
                    metrics::P2P_VERSION_ADDRESS.with_label_values(&[&ip]).inc();
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
                    metrics::P2P_REJECT_ADDR
                        .with_label_values(&[
                            &ip,
                            &r.rejected_command,
                            &RejectReason::from_i32(r.reason).unwrap().to_string(),
                        ])
                        .inc();
                    metrics::P2P_REJECT_MESSAGE
                        .with_label_values(&[
                            &r.rejected_command,
                            &RejectReason::from_i32(r.reason).unwrap().to_string(),
                            &r.reason_details,
                        ])
                        .inc();
                }
            }
            _ => (),
        }
    }
}

/// Split and return the IP from an ip:port combination.
pub fn ip(addr: String) -> String {
    match addr.rsplit_once(":") {
        Some((ip, _)) => ip.replace("[", "").replace("]", "").to_string(),
        None => addr,
    }
}

/// Returns the /24 subnet for IPv4 or the /64 subnet for IPv6 address.
/// If [ip] is not a valid IPv4 or IPv6 address, the original ip is returned.
/// This is the case for Tor and I2P addresses.
/// TODO: make sure this works for CJDNS IPs too.
pub fn subnet_24_or_64_or_ip(ip: String) -> String {
    let cleaned_ip = ip.replace("[", "").replace("]", "");
    if let Ok(ip_addr) = cleaned_ip.parse() {
        match ip_addr {
            IpAddr::V4(a) => {
                let o = a.octets();
                return Ipv4Addr::new(o[0], o[1], o[2], 0).to_string();
            }
            IpAddr::V6(a) => {
                let s = a.segments();
                return Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0).to_string();
            }
        }
    }
    return ip;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_ip() {
        assert_eq!(ip(String::from("127.0.0.1:8333")).as_str(), "127.0.0.1");
        assert_eq!(ip(String::from("::1:8333")).as_str(), "::1");
        assert_eq!(
            ip(String::from("::ffff:a.b.c.d:8333")).as_str(),
            "::ffff:a.b.c.d"
        );
        assert_eq!(
            ip(String::from("[::ffff:a.b.c.d]:8333")).as_str(),
            "::ffff:a.b.c.d"
        );
    }

    #[test]
    fn test_subnet_24_or_64_or_ip() {
        assert_eq!(
            subnet_24_or_64_or_ip(String::from("127.0.0.1")).as_str(),
            "127.0.0.0"
        );
        assert_eq!(
            subnet_24_or_64_or_ip(String::from("2604:d500:4:1::3:a2")).as_str(),
            "2604:d500:4:1::"
        );
        assert_eq!(
            subnet_24_or_64_or_ip(String::from("[2604:d500:4:1::3:a2]")).as_str(),
            "2604:d500:4:1::"
        );
    }
}
