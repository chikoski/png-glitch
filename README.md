# png-glitch: a tool to glitch PNG images

## Installation

To install `png-glitch` from source, run the following command:

```zsh
% cargo install png-glitch --locked 
```

## Usage

`png-glitch` glitches given PNG file (or directory) and emits the result. By default, it saves to `glitched.png`.

```zsh
Usage: png-glitch [OPTIONS] <PNG_FILE_OR_DIR>

Arguments:
  <PNG_FILE_OR_DIR>  Path to a PNG file or a directory for batch processing

Options:
  -o <OUTPUT_FILE>
          [default: glitched.png]
      --change-filter-type <CHANGE_FILTER_TYPE>
          Magnitude for Change Filter Type filter (0.0 to 1.0)
      --replace <REPLACE>
          Magnitude for Replace filter (0.0 to 1.0)
      --transpose <TRANSPOSE>
          Magnitude for Transpose filter (0.0 to 1.0)
      --set-zero <SET_ZERO>
          Magnitude for Set Zero filter (0.0 to 1.0)
      --invert
          Invert colors of each pixel
      --brighten <BRIGHTEN>
          Brighten image by specified strength
      --shift-channels <R,G,B>
          Shift R, G, and B channels independently (e.g., "10,-5,20")
      --seed <SEED>
          Random seed for reproducible glitches
      --batch-output <BATCH_OUTPUT_DIR>
          Batch process all PNGs in a directory and save to this output directory
      --config <CONFIG>
          Path to YAML config file
      --remove-filter
          Remove filter from all scan lines
      --sub
          Change filter type of all scan lines to Sub
      --up
          Change filter type of all scan lines to Up
      --average
          Change filter type of all scan lines to Average
      --paeth
          Change filter type of all scan lines to Paeth
      --pre-process <PRE_PROCESS>
          Apply a filter before other glitch filters [possible values: remove-filter, sub-filter, up-filter, average-filter, paeth-filter]
  -h, --help
          Print help
  -V, --version
          Print version
```

## Batch Processing

To glitch multiple images at once, provide a directory as the main argument and use the `--batch-output` flag:

```zsh
% png-glitch ./my_images --batch-output ./glitched_results --invert --seed 12345
```

The tool will recursively find all `.png` files and process them with a progress bar.

## Configuration File

You can define complex glitch pipelines in a YAML file:

```yaml
filters:
  - type: RemoveFilter
  - type: Invert
  - type: Brighten
    strength: 30
  - type: Transpose
    magnitude: 0.1
  - type: ChangeFilterType
    magnitude: 0.05
```

Run it with:
```zsh
% png-glitch input.png --config my_glitch.yaml
```

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
