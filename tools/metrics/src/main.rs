use metrics::Args;
use shared::log::error;
use shared::tokio;
use shared::tokio::sync::watch;
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("metrics tool error: {}", e);
    }

    // For now, the shutdown channel is only used in integration tests.
    let (_, shutdown_rx) = watch::channel(false);
    if let Err(e) = metrics::run(args, shutdown_rx).await {
        error!("metrics tool error: {}", e);
    };
}
