use clap::Parser;
use glitch_context::{BitOp, FilterConfig, PreProcess, SortCriterion, SwapTarget};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, default_value = "glitched.png")]
    pub output_file: String,

    pub png_file: String,

    /// Magnitude for Change Filter Type filter
    #[arg(long)]
    pub change_filter_type: Option<f64>,

    /// Magnitude for Replace filter
    #[arg(long)]
    pub replace: Option<f64>,

    /// Magnitude for Transpose filter
    #[arg(long)]
    pub transpose: Option<f64>,

    /// Magnitude for Set Zero filter
    #[arg(long)]
    pub set_zero: Option<f64>,

    /// Number of times for Random Copy filter
    #[arg(long)]
    pub random_copy: Option<u32>,

    /// Substitute byte at index with value (index:value)
    #[arg(long)]
    pub substitute: Option<String>,

    /// Pixel Sort magnitude
    #[arg(long)]
    pub pixel_sort: Option<f64>,

    /// Pixel Sort criterion (brightness, hue)
    #[arg(long, value_enum, default_value_t = SortCriterion::Brightness)]
    pub pixel_sort_criterion: SortCriterion,

    /// Bitwise operation magnitude
    #[arg(long)]
    pub bitwise: Option<f64>,

    /// Bitwise operation (and, or, xor)
    #[arg(long, value_enum, default_value_t = BitOp::Xor)]
    pub bitwise_op: BitOp,

    /// Bitwise operation value
    #[arg(long, default_value_t = 0)]
    pub bitwise_value: u8,

    /// Channel Swap magnitude
    #[arg(long)]
    pub channel_swap: Option<f64>,

    /// Channel Swap target (rg, gb, br)
    #[arg(long, value_enum, default_value_t = SwapTarget::Rg)]
    pub channel_swap_target: SwapTarget,

    /// Horizontal Shift magnitude
    #[arg(long)]
    pub horizontal_shift: Option<f64>,

    /// Block Scramble magnitude
    #[arg(long)]
    pub block_scramble: Option<f64>,

    /// Block Scramble block size
    #[arg(long, default_value_t = 16)]
    pub block_scramble_size: u32,

    /// Color Distortion magnitude
    #[arg(long)]
    pub color_distortion: Option<f64>,

    /// Color Distortion strength
    #[arg(long, default_value_t = 20)]
    pub color_distortion_strength: i16,

    /// Color Space Glitch magnitude (HSL)
    #[arg(long)]
    pub color_space_glitch: Option<f64>,

    /// Hue shift (0.0 - 360.0)
    #[arg(long, default_value_t = 0.0)]
    pub hue_shift: f64,

    /// Saturation multiplier
    #[arg(long, default_value_t = 1.0)]
    pub saturation_mult: f64,

    /// Lightness multiplier
    #[arg(long, default_value_t = 1.0)]
    pub lightness_mult: f64,

    /// Chromatic Aberration magnitude
    #[arg(long)]
    pub chromatic_aberration: Option<f64>,

    /// Red channel offset
    #[arg(long, default_value_t = 2)]
    pub r_offset: i32,

    /// Green channel offset
    #[arg(long, default_value_t = 0)]
    pub g_offset: i32,

    /// Blue channel offset
    #[arg(long, default_value_t = -2)]
    pub b_offset: i32,

    /// Invert colors
    #[arg(long)]
    pub invert: bool,

    /// Brighten image (strength)
    #[arg(long)]
    pub brighten: Option<u16>,

    /// Shift channels (r,g,b)
    #[arg(long, value_delimiter = ',')]
    pub shift_channels: Option<Vec<i16>>,

    /// Random seed
    #[arg(long)]
    pub seed: Option<u64>,

    /// Batch process all PNGs in a directory and save to this output directory
    #[arg(long)]
    pub batch_output: Option<String>,

    /// Path to YAML config file
    #[arg(long)]
    pub config: Option<String>,

    /// Remove filter from all scan lines
    #[arg(long)]
    pub remove_filter: bool,

    /// Change filter type of all scan lines to Sub
    #[arg(long)]
    pub sub: bool,

    /// Change filter type of all scan lines to Up
    #[arg(long)]
    pub up: bool,

    /// Change filter type of all scan lines to Average
    #[arg(long)]
    pub average: bool,

    /// Change filter type of all scan lines to Paeth
    #[arg(long)]
    pub paeth: bool,

    /// Apply a filter before other glitch filters
    #[arg(long, value_enum)]
    pub pre_process: Option<PreProcess>,

    // WebP 専用オプション
    /// Macroblock glitch magnitude (WebP only)
    #[arg(long)]
    pub macroblock_glitch: Option<f64>,

    /// Alpha channel glitch strategy: invert, randomize, zero, one (WebP only)
    #[arg(long)]
    pub alpha_glitch: Option<String>,

    /// Re-encode quality for lossy artifact effect, 0.0-100.0 (WebP only)
    #[arg(long)]
    pub lossy_quality: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub filters: Vec<FilterConfig>,
}
