//! Generates `hp2_rs.h` with cbindgen. The CMake build points
//! `HP2_RS_INCLUDE_DIR` at `<build>/rust-include`; standalone `cargo` builds
//! write into `OUT_DIR`. cbindgen only rewrites the file when its contents
//! change, so C++ sources are not recompiled on every Rust edit.

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = env::var_os("HP2_RS_INCLUDE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")));

    println!("cargo:rerun-if-env-changed=HP2_RS_INCLUDE_DIR");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    // Exports live in this crate and in the re-exported port crates.
    println!("cargo:rerun-if-changed=..");

    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml"))
        .expect("crates/hp2-ffi/cbindgen.toml");
    std::fs::create_dir_all(&out_dir).expect("create hp2_rs.h directory");
    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        // Dependency parsing (re-exported port crates) resolves through the
        // workspace lockfile, which lives at the repository root.
        .with_lockfile(crate_dir.join("../../Cargo.lock"))
        .with_config(config)
        .generate()
        .expect("cbindgen failed to generate hp2_rs.h")
        .write_to_file(out_dir.join("hp2_rs.h"));
}
