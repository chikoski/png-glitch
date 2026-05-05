# png-glitch: a tool to glitch PNG images

`png-glitch` is a powerful tool and library for the intentional corruption of PNG images, creating unique "glitch art" by manipulating both pixel data and the underlying PNG compression structures.

## Installation

To install `png-glitch` from source, run the following command:

```zsh
% cargo install png-glitch --locked 
```

## Usage

`png-glitch` glitches given PNG file (or directory) and emits the result. By default, it saves to `glitched.png`.

### Command Line Options

| Category | Option | Description |
| :--- | :--- | :--- |
| **Color** | `--invert` | Inverts all color values. |
| | `--color-space-glitch <MAG>` | Artistic control over HSL (Hue, Saturation, Lightness). |
| | `--chromatic-aberration <MAG>` | Classic retro "color fringe" effect. |
| | `--brighten <N>` | Adjusts brightness (0-65535). |
| | `--shift-channels <R,G,B>` | Shifts color channels independently. |
| **Structure** | `--block-scramble <MAG>` | Shuffles grid-based blocks (optimized). |
| | `--transpose <MAG>` | Randomly swaps blocks of scanlines. |
| | `--horizontal-shift <MAG>` | Shifts scanlines horizontally. |
| **Noise** | `--replace <MAG>` | Replaces pixels with noise (0.0 - 1.0). |
| | `--set-zero <MAG>` | Sets random pixels to zero (0.0 - 1.0). |
| **PNG Logic** | `--remove-filter` | Strips filters for predictable results. |
| | `--sub`, `--up`, `--paeth` | Forces specific PNG filter types for streaking. |
| **Utility** | `--batch-output <DIR>` | Processes a directory in parallel. |
| | `--seed <N>` | Set a seed for reproducible glitch patterns. |

## Batch Processing

To glitch multiple images at once, provide a directory as the main argument and use the `--batch-output` flag. This uses **parallel processing** for high performance:

```zsh
% png-glitch ./my_images --batch-output ./glitched_results --invert --block-scramble 0.05
```

## Configuration File (YAML)

Define complex glitch pipelines for repeatable results:

```yaml
filters:
  - type: RemoveFilter
  - type: BlockScramble
    magnitude: 0.1
    block_size: 32
  - type: ColorSpaceGlitch
    magnitude: 0.2
    hue_shift: 90.0
  - type: ChromaticAberration
    magnitude: 0.1
    r_offset: 3
    b_offset: -3
  - type: Invert
  - type: Invert
  - type: PixelSort
    magnitude: 0.1
    criterion: hue
  - type: HorizontalShift
    magnitude: 0.05
  - type: Bitwise
    magnitude: 0.02
    op: xor
    value: 255
```

Run it with:
```zsh
% png-glitch input.png --config my_glitch.yaml
```

## Documentation

- **[Developer & Architecture Guide](DEVELOPER_GUIDE.md)**: Deep dive into the internal design and core concepts.
- **[Spec: Filter System](crates/png-glitch/specs/FILTER_SYSTEM.md)**: Technical details on PNG filters.
- **[Spec: Pixel Formats](crates/png-glitch/specs/PIXEL_FORMATS.md)**: How we handle different bit depths and color types.

## Example

The original image:
![The original PNG file is a photo of a media art placed in a slightly darker space.](crates/png-glitch/etc/sample00.png)

And the glitched one:
![](crates/png-glitch/etc/sample00-glitched.png)

# In this repository
This repository consists of the following things:

- png-glitch-cli, a binary crate for a command line interface (CLI) to glitch PNG files.
- [png-glitch crate](crates/png-glitch), a library to glitch PNG images.
- [glitch-context crate](crates/glitch-context), a high-level API to orchestrate glitch filters.

# Licence

MIT License. Please refer to [LICENCE](LICENSE) file for details.
FilterType
    magnitude: 0.05
```

Run it with:
```zsh
% png-glitch input.png --config my_glitch.yaml
```

## Documentation

- **[CLI Usage & Visual Gallery](docs/CLI_GUIDE.md)**: A comprehensive guide with visual examples.
- **[Developer & Architecture Guide](DEVELOPER_GUIDE.md)**: Technical details on the internal design.
- **[Spec: Filter System](crates/png-glitch/specs/FILTER_SYSTEM.md)**: Details on PNG filter abuse.
- **[Spec: Pixel Formats](crates/png-glitch/specs/PIXEL_FORMATS.md)**: How we handle different bit depths.

## Example

The original image:
![Original photo](crates/png-glitch/etc/sample00.png)

And a glitched version:
![](crates/png-glitch/etc/sample00-glitched.png)

# In this repository
- [png-glitch crate](crates/png-glitch): Core library for PNG manipulation.
- [glitch-context crate](crates/glitch-context): High-level API for filter orchestration.
- **png-glitch-cli**: High-performance CLI tool.

# Licence
MIT License. Refer to [LICENCE](LICENSE) for details.
