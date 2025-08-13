use metrics::Args;
use shared::log::error;
use shared::tokio;
use shared::{clap::Parser, simple_logger};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("metrics tool error: {}", e);
    }

    if let Err(e) = metrics::run(args).await {
        error!("metrics tool error: {}", e);
    };
}
