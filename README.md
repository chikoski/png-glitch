# png-glitch: a tool to glitch PNG images

`png-glitch` is a powerful tool and library for the intentional corruption of PNG images, creating unique "glitch art" by manipulating both pixel data and the underlying PNG compression structures.

## Installation

To install `png-glitch` from source, run the following command:

```zsh
% cargo install png-glitch --locked 
```

## Usage

`png-glitch` glitches given PNG file (or directory) and emits the result. By default, it saves to `glitched.png`.

```zsh
% png-glitch [OPTIONS] <PNG_FILE>
% png-glitch input.png -o result.png --invert
% png-glitch input.png --output result.png --block-scramble 0.1
```

### Glitch Filters

Multiple filters can be combined and are applied in the order they are specified.

| Option | Description |
| :--- | :--- |
| `--invert` | Inverts all color values. |
| `--color-space-glitch <MAG>` | Artistic control over HSL (Hue, Saturation, Lightness). |
| `--chromatic-aberration <MAG>` | Classic retro "color fringe" effect. |
| `--brighten <N>` | Saturating brightness increase (0–65535). Caps at the maximum value for the image's bit depth. |
| `--shift-channels <R,G,B>` | Shifts color channels independently (wrapping). |
| `--block-scramble <MAG>` | Shuffles grid-based blocks. |
| `--transpose <MAG>` | Randomly swaps blocks of scanlines. |
| `--horizontal-shift <MAG>` | Shifts scanlines horizontally. |
| `--replace <MAG>` | Replaces pixels with noise (0.0–1.0). |
| `--set-zero <MAG>` | Sets random pixels to zero (0.0–1.0). |
| `--random-copy <N>` | Copies random scanlines N times. |
| `--substitute <INDEX:VALUE>` | Substitutes a single byte by index. |
| `--pixel-sort <MAG>` | Sorts pixels by brightness or hue within scanlines. |
| `--bitwise <MAG>` | Applies a bitwise operation (AND/OR/XOR) to pixel data. |
| `--channel-swap <MAG>` | Swaps two color channels (RG, GB, or BR). |
| `--color-distortion <MAG>` | Adds per-scanline color noise. |
| `--change-filter-type <MAG>` | Randomly changes PNG filter types across scanlines. |

### Pre-processing (applied before glitch filters)

`--pre-process` re-encodes the PNG with a specific filter type before glitching. This changes the underlying byte patterns that glitch filters operate on, producing different visual results.

```zsh
% png-glitch input.png --pre-process sub-filter --replace 0.05
```

Accepted values: `remove-filter`, `sub-filter`, `up-filter`, `average-filter`, `paeth-filter`

### Filter Type Override (force all scan lines to a fixed filter type)

These flags forcibly set all scanlines to a single PNG filter type. Unlike `--pre-process`, they are applied as a glitch step in the pipeline and are useful in batch processing workflows.

| Option | Filter type applied to all scanlines |
| :--- | :--- |
| `--remove-filter` | None |
| `--sub` | Sub |
| `--up` | Up |
| `--average` | Average |
| `--paeth` | Paeth |

### Output

| Option | Description |
| :--- | :--- |
| `-o`, `--output <FILE>` | Output file path (default: `glitched.png`). |
| `--seed <N>` | Random seed for reproducible results. |

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
