use corepc_client::client_sync::v29::Client;
use corepc_client::client_sync::Auth;
use shared::prost::Message;
use shared::clap::Parser;
use shared::event_msg::event_msg::Event;
use shared::event_msg::EventMsg;
use shared::log::{self, error};
use shared::simple_logger;
use shared::{addrman, mempool, nats_subjects::Subject, net_conn, net_msg, validation};
use shared::{clap, nats};

/// The peer-observer rpc-extractor periodically queries data from the
/// Bitcoin Core RPC endpoint and publishes the results as events into
/// a NATS pub-sub queue.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Address of the NATS server where the extractor will publish messages to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    nats_address: String,

    /// The log level the extractor should run with. Valid log levels are "trace",
    /// "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,

    // TODO: RPC host, RPC port, RPC password, RPC user, or cookiefile
    // TODO: query interval
}

fn main() {
    let auth = Auth::CookieFile("/home/user/.bitcoin/.cookie".into());

    let client = Client::new_with_auth("http://localhost:8332", auth).expect("client");

    println!("Hello, world!");

    let peerinfo = client.get_peer_info().expect("peer-info");

    println!("peer info {:?}", peerinfo);
}
