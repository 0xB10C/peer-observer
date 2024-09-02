#![cfg_attr(feature = "strict", deny(warnings))]

use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use shared::clap;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::prost::Message;
use shared::simple_logger;

/// Simple peer-observer tool that logs all received event messages
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // The extractor address the tool should connect to.
    #[arg(short, long, default_value = "tcp://127.0.0.1:8883")]
    address: String,
    // The log level the tool should run on. Events are logged with
    // the INFO log level. Valid log levels are "trace", "debug",
    // "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

fn main() {
    let args = Args::parse();

    simple_logger::init_with_level(args.log_level).unwrap();

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(&args.address).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    loop {
        let msg = sub.recv().unwrap();
        let unwrapped = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;

        if let Some(event) = unwrapped {
            match event {
                Event::Msg(msg) => {
                    log::info! {
                        "{} {} id={} (conn_type={:?}): {}",
                        if msg.meta.inbound { "<--"} else { "-->" },
                        if msg.meta.inbound { "from"} else { "to" },
                        msg.meta.peer_id,
                        msg.meta.conn_type,
                        msg.msg.unwrap(),
                    };
                }
                Event::Conn(c) => {
                    log::info! {
                        "# CONN {}", c.event.unwrap()
                    };
                }
                Event::Addrman(a) => {
                    log::info! {
                        "@Addrman {}", a.event.unwrap()
                    };
                }
                Event::Mempool(m) => {
                    log::info! {
                        "$Mempool {}", m.event.unwrap()
                    };
                }
                Event::Validation(v) => {
                    log::info! {
                        "+Validation {}", v.event.unwrap()
                    };
                }
            }
        }
    }
}
