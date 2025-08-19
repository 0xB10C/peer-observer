use shared::log::error;
use shared::tokio;
use shared::tokio::sync::watch;
use shared::{clap::Parser, simple_logger};
use websocket::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("websocket tool error: {}", e);
    }

    // For now, the shutdown channel is only used in integration tests.
    let (_, shutdown_rx) = watch::channel(false);
    if let Err(e) = websocket::run(args, shutdown_rx).await {
        error!("websocket tool error: {}", e);
    };
}
