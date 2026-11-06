# API stability policy (0.x → 1.0)

This policy covers the library crates: `png-glitch`, `png-glitch-recipe`, `webp-glitch` and `webp-glitch-context`. The CLI's flags and the `--config` / `-f` formats are covered under *Recipes and the CLI* below.

## Before 1.0

- Breaking changes are allowed in a **minor** release (`0.x` → `0.(x+1)`), and are listed under **Changed** or **Removed** in `CHANGELOG.md` with a migration note.
- Patch releases (`0.x.y`) do not break the API.
- A name that is being replaced is first marked `#[deprecated(since = "…", note = "…")]`. The note names the replacement and says "removed before 1.0". The old name stays for **at least one minor release**, then is removed in a later one; the removal is listed in the CHANGELOG. Items that cannot carry the attribute (such as a re-exported trait) are removed with a CHANGELOG entry instead.
- Items marked `#[non_exhaustive]` (for example `RecipeError`, `ResolvedStep`) may gain variants in a minor release.

## At 1.0

- Semantic versioning applies: breaking changes need a major release.
- Deprecated items from 0.x are removed at 1.0; nothing deprecated is carried into 1.0.
- The minimum supported Rust version (`rust-version` in `Cargo.toml`) may be raised in a minor release and is listed in the CHANGELOG.

## Reproducibility

- `png-glitch`: the same image and the same effect parameters give the same bytes.
- `png-glitch-recipe`: the same recipe, seed and image give the same bytes **as long as the RNG's value stream does not change**. A change to the draw order, to `rand` / `rand_chacha` value streams, or to a filter's parameter semantics is a deliberate change: it is listed in the CHANGELOG and the golden snapshots are updated in the same pull request. A resolved recipe does not depend on the RNG.

## Recipes and the CLI

- Existing YAML recipes and `-f` specs keep loading. A change that would break them gets a CHANGELOG entry with a migration note and, where possible, a deprecation period.
- CLI flags are deprecated before they are removed.

## Deprecations currently pending

| Item | Replacement | Deprecated in |
| :--- | :---------- | :------------ |
| `png_glitch::presets` (`GlitchPreset`) | `png_glitch::effects` (`LineEffect`) | png-glitch 0.8.0 |
| `PngGlitch::new` | `PngGlitch::from_bytes` | png-glitch 0.8.0 |
| `PngGlitch::change_filter_type` | `PngGlitch::reencode_with` (recomputes values) or `PngGlitch::set_filter_type` (tag only) | png-glitch 0.8.0 |
| `GlitchContext::change_filter_type` | `png_mut().reencode_with(..)` / `png_mut().set_filter_type(..)` | png-glitch-recipe 0.3.0 |
| `RandomTranspose` | `Transpose` | png-glitch-recipe 0.3.0 |
