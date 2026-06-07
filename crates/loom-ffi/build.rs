//! build.rs — runs cbindgen to generate `include/loom.h` from the crate's
//! `extern "C"` surface (CORE-03, T-01-09).
//!
//! # What this does
//!
//! 1. Parses `src/ffi.rs` (and transitively `src/lib.rs`) for `#[no_mangle]
//!    pub extern "C"` functions.
//! 2. Emits a C header to `include/loom.h`.
//!
//! # What this must NOT do (PITFALLS integration gotcha, T-01-09)
//!
//! cbindgen must not redefine `FFI_ArrowArray` or `FFI_ArrowSchema` in the
//! generated header.  Those structs are defined by the Arrow C Data Interface
//! headers on the C++ side.  If cbindgen emits its own definition, the struct
//! layouts seen by C++ will conflict with the ones the C++ Arrow headers emit,
//! causing ABI mismatches.  This is prevented via `cbindgen.toml`:
//! `[export] exclude = ["FFI_ArrowArray", "FFI_ArrowSchema"]`.

fn main() {
    // Re-run this build script only when the FFI source changes.
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    // Locate the crate root.
    let crate_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");

    // Ensure the output directory exists.
    let include_dir = std::path::PathBuf::from(&crate_dir).join("include");
    std::fs::create_dir_all(&include_dir).expect("failed to create include/ directory");

    let out_file = include_dir.join("loom.h");

    // Build and generate.  `cbindgen::generate_with_config` picks up
    // `cbindgen.toml` from the crate root automatically when we pass the
    // manifest dir as the crate location.
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
