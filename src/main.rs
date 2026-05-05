use anyhow::{anyhow, Context};
use clap::Parser;
use glitch_context::{
    AverageFilter, Bitwise, BlockScramble, Brighten, ChangeFilterType, ChannelSwap, ColorDistortion,
    FilterConfig, GlitchContext, HorizontalShift, Invert, PaethFilter, PixelSort, RandomCopy,
    RemoveFilter, Replace, SetZero, ShiftChannels, SubFilter, Substitute, Transpose, UpFilter,
};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod cli;

use crate::cli::{Cli, ConfigFile};

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    if let Some(batch_output) = &args.batch_output {
        batch_process(&args, batch_output)
    } else {
        single_process(&args)
    }
}

fn single_process(args: &Cli) -> anyhow::Result<()> {
    let mut context =
        GlitchContext::open(&args.png_file, args.seed).context("Failed to open PNG file")?;
    apply_filters(args, &mut context)?;
    context.execute();
    context
        .save(&args.output_file)
        .context("Failed to save output file")?;
    Ok(())
}

fn batch_process(args: &Cli, output_dir: &str) -> anyhow::Result<()> {
    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path).context("Failed to create output directory")?;
    }

    let png_files: Vec<PathBuf> = WalkDir::new(&args.png_file)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("png"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if png_files.is_empty() {
        return Err(anyhow!("No PNG files found in {}", args.png_file));
    }

    let pb = ProgressBar::new(png_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    png_files.par_iter().for_each(|path| {
        let file_name = path
            .file_name()
            .unwrap_or_default();
        let dest_path = output_path.join(file_name);

        pb.set_message(format!("Processing {:?}", file_name));

        let mut context = match GlitchContext::open(path, args.seed) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("Failed to open {:?}: {}", path, e);
                return;
            }
        };

        if let Err(e) = apply_filters(args, &mut context) {
            eprintln!("Failed to apply filters to {:?}: {}", path, e);
            return;
        }

        context.execute();
        if let Err(e) = context.save(&dest_path) {
            eprintln!("Failed to save {:?}: {}", dest_path, e);
        }

        pb.inc(1);
    });

    pb.finish_with_message("Batch processing complete");
    Ok(())
}

fn apply_filters(args: &Cli, context: &mut GlitchContext) -> anyhow::Result<()> {
    // Apply pre-process filter first
    if let Some(pre_process) = args.pre_process {
        context.pre_process(pre_process);
    }

    // Apply config file filters if present
    if let Some(config_path) = &args.config {
        let file = File::open(config_path).context("Failed to open config file")?;
        let config: ConfigFile =
            serde_yaml::from_reader(file).context("Failed to parse config file")?;
        for filter in config.filters {
            context.add_from_config(filter);
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
    if let Some(times) = args.random_copy {
        context.add_filter(RandomCopy { times });
    }
    if let Some(sub) = &args.substitute {
        let parts: Vec<&str> = sub.split(':').collect();
        if parts.len() == 2 {
            let index = parts[0].parse::<usize>()?;
            let value = parts[1].parse::<u8>()?;
            context.add_filter(Substitute { index, value });
        }
    }
    if let Some(magnitude) = args.pixel_sort {
        context.add_filter(PixelSort {
            magnitude,
            criterion: args.pixel_sort_criterion,
        });
    }
    if let Some(magnitude) = args.bitwise {
        context.add_filter(Bitwise {
            magnitude,
            op: args.bitwise_op,
            value: args.bitwise_value,
        });
    }
    if let Some(magnitude) = args.channel_swap {
        context.add_filter(ChannelSwap {
            magnitude,
            target: args.channel_swap_target,
        });
    }
    if let Some(magnitude) = args.horizontal_shift {
        context.add_filter(HorizontalShift { magnitude });
    }
    if let Some(magnitude) = args.block_scramble {
        context.add_filter(BlockScramble {
            magnitude,
            block_size: args.block_scramble_size,
        });
    }
    if let Some(magnitude) = args.color_distortion {
        context.add_filter(ColorDistortion {
            magnitude,
            strength: args.color_distortion_strength,
        });
    }
    if args.invert {
        context.add_filter(Invert);
    }
    if let Some(strength) = args.brighten {
        context.add_filter(Brighten { strength });
    }
    if let Some(channels) = &args.shift_channels {
        if channels.len() == 3 {
            context.add_filter(ShiftChannels {
                r: channels[0],
                g: channels[1],
                b: channels[2],
            });
        }
    }
    if args.remove_filter {
        context.add_filter(RemoveFilter);
    }
    if args.sub {
        context.add_filter(SubFilter);
    }
    if args.up {
        context.add_filter(UpFilter);
    }
    if args.average {
        context.add_filter(AverageFilter);
    }
    if args.paeth {
        context.add_filter(PaethFilter);
    }

    Ok(())
}
