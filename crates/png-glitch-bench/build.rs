fn main() {
    // Lets the report record the compilation target (native triple or wasm32-wasip1).
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=BENCH_TARGET={target}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
