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
    println!("cargo::rustc-check-cfg=cfg(has_renderer)");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let lib = Path::new(&manifest).join("../png-glitch/src/lib.rs");
    println!("cargo:rerun-if-changed={}", lib.display());
    if std::fs::read_to_string(&lib).is_ok_and(|src| src.contains("pub fn from_pixels")) {
        println!("cargo:rustc-cfg=has_from_pixels");
    }
    if std::fs::read_to_string(&lib).is_ok_and(|src| src.contains("Renderer")) {
        println!("cargo:rustc-cfg=has_renderer");
    }

    // The live pipeline (#160) and the live recipe runner (#163) are defined in modules, not in
    // `lib.rs`, so look through the whole source tree of their crate.
    println!("cargo::rustc-check-cfg=cfg(has_live_pipeline)");
    println!("cargo::rustc-check-cfg=cfg(has_live_recipe)");
    for (cfg, krate, needle) in [
        ("has_live_pipeline", "png-glitch", "pub struct LivePipeline"),
        (
            "has_live_recipe",
            "png-glitch-recipe",
            "pub struct LiveRecipe",
        ),
    ] {
        let src = Path::new(&manifest).join("..").join(krate).join("src");
        println!("cargo:rerun-if-changed={}", src.display());
        if source_contains(&src, needle) {
            println!("cargo:rustc-cfg={cfg}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}

/// Whether any `.rs` file under `dir` contains `needle`.
fn source_contains(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            source_contains(&path, needle)
        } else {
            path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path).is_ok_and(|src| src.contains(needle))
        }
    })
}
