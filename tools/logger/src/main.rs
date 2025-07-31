#![cfg_attr(feature = "strict", deny(warnings))]

use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::prost::Message;
use shared::simple_logger;
use shared::{clap, nats};

// Note: when modifying this struct, make sure to also update the usage
// instructions in the README of this tool.
/// A peer-observer tool that logs all received event messages.
/// By default, all events are shown. This can be a lot. Events can be
/// filtered by type. For example, `--messages` only shows P2P messages.
/// Using `--messages --connections` together shows P2P messages and connections.
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

    /// If passed, show P2P message events
    #[arg(long)]
    messages: bool,

    /// If passed, show P2P connection events
    #[arg(long)]
    connections: bool,

    /// If passed, show addrman events
    #[arg(long)]
    addrman: bool,

    /// If passed, show mempool events
    #[arg(long)]
    mempool: bool,

    /// If passed, show validation events
    #[arg(long)]
    validation: bool,

    /// If passed, show RPC events
    #[arg(long)]
    rpc: bool,
}

impl Args {
    fn should_show_all(&self) -> bool {
        !(self.messages
            || self.connections
            || self.addrman
            || self.mempool
            || self.validation
            || self.rpc)
    }
}

fn main() {
    //Add early binding of filter flags
    let args = Args::parse();
    let should_show_all = args.should_show_all();
    let messages = args.messages;
    let conn = args.connections;
    let addrman = args.addrman;
    let mempool = args.mempool;
    let validation = args.validation;
    let rpc = args.rpc;
    simple_logger::init_with_level(args.log_level).unwrap();

    // TODO: handle unwraps
    let nc = nats::connect(args.nats_address).unwrap();
    let sub = nc.subscribe("*").unwrap();
    for msg in sub.messages() {
        if let Ok(event_msg) = event_msg::EventMsg::decode(msg.data.as_slice()) {
            if let Some(event) = event_msg.event {
                match event {
                    Event::Msg(msg) => {
                        if should_show_all || messages {
                            log::info!(
                                "{} {} id={} (conn_type={:?}): {}",
                                if msg.meta.inbound { "<--" } else { "-->" },
                                if msg.meta.inbound { "from" } else { "to" },
                                msg.meta.peer_id,
                                msg.meta.conn_type,
                                msg.msg.unwrap()
                            );
                        }
                    }
                    Event::Conn(c) => {
                        if should_show_all || conn {
                            log::info!("# CONN {}", c.event.unwrap());
                        }
                    }
                    Event::Addrman(a) => {
                        if should_show_all || addrman {
                            log::info!("@Addrman {}", a.event.unwrap());
                        }
                    }
                    Event::Mempool(m) => {
                        if should_show_all || mempool {
                            log::info!("$Mempool {}", m.event.unwrap());
                        }
                    }
                    Event::Validation(v) => {
                        if should_show_all || validation {
                            log::info!("+Validation {}", v.event.unwrap());
                        }
                    }
                    Event::Rpc(r) => {
                        if should_show_all || rpc {
                            log::info!("!RPC {}", r.event.unwrap());
                        }
                    }
                }
            }
        }
    }
}
