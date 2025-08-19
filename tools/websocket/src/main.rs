use shared::log::error;
use shared::tokio;
use shared::{clap::Parser, simple_logger};
use websocket::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("websocket tool error: {}", e);
    }

    if let Err(e) = websocket::run(args).await {
        error!("websocket tool error: {}", e);
    };
}
