# 0004. Split responsibilities between `png-glitch` and `png-glitch-recipe`

- **Status:** Accepted
- **Date:** 2026-11-06
- **Issue:** #115 (follows from #63 and [ADR 0003](0003-home-of-high-level-effects.md))

## Context

[ADR 0003](0003-home-of-high-level-effects.md) decided that high-level effects live in `png-glitch`, that `png-glitch` has no `rand` dependency, and that every effect in it is deterministic, with random choices taken as explicit parameters. That leaves the question of what `glitch-context` is for.

`glitch-context` was created for WebAssembly support, a role now taken by the planned component crate (#83, #86). Today it bundles six responsibilities:

| # | Responsibility | On `master` |
| :- | :------------- | :---------- |
| 1 | The glitch operations themselves | ~25 effects that swap rows, overwrite bytes, shift channels, … |
| 2 | Random choices | e.g. `Transpose` picks rows with `rng.random_bool(magnitude)` and then calls `png.transpose(i, target, 1)` |
| 3 | RNG and seed management | `ChaCha8Rng`, `seed: Option<u64>`, fallback to an OS seed |
| 4 | Pipeline | `add_filter` / `execute`: effects run in order |
| 5 | Configuration as data | `FilterConfig`, YAML via `serde`, `clap` `ValueEnum` |
| 6 | Multiple formats | PNG and WebP behind `AnyGlitchContext` / `open_any`, `PixelFilter` adapters |

After ADR 0003, responsibility 1 moves to `png-glitch`. Responsibilities 2 to 6 still need a home. Facts that matter:

- The CLI, the planned WebAssembly component (#86), the web playground (#79) and the exploration workflow (#76) all need the same random choices and the same configuration format. If these live in the CLI, the others must reimplement them.
- Several open issues are about configuration as data, independent of any image: #68 (separate the recipe from the image), #69 (share and embed recipes), #80 (named looks), #85 (effect metadata as data).
- Today, reproducibility rests on the seed alone (#58). `rand` makes value-breaking changes only in breaking releases, but such an upgrade still changes the output for a given seed and breaks the golden outputs (#58, #120).
- [ADR 0002](0002-design-principles.md)'s principles cover the PNG path only. WebP has a different container and codec structure.

## Options

- **A. Randomness layer.** Turns a seed and a description of "how much" (probabilities, ranges) into concrete parameters for `png-glitch` effects (responsibilities 2, 3).
- **B. Recipe layer.** An ordered list of effects with parameters, separate from the image, serializable and introspectable (responsibilities 4, 5).
- **C. Multi-format layer.** Keep PNG and WebP together (responsibility 6).
- **D. Live-session layer.** A resident image, a frame loop and generations (#87, #100).
- **E. Dissolve.** Fold everything into the CLI and remove the crate.

## Decision

`glitch-context` becomes **`png-glitch-recipe`**: the layer that turns a recipe and a seed into deterministic calls on `png-glitch` (options A + B). WebP moves to a separate crate (not C). D is left to a later ADR. E is rejected because the CLI is not the only consumer.

### `png-glitch`: deterministic effects on a PNG

- The PNG model (IHDR plus the filtered scan-line stream), parsing and encoding.
- Deterministic effects: given an image and parameters, always the same bytes. This includes the deterministic core of today's `glitch-context` effects (for `Transpose`: "swap these rows").
- The trait users implement for their own effects (ADR 0003, Q2).
- No `rand`, no `serde`.

### `png-glitch-recipe`: recipes, randomness and execution

It depends on `png-glitch` only (plus `rand`, `rand_chacha`, `serde`), and has two stages that are **separate in the public API**:

1. **Resolve: `Recipe` + seed → `ResolvedRecipe`.**
   - A `Recipe` is an ordered list of steps. Each step names a `png-glitch` effect, and each parameter is either a concrete value or a random specification (a probability, a range, …). A recipe is not tied to an image; it can be serialized (YAML) and listed with its metadata (#85).
   - Resolving draws every random specification from an RNG seeded with the given seed (or an OS seed, which is then reported, as the CLI does today). It needs the image's header (width, height, color type, bit depth), because choices such as "which rows" depend on the size, but not its pixel data.
   - All randomness in the project's own code happens here, and only here.
2. **Execute: `ResolvedRecipe` + image → glitched image.**
   - A `ResolvedRecipe` has only concrete parameters. It is serializable, so it can be printed and embedded in the output (#69).
   - Executing calls the `png-glitch` effects in order. It is deterministic and uses no RNG.

A `ResolvedRecipe` reproduces its output without a seed and without `rand`, so reproducibility no longer depends on the RNG implementation. A seed reproduces the output only as long as the RNG's value stream stays the same.

### WebP

WebP support (today's `WebpGlitchContext`, the WebP effects and the WebP side of `AnyGlitchContext` and `PixelFilterAdapter`) moves out of `png-glitch-recipe` into a separate crate. That crate's name and whether it mirrors the recipe structure are decided when it is split out. It is not covered by ADR 0002.

### The CLI

The CLI becomes a thin layer: it builds a `Recipe` from flags or a YAML file, detects the input format and dispatches to the PNG or WebP crate, resolves, executes, and reads and writes files.

## Consequences

- **Rename.** `crates/glitch-context` is renamed to `crates/png-glitch-recipe`. On crates.io, `png-glitch-recipe` is a new crate; the last `glitch-context` release should say in its README and crate docs that it has moved.
- **Splitting the existing effects.** Each of today's randomized effects is split into a deterministic operation in `png-glitch` and a random specification in `png-glitch-recipe`. This is the answer to ADR 0003, Q5: the overlapping effects (`PixelSort`, `ChromaticAberration`) share one deterministic core with the PR #35 ports.
- **Configuration compatibility.** `FilterConfig` becomes the step format of a `Recipe`. Existing YAML files and CLI flags should keep working, or the change is listed in `CHANGELOG.md` with a migration note.
- **Golden outputs.** Existing seeds should keep producing the same bytes after the split, which the golden tests (#58, #120) check. Where that is impossible, the change is deliberate and noted.
- **Issues that move to `png-glitch-recipe`:** #68, #69, #76, #80, #85. #70 (escape hatches for custom effects) is re-scoped: a custom effect is a `png-glitch` effect, and a recipe step can refer to it.
- **WebAssembly.** #84 ("make glitch-context build for every wasm target") is retitled for `png-glitch-recipe`; with WebP split out, the recipe crate no longer pulls in `libwebp-sys2`. #86 builds on `png-glitch` and `png-glitch-recipe`.
- **Fuzzing.** The `glitch_context_pipeline` target is renamed and split into a recipe target (resolve and execute) and a WebP target.
- **`PixelFilter`.** Its purpose was a filter shared by PNG and WebP. With WebP split out, it is revisited in #73, together with #71 (8-bit only).
- **Terminology.** The glossary in `DEVELOPER_GUIDE.md` gains *recipe*, *resolved recipe*, *resolve* and *execute* when the rename lands.
- **Live mode** (#87, #100): whether a live session re-resolves per frame or reuses one `ResolvedRecipe` is decided with the live-session design.
