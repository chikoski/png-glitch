# webp-glitch-context

A high-level library to manage and apply sequential glitch filter pipelines to **WebP** images, built on top of [`webp-glitch`](../webp-glitch) and [`png-glitch-recipe`](../png-glitch-recipe).

This crate was split out of `png-glitch-recipe` (#127, ADR 0004) so that the PNG crate has no dependency on `webp-glitch` or `libwebp-sys2`. It reuses the configuration types of `png-glitch-recipe` (`FilterConfig`, `SortCriterion`, `BitOp`, `SwapTarget`, `AlphaGlitchStrategy`, `PixelFilter`).

## Installation

```toml
[dependencies]
webp-glitch-context = "0.1.0"
```

## Usage

- `WebpGlitchContext`: queue filters, set a seed, execute, and encode a WebP image.
- `ForWebp<T>`: applies an effect that `png-glitch-recipe` defines (`PixelSort`, `Bitwise`, `ChannelSwap`, `HorizontalShift`, `ColorDistortion`, `ChromaticAberration`, or a `PixelFilterAdapter`) to WebP. The wrapper is needed because of Rust's orphan rule.
- `MacroblockGlitch`, `AlphaGlitch`, `LossyArtifact`: effects that exist only for WebP.
- `WebpGlitchContext::add_from_config`: adds a `FilterConfig` step; PNG-only steps return an error.

```rust
use webp_glitch_context::{ForWebp, MacroblockGlitch, PixelSort, WebpGlitchContext};

fn main() -> anyhow::Result<()> {
    let mut context = WebpGlitchContext::open("input.webp", Some(12345))?;
    context.add_filter(MacroblockGlitch::new(0.2));
    context.add_filter(ForWebp(PixelSort::new(0.3)));
    context.execute();
    context.save("output.webp")?;
    Ok(())
}
```

## License

MIT License.
