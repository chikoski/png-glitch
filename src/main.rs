use anyhow::{anyhow, Context};
use clap::Parser;
use glitch_context::{
    AverageFilter, BitOp, Bitwise, Brighten, ChangeFilterType, ChannelSwap, GlitchContext, Invert,
    PaethFilter, PixelSort, RandomCopy, RemoveFilter, Replace, SetZero, ShiftChannels,
    SortCriterion, SubFilter, Substitute, SwapTarget, Transpose, UpFilter, HorizontalShift,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod cli;

use crate::cli::{Cli, ConfigFile, FilterConfig, PreProcess};

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

    for path in png_files {
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("Failed to get file name"))?;
        let dest_path = output_path.join(file_name);

        pb.set_message(format!("Processing {:?}", file_name));

        let mut context = GlitchContext::open(&path, args.seed)
            .with_context(|| format!("Failed to open {:?}", path))?;

        apply_filters(args, &mut context)?;
        context.execute();
        context
            .save(&dest_path)
            .with_context(|| format!("Failed to save {:?}", dest_path))?;

        pb.inc(1);
    }

    pb.finish_with_message("Batch processing complete");
    Ok(())
}

fn apply_filters(args: &Cli, context: &mut GlitchContext) -> anyhow::Result<()> {
    // Apply pre-process filter first
    if let Some(pre_process) = args.pre_process {
        match pre_process {
            PreProcess::RemoveFilter => context.add_filter(RemoveFilter),
            PreProcess::SubFilter => context.add_filter(SubFilter),
            PreProcess::UpFilter => context.add_filter(UpFilter),
            PreProcess::AverageFilter => context.add_filter(AverageFilter),
            PreProcess::PaethFilter => context.add_filter(PaethFilter),
        }
    }

    // Apply config file filters if present
    if let Some(config_path) = &args.config {
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
                FilterConfig::RandomCopy { times } => {
                    context.add_filter(RandomCopy { times });
                }
                FilterConfig::Substitute { index, value } => {
                    context.add_filter(Substitute { index, value });
                }
                FilterConfig::PixelSort {
                    magnitude,
                    criterion,
                } => {
                    context.add_filter(PixelSort {
                        magnitude,
                        criterion: match criterion.unwrap_or(crate::cli::SortCriterionCli::Brightness) {
                            crate::cli::SortCriterionCli::Brightness => SortCriterion::Brightness,
                            crate::cli::SortCriterionCli::Hue => SortCriterion::Hue,
                        },
                    });
                }
                FilterConfig::Bitwise {
                    magnitude,
                    op,
                    value,
                } => {
                    context.add_filter(Bitwise {
                        magnitude,
                        op: match op.unwrap_or(crate::cli::BitOpCli::Xor) {
                            crate::cli::BitOpCli::And => BitOp::And,
                            crate::cli::BitOpCli::Or => BitOp::Or,
                            crate::cli::BitOpCli::Xor => BitOp::Xor,
                        },
                        value: value.unwrap_or(0),
                    });
                }
                FilterConfig::ChannelSwap { magnitude, target } => {
                    context.add_filter(ChannelSwap {
                        magnitude,
                        target: match target.unwrap_or(crate::cli::SwapTargetCli::Rg) {
                            crate::cli::SwapTargetCli::Rg => SwapTarget::Rg,
                            crate::cli::SwapTargetCli::Gb => SwapTarget::Gb,
                            crate::cli::SwapTargetCli::Br => SwapTarget::Br,
                        },
                    });
                }
                FilterConfig::HorizontalShift { magnitude } => {
                    context.add_filter(HorizontalShift { magnitude });
                }
                FilterConfig::Invert => {
                    context.add_filter(Invert);
                }
                FilterConfig::Brighten { strength } => {
                    context.add_filter(Brighten { strength });
                }
                FilterConfig::ShiftChannels { r, g, b } => {
                    context.add_filter(ShiftChannels { r, g, b });
                }
                FilterConfig::RemoveFilter => {
                    context.add_filter(RemoveFilter);
                }
                FilterConfig::SubFilter => {
                    context.add_filter(SubFilter);
                }
                FilterConfig::UpFilter => {
                    context.add_filter(UpFilter);
                }
                FilterConfig::AverageFilter => {
                    context.add_filter(AverageFilter);
                }
                FilterConfig::PaethFilter => {
                    context.add_filter(PaethFilter);
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
            criterion: match args.pixel_sort_criterion {
                crate::cli::SortCriterionCli::Brightness => SortCriterion::Brightness,
                crate::cli::SortCriterionCli::Hue => SortCriterion::Hue,
            },
        });
    }
    if let Some(magnitude) = args.bitwise {
        context.add_filter(Bitwise {
            magnitude,
            op: match args.bitwise_op {
                crate::cli::BitOpCli::And => BitOp::And,
                crate::cli::BitOpCli::Or => BitOp::Or,
                crate::cli::BitOpCli::Xor => BitOp::Xor,
            },
            value: args.bitwise_value,
        });
    }
    if let Some(magnitude) = args.channel_swap {
        context.add_filter(ChannelSwap {
            magnitude,
            target: match args.channel_swap_target {
                crate::cli::SwapTargetCli::Rg => SwapTarget::Rg,
                crate::cli::SwapTargetCli::Gb => SwapTarget::Gb,
                crate::cli::SwapTargetCli::Br => SwapTarget::Br,
            },
        });
    }
    if let Some(magnitude) = args.horizontal_shift {
        context.add_filter(HorizontalShift { magnitude });
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
