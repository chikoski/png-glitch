use clap::Parser;
use glitch_context::{
    AlphaGlitchStrategy, BitOp, FilterConfig, PreProcess, SortCriterion, SwapTarget,
};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubstituteArg {
    pub index: usize,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftChannelsArg {
    pub r: i16,
    pub g: i16,
    pub b: i16,
}

pub fn parse_magnitude(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "magnitude must be between 0.0 and 1.0 (inclusive), got {val}"
        ))
    }
}

pub fn parse_quality(s: &str) -> Result<f32, String> {
    let val: f32 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if (0.0..=100.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "quality must be between 0.0 and 100.0 (inclusive), got {val}"
        ))
    }
}

pub fn parse_substitute(s: &str) -> Result<SubstituteArg, String> {
    let (idx_str, val_str) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid format `{s}`, expected INDEX:VALUE (e.g. 10:255)"))?;
    let index = idx_str
        .parse::<usize>()
        .map_err(|e| format!("invalid index `{idx_str}`: {e}"))?;
    let value = val_str
        .parse::<u8>()
        .map_err(|e| format!("invalid byte value `{val_str}` (must be 0..=255): {e}"))?;
    Ok(SubstituteArg { index, value })
}

pub fn parse_shift_channels(s: &str) -> Result<ShiftChannelsArg, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(format!(
            "invalid format `{s}`, expected 3 comma-separated integers R,G,B (e.g. 10,0,-10)"
        ));
    }
    let r = parts[0]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid R shift `{}`: {e}", parts[0]))?;
    let g = parts[1]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid G shift `{}`: {e}", parts[1]))?;
    let b = parts[2]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid B shift `{}`: {e}", parts[2]))?;
    Ok(ShiftChannelsArg { r, g, b })
}

#[derive(Parser, Debug)]
#[command(name = "png-glitch", author, version, about, long_about = None)]
pub struct Cli {
    /// Output file path [default: <input-stem>-glitched.<ext>]
    #[arg(short, long = "output", conflicts_with = "batch_output")]
    pub output_file: Option<String>,

    /// Overwrite existing output file(s) without prompting
    #[arg(long)]
    pub force: bool,

    pub png_file: String,

    /// Random seed
    #[arg(long)]
    pub seed: Option<u64>,

    /// Batch process all PNG and WebP images in a directory and save to this output directory
    #[arg(long)]
    pub batch_output: Option<String>,

    /// Path to YAML config file
    #[arg(long)]
    pub config: Option<String>,

    /// Magnitude for Change Filter Type filter (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub change_filter_type: Option<f64>,

    /// Magnitude for Replace filter (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub replace: Option<f64>,

    /// Magnitude for Transpose filter (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub transpose: Option<f64>,

    /// Magnitude for Set Zero filter (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub set_zero: Option<f64>,

    /// Number of times for Random Copy filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub random_copy: Option<u32>,

    /// Substitute byte at index with value (index:value)
    #[arg(long, value_parser = parse_substitute, help_heading = "Glitch Filters")]
    pub substitute: Option<SubstituteArg>,

    /// Pixel Sort magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub pixel_sort: Option<f64>,

    /// Pixel Sort criterion (brightness, hue)
    #[arg(long, value_enum, default_value_t = SortCriterion::Brightness, help_heading = "Glitch Filters")]
    pub pixel_sort_criterion: SortCriterion,

    /// Bitwise operation magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub bitwise: Option<f64>,

    /// Bitwise operation (and, or, xor)
    #[arg(long, value_enum, default_value_t = BitOp::Xor, help_heading = "Glitch Filters")]
    pub bitwise_op: BitOp,

    /// Bitwise operation value
    #[arg(long, default_value_t = 0, help_heading = "Glitch Filters")]
    pub bitwise_value: u8,

    /// Channel Swap magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub channel_swap: Option<f64>,

    /// Channel Swap target (rg, gb, br)
    #[arg(long, value_enum, default_value_t = SwapTarget::Rg, help_heading = "Glitch Filters")]
    pub channel_swap_target: SwapTarget,

    /// Horizontal Shift magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub horizontal_shift: Option<f64>,

    /// Block Scramble magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub block_scramble: Option<f64>,

    /// Block Scramble block size
    #[arg(long, default_value_t = 16, help_heading = "Glitch Filters")]
    pub block_scramble_size: u32,

    /// Color Distortion magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub color_distortion: Option<f64>,

    /// Color Distortion strength
    #[arg(long, default_value_t = 20, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub color_distortion_strength: i16,

    /// Color Space Glitch magnitude (HSL, 0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub color_space_glitch: Option<f64>,

    /// Hue shift (0.0 - 360.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub hue_shift: f64,

    /// Saturation multiplier
    #[arg(long, default_value_t = 1.0, help_heading = "Glitch Filters")]
    pub saturation_mult: f64,

    /// Lightness multiplier
    #[arg(long, default_value_t = 1.0, help_heading = "Glitch Filters")]
    pub lightness_mult: f64,

    /// Chromatic Aberration magnitude (0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub chromatic_aberration: Option<f64>,

    /// Red channel offset
    #[arg(long, default_value_t = 2, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub r_offset: i32,

    /// Green channel offset
    #[arg(long, default_value_t = 0, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub g_offset: i32,

    /// Blue channel offset
    #[arg(long, default_value_t = -2, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub b_offset: i32,

    /// Invert colors
    #[arg(long, help_heading = "Glitch Filters")]
    pub invert: bool,

    /// Brighten image (strength)
    #[arg(long, help_heading = "Glitch Filters")]
    pub brighten: Option<u16>,

    /// Shift channels (r,g,b)
    #[arg(long, value_parser = parse_shift_channels, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub shift_channels: Option<ShiftChannelsArg>,

    /// Change the PNG filter type of all scan lines before applying glitch filters
    #[arg(long, value_enum, help_heading = "Pre-processing (applied before glitch filters)")]
    pub pre_process: Option<PreProcess>,

    /// Set all scan lines to filter type None
    #[arg(long, help_heading = "Filter Type Override (force all scan lines to a fixed filter type)")]
    pub remove_filter: bool,

    /// Set all scan lines to filter type Sub
    #[arg(long, help_heading = "Filter Type Override (force all scan lines to a fixed filter type)")]
    pub sub: bool,

    /// Set all scan lines to filter type Up
    #[arg(long, help_heading = "Filter Type Override (force all scan lines to a fixed filter type)")]
    pub up: bool,

    /// Set all scan lines to filter type Average
    #[arg(long, help_heading = "Filter Type Override (force all scan lines to a fixed filter type)")]
    pub average: bool,

    /// Set all scan lines to filter type Paeth
    #[arg(long, help_heading = "Filter Type Override (force all scan lines to a fixed filter type)")]
    pub paeth: bool,

    /// Macroblock glitch magnitude (WebP only, 0.0-1.0)
    #[arg(long, value_parser = parse_magnitude, help_heading = "WebP Filters")]
    pub macroblock_glitch: Option<f64>,

    /// Alpha channel glitch strategy: invert, randomize, zero, one (WebP only)
    #[arg(long, value_enum, help_heading = "WebP Filters")]
    pub alpha_glitch: Option<AlphaGlitchStrategy>,

    /// Re-encode quality for lossy artifact effect, 0.0-100.0 (WebP only)
    #[arg(long, value_parser = parse_quality, help_heading = "WebP Filters")]
    pub lossy_quality: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub filters: Vec<FilterConfig>,
}
