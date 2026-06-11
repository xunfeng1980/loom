//! build.rs — runs cbindgen to generate `include/loom_sidecar.h` from the
//! crate's `extern "C"` surface.
//!
//! # What this does
//!
//! 1. Parses `src/ffi.rs` for `#[no_mangle] pub extern "C"` functions.
//! 2. Emits a C header to `include/loom_sidecar.h`.
//!
//! # What this must NOT do
//!
//! No Arrow FFI types cross the sidecar C ABI boundary — sidecar operations
//! only deal with raw byte pointers.  The generated header needs no forward
//! declarations or Arrow struct exclusions.

fn main() {
    // Re-run this build script only when the FFI source or config changes.
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    // Locate the crate root.
    let crate_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");

    // Ensure the output directory exists.
    let include_dir = std::path::PathBuf::from(&crate_dir).join("include");
    std::fs::create_dir_all(&include_dir).expect("failed to create include/ directory");

    let out_file = include_dir.join("loom_sidecar.h");

    // Build and generate.
    let config =
        cbindgen::Config::from_file(std::path::PathBuf::from(&crate_dir).join("cbindgen.toml"))
            .expect("failed to read cbindgen.toml");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("unable to generate cbindgen bindings")
        .write_to_file(&out_file);
}
