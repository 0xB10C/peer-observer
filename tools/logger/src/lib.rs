#![cfg_attr(feature = "strict", deny(warnings))]

use shared::clap::Parser;
use shared::futures::stream::StreamExt;
use shared::log;
use shared::prost::Message;
use shared::protobuf::event_msg::event_msg::Event;
use shared::protobuf::event_msg::{self, EventMsg};
use shared::tokio::sync::watch;
use shared::{async_nats, clap};

// Note: when modifying this struct, make sure to also update the usage
// instructions in the README of this tool.
/// A peer-observer tool that logs all received event messages.
/// By default, all events are shown. This can be a lot. Events can be
/// filtered by type. For example, `--messages` only shows P2P messages.
/// Using `--messages --connections` together shows P2P messages and connections.
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The NATS server address the tool should connect and subscribe to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    pub nats_address: String,
    /// The log level the tool should run on. Events are logged with
    /// the INFO log level. Valid log levels are "trace", "debug",
    /// "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    pub log_level: log::Level,

    /// If passed, show P2P message events
    #[arg(long)]
    pub messages: bool,

    /// If passed, show P2P connection events
    #[arg(long)]
    pub connections: bool,

    /// If passed, show addrman events
    #[arg(long)]
    pub addrman: bool,

    /// If passed, show mempool events
    #[arg(long)]
    pub mempool: bool,

    /// If passed, show validation events
    #[arg(long)]
    pub validation: bool,

    /// If passed, show RPC events
    #[arg(long)]
    pub rpc: bool,

    /// If passed, show p2p-extractor events
    #[arg(long)]
    pub p2p_extractor: bool,
}

impl Args {
    pub fn should_show_all(&self) -> bool {
        !(self.messages
            || self.connections
            || self.addrman
            || self.mempool
            || self.validation
            || self.rpc
            || self.p2p_extractor)
    }
}

pub async fn run(args: Args, mut shutdown_rx: watch::Receiver<bool>) {
    // TODO: handle unwraps
    let nc = async_nats::connect(args.nats_address.clone())
        .await
        .unwrap();
    let mut sub = nc.subscribe("*").await.unwrap();
    loop {
        shared::tokio::select! {
            maybe_msg = sub.next() => {
                if let Some(msg) = maybe_msg {
                    match event_msg::EventMsg::decode(msg.payload) {
                        Ok(event) =>  {
                            log_event(event, args.clone());
                        },
                        Err(e) => {
                            log::warn!("Could not decode protobuf message as EventMsg: {}", e);
                        }
                    }
                } else {
                    break; // subscription ended
                }
            }
            res = shutdown_rx.changed() => {
                match res {
                    Ok(_) => {
                        if *shutdown_rx.borrow() {
                            log::info!("logger tool received shutdown signal.");
                            break;
                        }
                    }
                    Err(_) => {
                        // all senders dropped -> treat as shutdown
                        log::warn!("The shutdown notification sender was dropped. Shutting down.");
                        break;
                    }
                }
            }
        }
    }
}

fn log_event(event_msg: EventMsg, args: Args) {
    let log_all = args.should_show_all();
    match event_msg.event.unwrap() {
        Event::Msg(msg) => {
            if log_all || args.messages {
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
            if log_all || args.connections {
                log::info!("# CONN {}", c.event.unwrap());
            }
        }
        Event::Addrman(a) => {
            if log_all || args.addrman {
                log::info!("@Addrman {}", a.event.unwrap());
            }
        }
        Event::Mempool(m) => {
            if log_all || args.mempool {
                log::info!("$Mempool {}", m.event.unwrap());
            }
        }
        Event::Validation(v) => {
            if log_all || args.validation {
                log::info!("+Validation {}", v.event.unwrap());
            }
        }
        Event::Rpc(r) => {
            if log_all || args.rpc {
                log::info!("!RPC {}", r.event.unwrap());
            }
        }
        Event::P2pExtractorEvent(p) => {
            if log_all || args.p2p_extractor {
                log::info!("%P2P {}", p.event.unwrap());
            }
        }
    }
}
