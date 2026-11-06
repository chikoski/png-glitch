# Contributing to png-glitch

Thank you for your interest in contributing to `png-glitch`! This guide covers setting up your development environment, running tests and checks, and the step-by-step workflow for adding new glitch filters.

## Prerequisites

- **Rust**: `1.85` or later (Rust 2024 edition).
- A C compiler (used by `libwebp-sys2` when building `webp-glitch`).

## Workspace Structure

| Crate | Path | Description |
| :--- | :--- | :--- |
| `png-glitch-cli` | `./` (root) | Command-line interface (`png-glitch` binary) for PNG and WebP images. |
| `png-glitch` | `crates/png-glitch` | Core low-level PNG scanline, filter, and pixel manipulation library. |
| `webp-glitch` | `crates/webp-glitch` | Core WebP scanline, pixel, and RIFF chunk manipulation library. |
| `glitch-context` | `crates/glitch-context` | High-level filter pipeline, RNG management, and YAML/CLI config orchestration. |

## Development Commands

Run these commands from the workspace root before opening a pull request:

```zsh
# 1. Check formatting
cargo fmt --all -- --check

# 2. Run Clippy across all workspace members and targets (must pass with 0 warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Run unit, integration, and doc tests
cargo test --workspace --all-targets
cargo test --workspace --doc

# 4. Build documentation (must pass with 0 warnings; #![warn(missing_docs)] is enabled)
cargo doc --workspace --no-deps

# 5. Run examples (outputs are written to the system temp directory)
cargo run -p png-glitch --example glitch
cargo run -p glitch-context --example simple_glitch
cargo run -p webp-glitch --example glitch
```

## Design Principles and Decisions

New features and API changes are reviewed against the [Design principles in `DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md#design-principles): the output is always a valid PNG, and effects change only the scan-line data. The section ends with a review checklist.

Significant design decisions are recorded as [Architecture Decision Records](docs/adr/README.md). If your change adds or renames a public abstraction, or decides something other work depends on, propose an ADR in the same pull request.

## Coding Guidelines

1. **No Panics in Library Code**: Library crates (`png-glitch`, `webp-glitch`, `glitch-context`) must never panic (`unwrap()`, `expect()`, out-of-bounds slice indexing, or divide-by-zero) on malformed image bytes or caller-supplied parameters. Return a typed `Err` or clamp/no-op on empty/out-of-range inputs.
2. **Deterministic RNG**: All random decisions inside filters must use the `&mut dyn rand::RngCore` passed into `GlitchFilter::apply` / `WebpGlitchFilter::apply`—never `rand::rng()`—so that `--seed` produces identical results across runs.
3. **Documentation**: `png-glitch` and `glitch-context` enforce `#![warn(missing_docs)]`. Every public struct, enum, variant, field, trait, and method must have a clear English doc comment.
4. **Terminology**: Refer to the [Terminology Glossary in `DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md#terminology-glossary) for the distinction between *PNG filter types*, *presets*, and *glitch filters*.

---

## How to Add a New Glitch Filter

Adding a filter end-to-end touches four places:

### Step 1: Implement the Filter Struct in `crates/glitch-context/src/lib.rs`

1. Define the filter struct with `#[derive(Debug, Clone, Copy, PartialEq)]`, doc comments on the struct and all public fields, `Default`, `pub fn new(...)`, and `pub fn with_*(mut self, ...)` builder methods.
2. Implement the appropriate trait(s):
   - **Format-independent pixel filter**: Implement [`PixelFilter`](crates/glitch-context/src/lib.rs) (or both [`GlitchFilter`](crates/glitch-context/src/lib.rs) for `PngGlitch` and [`WebpGlitchFilter`](crates/glitch-context/src/lib.rs) for `WebpGlitch`).
   - **PNG-specific scanline/filter-byte effect**: Implement [`GlitchFilter`](crates/glitch-context/src/lib.rs).
   - **WebP-specific RIFF/VP8 effect**: Implement [`WebpGlitchFilter`](crates/glitch-context/src/lib.rs).
3. Ensure `magnitude <= 0.0` is a no-op and guard against zero-width/zero-height scanlines.

### Step 2: Register in `FilterConfig` (`crates/glitch-context/src/lib.rs`)

1. Add a variant to `FilterConfig` with `#[serde(alias = "PascalCaseName")]` (since `FilterConfig` uses `#[serde(rename_all = "kebab-case")]`) and `#[serde(deserialize_with = "deserialize_magnitude")]` on any `0.0..=1.0` `magnitude` field.
2. Handle the new variant in:
   - `GlitchContext::add_from_config` (return `Err` if the filter is WebP-only).
   - `WebpGlitchContext::add_from_config` (return `Err` if the filter is PNG-only).

### Step 3: Expose in the CLI (`src/cli.rs` and `src/main.rs`)

1. Add the CLI flag(s) to `struct Cli` in `src/cli.rs` with appropriate `value_name` and `value_parser` (e.g. `parse_magnitude`).
2. Add a match arm in `parse_filter_spec` (`src/cli.rs`) so the filter works with `-f / --filter <SPEC>`.
3. Add an entry to `format_filter_list` (`src/cli.rs`) so `--list-filters` documents the new filter.
4. Push the step in `collect_ordered_steps` (`src/main.rs`) and, if format-specific, add validation in `validate_flags_for_format` (`src/main.rs`).

### Step 4: Tests and Documentation

1. Add unit tests in `crates/glitch-context/src/lib.rs` (including `test_all_png_filters_across_color_types_and_bit_depths` and/or `test_all_webp_filters_and_reproducibility`, `magnitude = 0.0` boundary test, and YAML deserialization test).
2. Add CLI parsing tests in `src/main.rs`.
3. Add a row to the filter compatibility tables in `README.md` and `crates/glitch-context/README.md`.

---

## Releasing & Publishing

Workspace crates must be published to crates.io in dependency order:

1. `cargo publish -p webp-glitch`
2. `cargo publish -p png-glitch`
3. `cargo publish -p glitch-context`
4. `cargo publish -p png-glitch-cli`

Update [`CHANGELOG.md`](CHANGELOG.md) and bump versions in `Cargo.toml` (including `[workspace.dependencies]`) before tagging a release.

