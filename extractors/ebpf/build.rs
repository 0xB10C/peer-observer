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

    let mut binding = SkeletonBuilder::new();
    let mut builder = binding
        .source(SOURCE)
        .clang_args([OsStr::new("-I"), vmlinux_path.as_os_str()]);

    // If KERNEL_HEADERS env var is set, add it as an include path
    if let Ok(kernel_headers) = env::var("KERNEL_HEADERS") {
        println!(
            "using kernel headers path from KERNEL_HEADERS: {}",
            kernel_headers
        );
        builder = builder.clang_args([
            OsStr::new("-I"),
            vmlinux_path.as_os_str(),
            // don't include any standard includes; this helps in the Ubuntu CI
            // when building the package with Nix
            OsStr::new("-nostdinc"),
            OsStr::new("-v"),
            OsStr::new("-I"),
            OsStr::new(&kernel_headers),
        ]);
    } else {
        println!("KERNEL_HEADERS env var not set; skipping kernel headers include");
        builder = builder.clang_args([OsStr::new("-I"), vmlinux_path.as_os_str()]);
    }

    match builder.build_and_generate(DEST) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to build BPF skeleton:");
            panic!("could not compile {}: {:?}", SOURCE, e);
        }
    }
    println!("cargo:rerun-if-changed={}", SOURCE);
}
