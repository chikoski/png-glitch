# 0006. Pixel-level targeting: how `rect=` and `mask=` restrict an effect

- **Status:** Proposed
- **Date:** 2026-11-08
- **Issue:** #187 (part of #77, Phase 4 #118; builds on #72 and #185; affects #186)

## Context

#77 asks for effects that act only on part of an image: a row range, a rectangle (`rect=x,y,w,h`) or a grayscale mask image (`mask=mask.png`, with `invert-mask`). Row ranges are split out into #185 (library: `ScanLines::rows(range)` and `WithinRows<E>`) and #186 (recipe and CLI: `rows=a..b`). A row range needs no new mechanism, because an effect simply runs on a sub-view. A rectangle or a mask can't be done that way: most effects that move data (`ShiftLines`, `ChannelShift`, `PixelSort`, `SwapBlocks`, …) read and write across the whole width of a row.

Constraints:

- **The scan-line layer is what effects edit** ([ADR 0002](0002-design-principles.md), `DEVELOPER_GUIDE.md`). Effects read and write the stored bytes: the filter-type tag and the filtered bytes. Pixel access on filtered bytes is a deliberate technique (`docs/CLI_GUIDE.md`, "Filtered vs. unfiltered"). A decoder reconstructs each row from the one above (Up / Average / Paeth). A change inside a region therefore also changes *decoded* pixels below it, unless the rows below use None or Sub.
- **A glitch step never recomputes filtered values.** Recomputing them is a new generation, an explicit boundary ([ADR 0005](0005-live-pipeline.md), decision 4: `regenerate` / `Refilter`).
- **Live mode** ([ADR 0005](0005-live-pipeline.md)): a `ScanLinesEffect` runs once per frame and should make no heap allocation. `Effect` and `ScanLinesEffect` take `&self`. A recipe step runs live through `ResolvedEffect::new_live`, which requires `Send + Sync`.
- **`png-glitch-recipe` does no file I/O.** Recipes are data (`Recipe`, `FilterConfig`), and the crate builds for every wasm target (#84). The CLI reads files.
- **Reproducibility** (`docs/API_STABILITY.md`): the same recipe, seed and image give the same bytes. `--embed-recipe` stores the effective recipe in the output so a result can be re-run.

Five questions are open (#187):

1. How the bytes outside the target are kept unchanged.
2. What a mask value means.
3. Whether masking works on filtered bytes, or requires unfiltered data.
4. Where and how a mask image is loaded, and how a recipe refers to it.
5. Whether masks work in the live pipeline.

## Options

### Question 1: keeping the outside unchanged

#### Option 1A: run the effect, then restore the outside from a snapshot (a generic wrapper)

A wrapper `Masked<E>` copies the stored bytes before running `E` on the whole view, then writes the original bytes back where the mask is 0, and blends where it is between 0 and 255.

- Works for every existing and future `ScanLinesEffect` with no change to the effects.
- Needs a snapshot buffer of the image's scan-line data (`stride × height` bytes; about 8.3 MB at 1080p RGBA). Copying it costs about 0.6 ms in wasm (#99's measurement for an 8.3 MB copy), plus the blend.
- If the buffer is allocated per call, the wrapper breaks the live allocation rule.

#### Option 1A′: as 1A, but the buffer is allocated once, when the wrapper is built

The same as 1A, but `Masked::new` allocates the snapshot buffer for a given image size. It is kept behind a `std::sync::Mutex<Vec<u8>>`, because `apply_to_scan_lines` takes `&self` and live steps must be `Send + Sync`. An uncontended lock doesn't allocate. The buffer grows only if the wrapper is used on a larger image than it was built for.

- Keeps all the advantages of 1A, and is allocation-free in steady state.
- Adds a lock and interior mutability to one effect type.

#### Option 1B: each effect takes a mask parameter

- No snapshot and no extra copy.
- Every effect needs its own masked code path, plus tests. That multiplies the work, and user-defined effects get nothing.

### Question 2: what a mask value means

- **2A: binary.** Inside the mask (value ≥ 128), the effect applies; outside, it doesn't.
- **2B: strength.** Each stored sample becomes `orig + (result − orig) × m / 255`. With option 1A/1A′ this is generic. A mask containing only 0 and 255 behaves exactly like 2A.
- **2C: probability.** The mask scales a step's per-row probability (`magnitude`). This lives in the recipe's random draws, means something different for each step, and doesn't help deterministic `png-glitch` effects.

### Question 3: filtered bytes

- **3A: mask the stored bytes as they are.** The mask selects and blends the stored samples, filtered or not. The downward spread of a change when the image is decoded is documented, not prevented.
- **3B: require unfiltered data.** `Masked` refuses (no-op, or an error in the recipe) unless every row has filter type None.
- **3C: the wrapper removes the filters, runs the effect, and re-applies the filters.** This recomputes filtered values inside a glitch step.

### Question 4: loading the mask and referring to it

- **4A: `png-glitch-recipe` reads the mask file** named in the recipe. That is simple for the CLI, but it adds file I/O to the recipe crate and makes recipes depend on the file system (wasm, servers).
- **4B: the library takes a mask as data.** The host (the CLI) loads the file and hands the `Mask` values to the recipe. The recipe stores only a reference.

How a saved or embedded recipe refers to the mask:

- **4B-i: path only.** A re-run silently uses whatever file is at that path now.
- **4B-ii: path plus a checksum of the file's bytes.** A re-run with a different file is an error.
- **4B-iii: embed the mask data** in the recipe or the PNG. Self-contained, but it can be as large as the image.

### Question 5: live

- **5A: supported.** This needs 1A′ or 1B.
- **5B: file workflow only.** Plain 1A is enough, and `Masked` is rejected by `LiveRecipe` (`RecipeError::NotLive`).

## Decision

**1A′, 2B, 3A, 4B with 4B-ii, 5A.**

1. **`Masked<E>` in `png-glitch`'s `effects` module** wraps any `ScanLinesEffect` (so it is itself a `ScanLinesEffect`, and therefore an `Effect`). It holds a `Mask` and a snapshot buffer allocated in `Masked::new` for the image size it is built for. The buffer is behind a `std::sync::Mutex`. Per call it:
   1. copies the stored bytes of every row into the buffer. An effect can move bytes between rows (`SwapLines`, `CopyLines`, `SwapBlocks`), so rows the mask doesn't touch have to be restored too;
   2. runs `E` on the whole view;
   3. for each pixel, writes back `orig`, `result` or the blend, according to the mask value (decision 2).
   
   `Masked` makes no heap allocation in steady state. If the image is larger than the buffer, the buffer grows once.
   
   Effects that are only an `Effect` (today `Refilter`, the generation boundary) can't be wrapped. In a recipe, a mask on a step that resolves to `Refilter` or `regenerate` (the remove-filter / sub / up / average / paeth filter steps and `regenerate`) is a `RecipeError`.
2. **Mask values are a strength.** For each stored sample, `out = (orig × (255 − m) + result × m + 127) / 255`, in integer arithmetic. 16-bit samples use the same formula on `u16` with `m` widened as `m × 257` and the divisor 65535. `invert` uses `255 − m`. Special cases:
   - `m == 0` → `orig`, and `m == 255` → `result`, exactly.
   - **Indexed colour:** blending palette indices is meaningless, so the mask is thresholded (`m ≥ 128` → `result`).
   - **Packed 1/2/4-bit samples:** thresholded the same way.
   - **The filter-type tag of a row** comes from `result` if any mask value in that row is ≥ 128, and from `orig` otherwise.
3. **Masking works on the stored bytes as they are** (3A). `Masked` never removes or re-applies filters. Rustdoc and `docs/CLI_GUIDE.md` state that a change inside the mask can show up in decoded rows below it, and that users who want an exact pixel region put `remove-filter` before the masked step. That is an ordinary recipe step, so it is the user's choice.
4. **`png-glitch` defines `Mask` as data**: width, height and one `u8` per pixel (`Mask::from_values(w, h, Vec<u8>)`), plus `Mask::rect(w, h, x, y, rw, rh)`. A rectangle is a mask with only 0 and 255, so `rect=` uses the same mechanism and needs no special case. It is stored without a per-pixel array. Neither `png-glitch` nor `png-glitch-recipe` reads files.
   - **The CLI loads `mask=FILE`:** a PNG decoded with `Renderer`, converted to a value per pixel by integer Rec. 601 luma (the `Pixel` helpers from #72), multiplied by alpha / 255 when the mask has alpha. The CLI supplies the result to the recipe.
   - **The mask must have the same width and height as the image.** Otherwise the CLI exits with an error, with no scaling.
   - **A recipe step refers to a mask by `mask: { path, crc32 }`**, the CRC-32 of the file's bytes (`crc32fast` is already a CLI dependency). `--print-recipe`, `--save-recipe` and `--embed-recipe` write both. When loading, a checksum mismatch is an error. A hand-written recipe without `crc32` loads with a warning, and the printed recipe fills the checksum in.
5. **Masks work live.** `Masked` is allocation-free in steady state, so it can be used through `ResolvedEffect::new_live` and `LiveRecipe`. Its cost (the snapshot copy plus the blend, at 1080p) is added as a stage to the bench harness, so its share of #99's 5 ms effects budget is visible.

Row ranges (#185 / #186) stay the cheap path for row-only targeting. `rows=` and `mask=` may be combined. The step then runs on the row sub-view, and the mask is cropped to the same rows.

## Consequences

- One wrapper makes every `ScanLinesEffect`, including user-defined ones, maskable. No effect needs a masked code path.
- `rect=` and `mask=` share the mechanism, the tests and the documentation.
- Live use costs one extra copy of the scan-line data plus a blend per masked step, which has to be measured against #99's budget. An optimisation for sparse masks (skipping rows the effect can't reach) would need effects to declare which rows they touch. It is out of scope here.
- Recipes that use masks are reproducible only together with the mask file. The checksum makes a wrong or edited file an error rather than a silently different result. A self-contained alternative (embedding the mask, 4B-iii) is left for a later ADR if it is needed.
- Blending stored filtered bytes produces its own kind of glitch at soft mask edges. That is intended (ADR 0002), and the docs show an example both with and without `remove-filter`.
- Follow-up work: #187 is implemented in two PRs after this ADR is accepted:
  - `png-glitch`: `Mask` and `Masked<E>`, with the tests and the allocation test;
  - `png-glitch-recipe` + CLI: `rect=` / `mask=` / `invert-mask` keys, mask references with CRC-32, loading in the CLI, and docs.
  
  #186's `rows` key and this ADR's `mask` key should share how a step's target is stored next to its `FilterConfig`.
