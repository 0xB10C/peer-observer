use std::env;
use std::fs;
use std::path::Path;

fn generate_ip_match(addr: &str) -> String {
    return format!(" \"{}\" ", addr);
}

// generates a 'rs_file' with a 'match_fn_name' IP match function from a 'input' list
// of IP addresses
fn gen_ip_match_fn(input: &str, rs_file: &str, match_fn_name: &str) {
    let input_list = fs::read_to_string(input).expect("a valid input file path");
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join(rs_file);

    let match_statement = input_list
        .lines()
        .map(|ip| generate_ip_match(ip))
        .collect::<Vec<String>>()
        .join("|");

    fs::write(
        &out_path,
        format!(
            "\
// DON'T CHANGE THIS FILE MANUALLY. IT WILL BE OVERWRITTEN.
// This file is generated in `build.rs`.

#[allow(dead_code)]
pub fn {}(ip: &str) -> bool {{
    matches!(ip, {})
}}

",
            match_fn_name, match_statement
        ),
    )
    .expect("can write to file");
}

fn main() {
    gen_ip_match_fn("./torbulkexitlist", "torexitlist.rs", "is_tor_exit_node");
    gen_ip_match_fn("./gmaxbanlist.txt", "gmaxbanlist.rs", "is_on_gmax_banlist");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=./torbulkexitlist");
}
