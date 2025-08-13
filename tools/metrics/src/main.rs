use metrics::Args;
use shared::clap::Parser;

fn main() {
    let args = Args::parse();
    if let Err(e) = metrics::run(args) {
        eprintln!("metrics tool error: {}", e);
    };
}
