use prost_build;

fn main() {
    if let Err(e) = prost_build::compile_protos(
        &[
            "../protobuf/proto-types/wrapper.proto",
            "../protobuf/proto-types/primitive.proto",
            "../protobuf/proto-types/net_msg.proto",
            "../protobuf/proto-types/net_conn.proto",
        ],
        &["../protobuf/proto-types/"],
    ) {
        println!("Error while compiling protos: {}", e);
        panic!("Failed to code-gen the Rust structs from the Protobuf definitions");
    }
}
