use clap::{Parser, ValueEnum};
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
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PreProcess {
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub filters: Vec<FilterConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum FilterConfig {
    ChangeFilterType { magnitude: f64 },
    Replace { magnitude: f64 },
    Transpose { magnitude: f64 },
    SetZero { magnitude: f64 },
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
}
