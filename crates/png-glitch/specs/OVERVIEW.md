# png-glitch: Overview

`png-glitch` is a Rust library designed for the intentional and creative corruption of PNG images, often referred to as "glitch art." Unlike standard PNG decoders that aim for perfect fidelity, this library provides fine-grained control over the low-level structures of the PNG format to produce visual distortions.

## High-Level Goals

1.  **Low-Level Access:** Provide direct access to raw scanlines and individual pixels before and after filter application.
2.  **Intentional Misinterpretation:** Allow users to manipulate PNG filter types and data in ways that standard decoders must "fight" to render, resulting in glitches.
3.  **Performance:** Enable high-speed glitching through optimized pixel processing and parallel scanline manipulation.
4.  **Simplicity:** Offer a fluent, chainable API for common glitching recipes (presets).

## Core Architecture

The library is built around three main components:

### 1. `Parser`
Located in `src/png/parser.rs`, it handles the initial reading of the PNG signature and chunks. It specifically focuses on decompressing the `IDAT` stream while preserving the order and structure of ancillary chunks.

### 2. `Png` & `PngGlitch`
*   `Png` (`src/png.rs`) is the internal representation of a decoded PNG image, holding metadata (header), pixel data, and chunks.
*   `PngGlitch` (`src/lib.rs`) is the primary public entry point. It wraps a `Png` object and provides the fluent API for glitching.

### 3. `ScanLine`
The `ScanLine` struct (`src/png/scan_line.rs`) represents a single row of pixels in the image. It is the fundamental unit of manipulation, providing methods to apply/remove PNG filters and access pixels regardless of bit depth.

## Key Concepts

*   **Filtered vs. Unfiltered Data:** PNG data on disk is filtered. To glitch it effectively, you typically "remove" the filter (undoing the prediction), manipulate the raw pixel values, and then either leave it unfiltered or "apply" a different filter type to cause divergent rendering.
*   **Transposition:** Swapping scanlines or chunks of scanlines to create horizontal shearing effects.
*   **Presets:** Pre-defined closures or structs that implement the `GlitchPreset` trait for common effects like inverting colors or shifting channels.
