use libbpf_cargo::SkeletonBuilder;
use std::env;
use std::path::Path;

const SOURCE: &str = "./src/bpf/tracing.bpf.c";
const DEST: &str = "./src/tracing.gen.rs";

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("CARGO_CFG_TARGET_ARCH must be set in build script");
    let vmlinux_path = Path::new("./src/bpf/vmlinux").join(arch);
    println!("using vmlinux path: {}", vmlinux_path.display());
    match SkeletonBuilder::new()
        .source(SOURCE)
        .clang_args(format!("-I {}", vmlinux_path.display()))
        .build_and_generate(DEST)
    {
        Ok(_) => (),
        Err(e) => {
            println!("{}", e);
            panic!("could not compile {}", SOURCE);
        }
    }
    println!("cargo:rerun-if-changed={}", SOURCE);
}
