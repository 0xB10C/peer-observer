use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use prost::Message;
use shared::connection::connection_event::Event;
use shared::p2p;
use shared::p2p::reject::RejectReason;
use shared::wrapper;
use shared::wrapper::wrapper::Wrap;

use simple_logger::SimpleLogger;

use std::env;
use std::time;

use std::collections::HashMap;

mod metrics;
mod metricserver;

const LOG_TARGET: &str = "main";
const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let metricserver_address = env::args()
        .nth(1)
        .expect("No metric server address to bind on provided (.e.g. 'localhost:8282').");

    SimpleLogger::new()
        .init()
        .expect("Could not setup logging.");

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
        let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap().wrap;

        if let Some(event) = unwrapped {
            match event {
                Wrap::Msg(msg) => {
                    handle_p2p_message(&msg);
                }
                Wrap::Conn(c) => match c.event.unwrap() {
                    Event::Inbound(i) => {
                        metrics::CONN_INBOUND.inc();
                        metrics::CONN_INBOUND_ADDRESS
                            .with_label_values(&[&i.conn.addr.split(":").next().unwrap_or("")])
                            .inc();
                        metrics::CONN_INBOUND_NETWORK
                            .with_label_values(&[&i.conn.network.to_string()])
                            .inc();
                        metrics::CONN_INBOUND_NETGROUP
                            .with_label_values(&[&i.conn.net_group.to_string()])
                            .inc();
                        metrics::CONN_INBOUND_SERVICE
                            .with_label_values(&[&i.services.to_string()])
                            .inc();
                        metrics::CONN_INBOUND_CURRENT.set(i.existing_connections as i64 + 1);
                    }
                    Event::Outbound(o) => {
                        metrics::CONN_OUTBOUND.inc();
                        metrics::CONN_OUTBOUND_NETWORK
                            .with_label_values(&[&o.conn.network.to_string()])
                            .inc();
                        metrics::CONN_OUTBOUND_NETGROUP
                            .with_label_values(&[&o.conn.net_group.to_string()])
                            .inc();
                        metrics::CONN_OUTBOUND_CURRENT.set(o.existing_connections as i64 + 1);
                    }
                    Event::Closed(c) => {
                        metrics::CONN_CLOSED.inc();
                        metrics::CONN_CLOSED_ADDRESS
                            .with_label_values(&[&c.conn.addr.split(":").next().unwrap_or("")])
                            .inc();
                        metrics::CONN_CLOSED_NETWORK
                            .with_label_values(&[&c.conn.network.to_string()])
                            .inc();
                        metrics::CONN_CLOSED_NETGROUP
                            .with_label_values(&[&c.conn.net_group.to_string()])
                            .inc();
                    }
                    Event::Evicted(e) => {
                        metrics::CONN_EVICTED.inc();
                        metrics::CONN_EVICTED_WITHINFO
                            .with_label_values(&[
                                &e.conn.addr.split(":").next().unwrap_or(""),
                                &e.conn.network.to_string(),
                            ])
                            .inc();
                    }
                    Event::Misbehaving(m) => {
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
                },
            }
        }
    }

    fn handle_p2p_message(msg: &p2p::Message) {
        let conn_type = msg.meta.conn_type.to_string();
        let direction = if msg.meta.inbound {
            "inbound"
        } else {
            "outbound"
        };
        let mut labels = HashMap::<&str, &str>::new();
        labels.insert(metrics::LABEL_P2P_MSG_TYPE, &msg.meta.command);
        labels.insert(metrics::LABEL_P2P_CONNECTION_TYPE, &conn_type);
        labels.insert(metrics::LABEL_P2P_DIRECTION, &direction);

        metrics::P2P_MESSAGE_COUNT.with(&labels).inc();
        metrics::P2P_MESSAGE_BYTES
            .with(&labels)
            .inc_by(msg.meta.size);

        match msg.msg.as_ref().unwrap() {
            shared::p2p::message::Msg::Addr(addr) => {
                metrics::P2P_ADDR_ADDRESS_HISTOGRAM
                    .with_label_values(&[direction])
                    .observe(addr.addresses.len() as f64);
                let future_offset = metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[direction, "future"]);
                let past_offset = metrics::P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[direction, "past"]);
                for address in addr.addresses.iter() {
                    // We substract the timestamp in the address from the time we received the
                    // message. If the remaining offset is larger than or equal to zero, the address
                    // timestamp lies in the past. If the offset is smaller than zero, the address
                    // timestamp lies in the future.
                    let offset = msg.meta.timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64);
                    } else {
                        future_offset.observe((offset * -1) as f64);
                    }
                    for i in 0..32 {
                        if (1 << i) & address.services > 0 {
                            metrics::P2P_ADDR_SERVICES_HISTOGRAM
                                .with_label_values(&[direction])
                                .observe(i as f64)
                        }
                    }
                    metrics::P2P_ADDR_SERVICES
                        .with_label_values(&[direction, &address.services.to_string()])
                        .inc();
                }
            }
            shared::p2p::message::Msg::Addrv2(addrv2) => {
                metrics::P2P_ADDRV2_ADDRESS_HISTOGRAM
                    .with_label_values(&[direction])
                    .observe(addrv2.addresses.len() as f64);
                let future_offset = metrics::P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[direction, "future"]);
                let past_offset = metrics::P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM
                    .with_label_values(&[direction, "past"]);
                for address in addrv2.addresses.iter() {
                    // We substract the timestamp in the address from the time we received the
                    // message. If the remaining offset is larger than or equal to zero, the address
                    // timestamp lies in the past. If the offset is smaller than zero, the address
                    // timestamp lies in the future.
                    let offset = msg.meta.timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64);
                    } else {
                        future_offset.observe((offset * -1) as f64);
                    }

                    for i in 0..32 {
                        if (1 << i) & address.services > 0 {
                            metrics::P2P_ADDRV2_SERVICES_HISTOGRAM
                                .with_label_values(&[direction])
                                .observe(i as f64)
                        }
                    }
                    metrics::P2P_ADDRV2_SERVICES
                        .with_label_values(&[direction, &address.services.to_string()])
                        .inc();
                }
            }
            shared::p2p::message::Msg::Inv(inv) => {
                let mut count_by_invtype: HashMap<String, u64> = HashMap::new();
                for item in inv.items.iter() {
                    let count = count_by_invtype
                        .entry(String::from((*item).inv_type()))
                        .or_insert(0);
                    *count += 1;
                }
                for (inv_type, count) in &count_by_invtype {
                    metrics::P2P_INV_ENTRIES
                        .with_label_values(&[direction, inv_type])
                        .inc_by(*count);
                }
                metrics::P2P_INV_ENTRIES_HISTOGRAM
                    .with_label_values(&[direction])
                    .observe(inv.items.len() as f64);
                if count_by_invtype.len() == 1 {
                    metrics::P2P_INV_ENTRIES_HOMOGENOUS
                        .with_label_values(&[direction])
                        .inc();
                } else {
                    metrics::P2P_INV_ENTRIES_HETEROGENEOUS
                        .with_label_values(&[direction])
                        .inc();
                }
            }
            shared::p2p::message::Msg::Ping(_) => {
                if msg.meta.inbound {
                    metrics::P2P_PING_ADDRESS
                        .with_label_values(&[&msg.meta.addr.split(":").next().unwrap_or("")])
                        .inc();
                }
            }
            shared::p2p::message::Msg::Version(v) => {
                if msg.meta.inbound {
                    metrics::P2P_VERSION_ADDRESS
                        .with_label_values(&[&msg.meta.addr.split(":").next().unwrap_or("")])
                        .inc();
                    metrics::P2P_VERSION_USERAGENT
                        .with_label_values(&[&v.user_agent])
                        .inc();
                }
            }
            shared::p2p::message::Msg::Feefilter(f) => {
                metrics::P2P_FEEFILTER_FEERATE
                    .with_label_values(&[direction, &f.fee.to_string()])
                    .inc();
            }
            shared::p2p::message::Msg::Reject(r) => {
                if msg.meta.inbound {
                    metrics::P2P_REJECT_ADDR
                        .with_label_values(&[
                            &msg.meta.addr.split(":").next().unwrap_or(""),
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
