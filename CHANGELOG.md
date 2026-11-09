# Changelog

All notable changes to the crates in this workspace (`png-glitch`, `webp-glitch`, `png-glitch-recipe` (formerly `glitch-context`), `webp-glitch-context`, and `png-glitch-cli`) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### png-glitch-recipe, CLI: `rows=a..b` targeting on every step (#186, part of #77)

#### Added
- `RowRange` (`a..b`, `a..`, `..b`; serialises as that string) and `FilterEntry { config, rows }` (YAML: an optional `rows:` key next to `type:`). `FilterConfig` is unchanged.
- `Step::Rows`, `Recipe::then_rows`, `Recipe::from_entries`, `Recipe::entries`, `GlitchContext::add_from_config_in_rows` and `ResolvedStep::Rows` / `ResolvedStep::in_rows`. A targeted step is resolved for an image as tall as the range and runs on `ScanLines::rows`; `sub-filter` and the other re-encodes run as `Refilter::within_rows`. `regenerate` and file-only custom effects are rejected. Live recipes accept targeted scan-line steps and reject targeted re-encodes.
- CLI: `-f 'replace:magnitude=0.2,rows=300..452'` and `rows:` in `--config` files. `--print-recipe`, `--save-recipe` and `--embed-recipe` include it. WebP input rejects `rows`.
- Docs: `rows` in the README and `docs/CLI_GUIDE.md`, including the decoded-pixel caveat.

#### Changed
- `Step` has a new variant `Rows` and `ResolvedStep` a new variant `Rows` (`ResolvedStep` is `non_exhaustive`; a `match` on `Step` needs an arm). `Recipe::configs` returns `None` for a recipe with a targeted step; use `Recipe::entries`.
- A step with `rows` draws a different random stream than the same step without it. Recipes without `rows` give the same bytes as before (golden outputs unchanged).

### png-glitch: row sub-views and `WithinRows` (#185, part of #77)

#### Added
- `ScanLines::rows(range)`: a view of a range of rows (clamped, allocation-free, shares the scratch buffer). Row indices inside it are relative to the range.
- `effects::WithinRows<E>` and `ScanLinesEffect::within_rows(range)`: apply any scan-line effect to a range of rows only.
- `Refilter::within_rows(range)` (`effects::RefilterRows`), `PngGlitch::reencode_rows_with`: re-encode only a range of rows.
- The stored bytes (tag and filtered bytes) of rows outside the range are unchanged. The decoded pixels below the range can still change, because filters propagate downwards (ADR 0002).

### png-glitch-recipe, CLI: separate "set the filter type tag" from "recompute filtered values" (#139)

#### Added
- `FilterConfig::SetFilterType { filter }` (YAML `type: set-filter-type`, CLI `-f set-filter-type=paeth`): overwrites the tag of every row, tag only. Resolves to `SetFilterTypes`.
- `FilterConfig::Regenerate { strategy }` (YAML `type: regenerate`, CLI `-f regenerate=paeth|adaptive|sub,up,paeth`): recomputes the filtered values with `PngGlitch::regenerate`. `RegenerateStrategy` (`Fixed`, `Pattern`, `Adaptive`) is the serializable strategy; YAML `strategy: adaptive`, `{ fixed: paeth }`, `{ pattern: [sub, up, paeth] }`. An empty pattern is an error when the recipe is parsed or resolved. `ResolvedStep::Regenerate`, the filters `SetFilterType` and `Regenerate`, and `PreProcess::filter_type()`. A `LiveRecipe` runs `Regenerate` as a generation boundary.
- A trailing comma in `-f regenerate=` makes a one-element pattern (`regenerate=sub,`).

#### Deprecated
- The structs `RemoveFilter`, `SubFilter`, `UpFilter`, `AverageFilter`, `PaethFilter` (png-glitch-recipe 0.3.0) and the CLI flags `--remove-filter`, `--sub`, `--up`, `--average`, `--paeth` (they print a one-line warning to stderr; stdout and the output file are unchanged). The YAML types and `-f` specs `remove-filter`, `sub-filter`, ... keep loading and give the same bytes. `--pre-process` is kept.
- Migration: `remove-filter` → `regenerate=none`, `sub-filter` → `regenerate=sub`, `up-filter` → `regenerate=up`, `average-filter` → `regenerate=average`, `paeth-filter` → `regenerate=paeth` (YAML: `type: regenerate`, `strategy: { fixed: <type> }`). Use `set-filter-type` if you want to overwrite the tag only.

#### Changed
- Output bytes of existing recipes are unchanged. The `--help` text of the five deprecated flags now starts with `[deprecated: ...]`.
- `GlitchContext::pre_process` now queues `Regenerate` (same bytes).

### png-glitch-recipe: allocation-free `PerFrame` frames (#165)

#### Added
- `Recipe::resolve_into(&self, header, seed, &mut ResolvedRecipe) -> Result<()>`: the same result as `resolve` (same RNG draws, so seeds reproduce the same output), written into an existing `ResolvedRecipe` whose step lists are cleared and refilled. Lists reserve their upper bound (the image height for per-row steps, `times` for `random-copy`, the swap count for `block-scramble`) the first time, so later calls make no heap allocation for built-in steps; `color-distortion` keeps one spare offset list per row (6 bytes per pixel). User-defined steps are resolved as before.

#### Changed
- `SeedMode::PerFrame` resolves each frame with `resolve_into` into the previous frame's recipe, and `LiveRecipe::new` sizes those buffers, so a `PerFrame` frame with built-in steps makes no heap allocation. Outputs are unchanged.
- Resolving a built-in step no longer boxes the filter, and per-row lists reserve the image height. `ResolvedRecipe`'s `Clone`, `PartialEq` and `Debug` are now hand-written; they behave as before.

### png-glitch-recipe: live recipes (#163)

#### Added
- `LiveRecipe`, `SeedMode` (`Static`, `PerFrame`) and `frame_seed(seed, frame)` (SplitMix64, with the ADR 0005 test vectors): run a `Recipe` on every frame of a `png_glitch::LivePipeline`. In both modes a frame equals `Recipe::run` on that frame's pixels with the same (per-frame) seed; `Refilter`-type steps (`sub-filter`, ...) run as `regenerate(Fixed(t))`. Without a seed, one OS seed is drawn when the runner is built and reported by `seed()`. A `Static` frame makes no heap allocation.
- `ResolvedEffect::new_live` wraps a `ScanLinesEffect` so a user-defined step can run live; `ResolvedEffect::is_live`.
- `RecipeError::NotLive`: a step that cannot run live (a user-defined `Effect` that is not a `ScanLinesEffect`), returned when the `LiveRecipe` is built.

### png-glitch: `regenerate` (#162)

#### Added
- `PngGlitch::regenerate(&FilterStrategy) -> Result<&mut Self, PngError>` and `LivePipeline::regenerate`: an explicit generation boundary that removes the filters (`D`) and applies `Fixed`, `Pattern` or `Adaptive` (`E`) in place, on any model. An empty `Pattern` is `PngError::EmptyFilterPattern`, returned before anything changes. `regenerate(&FilterStrategy::Fixed(t))` gives the same bytes as `reencode_with(t)`. It makes no heap allocation for `Fixed` and `Pattern`.

### png-glitch: `ScanLinesEffect` (#161)

#### Added
- `effects::ScanLinesEffect` (`apply_to_scan_lines(&self, &mut ScanLines)`). The blanket impls are now a chain, `LineEffect → ScanLinesEffect → Effect`; existing `LineEffect` and `Effect` implementations keep compiling. A built-in `ScanLinesEffect` makes no heap allocation, and uses the view's scratch buffer for temporary rows.

#### Changed
- All built-in effects except `Refilter` implement `ScanLinesEffect` instead of `Effect`. Their output bytes are unchanged. `PixelSort` still allocates a pixel list per sorted row (a stable sort); this is documented.

### png-glitch-bench (#98)

#### Added
- New unpublished crate `png-glitch-bench`: a stage-timing harness (p50/p95/p99/max, allocation per iteration, share of frames over 33.3 ms) that runs natively and as `wasm32-wasip1` under wasmtime, with a `compare` subcommand and a `bench.yml` workflow. See the "Benchmarks" section of `DEVELOPER_GUIDE.md`.

### All library crates (panic-free code, #57)

#### Changed
- `#![forbid(unsafe_code)]` in `png-glitch`, `png-glitch-recipe`, `webp-glitch` and `webp-glitch-context`, so the "100% safe Rust" claim is a compiler guarantee.
- Outside tests, `clippy::unwrap_used`, `expect_used`, `panic` and `indexing_slicing` are denied. Sites that could be reached from file bytes or caller arguments now use checked access: IHDR parsing, chunk types, the PNG signature and chunk walk, the RIFF walk (chunk sizes use saturating arithmetic), `ScanLine` lookups in the new effects, and the WebP macroblock / chromatic-aberration filters.
- The pixel and filter hot paths (`png/scan_line.rs`, `png/scan_line/filter/*`, `png.rs`, `webp-glitch/src/scan_line.rs`) keep a documented file-level `allow(clippy::indexing_slicing)`; converting them is tracked in #140.

### WebAssembly (#84)

`png-glitch` and `png-glitch-recipe` now build for `wasm32-unknown-unknown` and `wasm32-wasip2` with `--no-default-features`; CI has a `wasm` job. Default features on native are unchanged.

### png-glitch-recipe

#### Added
- `os-rng` feature (default on). Without it `seed: None` is an error instead of an OS-random seed, and `getrandom` is not in the dependency graph. `GlitchContext::set_seed(None)` leaves the RNG unchanged.
- `RecipeError::SeedRequired`, returned by `GlitchContext::open*` / `new*` and `Recipe::resolve` when no seed is given and `os-rng` is off.
- `parallel` feature (default on), forwarding to `png-glitch/parallel`.

### png-glitch

#### Added
- Live input (#95): `PngGlitch::from_pixels(PixelSource, Encoding)` builds a model directly from RGBA8 pixels (filter step only, no compression), and `PngGlitch::reload_pixels` refills it without allocating. New types `PixelSource`, `Encoding` and `FilterStrategy` (`Fixed`, `Pattern`, `Adaptive`; `#[non_exhaustive]`). Supported: 8-bit `TrueColorAlpha`, `TrueColor`, `GrayScaleAlpha`, `GrayScale`. New example `encodings`.

#### Changed
- `PngError` (not `#[non_exhaustive]`) gained the variants `UnsupportedEncoding`, `EmptyFilterPattern`, `PixelDataLength` and `NotCreatedFromPixels`. Code that matches `PngError` exhaustively needs a wildcard arm.
- Rewrote the scan-line filter and unfilter kernels (#97): slice-based loops specialised per filter type and bytes per pixel, wrapping arithmetic, no per-byte bounds checks, a branch-free Paeth predictor, and `u32` SWAR arithmetic for 4-byte pixels. Output is byte-identical (checked against the previous implementation, kept as a test-only oracle, with property tests). 1080p RGBA at one thread, p50, native: unfilter Sub 9.8 → 2.3 ms, Average 14.0 → 4.9 ms, Paeth 38.4 → 10.3 ms; filter Paeth 40.6 → 6.3 ms. See the PR for wasm numbers.
- `proptest` is a new dev-dependency of `png-glitch`.
- Faster Paeth kernels (#97, follow-up). Filtering works in blocks with one flat, vectorisable loop per block and `i16` lanes. On wasm, RGBA reconstruction computes the four channels of a pixel as one `[i32; 4]` step, which `simd128` turns into `i32x4` instructions. Output is unchanged. 1080p RGBA, one thread, p50: wasm (+simd128) unfilter Paeth 29.0 → 13.6 ms, filter Paeth 26.0 → 3.9 ms; native filter Paeth 6.3 → 3.0 ms (native unfilter unchanged).
- Multi-row wavefront reconstruction on wasm with `simd128` (#152). `remove_filter` / `remove_filter_from` reconstruct runs of consecutive Paeth or Average rows of 4-byte pixels (8-bit RGBA, 16-bit gray + alpha) four (or two) at a time, each row one pixel behind the row above, with the rows sharing `i16x8` vectors built with safe `core::arch::wasm32` intrinsics (`forbid(unsafe_code)` is unchanged). Rows of other filter types, other pixel sizes, the first image row and native targets keep the one-row kernels. Output is unchanged (property tests against the reference implementation and against the `png` crate's decoder, also run on `wasm32-wasip1` in CI). 1080p RGBA, wasm (+simd128), one thread, p50: unfilter Paeth 13.6 → 5.2 ms, Average 4.8 → 1.9 ms.
- `proptest` is used without its `fork` and `timeout` features, so the tests build for `wasm32-wasip1`.

#### Added
- `parallel` feature (default on). Without it `par_foreach_scanline` runs sequentially with the same signature and output.

#### Changed
- Introduced the `Effect` trait (whole image, `&mut PngGlitch`) and the `LineEffect` helper trait (per scan line, parallel); every `LineEffect` is an `Effect`. `PngGlitch::apply` accepts any `Effect` (#126, ADR 0003).
- Renamed the `presets` module to `effects` and `GlitchPreset` to `LineEffect`. The old paths remain as a `#[deprecated]` `presets` module for one release.

#### Deprecated
- `PngGlitch::new` (use `from_bytes`) and `PngGlitch::change_filter_type` (use `reencode_with`, which recomputes values, or `set_filter_type`, which overwrites only the tag). Both are removed before 1.0; see [docs/API_STABILITY.md](docs/API_STABILITY.md). The crate docs gain a table that separates tag-only operations from value-recomputing ones (#73).

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

### png-glitch-recipe (API tidy-up, #73)

#### Changed
- **Breaking**: the alias `Effect` for `GlitchFilter` is removed (it clashed with `png_glitch::effects::Effect`); use `GlitchFilter`.
- Split `lib.rs` into the modules `config`, `filters`, `context` and `error`; every public name is still re-exported at the crate root, and `png_glitch_recipe::prelude` lists the commonly used ones.
- `RecipeError` already replaced `anyhow` (#128); WebP already lives in its own crates (#127).

#### Deprecated
- `RandomTranspose` (use `Transpose`) and `GlitchContext::change_filter_type` (use `png_mut().reencode_with(..)` or `set_filter_type(..)`); removed before 1.0.

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
