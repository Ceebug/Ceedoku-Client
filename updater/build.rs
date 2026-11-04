use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CEEDOKU_INSTALLER_PAYLOAD");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("installer-payload.zip");

    match env::var_os("CEEDOKU_INSTALLER_PAYLOAD") {
        Some(payload) => {
            let payload = PathBuf::from(payload);
            println!("cargo:rerun-if-changed={}", payload.display());
            fs::copy(payload, output).expect("could not copy the installer payload");
        }
        None => {
            fs::write(output, []).expect("could not create the empty installer payload");
        }
    }
}
