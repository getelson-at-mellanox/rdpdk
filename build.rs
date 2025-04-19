use std::process::Command;

pub fn main() {
    let mut pkgconfig = Command::new("pkg-config");

    match pkgconfig.args(["--libs", "--static", "libdpdk"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string();
            for token in stdout
                .split_ascii_whitespace()
                .filter(|s| !s.is_empty()) {

                println!("cargo:rustc-link-arg={}", token);
            }

            println!("cargo:rustc-link-arg=-lc");
            println!("cargo:rerun-if-changed=build.rs");
        }
        Err(error) => {
            panic!("failed to read libdpdk package: {:?}", error);
        }
    }
}
