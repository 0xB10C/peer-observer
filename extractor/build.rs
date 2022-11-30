use libbpf_cargo::SkeletonBuilder;

const SOURCE: &str = "./src/bpf/tracing.bpf.c";
const DEST: &str = "./src/tracing.gen.rs";

fn main() {
    match SkeletonBuilder::new()
        .source(SOURCE)
        .debug(true)
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
