use shared::log::error;
use shared::tokio::{self, signal, sync::watch};
use shared::{clap::Parser, simple_logger};
use websocket::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("websocket tool error: {}", e);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let websocket_handle = tokio::spawn(websocket::run(args, shutdown_rx));

    signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    let _ = shutdown_tx.send(true);

    if let Err(e) = websocket_handle.await {
        error!("websocket task failed: {:?}", e);
    }
}
