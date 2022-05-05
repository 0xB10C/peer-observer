use prost_build;

fn main() {
    if let Err(e) = prost_build::compile_protos(
        &[
            "./src/wrapper.proto",
            "./src/primitive.proto",
            "./src/p2p.proto",
            "./src/connection.proto",
        ],
        &["./src/"],
    ) {
        println!("Error while compiling protos: {}", e);
        panic!("Failed to code-gen the Rust structs from the Protobuf definitions");
    }
}
