use p2p_extractor::Args;
use shared::log::error;
use shared::tokio::{self, signal, sync::watch};
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("p2p extractor error: {}", e);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let p2p_handle = tokio::spawn(p2p_extractor::run(args, shutdown_rx));

    signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    let _ = shutdown_tx.send(true);

    if let Err(e) = p2p_handle.await {
        error!("p2p_extractor task failed: {:?}", e);
    }
}
