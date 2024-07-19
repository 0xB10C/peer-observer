#![cfg_attr(feature = "strict", deny(warnings))]

use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::prost::Message;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    loop {
        let msg = sub.recv().unwrap();
        let unwrapped = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;

        if let Some(event) = unwrapped {
            match event {
                Event::Msg(msg) => {
                    println! {
                        "{} {} id={} (conn_type={:?}): {}",
                        if msg.meta.inbound { "<--"} else { "-->" },
                        if msg.meta.inbound { "from"} else { "to" },
                        msg.meta.peer_id,
                        msg.meta.conn_type,
                        msg.msg.unwrap(),
                    };
                }
                Event::Conn(c) => {
                    println! {
                        "# CONN {}", c.event.unwrap()
                    };
                }
                Event::Addrman(a) => {
                    println! {
                        "@Addrman {}", a.event.unwrap()
                    };
                }
                Event::Mempool(m) => {
                    println! {
                        "$Mempool {}", m.event.unwrap()
                    };
                }
                Event::Validation(v) => {
                    println! {
                        "+Validation {}", v.event.unwrap()
                    };
                }
            }
        }
    }
}
