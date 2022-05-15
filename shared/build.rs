use prost_build;

fn main() {
    if let Err(e) = prost_build::compile_protos(
        &[
            "./../protobuf/protos/wrapper.proto",
            "./../protobuf/protos/primitive.proto",
            "./../protobuf/protos/p2p.proto",
            "./../protobuf/protos/connection.proto",
        ],
        &["./../protobuf/protos/"],
    ) {
        println!("Error while compiling protos: {}", e);
        panic!("Failed to code-gen the Rust structs from the Protobuf definitions");
    }
}
