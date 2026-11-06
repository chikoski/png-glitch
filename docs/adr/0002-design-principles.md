# 0002. Adopt "a valid PNG with strange data inside" as the design principle

- **Status:** Accepted
- **Date:** 2026-11-06
- **Issue:** #94

## Context

While planning real-time (webcam) and WebAssembly support (#99), a proposal was made to exchange plain RGBA frames between stages. That would be faster, but it would reduce png-glitch to a generic RGBA effect library and remove what makes its output distinctive.

On `master`, the look comes from a specific mechanism:

- Every glitch-context effect mutates only the **scan-line layer**: the per-row filter-type byte and the filtered bytes.
- The **compressed layer** (IDAT / zlib, CRC, Adler-32) is regenerated cleanly on save. Other chunks pass through.
- A standard decoder **reconstructs** rows according to the PNG specification, and prediction errors propagate right (Sub), down (Up) and diagonally (Average, Paeth).

The existing documentation did not say this, and in one place (`DEVELOPER_GUIDE.md`, "The Glitch Philosophy") it suggested the opposite: that the library performs operations that are "illegal" or "undefined" in standard decoders. Without a written principle, API and performance work (#68, #95, #96, #97, #100) has no shared criterion to be reviewed against.

## Options

### Option A: No explicit principle

Each feature is judged on its own. Performance work is likely to drift towards RGBA-only pipelines, and features that corrupt the compressed stream are as acceptable as any other.

### Option B: "Glitch anything"

Any byte of the file, including the zlib stream and chunk structure, is fair game. This maximises variety, but most zlib corruption produces files that decoders reject or truncate, so results depend on the decoder rather than on the PNG specification.

### Option C: "A valid PNG with strange data inside"

The file format is always valid; only the scan-line data is strange. The look is whatever a conforming decoder makes of that data.

## Decision

Adopt Option C. The full text of the principle and its consequences is maintained in [`DEVELOPER_GUIDE.md` → Design principles](../../DEVELOPER_GUIDE.md#design-principles), and is summarised in the `png-glitch` crate documentation.

In particular, this ADR decides that:

1. **The core model is always a PNG:** IHDR plus the filtered scan-line stream. Effects operate only on that model.
2. **Live mode is "a PNG whose compression is deferred"** (`D(G(E(frame)))`, #100). Compression and checksums happen only when a file is produced.
3. **Live rendering must be identical to a PNG decoder** (#96).
4. **PNG encoding choices are expressive parameters,** not plumbing (#95).
5. **Performance trade-offs reduce resolution or frame rate, not expressive range.** Paeth is not dropped for speed (#97, #99).
6. **Setting a filter-type tag and computing filtered values are different operations.** Glitches only overwrite bytes; recomputing filtered values after glitches is an explicit *generation boundary*.
7. **Corrupting the compressed stream is out of scope.** If it is ever added, it is a separate, clearly labelled mode with its own ADR.

The principle applies to the PNG path (`png-glitch`, the PNG side of `glitch-context`, the CLI). `webp-glitch` has a different container and codec structure and is not covered by this ADR.

## Consequences

- New effects and API changes are reviewed against the checklist in `DEVELOPER_GUIDE.md`; `CONTRIBUTING.md` links to it.
- The "Glitch Philosophy" paragraph in `DEVELOPER_GUIDE.md` is rewritten to match.
- #95, #96, #97 and #100 use this ADR as their acceptance criterion.
- A future "corrupt the zlib stream" feature needs a new ADR rather than a pull request alone.
- Operations that recompute filtered values (`remove_filter`, `apply_filter`, `reencode_with` / `change_filter_type`, and the glitch-context filters built on them) keep working, but their documentation should say that they start a new generation. Renaming them, if needed, is left to #73.
