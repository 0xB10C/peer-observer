#![cfg_attr(feature = "strict", deny(warnings))]

use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use prost::Message;
use shared::wrapper;
use shared::wrapper::wrapper::Wrap;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    loop {
        let msg = sub.recv().unwrap();
        let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap().wrap;

        if let Some(event) = unwrapped {
            match event {
                Wrap::Msg(msg) => {
                    println! {
                        "{} {} id={} (conn_type={:?}): {}",
                        if msg.meta.inbound { "<--"} else { "-->" },
                        if msg.meta.inbound { "from"} else { "to" },
                        msg.meta.peer_id,
                        msg.meta.conn_type,
                        msg.msg.unwrap(),
                    };
                }
                Wrap::Conn(c) => {
                    println! {
                        "# CONN {}", c.event.unwrap()
                    };
                }
                Wrap::Fntime(t) => {
                    println! {
                        "X {}", t.function.unwrap()
                    };
                }
            }
        }
    }
}
