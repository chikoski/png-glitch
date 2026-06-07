use anyhow::{anyhow, Context};
use clap::Parser;
use glitch_context::{
    AlphaGlitch, AlphaGlitchStrategy, AverageFilter, Bitwise, BlockScramble, Brighten,
    ChangeFilterType, ChannelSwap, ChromaticAberration, ColorDistortion, ColorSpaceGlitch,
    FilterConfig, GlitchContext, HorizontalShift, Invert, LossyArtifact, MacroblockGlitch,
    PaethFilter, PixelSort, RandomCopy, RemoveFilter, Replace, SetZero, ShiftChannels, SubFilter,
    Substitute, Transpose, UpFilter, WebpBrighten, WebpGlitchContext, WebpInvert,
    WebpShiftChannels,
};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod cli;

use crate::cli::{Cli, ConfigFile};

fn is_webp(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("webp"))
        .unwrap_or(false)
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    if let Some(batch_output) = &args.batch_output {
        batch_process(&args, batch_output)
    } else {
        single_process(&args)
    }
}

fn single_process(args: &Cli) -> anyhow::Result<()> {
    if is_webp(&args.png_file) {
        let mut context = WebpGlitchContext::open(&args.png_file, args.seed)
            .context("Failed to open WebP file")?;
        apply_webp_filters(args, &mut context)?;
        context.execute();
        context
            .save(&args.output_file)
            .context("Failed to save output file")?;
    } else {
        let mut context =
            GlitchContext::open(&args.png_file, args.seed).context("Failed to open PNG file")?;
        apply_filters(args, &mut context)?;
        context.execute();
        context
            .save(&args.output_file)
            .context("Failed to save output file")?;
    }
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
        .filter(|e| {
            matches!(
                e.path().extension().and_then(|s| s.to_str()),
                Some("png") | Some("webp")
            )
        })
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
        let file_name = path.file_name().unwrap_or_default();
        let dest_path = output_path.join(file_name);

        pb.set_message(format!("Processing {:?}", file_name));

        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("webp") {
            let mut context = match WebpGlitchContext::open(path, args.seed) {
                Ok(ctx) => ctx,
                Err(e) => { eprintln!("Failed to open {:?}: {}", path, e); return; }
            };
            if let Err(e) = apply_webp_filters(args, &mut context) {
                eprintln!("Failed to apply filters to {:?}: {}", path, e); return;
            }
            context.execute();
            if let Err(e) = context.save(&dest_path) {
                eprintln!("Failed to save {:?}: {}", dest_path, e);
            }
        } else {
            let mut context = match GlitchContext::open(path, args.seed) {
                Ok(ctx) => ctx,
                Err(e) => { eprintln!("Failed to open {:?}: {}", path, e); return; }
            };
            if let Err(e) = apply_filters(args, &mut context) {
                eprintln!("Failed to apply filters to {:?}: {}", path, e); return;
            }
            context.execute();
            if let Err(e) = context.save(&dest_path) {
                eprintln!("Failed to save {:?}: {}", dest_path, e);
            }
        }

        pb.inc(1);
    });

    pb.finish_with_message("Batch processing complete");
    Ok(())
}

fn apply_filters(args: &Cli, context: &mut GlitchContext) -> anyhow::Result<()> {
    if let Some(pre_process) = args.pre_process {
        context.pre_process(pre_process);
    }

    if let Some(config_path) = &args.config {
        let file = File::open(config_path).context("Failed to open config file")?;
        let config: ConfigFile =
            serde_yml::from_reader(file).context("Failed to parse config file")?;
        for filter in config.filters {
            context.add_from_config(filter);
        }
    }

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
    if let Some(magnitude) = args.color_space_glitch {
        context.add_filter(ColorSpaceGlitch {
            magnitude,
            hue_shift: args.hue_shift,
            saturation_mult: args.saturation_mult,
            lightness_mult: args.lightness_mult,
        });
    }
    if let Some(magnitude) = args.chromatic_aberration {
        context.add_filter(ChromaticAberration {
            magnitude,
            r_offset: args.r_offset,
            g_offset: args.g_offset,
            b_offset: args.b_offset,
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

fn apply_webp_filters(args: &Cli, context: &mut WebpGlitchContext) -> anyhow::Result<()> {
    if let Some(config_path) = &args.config {
        let file = File::open(config_path).context("Failed to open config file")?;
        let config: ConfigFile =
            serde_yml::from_reader(file).context("Failed to parse config file")?;
        for filter_cfg in config.filters {
            match filter_cfg {
                FilterConfig::Invert => context.add_filter(WebpInvert),
                FilterConfig::Brighten { strength } => {
                    context.add_filter(WebpBrighten { strength: strength.min(255) as u8 })
                }
                FilterConfig::ShiftChannels { r, g, b } => {
                    context.add_filter(WebpShiftChannels { r, g, b })
                }
                FilterConfig::PixelSort { magnitude, criterion } => {
                    context.add_filter(PixelSort {
                        magnitude,
                        criterion: criterion.unwrap_or(glitch_context::SortCriterion::Brightness),
                    })
                }
                FilterConfig::Bitwise { magnitude, op, value } => {
                    context.add_filter(Bitwise {
                        magnitude,
                        op: op.unwrap_or(glitch_context::BitOp::Xor),
                        value: value.unwrap_or(0),
                    })
                }
                FilterConfig::ChannelSwap { magnitude, target } => {
                    context.add_filter(ChannelSwap {
                        magnitude,
                        target: target.unwrap_or(glitch_context::SwapTarget::Rg),
                    })
                }
                FilterConfig::HorizontalShift { magnitude } => {
                    context.add_filter(HorizontalShift { magnitude })
                }
                FilterConfig::ColorDistortion { magnitude, strength } => {
                    context.add_filter(ColorDistortion {
                        magnitude,
                        strength: strength.unwrap_or(20),
                    })
                }
                FilterConfig::ChromaticAberration { magnitude, r_offset, g_offset, b_offset } => {
                    context.add_filter(ChromaticAberration {
                        magnitude,
                        r_offset: r_offset.unwrap_or(2),
                        g_offset: g_offset.unwrap_or(0),
                        b_offset: b_offset.unwrap_or(-2),
                    })
                }
                _ => {}
            }
        }
    }

    if args.invert { context.add_filter(WebpInvert); }
    if let Some(strength) = args.brighten {
        context.add_filter(WebpBrighten { strength: strength.min(255) as u8 });
    }
    if let Some(channels) = &args.shift_channels {
        if channels.len() == 3 {
            context.add_filter(WebpShiftChannels { r: channels[0], g: channels[1], b: channels[2] });
        }
    }
    if let Some(magnitude) = args.pixel_sort {
        context.add_filter(PixelSort { magnitude, criterion: args.pixel_sort_criterion });
    }
    if let Some(magnitude) = args.bitwise {
        context.add_filter(Bitwise { magnitude, op: args.bitwise_op, value: args.bitwise_value });
    }
    if let Some(magnitude) = args.channel_swap {
        context.add_filter(ChannelSwap { magnitude, target: args.channel_swap_target });
    }
    if let Some(magnitude) = args.horizontal_shift {
        context.add_filter(HorizontalShift { magnitude });
    }
    if let Some(magnitude) = args.color_distortion {
        context.add_filter(ColorDistortion { magnitude, strength: args.color_distortion_strength });
    }
    if let Some(magnitude) = args.chromatic_aberration {
        context.add_filter(ChromaticAberration {
            magnitude,
            r_offset: args.r_offset,
            g_offset: args.g_offset,
            b_offset: args.b_offset,
        });
    }
    if let Some(magnitude) = args.macroblock_glitch {
        context.add_filter(MacroblockGlitch { magnitude });
    }
    if let Some(quality) = args.lossy_quality {
        context.add_filter(LossyArtifact { quality });
    }
    if let Some(strategy_str) = &args.alpha_glitch {
        let strategy = match strategy_str.as_str() {
            "invert" => AlphaGlitchStrategy::Invert,
            "zero"   => AlphaGlitchStrategy::Zero,
            "one"    => AlphaGlitchStrategy::One,
            _        => AlphaGlitchStrategy::Randomize,
        };
        context.add_filter(AlphaGlitch { magnitude: 1.0, strategy });
    }
    Ok(())
}
