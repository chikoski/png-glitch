use clap::Parser;
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
}
