use metrics::Args;
use shared::log::error;
use shared::{clap::Parser, simple_logger};

fn main() {
    let args = Args::parse();

    if let Err(e) = simple_logger::init_with_level(args.log_level) {
        eprintln!("metrics tool error: {}", e);
    }

    if let Err(e) = metrics::run(args) {
        error!("metrics tool error: {}", e);
    };
}
