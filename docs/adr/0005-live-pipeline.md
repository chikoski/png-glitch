# 0005. The live pipeline: where it lives, what `G` may touch, per-frame seeds and generations

- **Status:** Accepted
- **Date:** 2026-11-07
- **Issue:** #100 (part of #117; builds on #95, #96, #97; affects #87, #139, #146, #147)

## Context

#100 describes live mode as `frame_out = D(G(E(frame_in)))`:

- `E`: pixels → scan lines. This is the PNG encoder's filter step.
- `G`: scan lines → scan lines. This is the glitch.
- `D`: scan lines → pixels. This is the PNG decoder's reconstruction step.

No compression and no checksums run per frame. The design principles ([ADR 0002](0002-design-principles.md), `DEVELOPER_GUIDE.md`) already require this. [ADR 0004](0004-png-glitch-and-recipe-crates.md) left the live-session layer (its option D) to a later ADR, and said that "whether a live session re-resolves per frame or reuses one `ResolvedRecipe` is decided with the live-session design". This is that ADR.

What exists on `master` (248a25a):

| Stage | API | Allocates per call? |
| :---- | :-- | :------------------ |
| `E` | `PngGlitch::from_pixels(PixelSource, Encoding)` once, then `PngGlitch::reload_pixels(&[u8])` per frame (#95) | `reload_pixels`: no (`crates/png-glitch/tests/alloc.rs`) |
| `D` | `Renderer::new(&PngGlitch)` once, then `Renderer::render_into(&PngGlitch, &mut [u8])` per frame (#96) | `render_into`: no |
| serialize | `PngGlitch::to_bytes` / `encode` / `save` | yes (deflate, CRC, Adler-32) |
| `G` | `Effect::apply_to(&self, &mut PngGlitch)` (`crates/png-glitch/src/effects.rs`); every `LineEffect` is an `Effect` through a blanket impl | depends on the effect, see below |
| recipe | `Recipe::resolve(&ImageHeader, Option<u64>) -> ResolvedRecipe`, then `ResolvedRecipe::execute(&mut PngGlitch)` (`crates/png-glitch-recipe/src/resolve.rs`) | `resolve`: yes (the steps and their parameter `Vec`s) |

#95 and #96 deliberately put `E` and `D` on `PngGlitch` and `Renderer`, so the model a live session works on is an ordinary `PngGlitch`.

Facts about `G` today:

- `Effect::apply_to` receives the whole `PngGlitch`. Through it, an effect can call `remove_filter`, `apply_filter` and `reencode_with`, which recompute filtered values. Among the built-in effects, only `Refilter` does this.
- Many built-in effects allocate on every call. `PngGlitch::scan_lines()` returns a `Vec<ScanLine>`, used by `SetFilterTypes`, `SetBytes`, `CopyLines`, `ShiftLines`, `OffsetColors` and `SwapBlocks`. Others build temporary rows (`ShiftLines`, `PixelSort`, `ChannelShift`). `LineEffect`s go through `par_foreach_scanline`, which iterates over chunks without allocating.
- The design principles already say that recomputing filtered values after `G` is a **new generation** (`D` then `E`), an explicit boundary rather than a glitch step. #139 proposes a "regenerate" recipe step for it.

Four decisions are open (listed in #100):

1. Where the live layer lives.
2. How `G` runs on scan lines, and which effects are allowed in live mode.
3. How per-frame randomness is seeded.
4. What a new generation is called, and which filter-strategy type it takes, so that the live API and the recipe step proposed in #139 agree.

## Options

### Decision 1: where the live layer lives

- **1A. Split by responsibility.** `LivePipeline` and the scan-line view go in `png-glitch`; the recipe-driven `G` with per-frame seeds goes in `png-glitch-recipe`. This follows ADR 0004: `png-glitch` stays deterministic and RNG-free, and all randomness stays in `png-glitch-recipe`.
- **1B. Everything in `png-glitch-recipe`.** Simpler to find, but then `png-glitch` users without recipes (the wasm component's low-level API, custom hosts) would have to depend on `rand` to get a live loop.
- **1C. A new crate (`png-glitch-live`).** Adds a crate for what is, after #95 and #96, a thin layer over `PngGlitch` and `Renderer`.

### Decision 2: how `G` runs on scan lines

- **2A. Keep `&mut PngGlitch` and rely on documentation.** Nothing stops an effect from recomputing values or allocating, and live mode can't tell which effects are safe.
- **2B. A separate type `ScanLines` that only the live pipeline exposes, with a separate set of effects.** The same glitch would need two implementations, one for files and one for live mode, and they could drift apart. That breaks "the same recipe and seed give the same result in both modes" (#100).
- **2C. A borrowed view `ScanLines<'_>` that both `PngGlitch` and `LivePipeline` hand out, plus a trait `ScanLinesEffect` between `LineEffect` and `Effect`.** One implementation serves both modes. Whether an effect may run live is decided by the type system: it may if it implements `ScanLinesEffect`.

### Decision 3: per-frame randomness

- **3A. Resolve once per session** and execute the same `ResolvedRecipe` on every frame. This allocates nothing per frame and always gives the same glitch (a "static" look).
- **3B. Resolve every frame** with a seed derived from `(seed, frame)`. The glitch changes from frame to frame (an "animated" look), but resolving allocates.
- **3C. Offer both, as an explicit mode.**

How to derive the frame seed:

- `std::hash::DefaultHasher`: its algorithm is not specified and may change between Rust releases, so it can't be part of a reproducibility guarantee.
- A ChaCha stream or word position (`rand_chacha`): this ties the guarantee to `rand_chacha`, which ADR 0004 tried to reduce.
- **SplitMix64**: a published, fixed, ten-line function that hosts outside Rust (for example JavaScript around the wasm component) can recompute.

### Decision 4: generations

- **4A. `LivePipeline::regenerate` as a live-only method**, separate from the recipe step proposed in #139. That gives two names and two code paths for one concept.
- **4B. One operation, `PngGlitch::regenerate(&FilterStrategy)`.** `LivePipeline` and the recipe's "regenerate" step (#139) both use it. `Refilter` and `reencode_with(t)` become the special case `regenerate(&FilterStrategy::Fixed(t))`.

## Decision

**1A, 2C, 3C with SplitMix64, and 4B.**

### 1. Crates

| Item | Crate |
| :--- | :---- |
| `ScanLines<'_>` (view), `ScanLinesEffect` (trait), `PngGlitch::regenerate`, `LivePipeline` | `png-glitch` |
| Running a `Recipe` inside a live loop (`LiveRecipe`, name not final), `frame_seed`, `SeedMode` | `png-glitch-recipe` |

`LivePipeline` is a thin owner of what already exists. It holds the model (made by `from_pixels`), a `Renderer`, the `Encoding` and a reusable scratch buffer (see the allocation rule in 2). Its API follows the sketch in #100:

```rust
let mut live = LivePipeline::new(PixelSource::rgba8(w, h, &first), encoding)?; // allocates once
// per frame:
let lines: ScanLines<'_> = live.encode(&frame_rgba)?;   // E  (reload_pixels)
effect.apply_to_scan_lines(&mut lines);                 // G  (only ScanLinesEffects)
live.decode_into(&mut out_rgba)?;                       // D  (Renderer::render_into)
// off the real-time path:
let png_bytes = live.snapshot()?;                       // serialize (to_bytes)
live.regenerate(&FilterStrategy::Fixed(FilterType::Paeth)); // new generation
```

The wasm `session` resource (#87) maps onto `LivePipeline` one to one.

### 2. `ScanLines` and `ScanLinesEffect`

- **`ScanLines<'a>`** borrows the filtered scan-line buffer, the header fields an effect needs (width, color type, bit depth) and a scratch buffer. It offers only scan-line-layer operations:
  - number of rows;
  - access to one row as a `ScanLine`;
  - iteration over the rows, sequential and parallel;
  - swapping and copying rows and row ranges;
  - reading and writing a row's filter-type tag;
  - the scratch buffer.

  It has **no** operation that recomputes filtered values, changes the header, resizes, compresses or allocates.
- `PngGlitch` gets a method that returns a `ScanLines` view (name decided in the implementation; the existing `scan_lines()` returns a `Vec` and stays as is for now). `LivePipeline::encode` returns the same view.
- **`ScanLinesEffect`** has one method, `apply_to_scan_lines(&self, &mut ScanLines<'_>)`. The blanket impls become a chain:

  ```text
  LineEffect  ──blanket──▶  ScanLinesEffect  ──blanket──▶  Effect
  ```

  A user type that implements only `LineEffect` or only `Effect` keeps working, so the change is not breaking. A chain of two blanket impls compiles. A type that implements `Effect` but not `ScanLinesEffect` (such as `Refilter`) is rejected at compile time where a `ScanLinesEffect` is required. Both were checked with a small prototype.
- **Live mode accepts only `ScanLinesEffect`.** Built-in effects that touch only the scan-line layer move from `impl Effect` to `impl ScanLinesEffect`. They are all built-in effects except `Refilter`. In file mode they behave exactly as before, because `Effect` is derived from `ScanLinesEffect`.
- **Allocation rule.** A built-in `ScanLinesEffect` makes no heap allocation in `apply_to_scan_lines`. Temporary rows come from the view's scratch buffer, which `LivePipeline` (or `PngGlitch`, for the file workflow) allocates once and sizes to at least one scan line. Parameters that are lists (rows to swap, byte edits, …) are owned by the effect value and only read. User effects are not checked, but the rule is documented for them.

### 3. Per-frame randomness

- **`SeedMode`** is part of the live configuration and is recorded with any snapshot or recording:
  - `Static`: resolve once with `seed` and execute the same `ResolvedRecipe` on every frame. A frame allocates nothing.
  - `PerFrame`: before frame `n`, resolve with `frame_seed(seed, n)`.
- **`frame_seed(seed, n)`** is the `n`-th output (0-based) of SplitMix64 started at `seed`:

  ```rust
  fn frame_seed(seed: u64, frame: u64) -> u64 {
      let mut z = seed.wrapping_add((frame.wrapping_add(1)).wrapping_mul(0x9E37_79B9_7F4A_7C15));
      z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
      z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
      z ^ (z >> 31)
  }
  ```

  Test vectors: `frame_seed(0, 0) = 0xE220A8397B1DCDAF`, `frame_seed(0, 1) = 0x6E789E6AA1B965F4`, `frame_seed(42, 0) = 0xBDD732262FEB6E95`, `frame_seed(42, 29) = 0xB1B59F1B53585E43`.

  The function is part of the reproducibility guarantee (#58). Changing it is a breaking change of the output and needs a new ADR. The frame counter starts at 0 when the live recipe starts and counts frames given to `G`, not frames captured.
- **Without a seed** (`None`), one OS seed is drawn when the live recipe starts and reported, as the CLI does today. It is never drawn per frame.
- **Equivalence with the file workflow** (testable): in `Static` mode every frame equals `Recipe::run(model, Some(seed))` applied to `from_pixels` of that frame. In `PerFrame` mode, frame `n` equals the same with `Some(frame_seed(seed, n))`.

### 4. Generations

- **`PngGlitch::regenerate(&FilterStrategy)`** runs `D` then `E` on the model in place: it removes the current filters, then applies the given strategy (`Fixed`, `Pattern` or `Adaptive`, the same `FilterStrategy` as #95). It works for any model, including ones opened from files.
- `regenerate(&FilterStrategy::Fixed(t))` produces the same bytes as `reencode_with(t)`. `Refilter` and `reencode_with` stay, documented as that special case.
- `LivePipeline::regenerate` calls it. It does not count as a frame.
- **#139's "new generation" recipe step** is called `regenerate` and takes a `FilterStrategy`, serialized in the recipe crate's own parameter type (ADR 0004, *Serialization*). In a live recipe, `regenerate` steps split the steps into generations. Each generation's steps must be `ScanLinesEffect`s. A live recipe that contains any other step is rejected with a `RecipeError` when it is built, not per frame.
- Existing recipe steps that resolve to `Refilter` (`RemoveFilter`, `SubFilter`, `UpFilter`, `AverageFilter`, `PaethFilter`) are treated in a live recipe as `regenerate(Fixed(t))`, that is, as generation boundaries. Existing recipes therefore run in live mode and give the same bytes as in the file workflow.
- "One `E` and one `D` per generation" (design principles) stays the rule. A frame is one generation, unless the recipe contains `regenerate` steps.

## Consequences

- **Laws in #100** become tests on `LivePipeline`. Law 1 (`D(E(x)) == x`) and law 3 (a snapshot is a valid PNG and equals `to_bytes()`) are new property tests. Law 2 (`D` equals a reference decoder) is already guaranteed by `Renderer` (#96).
- **Allocation-free frames.** The #100 criterion "a frame performs no allocation" applies to `E`, `D`, the view and `Static` mode. `PerFrame` mode allocates while resolving. That is reported by the bench harness, not asserted, until resolving into reused buffers is designed (a follow-up issue). The zero-heap-growth assertion in #147 covers `E`, `D` and `Static` `G`.
- **Sub-issues of #100** (replacing the list a–e in #100):
  - **a.** `ScanLines` view and `LivePipeline::{new, encode, decode_into, snapshot}`, with laws 1–3 as property tests and a counting-allocator test of a full `Static` frame.
  - **b.** `ScanLinesEffect`, the blanket chain, and porting the built-in scan-line effects to it with the scratch buffer. Tests: bytes are unchanged in the file workflow (golden outputs must not change), and the ported effects make no allocation.
  - **c.** `PngGlitch::regenerate(&FilterStrategy)` and `LivePipeline::regenerate`, with a test that `Fixed(t)` equals `reencode_with(t)`.
  - **d.** `png-glitch-recipe`: `frame_seed` (with the test vectors above), `SeedMode`, the live recipe runner, rejecting non-live steps, and the file-workflow equivalence tests. `ResolvedEffect` today wraps an `Effect`. It also gets a way to wrap a `ScanLinesEffect`, so that user-defined steps (`ResolvedStep::Custom`) can run live. It needs b and c.
  - **e.** #139 (`set-filter-type` and `regenerate` steps in recipes and the CLI), using c. It can be done before or after d.
  - **f.** Add a full live frame (`E`, `G`, `D`) to the #98 harness, in both seed modes, and compare it with the budget in #99.
- **#87** (wasm `session`) builds on `LivePipeline` and the live recipe runner, and **#146** (browser benchmark) on #87.
- **Documentation.** When this is implemented, the "Current operations" table and the glossary in `DEVELOPER_GUIDE.md` gain `ScanLines`, `ScanLinesEffect`, `regenerate`, `LivePipeline`, `SeedMode` and `frame_seed`.
- **Not decided here:** the final method names on `PngGlitch` for the view, the name of the live recipe type, and whether the old `scan_lines() -> Vec<ScanLine>` is deprecated. These are API-detail choices for the sub-issues, under `docs/API_STABILITY.md`.
