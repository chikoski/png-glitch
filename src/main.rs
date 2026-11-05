use anyhow::{anyhow, bail, Context};
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use glitch_context::{
    AlphaGlitch, AverageFilter, Bitwise, BlockScramble, Brighten, ChangeFilterType, ChannelSwap,
    ChromaticAberration, ColorDistortion, ColorSpaceGlitch, FilterConfig, GlitchContext,
    HorizontalShift, Invert, LossyArtifact, MacroblockGlitch, PaethFilter, PixelFilterAdapter,
    PixelSort, RandomCopy, RemoveFilter, Replace, SetZero, ShiftChannels, SubFilter, Substitute,
    Transpose, UpFilter, WebpGlitchContext,
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

#[derive(Debug, Clone)]
enum OrderedStep {
    Config(String),
    ChangeFilterType(f64),
    Replace(f64),
    Transpose(f64),
    SetZero(f64),
    RandomCopy(u32),
    Substitute { index: usize, value: u8 },
    PixelSort(f64),
    Bitwise(f64),
    ChannelSwap(f64),
    HorizontalShift(f64),
    BlockScramble(f64),
    ColorDistortion(f64),
    ColorSpaceGlitch(f64),
    ChromaticAberration(f64),
    Invert,
    Brighten(u16),
    ShiftChannels { r: i16, g: i16, b: i16 },
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
    MacroblockGlitch(f64),
    AlphaGlitch(glitch_context::AlphaGlitchStrategy),
    LossyQuality(f32),
}

fn collect_ordered_steps(args: &Cli, matches: Option<&ArgMatches>) -> Vec<OrderedStep> {
    let idx = |id: &str, fallback: usize| -> usize {
        matches.and_then(|m| m.index_of(id)).unwrap_or(fallback)
    };

    let mut steps: Vec<(usize, OrderedStep)> = Vec::new();

    if let Some(cfg) = &args.config {
        steps.push((idx("config", 10), OrderedStep::Config(cfg.clone())));
    }
    if let Some(m) = args.change_filter_type {
        steps.push((idx("change_filter_type", 20), OrderedStep::ChangeFilterType(m)));
    }
    if let Some(m) = args.replace {
        steps.push((idx("replace", 30), OrderedStep::Replace(m)));
    }
    if let Some(m) = args.transpose {
        steps.push((idx("transpose", 40), OrderedStep::Transpose(m)));
    }
    if let Some(m) = args.set_zero {
        steps.push((idx("set_zero", 50), OrderedStep::SetZero(m)));
    }
    if let Some(times) = args.random_copy {
        steps.push((idx("random_copy", 60), OrderedStep::RandomCopy(times)));
    }
    if let Some(sub) = args.substitute {
        steps.push((
            idx("substitute", 70),
            OrderedStep::Substitute {
                index: sub.index,
                value: sub.value,
            },
        ));
    }
    if let Some(m) = args.pixel_sort {
        steps.push((idx("pixel_sort", 80), OrderedStep::PixelSort(m)));
    }
    if let Some(m) = args.bitwise {
        steps.push((idx("bitwise", 90), OrderedStep::Bitwise(m)));
    }
    if let Some(m) = args.channel_swap {
        steps.push((idx("channel_swap", 100), OrderedStep::ChannelSwap(m)));
    }
    if let Some(m) = args.horizontal_shift {
        steps.push((idx("horizontal_shift", 110), OrderedStep::HorizontalShift(m)));
    }
    if let Some(m) = args.block_scramble {
        steps.push((idx("block_scramble", 120), OrderedStep::BlockScramble(m)));
    }
    if let Some(m) = args.color_distortion {
        steps.push((idx("color_distortion", 130), OrderedStep::ColorDistortion(m)));
    }
    if let Some(m) = args.color_space_glitch {
        steps.push((
            idx("color_space_glitch", 140),
            OrderedStep::ColorSpaceGlitch(m),
        ));
    }
    if let Some(m) = args.chromatic_aberration {
        steps.push((
            idx("chromatic_aberration", 150),
            OrderedStep::ChromaticAberration(m),
        ));
    }
    if args.invert {
        steps.push((idx("invert", 160), OrderedStep::Invert));
    }
    if let Some(s) = args.brighten {
        steps.push((idx("brighten", 170), OrderedStep::Brighten(s)));
    }
    if let Some(ch) = args.shift_channels {
        steps.push((
            idx("shift_channels", 180),
            OrderedStep::ShiftChannels {
                r: ch.r,
                g: ch.g,
                b: ch.b,
            },
        ));
    }
    if args.remove_filter {
        steps.push((idx("remove_filter", 190), OrderedStep::RemoveFilter));
    }
    if args.sub {
        steps.push((idx("sub", 200), OrderedStep::SubFilter));
    }
    if args.up {
        steps.push((idx("up", 210), OrderedStep::UpFilter));
    }
    if args.average {
        steps.push((idx("average", 220), OrderedStep::AverageFilter));
    }
    if args.paeth {
        steps.push((idx("paeth", 230), OrderedStep::PaethFilter));
    }
    if let Some(m) = args.macroblock_glitch {
        steps.push((
            idx("macroblock_glitch", 240),
            OrderedStep::MacroblockGlitch(m),
        ));
    }
    if let Some(s) = args.alpha_glitch {
        steps.push((idx("alpha_glitch", 250), OrderedStep::AlphaGlitch(s)));
    }
    if let Some(q) = args.lossy_quality {
        steps.push((idx("lossy_quality", 260), OrderedStep::LossyQuality(q)));
    }

    steps.sort_by_key(|(i, _)| *i);
    steps.into_iter().map(|(_, s)| s).collect()
}

fn validate_flags_for_format(args: &Cli, webp_input: bool) -> anyhow::Result<()> {
    if webp_input {
        let mut unsupported = Vec::new();
        if args.pre_process.is_some() {
            unsupported.push("--pre-process");
        }
        if args.change_filter_type.is_some() {
            unsupported.push("--change-filter-type");
        }
        if args.replace.is_some() {
            unsupported.push("--replace");
        }
        if args.transpose.is_some() {
            unsupported.push("--transpose");
        }
        if args.set_zero.is_some() {
            unsupported.push("--set-zero");
        }
        if args.random_copy.is_some() {
            unsupported.push("--random-copy");
        }
        if args.substitute.is_some() {
            unsupported.push("--substitute");
        }
        if args.block_scramble.is_some() {
            unsupported.push("--block-scramble");
        }
        if args.color_space_glitch.is_some() {
            unsupported.push("--color-space-glitch");
        }
        if args.remove_filter {
            unsupported.push("--remove-filter");
        }
        if args.sub {
            unsupported.push("--sub");
        }
        if args.up {
            unsupported.push("--up");
        }
        if args.average {
            unsupported.push("--average");
        }
        if args.paeth {
            unsupported.push("--paeth");
        }
        if !unsupported.is_empty() {
            bail!(
                "The following PNG-only options are not supported for WebP input: {}",
                unsupported.join(", ")
            );
        }
    } else {
        let mut unsupported = Vec::new();
        if args.macroblock_glitch.is_some() {
            unsupported.push("--macroblock-glitch");
        }
        if args.alpha_glitch.is_some() {
            unsupported.push("--alpha-glitch");
        }
        if args.lossy_quality.is_some() {
            unsupported.push("--lossy-quality");
        }
        if !unsupported.is_empty() {
            bail!(
                "The following WebP-only options are not supported for PNG input: {}",
                unsupported.join(", ")
            );
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let matches = Cli::command().get_matches();
    let args = Cli::from_arg_matches(&matches)?;
    let steps = collect_ordered_steps(&args, Some(&matches));

    if let Some(batch_output) = &args.batch_output {
        batch_process(&args, &steps, batch_output)
    } else {
        single_process(&args, &steps)
    }
}

fn single_process(args: &Cli, steps: &[OrderedStep]) -> anyhow::Result<()> {
    let webp_input = is_webp(&args.png_file);
    validate_flags_for_format(args, webp_input)?;

    if webp_input {
        let mut context = WebpGlitchContext::open(&args.png_file, args.seed)
            .context("Failed to open WebP file")?;
        apply_webp_filters(args, steps, &mut context)?;
        context.execute();
        context
            .save(&args.output_file)
            .context("Failed to save output file")?;
    } else {
        let mut context =
            GlitchContext::open(&args.png_file, args.seed).context("Failed to open PNG file")?;
        apply_filters(args, steps, &mut context)?;
        context.execute();
        context
            .save(&args.output_file)
            .context("Failed to save output file")?;
    }
    Ok(())
}

fn batch_process(args: &Cli, steps: &[OrderedStep], output_dir: &str) -> anyhow::Result<()> {
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
            if let Err(e) = validate_flags_for_format(args, true) {
                eprintln!("Skipping {:?}: {}", path, e);
                return;
            }
            let mut context = match WebpGlitchContext::open(path, args.seed) {
                Ok(ctx) => ctx,
                Err(e) => {
                    eprintln!("Failed to open {:?}: {}", path, e);
                    return;
                }
            };
            if let Err(e) = apply_webp_filters(args, steps, &mut context) {
                eprintln!("Failed to apply filters to {:?}: {}", path, e);
                return;
            }
            context.execute();
            if let Err(e) = context.save(&dest_path) {
                eprintln!("Failed to save {:?}: {}", dest_path, e);
            }
        } else {
            if let Err(e) = validate_flags_for_format(args, false) {
                eprintln!("Skipping {:?}: {}", path, e);
                return;
            }
            let mut context = match GlitchContext::open(path, args.seed) {
                Ok(ctx) => ctx,
                Err(e) => {
                    eprintln!("Failed to open {:?}: {}", path, e);
                    return;
                }
            };
            if let Err(e) = apply_filters(args, steps, &mut context) {
                eprintln!("Failed to apply filters to {:?}: {}", path, e);
                return;
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

fn apply_filters(
    args: &Cli,
    steps: &[OrderedStep],
    context: &mut GlitchContext,
) -> anyhow::Result<()> {
    if let Some(pre_process) = args.pre_process {
        context.pre_process(pre_process);
    }

    for step in steps {
        match step {
            OrderedStep::Config(config_path) => {
                let file = File::open(config_path).context("Failed to open config file")?;
                let config: ConfigFile =
                    serde_yml::from_reader(file).context("Failed to parse config file")?;
                for filter in config.filters {
                    context.add_from_config(filter);
                }
            }
            OrderedStep::ChangeFilterType(magnitude) => {
                context.add_filter(ChangeFilterType {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::Replace(magnitude) => {
                context.add_filter(Replace {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::Transpose(magnitude) => {
                context.add_filter(Transpose {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::SetZero(magnitude) => {
                context.add_filter(SetZero {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::RandomCopy(times) => {
                context.add_filter(RandomCopy { times: *times });
            }
            OrderedStep::Substitute { index, value } => {
                context.add_filter(Substitute {
                    index: *index,
                    value: *value,
                });
            }
            OrderedStep::PixelSort(magnitude) => {
                context.add_filter(PixelSort {
                    magnitude: *magnitude,
                    criterion: args.pixel_sort_criterion,
                });
            }
            OrderedStep::Bitwise(magnitude) => {
                context.add_filter(Bitwise {
                    magnitude: *magnitude,
                    op: args.bitwise_op,
                    value: args.bitwise_value,
                });
            }
            OrderedStep::ChannelSwap(magnitude) => {
                context.add_filter(ChannelSwap {
                    magnitude: *magnitude,
                    target: args.channel_swap_target,
                });
            }
            OrderedStep::HorizontalShift(magnitude) => {
                context.add_filter(HorizontalShift {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::BlockScramble(magnitude) => {
                context.add_filter(BlockScramble {
                    magnitude: *magnitude,
                    block_size: args.block_scramble_size,
                });
            }
            OrderedStep::ColorDistortion(magnitude) => {
                context.add_filter(ColorDistortion {
                    magnitude: *magnitude,
                    strength: args.color_distortion_strength,
                });
            }
            OrderedStep::ColorSpaceGlitch(magnitude) => {
                context.add_filter(ColorSpaceGlitch {
                    magnitude: *magnitude,
                    hue_shift: args.hue_shift,
                    saturation_mult: args.saturation_mult,
                    lightness_mult: args.lightness_mult,
                });
            }
            OrderedStep::ChromaticAberration(magnitude) => {
                context.add_filter(ChromaticAberration {
                    magnitude: *magnitude,
                    r_offset: args.r_offset,
                    g_offset: args.g_offset,
                    b_offset: args.b_offset,
                });
            }
            OrderedStep::Invert => {
                context.add_filter(Invert);
            }
            OrderedStep::Brighten(strength) => {
                context.add_filter(Brighten {
                    strength: *strength,
                });
            }
            OrderedStep::ShiftChannels { r, g, b } => {
                context.add_filter(ShiftChannels {
                    r: *r,
                    g: *g,
                    b: *b,
                });
            }
            OrderedStep::RemoveFilter => {
                context.add_filter(RemoveFilter);
            }
            OrderedStep::SubFilter => {
                context.add_filter(SubFilter);
            }
            OrderedStep::UpFilter => {
                context.add_filter(UpFilter);
            }
            OrderedStep::AverageFilter => {
                context.add_filter(AverageFilter);
            }
            OrderedStep::PaethFilter => {
                context.add_filter(PaethFilter);
            }
            OrderedStep::MacroblockGlitch(_)
            | OrderedStep::AlphaGlitch(_)
            | OrderedStep::LossyQuality(_) => {
                bail!("WebP-only filter is not supported for PNG input");
            }
        }
    }

    Ok(())
}

fn apply_webp_filters(
    args: &Cli,
    steps: &[OrderedStep],
    context: &mut WebpGlitchContext,
) -> anyhow::Result<()> {
    for step in steps {
        match step {
            OrderedStep::Config(config_path) => {
                let file = File::open(config_path).context("Failed to open config file")?;
                let config: ConfigFile =
                    serde_yml::from_reader(file).context("Failed to parse config file")?;
                for filter_cfg in config.filters {
                    match filter_cfg {
                        FilterConfig::Invert => context.add_filter(PixelFilterAdapter(Invert)),
                        FilterConfig::Brighten { strength } => {
                            context.add_filter(PixelFilterAdapter(Brighten { strength }))
                        }
                        FilterConfig::ShiftChannels { r, g, b } => {
                            context.add_filter(PixelFilterAdapter(ShiftChannels { r, g, b }))
                        }
                        FilterConfig::PixelSort {
                            magnitude,
                            criterion,
                        } => context.add_filter(PixelSort {
                            magnitude,
                            criterion: criterion
                                .unwrap_or(glitch_context::SortCriterion::Brightness),
                        }),
                        FilterConfig::Bitwise {
                            magnitude,
                            op,
                            value,
                        } => context.add_filter(Bitwise {
                            magnitude,
                            op: op.unwrap_or(glitch_context::BitOp::Xor),
                            value: value.unwrap_or(0),
                        }),
                        FilterConfig::ChannelSwap { magnitude, target } => {
                            context.add_filter(ChannelSwap {
                                magnitude,
                                target: target.unwrap_or(glitch_context::SwapTarget::Rg),
                            })
                        }
                        FilterConfig::HorizontalShift { magnitude } => {
                            context.add_filter(HorizontalShift { magnitude })
                        }
                        FilterConfig::ColorDistortion {
                            magnitude,
                            strength,
                        } => context.add_filter(ColorDistortion {
                            magnitude,
                            strength: strength.unwrap_or(20),
                        }),
                        FilterConfig::ChromaticAberration {
                            magnitude,
                            r_offset,
                            g_offset,
                            b_offset,
                        } => context.add_filter(ChromaticAberration {
                            magnitude,
                            r_offset: r_offset.unwrap_or(2),
                            g_offset: g_offset.unwrap_or(0),
                            b_offset: b_offset.unwrap_or(-2),
                        }),
                        other => {
                            bail!(
                                "Filter `{:?}` in config file is not supported for WebP images",
                                other
                            );
                        }
                    }
                }
            }
            OrderedStep::Invert => context.add_filter(PixelFilterAdapter(Invert)),
            OrderedStep::Brighten(strength) => {
                context.add_filter(PixelFilterAdapter(Brighten {
                    strength: *strength,
                }));
            }
            OrderedStep::ShiftChannels { r, g, b } => {
                context.add_filter(PixelFilterAdapter(ShiftChannels {
                    r: *r,
                    g: *g,
                    b: *b,
                }));
            }
            OrderedStep::PixelSort(magnitude) => {
                context.add_filter(PixelSort {
                    magnitude: *magnitude,
                    criterion: args.pixel_sort_criterion,
                });
            }
            OrderedStep::Bitwise(magnitude) => {
                context.add_filter(Bitwise {
                    magnitude: *magnitude,
                    op: args.bitwise_op,
                    value: args.bitwise_value,
                });
            }
            OrderedStep::ChannelSwap(magnitude) => {
                context.add_filter(ChannelSwap {
                    magnitude: *magnitude,
                    target: args.channel_swap_target,
                });
            }
            OrderedStep::HorizontalShift(magnitude) => {
                context.add_filter(HorizontalShift {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::ColorDistortion(magnitude) => {
                context.add_filter(ColorDistortion {
                    magnitude: *magnitude,
                    strength: args.color_distortion_strength,
                });
            }
            OrderedStep::ChromaticAberration(magnitude) => {
                context.add_filter(ChromaticAberration {
                    magnitude: *magnitude,
                    r_offset: args.r_offset,
                    g_offset: args.g_offset,
                    b_offset: args.b_offset,
                });
            }
            OrderedStep::MacroblockGlitch(magnitude) => {
                context.add_filter(MacroblockGlitch {
                    magnitude: *magnitude,
                });
            }
            OrderedStep::AlphaGlitch(strategy) => {
                context.add_filter(AlphaGlitch {
                    magnitude: 1.0,
                    strategy: *strategy,
                });
            }
            OrderedStep::LossyQuality(quality) => {
                context.add_filter(LossyArtifact { quality: *quality });
            }
            other => {
                bail!(
                    "Filter `{:?}` is not supported for WebP images",
                    other
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_command_name_is_png_glitch() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_name(), "png-glitch");
    }

    #[test]
    fn test_magnitude_out_of_range_rejected() {
        assert!(Cli::try_parse_from(["png-glitch", "input.png", "--replace", "1.5"]).is_err());
        assert!(Cli::try_parse_from(["png-glitch", "input.png", "--replace", "-0.1"]).is_err());
        assert!(Cli::try_parse_from(["png-glitch", "input.png", "--replace=-0.1"]).is_err());
        assert!(Cli::try_parse_from(["png-glitch", "input.png", "--replace", "0.5"]).is_ok());
    }

    #[test]
    fn test_negative_offsets_accepted_without_equals() {
        let cli = Cli::try_parse_from([
            "png-glitch",
            "input.png",
            "--chromatic-aberration",
            "0.2",
            "--r-offset",
            "-5",
            "--b-offset",
            "-10",
            "--shift-channels",
            "-10,20,-30",
        ])
        .expect("negative offsets should parse without `=`");
        assert_eq!(cli.r_offset, -5);
        assert_eq!(cli.b_offset, -10);
        let shift = cli.shift_channels.unwrap();
        assert_eq!((shift.r, shift.g, shift.b), (-10, 20, -30));
    }

    #[test]
    fn test_malformed_options_rejected() {
        assert!(Cli::try_parse_from(["png-glitch", "input.png", "--substitute", "12"]).is_err());
        assert!(
            Cli::try_parse_from(["png-glitch", "input.png", "--shift-channels", "1,2"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["png-glitch", "input.webp", "--alpha-glitch", "inverse"]).is_err()
        );
    }

    #[test]
    fn test_inapplicable_format_flags_rejected() {
        let cli_png_with_webp_flag =
            Cli::try_parse_from(["png-glitch", "input.png", "--macroblock-glitch", "0.1"])
                .unwrap();
        assert!(validate_flags_for_format(&cli_png_with_webp_flag, false).is_err());

        let cli_webp_with_png_flag =
            Cli::try_parse_from(["png-glitch", "input.webp", "--replace", "0.1"]).unwrap();
        assert!(validate_flags_for_format(&cli_webp_with_png_flag, true).is_err());
    }

    #[test]
    fn test_yaml_unknown_field_rejected() {
        let bad_yaml = r#"
filters:
  - type: BlockScramble
    magnitude: 0.1
    blocksize: 64
"#;
        let res: Result<ConfigFile, _> = serde_yml::from_str(bad_yaml);
        assert!(res.is_err(), "unknown field `blocksize` must be rejected");

        let bad_mag_yaml = r#"
filters:
  - type: Replace
    magnitude: 1.5
"#;
        let res_mag: Result<ConfigFile, _> = serde_yml::from_str(bad_mag_yaml);
        assert!(
            res_mag.is_err(),
            "out-of-range magnitude in YAML must be rejected"
        );
    }

    #[test]
    fn test_cli_filter_order_respected() {
        let bytes = include_bytes!("../crates/png-glitch/etc/sample00.png");

        let matches1 = Cli::command()
            .try_get_matches_from([
                "png-glitch",
                "sample.png",
                "--seed",
                "1",
                "--invert",
                "--replace",
                "0.5",
            ])
            .unwrap();
        let args1 = Cli::from_arg_matches(&matches1).unwrap();
        let steps1 = collect_ordered_steps(&args1, Some(&matches1));
        assert!(matches!(steps1[0], OrderedStep::Invert));
        assert!(matches!(steps1[1], OrderedStep::Replace(_)));

        let matches2 = Cli::command()
            .try_get_matches_from([
                "png-glitch",
                "sample.png",
                "--seed",
                "1",
                "--replace",
                "0.5",
                "--invert",
            ])
            .unwrap();
        let args2 = Cli::from_arg_matches(&matches2).unwrap();
        let steps2 = collect_ordered_steps(&args2, Some(&matches2));
        assert!(matches!(steps2[0], OrderedStep::Replace(_)));
        assert!(matches!(steps2[1], OrderedStep::Invert));

        let mut ctx1 = GlitchContext::new(bytes, Some(1)).unwrap();
        apply_filters(&args1, &steps1, &mut ctx1).unwrap();
        ctx1.execute();
        let out1 = ctx1.buffer().unwrap();

        let mut ctx2 = GlitchContext::new(bytes, Some(1)).unwrap();
        apply_filters(&args2, &steps2, &mut ctx2).unwrap();
        ctx2.execute();
        let out2 = ctx2.buffer().unwrap();

        assert_ne!(
            out1, out2,
            "different CLI filter orders should produce different output"
        );
    }
}
