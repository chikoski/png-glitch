# Glitch Operations

`png-glitch` provides a variety of operations to manipulate image data, ranging from high-level "presets" to low-level pixel manipulation and scanline transposition.

## 1. Transposition

The `transpose` method allows for the swapping of scanline chunks. This creates horizontal shearing effects.

*   **Signature:** `transpose(&mut self, src: u32, dst: u32, lines: u32)`
*   **Behavior:** It swaps the block of `lines` scanlines starting at `src` with the block of `lines` scanlines starting at `dst`.
*   **Glitch Utility:** Effective for "slicing" an image horizontally.

## 2. Glitch Presets (Recipes)

Presets are pre-defined glitch effects that can be applied to the entire image using a fluent API.

| Preset | Description |
| :--- | :--- |
| `Invert` | Inverts all color channels for every pixel in the scanline. |
| `ShiftChannels` | Shifts R, G, and B values by a specified `(i16, i16, i16)` amount. |
| `Brighten` | Adds a `strength` value to each color channel (saturating at the max value). |

**Fluent usage example:**
```rust
glitch.remove_filter()
      .apply(Invert)
      .apply(Brighten { strength: 50 })
      .save("output.png")?;
```

## 3. Advanced Glitch Filters (glitch-context)

The `glitch-context` crate provides more complex filters that leverage the low-level `PngGlitch` APIs.

| Filter | Description | Algorithm |
| :--- | :--- | :--- |
| `PixelSort` | Sorts pixels within a scanline. | Uses Brightness or Hue as sorting criteria. |
| `Bitwise` | Applies logical operations. | Per-byte `AND`, `OR`, or `XOR` with a constant value. |
| `ChannelSwap` | Exchanges color channels. | Swaps R↔G, G↔B, or B↔R channels using the Pixel API. |
| `HorizontalShift`| Shifts scanline data. | Rotates scanline bytes horizontally with wrap-around. |
| `RandomCopy` | Duplicates scanlines. | Randomly picks source and destination lines to copy. |
| `Substitute` | Replaces bytes at fixed index. | Sets a specific byte in all scanlines to a fixed value. |

## 4. High-Performance Pixel Processing

For custom glitch effects, the `process_pixels` API on `ScanLine` is the most efficient choice.

*   **Closure Signature:** `FnMut(usize, Pixel) -> Pixel`
*   **Why use it?** It resolves the scanline's bit depth and color type **once**, then iterates over pixels in a branch-free loop. 
*   **Utility:** Ideal for threshold-based glitches, noise addition, or channel swapping.

## 4. Parallel Processing

The library utilizes the `rayon` crate to provide parallel manipulation of scanlines.

*   **`par_foreach_scanline`:** Executes a closure on each scanline in parallel. 
*   **Ideal for:** Effects that are independent per scanline (like `Invert` or channel shifting).
*   **Restriction:** Operations that depend on neighbors (like transposition or sequential filter application) should be done on the main thread.
