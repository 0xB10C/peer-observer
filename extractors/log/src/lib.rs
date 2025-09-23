use shared::clap::{Parser};
use shared::{log};
use shared::tokio::sync::watch;
use shared::{clap};
use shared::tokio::fs::File;
use shared::tokio::io::{AsyncBufReadExt, BufReader};

mod error;

use error::{RuntimeError};

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
    // log::debug!("Connecting to NATS server at {}..", args.nats_address);
    // let nats_client = async_nats::connect(&args.nats_address).await?;
    // log::info!("Connected to NATS server at {}", &args.nats_address);

    log::info!("Opening bitcoind log pipe...");
    let file = File::open("bitcoind_pipe").await?;
    log::info!("Opened bitcoind log pipe");
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    loop {
        shared::tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => println!("{}", line),
                    Ok(None) => {
                        log::info!("End of log pipe reached.");
                        break;
                    }
                    Err(e) => return Err(e.into()),
                }
            },
            res = shutdown_rx.changed() => {
                match res {
                    Ok(_) => {
                        if *shutdown_rx.borrow() {
                            log::info!("log-extractor received shutdown signal.");
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

    Ok(())
}
