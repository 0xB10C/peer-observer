use rpc_extractor::Args;
use shared::log::error;
use shared::tokio::{self, signal, sync::watch};
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("rpc extractor error: {}", e);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let rpc_handle = tokio::spawn(rpc_extractor::run(args, shutdown_rx));

    signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    let _ = shutdown_tx.send(true);

    if let Err(e) = rpc_handle.await {
        error!("rpc_extractor task failed: {:?}", e);
    }
}
