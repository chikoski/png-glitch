use std::path::Path;

fn main() {
    // Lets the report record the compilation target (native triple or wasm32-wasip1).
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=BENCH_TARGET={target}");
    }

    // The CI benchmark job builds the base commit with this crate's (newer) harness, so the
    // harness must compile against a library that may predate an API it measures. For each such
    // API, probe the library source and enable a cfg only when it exists; stages the base lacks
    // are then simply absent from its report and `compare` skips them.
    println!("cargo::rustc-check-cfg=cfg(has_from_pixels)");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let lib = Path::new(&manifest).join("../png-glitch/src/lib.rs");
    println!("cargo:rerun-if-changed={}", lib.display());
    if std::fs::read_to_string(&lib).is_ok_and(|src| src.contains("pub fn from_pixels")) {
        println!("cargo:rustc-cfg=has_from_pixels");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
