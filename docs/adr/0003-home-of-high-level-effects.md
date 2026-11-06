# 0003. Where high-level effects live

- **Status:** Accepted
- **Date:** 2026-11-06
- **Issue:** #63 (part of epic #62; blocks #64; affects #65, #66, #73)

> All six questions are decided. Q2, Q3 and Q6 rest on two premises stated below (*Premises for Q2, Q3 and Q6*); Q5 follows from [ADR 0004](0004-png-glitch-and-recipe-crates.md).

## Context

PR #35 (`extension-by-kiro`, commit 92c92b5) adds a high-level API for common glitch effects. Its five implemented effects (`scanlines`, `noise`, `rgb_shift`, `degrade`, `pixel_sort`) are to be ported by #64; `datamosh` (#65) and `corrupt_blocks` (#66) are `todo!()`. Before porting, we need to decide where the code goes. After PR #35 there would be three overlapping ways to express "an effect":

| | Where | Shape | Randomness | Errors |
| :-- | :---- | :---- | :--------- | :----- |
| `GlitchPreset` (on `master`) | `png-glitch/src/presets.rs` | struct with `apply_to_line(&mut ScanLine)`, applied with `PngGlitch::apply`; per line, parallel | none (deterministic) | infallible |
| `GlitchPresets` (PR #35) | `png-glitch/src/presets/` | extension trait on `PngGlitch` with `xxx(config)` and `xxx_default()` methods; config structs with `validate()`; whole image; optional `Region` | hand-rolled PRNGs seeded from the region coordinates; no seed field | `Result<_, PresetError>` |
| `GlitchFilter` / `Effect` | `glitch-context` | struct with `apply(&self, &mut PngGlitch, &mut dyn RngCore)`; registered in `FilterConfig` for YAML and the CLI | shared, seeded `ChaCha8Rng` from the context | infallible; parameters are clamped, `magnitude <= 0.0` is a no-op |

Facts that constrain the answer:

- `png-glitch` has no `rand` dependency.
- `glitch-context` was created for WebAssembly support. That role is now taken by the planned component crate (#83, #86), and `glitch-context` no longer had a role of its own when this ADR was written. Today it is used by the CLI and the `glitch_context_pipeline` fuzz target.
- The PR #35 effects read and write scan-line bytes through `ScanLine` and the `Pixel` API. None of them recomputes filtered values, so all of them fit the design principles in [ADR 0002](0002-design-principles.md).
- PR #35's effects differ in how much they depend on randomness:

  | PR #35 effect | Uses randomness? |
  | :------------ | :--------------- |
  | `rgb_shift` | No |
  | `pixel_sort` | No |
  | `scanlines` | Only the `Noise` pattern; solid, dashed and dotted patterns are deterministic |
  | `degrade` | Only the compression and posterization artifacts; blocking and banding are deterministic |
  | `noise` | Yes, entirely |

  Its randomness comes from hand-rolled PRNGs seeded from region coordinates, so the output can never be varied by a seed.
- The terminology issue #22 is closed: the glossary in `DEVELOPER_GUIDE.md` defines *preset* as the `png-glitch` per-line transformation. PR #35 reuses "preset" for something else.
- The typed-error issue #21 is closed: `png-glitch` returns `PngError`.

## Questions

### Q1. Home — **decided**

Options considered: **A.** `png-glitch` (as in PR #35, with a new `rand` dependency); **B.** `glitch-context`, next to the existing effects; **C.** split by kind.

**Decision:** High-level effects live in **`png-glitch`**, and `png-glitch` takes **no `rand` dependency**. They do **not** move to `glitch-context`, whose new role is decided in [ADR 0004](0004-png-glitch-and-recipe-crates.md).

`png-glitch` provides **deterministic** effects only. Effects that need randomness are implemented by users on top of them (Q4).

### Q4. Randomness — **decided**

Options considered: **A.** a seed field in each config; **B.** an RNG argument (`&mut dyn RngCore`); **C.** no randomness in the library.

**Decision: C. The caller decides, the library executes.** Every effect in `png-glitch` is a deterministic function of the image and its parameters. Where an effect has a "where" or "how much" that a random effect would choose (which rows, which pixels, which offsets), that choice is an explicit parameter. A user who wants randomness draws those parameters from their own RNG, with their own seed, and calls the effect.

For the PR #35 effects this means:

- `rgb_shift`, `pixel_sort`: ported as they are.
- `scanlines`: the solid, dashed and dotted patterns are ported. The `Noise` pattern is not.
- `degrade`: blocking and banding are ported. Compression and posterization are ported only if they can be expressed deterministically, for example by taking an explicit dither or error pattern as a parameter; otherwise they are dropped.
- `noise`: not ported as a library effect. It becomes an **example** (`crates/png-glitch/examples/`) that implements random noise on top of `ScanLine` / `Pixel` with `rand` as a dev-dependency, as a model for user-implemented random effects.
- The hand-rolled PRNGs are removed.

Reproducibility is then the caller's responsibility: the library guarantees that the same image and the same parameters give the same bytes.

### Premises for Q2, Q3 and Q6

1. **A preset is a kind of recipe:** a named recipe shipped with the library (#80). It is data, and lives in `png-glitch-recipe` (ADR 0004).
2. **A deterministic operation on a PNG belongs in `png-glitch`.** Whether it processes one row or the whole image is an implementation and performance detail, not a conceptual difference.

It follows that today's `GlitchPreset`s (`Invert`, `Brighten`, `Darken`, `ShiftChannels`) are not presets: they are deterministic operations, so they are `png-glitch` **effects** that were misnamed. A preset whose steps are all concrete is still a preset (data), and stays in `png-glitch-recipe`; premise 2 applies to operations as code. Composing effects in code needs no preset: call `PngGlitch::apply` repeatedly.

| Concept | What it is | Crate | Deterministic? |
| :------ | :--------- | :---- | :------------- |
| **Effect** | Code: a deterministic operation on a PNG, with parameters | `png-glitch` | Always |
| **Recipe** | Data: an ordered list of steps that refer to effects; each parameter is a value or a random specification | `png-glitch-recipe` | May contain random specifications |
| **Resolved recipe** | Data: a recipe whose parameters are all concrete | `png-glitch-recipe` | Yes |
| **Preset** | Data: a named recipe shipped with the library | `png-glitch-recipe` | May contain random specifications |

### Q2. One abstraction or several? — **decided**

Some PR #35 effects need more than one row (vertical `pixel_sort`, 2D `rgb_shift` offsets, `degrade` blocking), so the per-line `GlitchPreset::apply_to_line` is not enough for all of them.

- **A. Keep both styles.** Keep `GlitchPreset` and add PR #35's `GlitchPresets` extension trait, with its `xxx(config)` and `xxx_default()` methods.
- **B. One trait with two levels.** One effect trait that receives the whole image (`&mut PngGlitch`), plus a per-line trait (today's `GlitchPreset`) that automatically implements it and keeps the parallel per-line path. Each effect is a struct whose fields are its parameters, with `Default`, `new` and `with_*` builders. `PngGlitch::apply` accepts either.
- **C. Only whole-image.** Replace `GlitchPreset` with a whole-image trait. Simpler, but per-line effects lose automatic parallelism.

**Decision: B, with one public concept.** The public concept is the **effect**: a trait whose implementations receive the whole image (`&mut PngGlitch`). The per-line trait is a helper for implementing effects: anything that implements it implements the effect trait automatically and keeps the parallel per-line path. It is not a separate concept for users.

- There is no `GlitchPresets` trait and no `xxx_default()` methods; `RgbShift::default()` plays that role.
- User-implemented random effects (Q4) implement the same trait, so they compose with built-in ones through `PngGlitch::apply`.
- Methods on `PngGlitch` (`transpose`, `set_filter_type`, …) stay as the primitives effects are built from. **Every operation a recipe step can refer to is also available as an effect struct**, because an effect holds its parameters as a value and a recipe needs that.

### Q3. Naming — **decided**

- **A. Keep "preset"** for everything, including the whole-image effects.
- **B. "Effect"** for the whole-image trait and its implementations (`Scanlines`, `RgbShift`, `PixelSort`, `Degrade`, …). Keep "preset" for the per-line trait until #73.
- **C. As B, and rename the per-line trait now.**

**Decision: C, with "effect".** Given the premises, "preset" means only a named recipe, in `png-glitch-recipe`. In `png-glitch`:

- `GlitchPreset` is renamed to an effect name, and the `presets` module to `effects`. The deprecated old names are kept for one release.
- The unit is called **effect**, not "operation". This matches the existing glossary and the `Effect` alias in `glitch-context`, and keeps "operation" free for the internal `operation` module (`Encode`, `Scan`).

This removes the double meaning of "preset" (#22, PR #35). The exact identifiers (for example `Effect` and `LineEffect`) are settled in #73.

### Q5. Overlap with the existing `glitch-context` effects — **decided**

`pixel_sort` and `rgb_shift` overlap `glitch-context`'s `PixelSort` and `ChromaticAberration`, which take an RNG today.

- **A. Ignore the overlap here** and resolve it in the decision on `glitch-context`'s future (see *Related decision*).
- **B. Port the deterministic core** of the overlapping `glitch-context` effects into the new `png-glitch` effects now. The `glitch-context` versions then only draw the random choices and call them.

**Decision: B**, as a consequence of [ADR 0004](0004-png-glitch-and-recipe-crates.md). Every randomized `glitch-context` effect is split into a deterministic operation in `png-glitch` and a random specification in `png-glitch-recipe`. Overlapping effects share one deterministic core: `pixel_sort` and `PixelSort` become one `png-glitch` effect, and so do `rgb_shift` and `ChromaticAberration`.

### Q6. Errors — **decided**

- **A. Keep `PresetError`** as a second error type.
- **B. No new error type; effects are infallible.** Out-of-range parameters are clamped or become a no-op, as `CONTRIBUTING.md` guideline 1 already requires. Validation that must report failure, if any, uses `PngError`.

**Decision: B, one typed error per crate.**

| Crate | Error | Raised by |
| :---- | :---- | :-------- |
| `png-glitch` | `PngError` | Parsing and encoding. Effects are infallible: out-of-range parameters are clamped or become a no-op. |
| `png-glitch-recipe` | `RecipeError` (new; replaces `anyhow`) | Loading a recipe (YAML, unknown effect), validating random specifications, resolving, and executing a resolved recipe on an image whose header differs from the one it was resolved for |

`PresetError` is not ported: a preset is a recipe, so its errors are `RecipeError`s.

## Related decision

The future of `glitch-context`, and with it where the CLI's randomized effects, `FilterConfig` (YAML) and `--seed` go, is decided in [ADR 0004](0004-png-glitch-and-recipe-crates.md): it becomes `png-glitch-recipe`, which resolves a recipe and a seed into concrete parameters and executes them on `png-glitch`.

## Decision

High-level effects live in `png-glitch`, which stays free of `rand`. Every library effect is deterministic. Randomness is the caller's: random choices are explicit parameters. The project's own randomized effects draw those parameters in `png-glitch-recipe` (ADR 0004); effects that are random by nature and have no recipe step (such as noise) are implemented by users, with an example showing how.

There is one public concept in `png-glitch`, the **effect**; today's `GlitchPreset`s are effects and are renamed accordingly. "Preset" means a named recipe in `png-glitch-recipe`. Effects are infallible; `png-glitch` has `PngError` and `png-glitch-recipe` has `RecipeError`.

## Consequences

- **#64** ports `rgb_shift`, `pixel_sort`, and the deterministic parts of `scanlines` and `degrade` into `png-glitch`, and adds a random-noise example. Recipe steps and CLI flags for them are added in `png-glitch-recipe` (ADR 0004).
- **#65, #66** (`datamosh`, `corrupt_blocks`) are designed as deterministic `png-glitch` effects whose targets (blocks, rows, offsets) are explicit parameters. `corrupt_blocks` corrupts scan-line data, not the compressed stream (ADR 0002, consequence 7).
- **#62** is updated: the "high-level preset API" becomes "deterministic high-level effects in png-glitch".
- **#73** renames `GlitchPreset` / `presets` (with deprecated aliases for one release) and fixes the exact identifiers (Q3), and replaces `anyhow` with `RecipeError` (Q6).
- **#80** (built-in recipes) is where presets in the new sense are added.
- **#77** decides how regions and masks are expressed; PR #35's `Region` is not ported on its own.
- `png-glitch`'s guarantee becomes "same image and parameters → same bytes". A seed-based guarantee (#58) applies only where a caller, such as the CLI, owns the RNG.
