use logger::Args;
use shared::log;
use shared::tokio::{self, signal, sync::watch};
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("logger tool error: {}", e);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let logger_handle = tokio::spawn(logger::run(args, shutdown_rx));

    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Received Ctrl+C. Stopping...");
            let _ = shutdown_tx.send(true);
        }
        result = logger_handle => {
            match result.unwrap() {
                Ok(_) => log::info!("logger task completed."),
                Err(e) => log::error!("logger task failed: {e}"),
            }
        }
    }
}
