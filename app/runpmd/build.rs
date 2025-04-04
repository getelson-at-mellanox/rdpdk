use std::process::Command;

pub fn main() {
    let mut pkgconfig = Command::new("pkg-config");

    match pkgconfig.args(["--libs", "libdpdk"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string();
            for token in stdout.split_ascii_whitespace().filter(|s| !s.is_empty()) {
                if token.starts_with("-L") {
                    println!("cargo::rustc-link-search=native={}", &token[2..]);
                } else if token.starts_with("-l") {
                    println!("cargo::rustc-link-lib={}", &token[2..]);
                }
            }
            println!("cargo:rerun-if-changed=build.rs");
        }
        Err(error) => {
            panic!("failed to read libdpdk package: {:?}", error);
        }
    }
}
