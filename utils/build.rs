use std::env;
use std::fs;
use std::path::Path;

fn generate_tor_exit_ip_match(addr: &str) -> String {
    return format!(" \"{}\" ", addr);
}

fn main() {
    let tor_exit_list = fs::read_to_string("./torbulkexitlist").unwrap();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("torexitlist.rs");

    let tor_exit_match = tor_exit_list
        .lines()
        .map(|ip| generate_tor_exit_ip_match(ip))
        .collect::<Vec<String>>()
        .join("|");

    fs::write(
        &out_path,
        format!(
            "\
// DON'T CHANGE THIS FILE MANUALLY. IT WILL BE OVERWRITTEN.
// This file is generated in `build.rs`.

#[allow(dead_code)]
pub fn is_tor_exit_node(ip: &str) -> bool {{
    matches!(ip, {})
}}

",
            tor_exit_match
        ),
    )
    .unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=./torbulkexitlist");
}
