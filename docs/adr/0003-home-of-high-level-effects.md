# 0003. Where high-level effects live

- **Status:** Proposed (Q1 and Q4 decided; Q2, Q3, Q5, Q6 open)
- **Date:** 2026-11-06
- **Issue:** #63 (part of epic #62; blocks #64; affects #65, #66, #73)

> This ADR is partly decided. Q1 and Q4 record the maintainer's decision. The remaining questions list options and a **recommendation**; they are settled on the pull request, and the status changes to Accepted once all are decided.

## Context

PR #35 (`extension-by-kiro`, commit 92c92b5) adds a high-level API for common glitch effects. Its five implemented effects (`scanlines`, `noise`, `rgb_shift`, `degrade`, `pixel_sort`) are to be ported by #64; `datamosh` (#65) and `corrupt_blocks` (#66) are `todo!()`. Before porting, we need to decide where the code goes. After PR #35 there would be three overlapping ways to express "an effect":

| | Where | Shape | Randomness | Errors |
| :-- | :---- | :---- | :--------- | :----- |
| `GlitchPreset` (on `master`) | `png-glitch/src/presets.rs` | struct with `apply_to_line(&mut ScanLine)`, applied with `PngGlitch::apply`; per line, parallel | none (deterministic) | infallible |
| `GlitchPresets` (PR #35) | `png-glitch/src/presets/` | extension trait on `PngGlitch` with `xxx(config)` and `xxx_default()` methods; config structs with `validate()`; whole image; optional `Region` | hand-rolled PRNGs seeded from the region coordinates; no seed field | `Result<_, PresetError>` |
| `GlitchFilter` / `Effect` | `glitch-context` | struct with `apply(&self, &mut PngGlitch, &mut dyn RngCore)`; registered in `FilterConfig` for YAML and the CLI | shared, seeded `ChaCha8Rng` from the context | infallible; parameters are clamped, `magnitude <= 0.0` is a no-op |

Facts that constrain the answer:

- `png-glitch` has no `rand` dependency.
- `glitch-context` was created for WebAssembly support. That role is now taken by the planned component crate (#83, #86), and `glitch-context` no longer has a role of its own. Today it is used by the CLI and the `glitch_context_pipeline` fuzz target.
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

**Decision:** High-level effects live in **`png-glitch`**, and `png-glitch` takes **no `rand` dependency**. They do **not** move to `glitch-context`, which has no role any more (see *Out of scope*).

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

### Q2. One abstraction or several? — open

Some PR #35 effects need more than one row (vertical `pixel_sort`, 2D `rgb_shift` offsets, `degrade` blocking), so the per-line `GlitchPreset::apply_to_line` is not enough for all of them.

- **A. Keep both styles.** Keep `GlitchPreset` and add PR #35's `GlitchPresets` extension trait, with its `xxx(config)` and `xxx_default()` methods.
- **B. One trait with two levels.** One effect trait that receives the whole image (`&mut PngGlitch`), plus a per-line trait (today's `GlitchPreset`) that automatically implements it and keeps the parallel per-line path. Each effect is a struct whose fields are its parameters, with `Default`, `new` and `with_*` builders. `PngGlitch::apply` accepts either.
- **C. Only whole-image.** Replace `GlitchPreset` with a whole-image trait. Simpler, but per-line effects lose automatic parallelism.

**Recommendation: B.** There is no `GlitchPresets` trait and no `xxx_default()` methods; `RgbShift::default()` plays that role. User-implemented random effects (Q4) implement the same trait, so they compose with built-in ones through `PngGlitch::apply`.

### Q3. Naming — open

- **A. Keep "preset"** for everything, including the whole-image effects.
- **B. "Effect"** for the whole-image trait and its implementations (`Scanlines`, `RgbShift`, `PixelSort`, `Degrade`, …). Keep "preset" for the per-line trait until #73.
- **C. As B, and rename the per-line trait now.**

**Recommendation: B.** It avoids PR #35's second, different meaning of "preset". The final names, including whether the per-line trait keeps the name `GlitchPreset`, are part of the API tidy-up in #73. "Recipe" stays reserved for a named combination of effects (#69, #80).

### Q5. Overlap with the existing `glitch-context` effects — open

`pixel_sort` and `rgb_shift` overlap `glitch-context`'s `PixelSort` and `ChromaticAberration`, which take an RNG today.

- **A. Ignore the overlap here** and resolve it in the decision on `glitch-context`'s future (see *Out of scope*).
- **B. Port the deterministic core** of the overlapping `glitch-context` effects into the new `png-glitch` effects now. The `glitch-context` versions then only draw the random choices and call them.

**Recommendation: A.** The new `png-glitch` effects are designed from PR #35 and the principles alone, without being constrained by the current `glitch-context` API. How the existing randomized effects map onto them is part of the separate decision.

### Q6. Errors — open

- **A. Keep `PresetError`** as a second error type.
- **B. No new error type; effects are infallible.** Out-of-range parameters are clamped or become a no-op, as `CONTRIBUTING.md` guideline 1 already requires. Validation that must report failure, if any, uses `PngError`.

**Recommendation: B.** It is consistent with `PngGlitch::apply` and the existing presets, and keeps one typed error per crate. `PresetError` is not ported.

## Out of scope

**The future of `glitch-context`**, and with it where the CLI's existing randomized effects, `FilterConfig` (YAML) and `--seed` go. Since the effects stay in `png-glitch` and `glitch-context` has no role, that is a separate decision with its own ADR. It is needed before #64 touches the CLI and before #73 settles the module layout.

## Decision (so far)

High-level effects live in `png-glitch`, which stays free of `rand`. Every library effect is deterministic. Randomness is the caller's: random choices are explicit parameters, and effects that are random by nature (such as noise) are implemented by users, with an example showing how.

## Consequences

- **#64** ports `rgb_shift`, `pixel_sort`, and the deterministic parts of `scanlines` and `degrade` into `png-glitch`, and adds a random-noise example. It does not add `glitch-context` effects or CLI flags until the `glitch-context` decision is made.
- **#65, #66** (`datamosh`, `corrupt_blocks`) are designed as deterministic `png-glitch` effects whose targets (blocks, rows, offsets) are explicit parameters. `corrupt_blocks` corrupts scan-line data, not the compressed stream (ADR 0002, consequence 7).
- **#62** is updated: the "high-level preset API" becomes "deterministic high-level effects in png-glitch".
- **#73** decides the final names (Q3).
- **#77** decides how regions and masks are expressed; PR #35's `Region` is not ported on its own.
- `png-glitch`'s guarantee becomes "same image and parameters → same bytes". A seed-based guarantee (#58) applies only where a caller, such as the CLI, owns the RNG.
