# glitch-context

A high-level library to manage and apply sequential glitch filter pipelines to **PNG** images, built on top of [`png-glitch`](../png-glitch).

> WebP support moved to the separate [`webp-glitch-context`](../webp-glitch-context) crate, which reuses the configuration types of this crate. Format detection and dispatch between PNG and WebP live in the `png-glitch` CLI.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
glitch-context = "0.1.0"
```

### Cargo Features

- `clap` *(enabled by default)*: Derives `clap::ValueEnum` on configuration enums (`PreProcess`, `SortCriterion`, `BitOp`, `SwapTarget`, `AlphaGlitchStrategy`).

To use `glitch-context` without pulling in `clap`:

```toml
[dependencies]
glitch-context = { version = "0.1.0", default-features = false }
```

## Usage

`glitch-context` provides:
- `GlitchContext`: PNG-specific glitch pipeline.
- `FilterConfig`: Declarative filter steps (YAML / CLI). The WebP-only variants (`MacroblockGlitch`, `AlphaGlitch`, `LossyArtifact`) are accepted by `webp-glitch-context` and rejected by `GlitchContext::add_from_config`.

### Example

```rust
use glitch_context::{BlockScramble, ChromaticAberration, GlitchContext, Invert, SubFilter};

fn main() -> anyhow::Result<()> {
    // Open a PNG file with an optional deterministic seed
    let mut context = GlitchContext::open("input.png", Some(12345))?;

    // Queue filters (using Default / builder constructors)
    context.add_filter(SubFilter);
    context.add_filter(BlockScramble::new(0.1).with_block_size(32));
    context.add_filter(ChromaticAberration::new(0.2).with_offsets(4, 0, -4));
    context.add_filter(Invert);

    // Execute all queued filters (drains the filter queue)
    context.execute();

    // Save the result
    context.save("output.png")?;

    Ok(())
}
```

## Available Filters

The WebP column shows which filters also work on WebP images through [`webp-glitch-context`](../webp-glitch-context); the WebP-only effects (`MacroblockGlitch`, `AlphaGlitch`, `LossyArtifact`) are defined there.

| Filter | Parameters & Defaults | PNG | WebP | Description |
| :--- | :--- | :---: | :---: | :--- |
| `Invert` | *(none)* | ✅ | ✅ | Inverts all color channels. |
| `Brighten` | `strength: u16` (`20`) | ✅ | ✅ | Increases brightness (saturating at bit-depth max). |
| `Darken` | `strength: u16` (`20`) | ✅ | ✅ | Decreases brightness (saturating at `0`). |
| `ShiftChannels` | `r: i16`, `g: i16`, `b: i16` (`0, 0, 0`) | ✅ | ✅ | Shifts R, G, B channels by signed offsets. |
| `PixelSort` | `magnitude: f64`, `criterion: SortCriterion` (`Brightness`) | ✅ | ✅ | Sorts pixels within scanlines by brightness or hue. |
| `Bitwise` | `magnitude: f64`, `op: BitOp` (`Xor`), `value: u8` (`0`) | ✅ | ✅ | Applies bitwise AND/OR/XOR to scanline bytes. |
| `ChannelSwap` | `magnitude: f64`, `target: SwapTarget` (`Rg`) | ✅ | ✅ | Swaps two color channels (`Rg`, `Gb`, or `Br`). |
| `HorizontalShift` | `magnitude: f64` | ✅ | ✅ | Rotates scanline bytes horizontally by a random offset. |
| `ColorDistortion` | `magnitude: f64`, `strength: i16` (`20`) | ✅ | ✅ | Adds random per-pixel color noise. |
| `ChromaticAberration` | `magnitude: f64`, `r_offset: i32` (`2`), `g_offset: i32` (`0`), `b_offset: i32` (`-2`) | ✅ | ✅ | Horizontally offsets R, G, and B channels. |
| `BlockScramble` | `magnitude: f64`, `block_size: u32` (`16`) | ✅ | ❌ | Swaps square blocks of pixels across the image. |
| `ColorSpaceGlitch` | `magnitude: f64`, `hue_shift: f64` (`0.0`), `saturation_mult: f64` (`1.0`), `lightness_mult: f64` (`1.0`) | ✅ | ❌ | Rotates hue and scales saturation/lightness in HSL space. |
| `ChangeFilterType` | `magnitude: f64` | ✅ | ❌ | Randomly overwrites PNG scanline filter type bytes. |
| `Replace` | `magnitude: f64` | ✅ | ❌ | Replaces random scanline bytes with random values. |
| `Transpose` (`RandomTranspose`) | `magnitude: f64` | ✅ | ❌ | Randomly swaps scanlines. |
| `SetZero` | `magnitude: f64` | ✅ | ❌ | Sets random scanline bytes to `0`. |
| `RandomCopy` | `times: u32` (`1`) | ✅ | ❌ | Copies random scanlines onto other scanlines. |
| `Substitute` | `index: usize` (`0`), `value: u8` (`0`) | ✅ | ❌ | Sets byte at `index` to `value` on every scanline. |
| `RemoveFilter` / `SubFilter` / `UpFilter` / `AverageFilter` / `PaethFilter` | *(none)* | ✅ | ❌ | Re-encodes all PNG scanlines with a fixed prediction filter. |
| `MacroblockGlitch` | `magnitude: f64` | ❌ | ✅ | Swaps 16×16 macroblocks along VP8 block boundaries. |
| `AlphaGlitch` | `magnitude: f64` (`1.0`), `strategy: AlphaGlitchStrategy` (`Invert`) | ❌ | ✅ | Corrupts the alpha channel (`Invert`, `Randomize`, `Zero`, `One`). |
| `LossyArtifact` | `quality: f32` (`10.0`) | ❌ | ✅ | Lowers WebP re-encoding quality (`0.0..=100.0`). |

## License

MIT License.
