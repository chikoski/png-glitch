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

## Live use: the `session` resource

For sliders, webcams and other interactive hosts, `live.session` keeps the PNG model (header and filtered scan lines) in memory and defers compression:

```wit
resource session {
  from-png: static func(png: list<u8>) -> result<session, glitch-error>;
  from-pixels: static func(width: u32, height: u32, rgba: list<u8>, enc: encoding) -> result<session, glitch-error>;
  reload-pixels: func(rgba: list<u8>) -> result<_, glitch-error>;
  set-recipe: func(recipe: string, seed: u64, mode: seed-mode) -> result<_, glitch-error>;
  apply: func() -> result<_, glitch-error>;
  reset: func() -> result<_, glitch-error>;
  render-rgba: func() -> result<list<u8>, glitch-error>;
  snapshot: func() -> result<list<u8>, glitch-error>;
}
```

- `set-recipe` parses the recipe once (the same YAML or JSON as `glitch`); `apply` runs it and parses nothing, so it can run every frame. `apply` without a recipe is `no-recipe`.
- `seed-mode`: `fixed` uses the seed for every frame, `per-frame` derives frame n's seed from it (`frame_seed` in `png-glitch-recipe`). Record the seed and mode with a snapshot.
- `from-pixels` takes 8-bit `gray`, `gray-alpha`, `rgb` or `rgba`; any other bit depth is `unsupported-encoding`. `filter` is `fixed`, `pattern` or `adaptive` (about twice the cost of a fixed filter at 1080p; `fixed` and `pattern` are the cheapest).
- `reload-pixels` refills the model with the next frame and discards earlier glitches; `reset` restores the model as it was after the last load. Neither allocates.
- `render-rgba` is always 8-bit RGBA and equals what a PNG decoder reads from `snapshot` (tested byte for byte). `snapshot` is the only call that compresses.
- Steady state: the Rust `Session` makes no heap allocation in `reload_pixels` -> `apply` -> `render_into` (tested with a counting allocator, for both seed modes). Returning a `list<u8>` allocates in the guest by design of the canonical ABI. This holds for every built-in step, including `pixel-sort` and steps with `rows:` or `rect:`, in both modes.

```text
let s = session.from-pixels(w, h, first_frame, { rgba, 8, fixed(paeth) })
s.set-recipe("filters:\n  - type: transpose\n    magnitude: 0.2\n", 42, per-frame)
loop {
  s.reload-pixels(frame)     // E
  s.apply()                  // G
  show(s.render-rgba())      // D
}
save(s.snapshot())           // a real PNG
```

[`host-test/tests/smoke.rs`](host-test/tests/smoke.rs) runs such a loop with wasmtime and compares it with the native `Session`.

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
