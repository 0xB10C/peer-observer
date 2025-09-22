use shared::clap::{ArgGroup, Parser};
use shared::corepc_client::client_sync::Auth;
use shared::corepc_client::client_sync::v29::Client;
use shared::log;
use shared::nats_subjects::Subject;
use shared::prost::Message;
use shared::protobuf::event_msg::EventMsg;
use shared::protobuf::event_msg::event_msg::Event;
use shared::tokio::sync::watch;
use shared::tokio::time::{self, Duration};
use shared::{async_nats, clap};

mod error;

use error::{FetchOrPublishError, RuntimeError};

/// TODO: Update
/// The peer-observer log-extractor periodically queries data from the
/// Bitcoin Core Log endpoint and publishes the results as events into
/// a NATS pub-sub queue.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Address of the NATS server where the extractor will publish messages to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    pub nats_address: String,

    /// The log level the extractor should run with. Valid log levels are "trace",
    /// "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html.
    #[arg(short, long, default_value_t = log::Level::Debug)]
    pub log_level: log::Level,
}

impl Args {
    pub fn new(
        nats_address: String,
        log_level: log::Level,
    ) -> Args {
        Self {
            nats_address,
            log_level,
        }
    }
}

pub async fn run(args: Args, mut shutdown_rx: watch::Receiver<bool>) -> Result<(), RuntimeError> {
    Ok(())
}
