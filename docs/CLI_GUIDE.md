# png-glitch CLI Visual Guide

This guide showcases every command-line option available in `png-glitch`, using `crates/png-glitch/etc/sample00.png` as the base image for comparison.

## Original Image
![Original](../crates/png-glitch/etc/sample00.png)

---

## 1. Color Manipulation

### Invert
Inverts all color channels.
`--invert`
![Invert](./gallery/invert.png)

### Brighten
Increases brightness.
`--brighten 20000`
![Brighten](./gallery/brighten.png)

### Color Distortion
Adds random noise to each color channel.
`--color-distortion 0.1 --color-distortion-strength 50`
![Color Distortion](./gallery/color_distortion.png)

### Color Space Glitch (HSL)
Manipulates Hue, Saturation, and Lightness.
`--color-space-glitch 0.2 --hue-shift 120 --saturation-mult 1.5`
![Color Space Glitch](./gallery/color_space_glitch.png)

### Chromatic Aberration
Spatially shifts color channels to create fringes.
`--chromatic-aberration 0.3 --r-offset 10 --b-offset 10`
![Chromatic Aberration](./gallery/chromatic_aberration.png)

### Shift Channels
Swaps or offsets channel values directly.
`--shift-channels 10000,20000,-10000`
![Shift Channels](./gallery/shift_channels.png)

---

## 2. Structural Glitches

### Block Scramble
Shuffles image blocks in a grid.
`--block-scramble 0.1 --block-scramble-size 32`
![Block Scramble](./gallery/block_scramble.png)

### Transpose
Randomly swaps scanlines.
`--transpose 0.1`
![Transpose](./gallery/transpose.png)

### Horizontal Shift
Offsets scanlines horizontally with wrap-around.
`--horizontal-shift 0.1`
![Horizontal Shift](./gallery/horizontal_shift.png)

### Random Copy
Copies random scanlines to other positions.
`--random-copy 20`
![Random Copy](./gallery/random_copy.png)

---

## 3. Bit-Level Distortion

### Replace
Replaces random bytes with noise.
`--replace 0.05`
![Replace](./gallery/replace.png)

### Set Zero
Sets random bytes to zero.
`--set-zero 0.05`
![Set Zero](./gallery/set_zero.png)

### Bitwise
Performs logical operations on raw bytes.
`--bitwise 0.1 --bitwise-op xor --bitwise-value 128`
![Bitwise](./gallery/bitwise.png)

---

## 4. PNG Filter Abuse

These options force the PNG decoder to misinterpret pixels by manipulating internal filter types.

### Sub
`--sub`
![Sub](./gallery/sub.png)

### Up
`--up`
![Up](./gallery/up.png)

### Average
`--average`
![Average](./gallery/average.png)

### Paeth
`--paeth`
![Paeth](./gallery/paeth.png)

### Change Filter Type
Randomly assigns different filter types to scanlines.
`--change-filter-type 0.1`
![Change Filter Type](./gallery/change_filter_type.png)

---

## 5. Advanced Usage

### Batch Processing
Process all PNGs in a directory in parallel:
```zsh
png-glitch ./input_dir --batch-output ./output_dir --invert --block-scramble 0.05
```

### Reproducible Seeds
Use `--seed <N>` to get the same result every time.

### Configuration Files (YAML)
Combine multiple filters into a pipeline:
```yaml
filters:
  - type: RemoveFilter
  - type: BlockScramble
    magnitude: 0.1
  - type: ChromaticAberration
    magnitude: 0.2
    r_offset: 5
  - type: ColorSpaceGlitch
    magnitude: 0.1
    hue_shift: 180.0
```
Run with: `png-glitch input.png --config config.yaml`
