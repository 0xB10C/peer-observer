#![cfg_attr(feature = "strict", deny(warnings))]

use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::prost::Message;
use shared::simple_logger;
use shared::{clap, nats};

/// A peer-observer tool that logs all received event messages
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The NATS server address the tool should connect and subscribe to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    nats_address: String,
    /// The log level the tool should run on. Events are logged with
    /// the INFO log level. Valid log levels are "trace", "debug",
    /// "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

fn main() {
    let args = Args::parse();

    simple_logger::init_with_level(args.log_level).unwrap();

    // TODO: handle unwraps
    let nc = nats::connect(args.nats_address).unwrap();
    let sub = nc.subscribe("*").unwrap();
    for msg in sub.messages() {
        let unwrapped = event_msg::EventMsg::decode(msg.data.as_slice())
            .unwrap()
            .event;

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
