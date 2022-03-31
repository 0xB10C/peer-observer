use prost_build;

fn main() {
    prost_build::compile_protos(&["./src/p2p.proto"], &["./src/"]).unwrap();
}
