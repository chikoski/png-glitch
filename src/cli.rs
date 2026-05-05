use clap::Parser;
use glitch_context::{BitOp, FilterConfig, PreProcess, SortCriterion, SwapTarget};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long = "output", default_value = "glitched.png")]
    pub output_file: String,

    pub png_file: String,

    /// Random seed
    #[arg(long)]
    pub seed: Option<u64>,

    /// Batch process all PNGs in a directory and save to this output directory
    #[arg(long)]
    pub batch_output: Option<String>,

    /// Path to YAML config file
    #[arg(long)]
    pub config: Option<String>,

    /// Magnitude for Change Filter Type filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub change_filter_type: Option<f64>,

    /// Magnitude for Replace filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub replace: Option<f64>,

    /// Magnitude for Transpose filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub transpose: Option<f64>,

    /// Magnitude for Set Zero filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub set_zero: Option<f64>,

    /// Number of times for Random Copy filter
    #[arg(long, help_heading = "Glitch Filters")]
    pub random_copy: Option<u32>,

    /// Substitute byte at index with value (index:value)
    #[arg(long, help_heading = "Glitch Filters")]
    pub substitute: Option<String>,

    /// Pixel Sort magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub pixel_sort: Option<f64>,

    /// Pixel Sort criterion (brightness, hue)
    #[arg(long, value_enum, default_value_t = SortCriterion::Brightness, help_heading = "Glitch Filters")]
    pub pixel_sort_criterion: SortCriterion,

    /// Bitwise operation magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub bitwise: Option<f64>,

    /// Bitwise operation (and, or, xor)
    #[arg(long, value_enum, default_value_t = BitOp::Xor, help_heading = "Glitch Filters")]
    pub bitwise_op: BitOp,

    /// Bitwise operation value
    #[arg(long, default_value_t = 0, help_heading = "Glitch Filters")]
    pub bitwise_value: u8,

    /// Channel Swap magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub channel_swap: Option<f64>,

    /// Channel Swap target (rg, gb, br)
    #[arg(long, value_enum, default_value_t = SwapTarget::Rg, help_heading = "Glitch Filters")]
    pub channel_swap_target: SwapTarget,

    /// Horizontal Shift magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub horizontal_shift: Option<f64>,

    /// Block Scramble magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub block_scramble: Option<f64>,

    /// Block Scramble block size
    #[arg(long, default_value_t = 16, help_heading = "Glitch Filters")]
    pub block_scramble_size: u32,

    /// Color Distortion magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub color_distortion: Option<f64>,

    /// Color Distortion strength
    #[arg(long, default_value_t = 20, help_heading = "Glitch Filters")]
    pub color_distortion_strength: i16,

    /// Color Space Glitch magnitude (HSL)
    #[arg(long, help_heading = "Glitch Filters")]
    pub color_space_glitch: Option<f64>,

    /// Hue shift (0.0 - 360.0)
    #[arg(long, default_value_t = 0.0, help_heading = "Glitch Filters")]
    pub hue_shift: f64,

    /// Saturation multiplier
    #[arg(long, default_value_t = 1.0, help_heading = "Glitch Filters")]
    pub saturation_mult: f64,

    /// Lightness multiplier
    #[arg(long, default_value_t = 1.0, help_heading = "Glitch Filters")]
    pub lightness_mult: f64,

    /// Chromatic Aberration magnitude
    #[arg(long, help_heading = "Glitch Filters")]
    pub chromatic_aberration: Option<f64>,

    /// Red channel offset
    #[arg(long, default_value_t = 2, help_heading = "Glitch Filters")]
    pub r_offset: i32,

    /// Green channel offset
    #[arg(long, default_value_t = 0, help_heading = "Glitch Filters")]
    pub g_offset: i32,

    /// Blue channel offset
    #[arg(long, default_value_t = -2, help_heading = "Glitch Filters")]
    pub b_offset: i32,

    /// Invert colors
    #[arg(long, help_heading = "Glitch Filters")]
    pub invert: bool,

    /// Brighten image (strength)
    #[arg(long, help_heading = "Glitch Filters")]
    pub brighten: Option<u16>,

    /// Shift channels (r,g,b)
    #[arg(long, value_delimiter = ',', help_heading = "Glitch Filters")]
    pub shift_channels: Option<Vec<i16>>,

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

    /// Macroblock glitch magnitude (WebP only)
    #[arg(long, help_heading = "WebP Filters")]
    pub macroblock_glitch: Option<f64>,

    /// Alpha channel glitch strategy: invert, randomize, zero, one (WebP only)
    #[arg(long, help_heading = "WebP Filters")]
    pub alpha_glitch: Option<String>,

    /// Re-encode quality for lossy artifact effect, 0.0-100.0 (WebP only)
    #[arg(long, help_heading = "WebP Filters")]
    pub lossy_quality: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub filters: Vec<FilterConfig>,
}
