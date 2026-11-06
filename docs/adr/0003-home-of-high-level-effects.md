# 0003. Where high-level effects live

- **Status:** Proposed
- **Date:** 2026-11-06
- **Issue:** #63 (part of epic #62; blocks #64; affects #65, #66, #73)

> This ADR is a proposal. Each question below lists the options and a **recommendation**; the maintainer makes the final decision on the pull request, and the Decision section is updated to match before the status changes to Accepted.

## Context

PR #35 (`extension-by-kiro`, commit 92c92b5) adds a high-level API for common glitch effects. Its five implemented effects (`scanlines`, `noise`, `rgb_shift`, `degrade`, `pixel_sort`) are to be ported by #64; `datamosh` (#65) and `corrupt_blocks` (#66) are `todo!()`. Before porting, we need to decide where the code goes, because after PR #35 there would be three overlapping ways to express "an effect":

| | Where | Shape | Randomness | Errors |
| :-- | :---- | :---- | :--------- | :----- |
| `GlitchPreset` (on `master`) | `png-glitch/src/presets.rs` | struct with `apply_to_line(&mut ScanLine)`, applied with `PngGlitch::apply`; per line, parallel | none (deterministic) | infallible |
| `GlitchPresets` (PR #35) | `png-glitch/src/presets/` | extension trait on `PngGlitch` with `xxx(config)` and `xxx_default()` methods; config structs with `validate()`; whole image; optional `Region` | hand-rolled PRNGs seeded from the region coordinates; no seed field | `Result<_, PresetError>` |
| `GlitchFilter` / `Effect` | `glitch-context` | struct with `apply(&self, &mut PngGlitch, &mut dyn RngCore)`; registered in `FilterConfig` for YAML and the CLI | shared, seeded `ChaCha8Rng` from the context | infallible; parameters are clamped, `magnitude <= 0.0` is a no-op |

Facts that constrain the answer:

- `glitch-context` depends on `png-glitch`. `png-glitch` has no `rand` dependency, so effects in `png-glitch` cannot use the shared RNG without adding one.
- The project guarantees that `--seed` reproduces output (#58, `CONTRIBUTING.md` guideline 2). None of PR #35's configs has a seed, and its PRNGs are seeded from region coordinates, so two runs of `noise` always give the same result and a different `--seed` cannot vary it.
- `glitch-context` already adapts the four `GlitchPreset`s into `GlitchFilter` and `PixelFilter` (`impl GlitchFilter for Invert`, …), so "preset in `png-glitch`, adapter in `glitch-context`" is an existing pattern.
- The PR #35 effects read and write scan-line bytes through `ScanLine` and the `Pixel` API. None of them recomputes filtered values, so all of them fit the design principles in [ADR 0002](0002-design-principles.md).
- The terminology issue #22 is closed: the glossary in `DEVELOPER_GUIDE.md` defines *preset* as the `png-glitch` per-line transformation. PR #35 reuses "preset" for something else.
- The typed-error issue #21 is closed: `png-glitch` returns `PngError`. `glitch-context` still uses `anyhow` (to be revisited in #73).
- Three PR #35 effects overlap existing `glitch-context` effects:

  | PR #35 | Existing in `glitch-context` | Difference |
  | :----- | :--------------------------- | :--------- |
  | `pixel_sort` | `PixelSort` | PR #35 adds direction (horizontal, vertical, diagonal, radial), threshold and region |
  | `rgb_shift` | `ChromaticAberration`, `ShiftChannels` | PR #35 adds 2D offsets per channel and a wrap mode |
  | `noise` | `Replace`, `SetZero`, `ColorDistortion` | PR #35 adds distributions (uniform, clustered, linear) and region |

## Questions and options

### Q1. Home: `png-glitch` or `glitch-context`?

- **A. `png-glitch`** (as in PR #35). Usable without `glitch-context`. Needs a `rand` dependency (or a home-grown RNG) in the core crate, and CLI/YAML support still has to be written in `glitch-context`.
- **B. `glitch-context`**, next to the other effects. Gets the shared seeded RNG, `FilterConfig`, YAML and CLI support. Users of `png-glitch` alone do not get them.
- **C. Split by kind:** deterministic, RNG-free building blocks (like the current `GlitchPreset`s) in `png-glitch`; anything randomized or whole-image in `glitch-context`.

**Recommendation: C.** In practice this means all five PR #35 effects go to `glitch-context`, because all of them are randomized or whole-image. `png-glitch` stays the PNG model plus deterministic operations on it (consequence 1 in ADR 0002), and keeps no `rand` dependency.

### Q2. One abstraction or several?

- **A. Keep both styles.** Add `GlitchPresets` (extension trait, `Result`) alongside `GlitchFilter`.
- **B. Adapter.** Keep PR #35's config structs and add `impl GlitchFilter for ScanlineConfig`.
- **C. One abstraction.** Each new effect is a `GlitchFilter` struct whose fields are the config, in the style of the existing effects (`Default`, `new`, `with_*` builders), and a `FilterConfig` variant.

**Recommendation: C.** There is no `GlitchPresets` extension trait and no `xxx_default()` methods; `Scanlines::default()` plays that role. A fluent call style on `PngGlitch`, if wanted, is a separate API question for #68 (recipe vs. image) and #70 (escape hatches).

### Q3. Naming

- **A. Keep "preset"** for both `GlitchPreset` and the new effects.
- **B. "Effect"** for everything in `glitch-context` (the existing `Effect` alias of `GlitchFilter`), named by what it does: `Scanlines`, `Noise`, `RgbShift`, `Degrade`, … Keep "preset" for the `png-glitch` per-line transformations only.
- **C. As B, and also rename `GlitchPreset`** now.

**Recommendation: B.** It matches the existing glossary and introduces no new terms. Whether `GlitchPreset` itself is renamed (and whether `GlitchFilter` or `Effect` becomes the primary name) is part of the API tidy-up in #73. "Recipe" stays reserved for a named combination of effects (#69, #80).

### Q4. Randomness

- **A. Seed field in each config.** Self-contained, but every effect re-seeds independently, and the context seed no longer controls everything.
- **B. RNG argument.** Use the `&mut dyn RngCore` that `GlitchFilter::apply` already receives.

**Recommendation: B.** The hand-rolled PRNGs from PR #35 are removed during the port. This keeps a single seed per run, as the CLI prints and #58 tests.

### Q5. Duplicates

- **A. Keep both** the PR #35 effect and the existing one.
- **B. Delegate:** implement the PR #35 effect by calling the existing one.
- **C. Extend the existing effect** with the missing parameters, and do not add a second type.

**Recommendation: C.** Concretely:

- `pixel_sort` → extend `PixelSort` with direction, threshold (and region, once #77 decides how regions work).
- `rgb_shift` → extend `ChromaticAberration` with vertical offsets and a wrap mode. `ShiftChannels` (a value shift, not a spatial one) is unrelated despite the name.
- `noise` → add one `Noise` effect for the distributions, since `Replace`/`SetZero`/`ColorDistortion` each do something narrower; reuse their per-pixel code where possible.
- `scanlines` and `degrade` have no counterpart and are ported as new effects.

Each extension must keep the existing parameters' defaults, so that existing YAML recipes and golden outputs (#58, #120) do not change.

### Q6. Errors

- **A. Keep `PresetError`** as a second error type.
- **B. No new error type.** `GlitchFilter::apply` stays infallible: out-of-range parameters are clamped or become a no-op, as `CONTRIBUTING.md` guideline 1 already requires. Invalid configuration from YAML or the CLI is rejected where it is parsed (`FilterConfig` deserialization, `parse_filter_spec`), with the error type `glitch-context` uses there.

**Recommendation: B.** One typed error per crate: `PngError` in `png-glitch`, and whatever #73 decides to replace `anyhow` with in `glitch-context`. `PresetError` is not ported.

## Decision

*To be filled in when the maintainer decides. If all recommendations are accepted:*

High-level effects live in `glitch-context` as ordinary `GlitchFilter` structs registered in `FilterConfig`. They use the RNG passed to `apply`, have no seed field, and do not introduce a new trait, error type or use of the word "preset". `png-glitch` keeps only deterministic, RNG-free operations on the PNG model, including the existing `GlitchPreset`s. Effects that overlap existing ones extend them instead of duplicating them.

## Consequences (if the recommendations are accepted)

- **#64** ports `scanlines` and `degrade` as new effects, adds `Noise`, and extends `PixelSort` and `ChromaticAberration`. Each needs a `FilterConfig` variant, a CLI flag and tests, following "How to Add a New Glitch Filter" in `CONTRIBUTING.md`.
- **#65, #66** (`datamosh`, `corrupt_blocks`) are designed directly as `glitch-context` effects. `corrupt_blocks` must corrupt scan-line data, not the compressed stream (ADR 0002, consequence 7).
- **#62** is updated: the "high-level preset API" becomes "high-level effects in glitch-context".
- **#73** decides the remaining naming (`GlitchPreset`, `GlitchFilter` vs. `Effect`) and the `glitch-context` error type.
- **#77** decides how regions and masks are expressed; PR #35's `Region` is not ported on its own.
- Users who want these effects without `glitch-context` do not get them. If that need appears, a deterministic building block can move down into `png-glitch` later, following option C of Q1.
