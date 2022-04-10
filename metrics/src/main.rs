use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use prost::Message;
use shared::p2p;

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

    //SimpleLogger::new()
    //    .init()
    //    .expect("Could not setup logging.");

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
        let protobuf = p2p::Message::decode(msg.as_slice()).unwrap();
        println! {
            "{} from id={} (conn_type={:?}): {}",
            if protobuf.meta.inbound { "<--"} else { "-->" },
            protobuf.meta.peer_id,
            protobuf.meta.conn_type,
            protobuf.msg.as_ref().unwrap(),
        };

        let conn_type = protobuf.meta.conn_type.to_string();
        let direction = if protobuf.meta.inbound {
            "inbound"
        } else {
            "outbound"
        };
        let mut labels = HashMap::<&str, &str>::new();
        labels.insert(metrics::LABEL_P2P_MSG_TYPE, &protobuf.meta.command);
        labels.insert(metrics::LABEL_P2P_CONNECTION_TYPE, &conn_type);
        labels.insert(metrics::LABEL_P2P_DIRECTION, &direction);

        metrics::P2P_MESSAGE_COUNT.with(&labels).inc();
        metrics::P2P_MESSAGE_BYTES
            .with(&labels)
            .inc_by(protobuf.meta.size);

        match protobuf.msg.as_ref().unwrap() {
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
                    let offset = protobuf.meta.timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64)
                    } else {
                        future_offset.observe((offset * -1) as f64)
                    }
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
                    let offset = protobuf.meta.timestamp as i64 - address.timestamp as i64;
                    if offset >= 0 {
                        past_offset.observe(offset as f64)
                    } else {
                        future_offset.observe((offset * -1) as f64)
                    }
                }
            }
            _ => (),
        }
    }
}
