use metrics::Args;
use shared::clap::Parser;

fn main() {
    let args = Args::parse();
    metrics::run(args);
}
