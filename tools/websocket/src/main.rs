use shared::log;
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

    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Received Ctrl+C. Stopping...");
            let _ = shutdown_tx.send(true);
        }
        result = websocket_handle => {
            match result.unwrap() {
                Ok(_) => log::info!("websocket task completed."),
                Err(e) => log::error!("websocket task failed: {e}"),
            }
        }
    }
}
