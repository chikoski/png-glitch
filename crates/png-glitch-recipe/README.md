# png-glitch-recipe

A high-level library to manage and apply sequential glitch filter pipelines to **PNG** images, built on top of [`png-glitch`](../png-glitch).

> WebP support moved to the separate [`webp-glitch-context`](../webp-glitch-context) crate, which reuses the configuration types of this crate. Format detection and dispatch between PNG and WebP live in the `png-glitch` CLI.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
png-glitch-recipe = "0.3.0"
```

### Cargo Features

- `clap` *(enabled by default)*: Derives `clap::ValueEnum` on configuration enums (`PreProcess`, `SortCriterion`, `BitOp`, `SwapTarget`, `AlphaGlitchStrategy`).

- `os-rng` *(enabled by default)*: Lets `seed: None` draw a seed from the operating system. Without it every constructor and `Recipe::resolve` needs `Some(seed)` and returns `RecipeError::SeedRequired` otherwise (`GlitchContext::set_seed(None)` leaves the RNG unchanged). Turn it off for `wasm32-unknown-unknown`, which has no OS RNG.
- `parallel` *(enabled by default)*: Enables `png-glitch/parallel` (rayon). Output is identical either way.

To use `png-glitch-recipe` without these (for example on WebAssembly):

```toml
[dependencies]
png-glitch-recipe = { version = "0.3.0", default-features = false }
```

## Usage

`png-glitch-recipe` provides:
- `Recipe`: an ordered list of steps, independent of any image. Reuse it for any number of images and seeds, compare it, and serialize it.
- `ResolvedRecipe`: a recipe with every random choice drawn for one image header and seed. Executing it is deterministic and uses no RNG.
- `RecipeStep`: a user-defined step that draws its choices from the RNG and returns a `png-glitch` effect.
- `GlitchContext`: a convenience wrapper that bundles one image, a filter queue and an RNG.
- `FilterConfig`: Declarative filter steps (YAML / CLI). The WebP-only variants (`MacroblockGlitch`, `AlphaGlitch`, `LossyArtifact`) are accepted by `webp-glitch-context` and rejected by `GlitchContext::add_from_config`.

`png-glitch` itself has no RNG: its effects take every "random" choice as a parameter. This crate draws those choices from a seeded `ChaCha8Rng`, so the same recipe, image and seed always give the same bytes ([ADR 0003](../../docs/adr/0003-home-of-high-level-effects.md), [ADR 0004](../../docs/adr/0004-png-glitch-and-recipe-crates.md)).

### Recipes

```rust
use png_glitch_recipe::{FilterConfig, ImageHeader, PngGlitch, Recipe};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A recipe is not tied to an image.
    let recipe = Recipe::new()
        .then(FilterConfig::Replace { magnitude: 0.05 })
        .then(FilterConfig::Transpose { magnitude: 0.1 });

    // Resolve and execute in one call, for several images or seeds.
    for seed in 0..3 {
        let mut png = PngGlitch::open("input.png")?;
        recipe.run(&mut png, Some(seed))?;
        png.save(format!("output-{seed}.png"))?;
    }

    // Or split the two phases: `resolve` draws every random choice, `execute` only applies
    // concrete effects. The resolved recipe can be inspected and executed again.
    let mut png = PngGlitch::open("input.png")?;
    let resolved = recipe.resolve(&ImageHeader::of(&png), Some(42))?;
    println!("{:?}", resolved.steps());
    resolved.execute(&mut png)?;
    png.save("output.png")?;

    Ok(())
}
```

A `Recipe` implements `serde::Serialize` and `Deserialize` as a `filters:` list, the same format as the CLI's `--config` file and `--save-recipe` output:

```yaml
filters:
  - type: Replace
    magnitude: 0.05
  - type: Transpose
    magnitude: 0.1
```

A recipe that contains user-defined steps cannot be serialized, because code cannot be written out as data.

### Custom steps

Add a deterministic `png-glitch` effect with `Recipe::then_effect`, or a step that draws its parameters from the RNG with `Recipe::then_step` (a closure `Fn(&ImageHeader, &mut dyn RngCore) -> ResolvedEffect` works). In a `GlitchContext`, use `add_fn` for a closure, and `png()` / `png_mut()` / `into_inner()` to mix in direct `PngGlitch` calls. See [`examples/custom_effect.rs`](examples/custom_effect.rs):

```bash
cargo run -p png-glitch-recipe --example custom_effect
```

### GlitchContext

```rust
use png_glitch_recipe::{BlockScramble, ChromaticAberration, GlitchContext, Invert, SubFilter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
