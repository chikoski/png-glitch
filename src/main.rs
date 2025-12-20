use anyhow::Context;
use clap::Parser;
use glitch_context::{ChangeFilterType, GlitchContext, Replace, SetZero, Transpose};
use std::fs::File;

mod cli;

use crate::cli::{Cli, ConfigFile, FilterConfig};

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let mut context = GlitchContext::open(&args.png_file).context("Failed to open PNG file")?;

    // Apply config file filters if present
    if let Some(config_path) = args.config {
        let file = File::open(config_path).context("Failed to open config file")?;
        let config: ConfigFile =
            serde_yaml::from_reader(file).context("Failed to parse config file")?;
        for filter in config.filters {
            match filter {
                FilterConfig::ChangeFilterType { magnitude } => {
                    context.add_filter(ChangeFilterType { magnitude });
                }
                FilterConfig::Replace { magnitude } => {
                    context.add_filter(Replace { magnitude });
                }
                FilterConfig::Transpose { magnitude } => {
                    context.add_filter(Transpose { magnitude });
                }
                FilterConfig::SetZero { magnitude } => {
                    context.add_filter(SetZero { magnitude });
                }
            }
        }
    }

    // Apply CLI flags
    if let Some(magnitude) = args.change_filter_type {
        context.add_filter(ChangeFilterType { magnitude });
    }
    if let Some(magnitude) = args.replace {
        context.add_filter(Replace { magnitude });
    }
    if let Some(magnitude) = args.transpose {
        context.add_filter(Transpose { magnitude });
    }
    if let Some(magnitude) = args.set_zero {
        context.add_filter(SetZero { magnitude });
    }

    context.execute();
    context
        .save(&args.output_file)
        .context("Failed to save output file")?;

    Ok(())
}
