# png-glitch: a tool to glitch PNG images

## Installation

To install `png-glitch` from source, run the following command:

```zsh
% cargo install png-glitch --locked 
```

## Usage

`png-glitch` glitches given PNG file and emit it to `glitched.png`. The file name for the glitched PNG file can be specified with `-o` option. Please run the command with `--help` option for full option: 

```zsh
Usage: png-glitch [OPTIONS] <PNG_FILE>

Arguments:
  <PNG_FILE>

Options:
  -o <OUTPUT_FILE>
          [default: glitched.png]
      --change-filter-type <CHANGE_FILTER_TYPE>
          Magnitude for Change Filter Type filter
      --replace <REPLACE>
          Magnitude for Replace filter
      --transpose <TRANSPOSE>
          Magnitude for Transpose filter
      --set-zero <SET_ZERO>
          Magnitude for Set Zero filter
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

## Example

The original image:
![The original PNG file is a photo of a media art placed in a slightly darker space.](crates/png-glitch/etc/sample00.png)

And the glitched one:
![](crates/png-glitch/etc/sample00-glitched.png)

# In this repository
This repository consists of the following things:

- png-glitch-cli, a binary crate for a command line interface (CLI) to glitch PNG files.
- [png-glitch crate](crates/png-glitch), a library to glitch PNG images.

png-glitch-cli is a sort of sample code to show basic usage of png-glitch create, for now.

# Licence

MIT License. Please refer to [LICENCE](LICENSE) file for details.