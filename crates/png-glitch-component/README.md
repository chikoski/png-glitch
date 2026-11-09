# png-glitch-component

png-glitch as a WebAssembly component (WASI 0.2, [WIT](wit/world.wit) package `chikoski:png-glitch@0.1.0`). Stateless: PNG bytes in, PNG bytes out.

```wit
list-filters: func() -> list<filter-info>;
validate-recipe: func(recipe: string) -> result<_, glitch-error>;
glitch: func(image: list<u8>, recipe: string, seed: u64) -> result<list<u8>, glitch-error>;
```

- **Deterministic.** The seed is required and there is no OS randomness: the same image, recipe and seed give the same bytes, and they equal the CLI's output for the same `--config` and `--seed` (checked in CI).
- **Recipe** is the YAML or JSON of the CLI's `--config` file (`filters: [...]`). `rows` and `rect` work; `mask:` steps are rejected with `invalid-recipe` (the component has no file system).
- **Errors, never traps:** `invalid-image(message)`, `unsupported-format` (not a PNG, including WebP, which is not supported yet), `invalid-recipe(message)`, `too-large` (over 64 million pixels or 256 MiB decompressed; checked on the header, before allocating).
- `list-filters` is the table behind `png-glitch --list-filters --json`: names, aliases, parameters with kind, range, default and choices, and formats.

## Build

```sh
rustup target add wasm32-wasip2
cargo build -p png-glitch-component --target wasm32-wasip2 --release
wasm-tools component wit target/wasm32-wasip2/release/png_glitch_component.wasm
```

The logic is plain Rust (`list_filters`, `validate_recipe`, `glitch`) and is tested natively with `cargo test -p png-glitch-component`; only `src/component.rs` (the `wit-bindgen` layer, which needs `unsafe`) is compiled for `wasm32`.

## Host example (Rust, wasmtime)

[`host-test/`](host-test) is a small host and the smoke test. Run it after building the component and the CLI:

```sh
cargo build -p png-glitch-cli
cargo test --manifest-path crates/png-glitch-component/host-test/Cargo.toml
```

```rust
let mut host = Host::load("target/wasm32-wasip2/release/png_glitch_component.wasm")?;
let png = std::fs::read("in.png")?;
let out = host.glitch(&png, "filters:\n  - type: invert\n", 42)??;
std::fs::write("out.png", out)?;
```
