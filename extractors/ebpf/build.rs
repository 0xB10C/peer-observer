use libbpf_cargo::SkeletonBuilder;
use std::env;
use std::ffi::OsStr;
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
        .clang_args([OsStr::new("-I"), vmlinux_path.as_os_str()])
        .build_and_generate(DEST)
    {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to build BPF skeleton:");
            panic!("could not compile {}: {:?}", SOURCE, e);
        }
    }
    println!("cargo:rerun-if-changed={}", SOURCE);
}
