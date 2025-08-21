use rpc_extractor::Args;
use shared::log::error;
use shared::tokio;
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("rpc extractor error: {}", e);
    }

    if let Err(e) = rpc_extractor::run(args).await {
        error!("rpc extractor error: {}", e);
    }
}
