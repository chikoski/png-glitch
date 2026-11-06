# Changelog

All notable changes to the crates in this workspace (`png-glitch`, `webp-glitch`, `glitch-context`, `webp-glitch-context`, and `png-glitch-cli`) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### png-glitch

#### Changed
- Introduced the `Effect` trait (whole image, `&mut PngGlitch`) and the `LineEffect` helper trait (per scan line, parallel); every `LineEffect` is an `Effect`. `PngGlitch::apply` accepts any `Effect` (#126, ADR 0003).
- Renamed the `presets` module to `effects` and `GlitchPreset` to `LineEffect`. The old paths remain as a `#[deprecated]` `presets` module for one release.

#### Added
- Deterministic effects whose random choices are explicit parameters (#129, ADR 0003/0004): `Refilter`, `SetFilterTypes`, `SetBytes`, `Substitute`, `SwapLines`, `CopyLines`, `ShiftLines`, `PixelSort`, `Bitwise`, `SwapChannels`, `OffsetColors`, `HslShift`, `ChannelShift` and `SwapBlocks`. They need no `rand` and give the same bytes for the same image and parameters.
- `FilterType` now implements `Eq`.

### png-glitch-recipe `0.3.0`

#### Added
- `Recipe` (an ordered list of `FilterConfig` steps, independent of any image; `Clone`, `Debug`, `PartialEq`), `Recipe::resolve(&ImageHeader, seed)` → `ResolvedRecipe` (all random choices drawn, concrete `png-glitch` effects only), and `ResolvedRecipe::execute(&mut PngGlitch)` (deterministic, no RNG). One recipe can be reused for any number of images and seeds, and `Recipe::run` resolves and executes in one call (#68, ADR 0004).
- Custom recipe steps (#70, re-scoped by ADR 0003/0004): the `RecipeStep` trait (draws choices from the RNG and returns a `png-glitch` `Effect`; closures `Fn(&ImageHeader, &mut dyn RngCore) -> ResolvedEffect` implement it), `Recipe::then_step` and `Recipe::then_effect` (a fixed effect), `ResolvedEffect` and `ResolvedStep::Custom`. `Recipe::steps()` now returns `&[Step]` (`Step::Config` or `Step::Custom`).
- `GlitchContext::png()`, `png_mut()`, `into_inner()` and `add_fn(closure)` to mix filters with direct `PngGlitch` calls. Example: `examples/custom_effect.rs`.
- `ImageHeader`, `ResolvedStep` and `RecipeError::HeaderMismatch`: executing a resolved recipe on an image whose header differs from the one it was resolved for is an error.
- `GlitchContext` is unchanged for callers and now runs the same resolve/execute code, so seeded output is byte-identical (golden tests unchanged).

#### Changed
- **Breaking**: `glitch-context` was renamed to `png-glitch-recipe` (#128, ADR 0004). Replace `glitch-context` with `png-glitch-recipe` in `Cargo.toml` and `glitch_context` with `png_glitch_recipe` in `use` paths. CLI behaviour and seeded output are unchanged. A final `glitch-context` release that points to the new crate is published separately.
- Each randomized effect now only draws its choices (rows, bytes, offsets) from the RNG, in the same order as before, and calls the matching deterministic `png-glitch` effect (#129). Seeded output is byte-identical (golden tests unchanged).
- **Breaking**: `anyhow` was removed from the library. Fallible functions now return `png_glitch_recipe::Result<T>`, whose error is the new typed, `#[non_exhaustive]` `RecipeError` (`Open`, `Parse`, `Encode`, `Save`, `InvalidSpec`, `UnsupportedEffect`). Error messages are unchanged.

### png-glitch-recipe (shareable recipes, #69)

#### Added
- `FilterConfig` and the `SortCriterion` / `BitOp` / `SwapTarget` / `AlphaGlitchStrategy` / `PreProcess` enums implement `Serialize`; `FilterConfig` implements `Display`, which renders the `-f` spec syntax (`pixel-sort:magnitude=0.2,criterion=hue`). Both formats round-trip. `None` options are omitted when serializing.
- `Recipe` implements `Serialize` / `Deserialize` in the same `filters: [...]` shape as the CLI's `--config` file; `Recipe::configs()` returns the steps as data, or `None` when the recipe has a user-defined step (code cannot be serialized).

### png-glitch-cli (shareable recipes, #69)

#### Added
- `--print-recipe`, `--save-recipe <FILE>`, `--embed-recipe` (PNG `iTXt` chunk `png-glitch-recipe`; replaces an earlier recipe chunk on re-runs) and `--show-recipe`. The saved text starts with a `# seed: N` comment, so `--config FILE --seed N` reproduces the output byte for byte.
- `--pre-process` is now part of the effective recipe (it is written out as the equivalent first step).
- Not included yet: embedding in WebP, and serializing a `ResolvedRecipe` (it holds `png-glitch` effects, which have no `serde` by design; see ADR 0004).

### glitch-context (now `png-glitch-recipe`)

#### Changed
- **Breaking**: WebP support was split out into the new `webp-glitch-context` crate (#127, ADR 0004). `glitch-context` no longer depends on `webp-glitch` or `libwebp-sys2`, and the `webp` Cargo feature is removed. `WebpGlitchContext`, `WebpInvert`, `WebpBrighten`, `WebpShiftChannels`, `MacroblockGlitch`, `AlphaGlitch`, `LossyArtifact` and the WebP re-exports (`WebpGlitch`, `WebpGlitchFilter`, `WebpPixel`, `WebpError`) moved there; `AnyGlitchContext`, `ImageFormat`, `detect_format` and `open_any` moved into the CLI. `FilterConfig` still contains the WebP-only variants as data, and `GlitchContext::add_from_config` rejects them.

### webp-glitch-context `0.1.0`

#### Added
- New crate holding `WebpGlitchContext` and the WebP-only effects. The effects that exist for both formats are applied to WebP through the `ForWebp` wrapper (for example `ForWebp(PixelSort::new(0.5))`) because of the orphan rule. Seeded output is unchanged.

### png-glitch-cli

#### Changed
- Format detection and dispatch (`AnyGlitchContext`) now live in the CLI. Behavior, flags and seeded output for PNG and WebP are unchanged.

### Workspace

#### Changed
- The `glitch_context_pipeline` fuzz target now covers PNG only; the new `webp_pipeline` target covers WebP.
- Minimum supported Rust version raised from `1.85` to `1.89` (#56). `1.85` already failed to build because `image` (1.88), `built` (1.87) and `serde-saphyr` (1.89) require newer compilers; CI now checks the declared MSRV. Release builds now keep arithmetic overflow checks enabled.

#### Fixed
- CI is green again (#122): fuzz targets are built with `--target x86_64-unknown-linux-gnu` (a prebuilt musl `cargo-fuzz` defaults to musl, which AddressSanitizer does not support), `crossbeam-epoch` and `rand` are updated within their current minor series (RUSTSEC-2026-0204, RUSTSEC-2026-0097; seeded output unchanged), `image` is built with only the `png` feature (drops `paste`/`rav1e`), and `serde_yaml_ng` is replaced by `serde-saphyr` (drops unmaintained `unsafe-libyaml`).
- Fuzz targets build again and run in CI (#52): the fuzz crate moved from `crates/png-glitch/fuzz` to the top-level `fuzz/` with its own `[workspace]` table, the parser target uses the public API, and new `glitch_context_pipeline` and `webp_riff` targets were added. Every PR builds all targets; a scheduled workflow fuzzes each one and uploads crashes as artifacts.

#### Added
- Seed reproducibility is now a tested guarantee (#58): library-level golden hashes for every PNG filter (`crates/glitch-context/tests/golden.rs`), CLI tests comparing `RAYON_NUM_THREADS=1` and `8` (single files, chained filters and batch mode), and a documented policy in the README and CONTRIBUTING.md. No seeded output changed.

### `png-glitch-cli` `0.5.0`

#### Added
- Support for WebP images alongside PNG images with magic-byte format auto-detection.
- Repeatable `-f / --filter <SPEC>` flag supporting `<filter>`, `<filter>=<val>`, and `<filter>:k1=v1,k2=v2` to apply any filter any number of times in command-line order (#18).
- `--list-filters` flag to print all available filters, parameters, default values, and PNG/WebP compatibility (#18).
- `--darken <STRENGTH>` flag and `--sub-filter`, `--up-filter`, `--average-filter`, `--paeth-filter` aliases (#18, #19).
- `--force` flag to explicitly allow overwriting existing output files (#14).
- Print the generated random `--seed` to `stderr` when `--seed` is omitted so random glitches can be reproduced (#16).
- Format-specific flag validation (`--macroblock-glitch`, `--alpha-glitch`, `--lossy-quality` rejected for PNG; PNG-only flags rejected for WebP) (#9).
- YAML configuration validation (`#[serde(deny_unknown_fields)]`, `0.0..=1.0` magnitude validation, and support for both `kebab-case` and `PascalCase` filter names) (#10, #19).

#### Changed
- Renamed positional argument `<PNG_FILE>` to `<INPUT>` (#20).
- Set binary `#[command(name = "png-glitch")]` so `--help` and `--version` display `png-glitch` instead of `png-glitch-cli` (#12).
- Default output path is now `<input-stem>-glitched.<ext>` next to `<INPUT>` instead of `./glitched.png` in the current working directory (#14).
- CLI filter flags and `--config` are now applied in the exact order they appear on the command line (#13).
- Batch processing (`--batch-output`) now walks subdirectories case-insensitively (`.png`, `.PNG`, `.webp`, `.WEBP`), preserves relative subdirectory structure, uses a shared deterministic seed, and exits non-zero if any file fails (#15).

#### Fixed
- Allowed negative values on `--r-offset`, `--g-offset`, `--b-offset`, `--hue-shift`, and `--shift-channels` without requiring `=` (#8).
- Replaced `.expect()` panics on malformed `--substitute`, `--shift-channels`, and `--alpha-glitch` arguments with `clap` `value_parser` errors (#7).
- Included file paths in error messages without duplicate context prefixes (#17).

---

### `glitch-context` `0.2.0`

#### Added
- `WebpGlitchContext`, `AnyGlitchContext`, `ImageFormat`, `detect_format`, and `open_any` for format-agnostic PNG and WebP glitching (#27).
- `PixelFilter` trait and `PixelFilterAdapter` for format-independent pixel filters (#28).
- `Darken` and `WebpDarken` filters, plus `Darken`, `MacroblockGlitch`, `AlphaGlitch`, and `LossyArtifact` variants on `FilterConfig` (#22, #27).
- `Default`, `::new(...)` constructors, and `with_*` builder methods on all filter structs, along with `#[derive(Debug, Clone, Copy, PartialEq)]` (#28).
- Cargo features `webp` and `clap` (both enabled by default) so library consumers can disable `webp-glitch` or `clap` (#26).
- Re-exports of `rand`, `rand_chacha`, and `ChaCha8Rng` (#25).
- Full `#![warn(missing_docs)]` rustdoc coverage and expanded test suite across all 15 PNG color-type/bit-depth combinations and all WebP filters (#29, #30).

#### Changed
- **Breaking**: `GlitchFilter::apply` and `WebpGlitchFilter::apply` now take `&mut dyn rand::RngCore` instead of `&mut ChaCha8Rng` (#25).
- **Breaking**: `GlitchContext::add_from_config` and `WebpGlitchContext::add_from_config` now return `anyhow::Result<()>` and reject format-incompatible filters instead of silently ignoring them (#27).
- `execute()` now drains the internal filter queue so calling `execute()` multiple times on the same context does not re-apply previously executed filters (#28).

#### Fixed
- `Substitute`, `ChromaticAberration`, and `ColorDistortion` now clamp/guard against small images and 16-bit channel overflow instead of panicking (#5, #6).
- `WebpShiftChannels`, `PixelSort`, `ChannelSwap`, and `ColorDistortion` on WebP now use the context's seeded RNG instead of `rand::rng()` (#11).
- `ChromaticAberration` no longer panics with an arithmetic overflow when a channel offset is near `i32::MIN`/`i32::MAX` (PNG and WebP). Found by the new `glitch_context_pipeline` fuzz target (#52).

---

### `png-glitch` `0.7.0`

#### Added
- `PngGlitch::from_bytes(&[u8])`, `TryFrom<&[u8]>`, `PngGlitch::to_bytes(&self) -> Result<Vec<u8>, PngError>`, and `PngGlitch::encode(impl Write)` (#21).
- `PngGlitch::reencode_with(FilterType)` and `PngGlitch::set_filter_type(FilterType)` (with `change_filter_type` kept as a backward-compatible alias) (#23, #24).
- `presets::Darken` saturating brightness decrease preset (#22).
- `Effect` trait alias for `Glitch` and `RandomTranspose` type alias for `Transpose` (#24).

#### Changed
- **Breaking**: Public methods (`open`, `new`, `from_bytes`, `save`, `encode`, `to_bytes`) now return `Result<_, PngError>` instead of `anyhow::Result<_>`, and `anyhow` has been removed from `png-glitch` dependencies (#21).
- `PngError` is now publicly exported at the crate root and includes `PngError::Io(#[from] std::io::Error)` (#21).

---

### `webp-glitch` `0.1.0`

#### Added
- Initial release of `webp-glitch` providing `WebpGlitch`, `ScanLine`, `WebpPixel`, `RiffContainer`, `RiffChunk`, and `WebpError` for pixel-level and RIFF chunk-level WebP glitching.
