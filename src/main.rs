use anyhow::{Context, bail};
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use png_glitch_recipe::{FilterConfig, ParseOptions, PreProcess};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

mod any_context;
mod cli;
mod recipe_io;

use crate::any_context::AnyGlitchContext;
use crate::cli::{Cli, ConfigFile, format_filter_list};

#[derive(Debug, Clone)]
enum OrderedStep {
    Config(String),
    Filter(FilterConfig),
}

fn collect_ordered_steps(args: &Cli, matches: Option<&ArgMatches>) -> Vec<OrderedStep> {
    let idx = |id: &str, fallback: usize| -> usize {
        matches.and_then(|m| m.index_of(id)).unwrap_or(fallback)
    };

    let mut steps: Vec<(usize, OrderedStep)> = Vec::new();

    if let Some(cfg) = &args.config {
        steps.push((idx("config", 10), OrderedStep::Config(cfg.clone())));
    }
    if !args.filters.is_empty() {
        let indices: Vec<usize> = matches
            .and_then(|m| m.indices_of("filters"))
            .map(|it| it.collect())
            .unwrap_or_default();
        for (k, filter_cfg) in args.filters.iter().enumerate() {
            let pos = indices.get(k).copied().unwrap_or(15 + k);
            steps.push((pos, OrderedStep::Filter(filter_cfg.clone())));
        }
    }
    if let Some(magnitude) = args.change_filter_type {
        steps.push((
            idx("change_filter_type", 20),
            OrderedStep::Filter(FilterConfig::ChangeFilterType { magnitude }),
        ));
    }
    if let Some(magnitude) = args.replace {
        steps.push((
            idx("replace", 30),
            OrderedStep::Filter(FilterConfig::Replace { magnitude }),
        ));
    }
    if let Some(magnitude) = args.transpose {
        steps.push((
            idx("transpose", 40),
            OrderedStep::Filter(FilterConfig::Transpose { magnitude }),
        ));
    }
    if let Some(magnitude) = args.set_zero {
        steps.push((
            idx("set_zero", 50),
            OrderedStep::Filter(FilterConfig::SetZero { magnitude }),
        ));
    }
    if let Some(times) = args.random_copy {
        steps.push((
            idx("random_copy", 60),
            OrderedStep::Filter(FilterConfig::RandomCopy { times }),
        ));
    }
    if let Some(sub) = args.substitute {
        steps.push((
            idx("substitute", 70),
            OrderedStep::Filter(FilterConfig::Substitute {
                index: sub.index,
                value: sub.value,
            }),
        ));
    }
    if let Some(magnitude) = args.pixel_sort {
        steps.push((
            idx("pixel_sort", 80),
            OrderedStep::Filter(FilterConfig::PixelSort {
                magnitude,
                criterion: Some(args.pixel_sort_criterion),
            }),
        ));
    }
    if let Some(magnitude) = args.bitwise {
        steps.push((
            idx("bitwise", 90),
            OrderedStep::Filter(FilterConfig::Bitwise {
                magnitude,
                op: Some(args.bitwise_op),
                value: Some(args.bitwise_value),
            }),
        ));
    }
    if let Some(magnitude) = args.channel_swap {
        steps.push((
            idx("channel_swap", 100),
            OrderedStep::Filter(FilterConfig::ChannelSwap {
                magnitude,
                target: Some(args.channel_swap_target),
            }),
        ));
    }
    if let Some(magnitude) = args.horizontal_shift {
        steps.push((
            idx("horizontal_shift", 110),
            OrderedStep::Filter(FilterConfig::HorizontalShift { magnitude }),
        ));
    }
    if let Some(magnitude) = args.block_scramble {
        steps.push((
            idx("block_scramble", 120),
            OrderedStep::Filter(FilterConfig::BlockScramble {
                magnitude,
                block_size: Some(args.block_scramble_size),
            }),
        ));
    }
    if let Some(magnitude) = args.color_distortion {
        steps.push((
            idx("color_distortion", 130),
            OrderedStep::Filter(FilterConfig::ColorDistortion {
                magnitude,
                strength: Some(args.color_distortion_strength),
            }),
        ));
    }
    if let Some(magnitude) = args.color_space_glitch {
        steps.push((
            idx("color_space_glitch", 140),
            OrderedStep::Filter(FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift: Some(args.hue_shift),
                saturation_mult: Some(args.saturation_mult),
                lightness_mult: Some(args.lightness_mult),
            }),
        ));
    }
    if let Some(magnitude) = args.chromatic_aberration {
        steps.push((
            idx("chromatic_aberration", 150),
            OrderedStep::Filter(FilterConfig::ChromaticAberration {
                magnitude,
                r_offset: Some(args.r_offset),
                g_offset: Some(args.g_offset),
                b_offset: Some(args.b_offset),
            }),
        ));
    }
    if args.invert {
        steps.push((
            idx("invert", 160),
            OrderedStep::Filter(FilterConfig::Invert),
        ));
    }
    if let Some(strength) = args.brighten {
        steps.push((
            idx("brighten", 170),
            OrderedStep::Filter(FilterConfig::Brighten { strength }),
        ));
    }
    if let Some(strength) = args.darken {
        steps.push((
            idx("darken", 175),
            OrderedStep::Filter(FilterConfig::Darken { strength }),
        ));
    }
    if let Some(ch) = args.shift_channels {
        steps.push((
            idx("shift_channels", 180),
            OrderedStep::Filter(FilterConfig::ShiftChannels {
                r: ch.r,
                g: ch.g,
                b: ch.b,
            }),
        ));
    }
    if args.remove_filter {
        steps.push((
            idx("remove_filter", 190),
            OrderedStep::Filter(FilterConfig::RemoveFilter),
        ));
    }
    if args.sub {
        steps.push((
            idx("sub", 200),
            OrderedStep::Filter(FilterConfig::SubFilter),
        ));
    }
    if args.up {
        steps.push((idx("up", 210), OrderedStep::Filter(FilterConfig::UpFilter)));
    }
    if args.average {
        steps.push((
            idx("average", 220),
            OrderedStep::Filter(FilterConfig::AverageFilter),
        ));
    }
    if args.paeth {
        steps.push((
            idx("paeth", 230),
            OrderedStep::Filter(FilterConfig::PaethFilter),
        ));
    }
    if let Some(magnitude) = args.macroblock_glitch {
        steps.push((
            idx("macroblock_glitch", 240),
            OrderedStep::Filter(FilterConfig::MacroblockGlitch { magnitude }),
        ));
    }
    if let Some(strategy) = args.alpha_glitch {
        steps.push((
            idx("alpha_glitch", 250),
            OrderedStep::Filter(FilterConfig::AlphaGlitch {
                magnitude: 1.0,
                strategy: Some(strategy),
            }),
        ));
    }
    if let Some(quality) = args.lossy_quality {
        steps.push((
            idx("lossy_quality", 260),
            OrderedStep::Filter(FilterConfig::LossyArtifact { quality }),
        ));
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
        if args.embed_recipe {
            unsupported.push("--embed-recipe");
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

fn default_output_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("glitched");
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("png");
    let file_name = format!("{stem}-glitched.{ext}");
    match input.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(file_name),
        _ => PathBuf::from(file_name),
    }
}

fn resolve_output_path(args: &Cli, input: &str) -> PathBuf {
    match &args.output_file {
        Some(out) => PathBuf::from(out),
        None => default_output_path(Path::new(input)),
    }
}

fn check_overwrite(path: &Path, force: bool) -> anyhow::Result<()> {
    if path.exists() && !force {
        bail!(
            "output file '{}' already exists (use --force to overwrite)",
            path.display()
        );
    }
    Ok(())
}

fn resolve_seed(seed: Option<u64>) -> u64 {
    match seed {
        Some(s) => s,
        None => {
            let s: u64 = rand::random();
            eprintln!("seed: {s}");
            s
        }
    }
}

fn main() -> anyhow::Result<()> {
    let matches = Cli::command().get_matches();
    let args = Cli::from_arg_matches(&matches)?;
    if args.list_filters {
        print!("{}", format_filter_list());
        return Ok(());
    }

    if args.show_recipe {
        return show_recipe(&args);
    }

    let steps = collect_ordered_steps(&args, Some(&matches));

    if let Some(batch_output) = &args.batch_output {
        batch_process(&args, &steps, batch_output)
    } else {
        single_process(&args, &steps)
    }
}

fn single_process(args: &Cli, steps: &[OrderedStep]) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    let input_path = Path::new(input);
    if input_path.is_dir() {
        bail!(
            "'{}' is a directory; use --batch-output <DIR> to process all images in a directory",
            input_path.display()
        );
    }

    // Check format compatibility from extension first if unambiguous, before opening
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if ext.eq_ignore_ascii_case("webp") {
        validate_flags_for_format(args, true)?;
    } else if ext.eq_ignore_ascii_case("png") {
        validate_flags_for_format(args, false)?;
    }

    let parse_options = ParseOptions::default().with_strict(args.strict);
    let mut context = AnyGlitchContext::open_with_options(input, args.seed, parse_options)?;
    validate_flags_for_format(args, context.is_webp())?;
    for w in context.warnings() {
        eprintln!("warning: {w}");
    }

    let output_path = resolve_output_path(args, input);
    check_overwrite(&output_path, args.force)?;

    let seed = resolve_seed(args.seed);
    context.set_seed(Some(seed));
    let recipe = report_recipe(args, steps, seed)?;
    apply_steps(args, steps, &mut context)?;
    context.execute();
    save_output(args, &context, &output_path, &recipe)?;
    Ok(())
}

fn compute_batch_dest_path(input_dir: &Path, file_path: &Path, output_dir: &Path) -> PathBuf {
    let rel_path = file_path
        .strip_prefix(input_dir)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new(file_path.file_name().unwrap_or_default()));
    output_dir.join(rel_path)
}

fn process_batch_file(
    args: &Cli,
    steps: &[OrderedStep],
    path: &Path,
    dest_path: &Path,
    seed: u64,
    recipe: &str,
) -> anyhow::Result<()> {
    check_overwrite(dest_path, args.force)?;
    if let Some(parent) = dest_path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory '{}'", parent.display()))?;
    }

    let parse_options = ParseOptions::default().with_strict(args.strict);
    let mut context = AnyGlitchContext::open_with_options(path, Some(seed), parse_options)?;
    validate_flags_for_format(args, context.is_webp())?;
    for w in context.warnings() {
        eprintln!("warning ({}): {w}", path.display());
    }
    apply_steps(args, steps, &mut context)?;
    context.execute();
    save_output(args, &context, dest_path, recipe)?;
    Ok(())
}

fn find_batch_images(input_dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("webp"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn batch_process(args: &Cli, steps: &[OrderedStep], output_dir: &str) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> directory path")?;
    let input_dir = Path::new(input);
    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)
            .with_context(|| format!("failed to create output directory '{output_dir}'"))?;
    }

    let image_files = find_batch_images(input_dir);
    if image_files.is_empty() {
        bail!("No PNG or WebP files found in '{input}'");
    }

    let seed = resolve_seed(args.seed);
    let recipe = report_recipe(args, steps, seed)?;

    let pb = ProgressBar::new(image_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    let succeeded = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    image_files.par_iter().for_each(|path| {
        let dest_path = compute_batch_dest_path(input_dir, path, output_path);
        let display_name = path
            .strip_prefix(input_dir)
            .unwrap_or(path.as_path())
            .display()
            .to_string();

        pb.set_message(format!("Processing {display_name}"));

        match process_batch_file(args, steps, path, &dest_path, seed, &recipe) {
            Ok(()) => {
                succeeded.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                pb.println(format!("Error processing '{}': {:#}", path.display(), e));
            }
        }

        pb.inc(1);
    });

    let ok_count = succeeded.load(Ordering::Relaxed);
    let err_count = failed.load(Ordering::Relaxed);
    pb.finish_with_message(format!(
        "Batch processing complete: {ok_count} succeeded, {err_count} failed"
    ));

    if err_count > 0 {
        bail!("batch processing finished with {err_count} failure(s) ({ok_count} succeeded)");
    }

    Ok(())
}

/// The effective recipe: the pre-process option, then `--config` files and filter flags in the
/// order they were given.
fn effective_filters(args: &Cli, steps: &[OrderedStep]) -> anyhow::Result<Vec<FilterConfig>> {
    let mut filters = Vec::new();
    if let Some(pre_process) = args.pre_process {
        filters.push(match pre_process {
            PreProcess::RemoveFilter => FilterConfig::RemoveFilter,
            PreProcess::SubFilter => FilterConfig::SubFilter,
            PreProcess::UpFilter => FilterConfig::UpFilter,
            PreProcess::AverageFilter => FilterConfig::AverageFilter,
            PreProcess::PaethFilter => FilterConfig::PaethFilter,
        });
    }

    for step in steps {
        match step {
            OrderedStep::Config(config_path) => {
                let yaml = std::fs::read_to_string(config_path)
                    .with_context(|| format!("failed to open config file '{config_path}'"))?;
                let config: ConfigFile = serde_saphyr::from_str(&yaml)
                    .with_context(|| format!("failed to parse config file '{config_path}'"))?;
                filters.extend(config.filters);
            }
            OrderedStep::Filter(filter_cfg) => filters.push(filter_cfg.clone()),
        }
    }
    Ok(filters)
}

fn apply_steps(
    args: &Cli,
    steps: &[OrderedStep],
    context: &mut AnyGlitchContext,
) -> anyhow::Result<()> {
    for filter in effective_filters(args, steps)? {
        context.add_from_config(filter)?;
    }
    Ok(())
}

/// Prints and/or saves the effective recipe as requested, and returns its text.
fn report_recipe(args: &Cli, steps: &[OrderedStep], seed: u64) -> anyhow::Result<String> {
    let text = recipe_io::render_recipe(seed, &effective_filters(args, steps)?)?;
    if args.print_recipe {
        eprint!("{text}");
    }
    if let Some(path) = &args.save_recipe {
        fs::write(path, &text).with_context(|| format!("failed to write recipe to '{path}'"))?;
    }
    Ok(text)
}

/// Saves the glitched image, embedding the recipe when asked.
fn save_output(
    args: &Cli,
    context: &AnyGlitchContext,
    path: &Path,
    recipe: &str,
) -> anyhow::Result<()> {
    if args.embed_recipe {
        let bytes = recipe_io::embed_in_png(&context.buffer()?, recipe)?;
        fs::write(path, bytes).with_context(|| format!("failed to save '{}'", path.display()))
    } else {
        context.save(path)
    }
}

fn show_recipe(args: &Cli) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    let data = fs::read(input).with_context(|| format!("failed to open '{input}'"))?;
    match recipe_io::extract_from_png(&data) {
        Some(text) => {
            print!("{text}");
            Ok(())
        }
        None => bail!("'{input}' has no embedded recipe (it is written by `--embed-recipe`)"),
    }
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
            Cli::try_parse_from(["png-glitch", "input.png", "--macroblock-glitch", "0.1"]).unwrap();
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
        let res: Result<ConfigFile, _> = serde_saphyr::from_str(bad_yaml);
        assert!(res.is_err(), "unknown field `blocksize` must be rejected");

        let bad_mag_yaml = r#"
filters:
  - type: Replace
    magnitude: 1.5
"#;
        let res_mag: Result<ConfigFile, _> = serde_saphyr::from_str(bad_mag_yaml);
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
        assert!(matches!(
            steps1[0],
            OrderedStep::Filter(FilterConfig::Invert)
        ));
        assert!(matches!(
            steps1[1],
            OrderedStep::Filter(FilterConfig::Replace { .. })
        ));

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
        assert!(matches!(
            steps2[0],
            OrderedStep::Filter(FilterConfig::Replace { .. })
        ));
        assert!(matches!(
            steps2[1],
            OrderedStep::Filter(FilterConfig::Invert)
        ));

        let mut ctx1 = AnyGlitchContext::from_bytes(bytes, Some(1)).unwrap();
        apply_steps(&args1, &steps1, &mut ctx1).unwrap();
        ctx1.execute();
        let out1 = ctx1.buffer().unwrap();

        let mut ctx2 = AnyGlitchContext::from_bytes(bytes, Some(1)).unwrap();
        apply_steps(&args2, &steps2, &mut ctx2).unwrap();
        ctx2.execute();
        let out2 = ctx2.buffer().unwrap();

        assert_ne!(
            out1, out2,
            "different CLI filter orders should produce different output"
        );
    }

    #[test]
    fn test_default_output_path_preserves_extension_and_directory() {
        assert_eq!(
            default_output_path(Path::new("photo.png")),
            PathBuf::from("photo-glitched.png")
        );
        assert_eq!(
            default_output_path(Path::new("dir/anim.webp")),
            PathBuf::from("dir/anim-glitched.webp")
        );
    }

    #[test]
    fn test_output_and_batch_output_conflict() {
        assert!(
            Cli::try_parse_from([
                "png-glitch",
                "in_dir",
                "-o",
                "out.png",
                "--batch-output",
                "out_dir"
            ])
            .is_err()
        );
    }

    #[test]
    fn test_batch_preserves_subdirectory_structure() {
        let in_dir = Path::new("/tmp/in");
        let out_dir = Path::new("/tmp/out");
        assert_eq!(
            compute_batch_dest_path(in_dir, Path::new("/tmp/in/a/x.png"), out_dir),
            PathBuf::from("/tmp/out/a/x.png")
        );
        assert_eq!(
            compute_batch_dest_path(in_dir, Path::new("/tmp/in/b/x.png"), out_dir),
            PathBuf::from("/tmp/out/b/x.png")
        );
    }

    #[test]
    fn test_overwrite_protection_requires_force() {
        let tmp = std::env::temp_dir().join("png_glitch_test_overwrite.png");
        fs::write(&tmp, b"dummy").unwrap();
        assert!(check_overwrite(&tmp, false).is_err());
        assert!(check_overwrite(&tmp, true).is_ok());
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_directory_without_batch_output_gives_helpful_error() {
        let tmp_dir = std::env::temp_dir().join("png_glitch_test_dir_err");
        fs::create_dir_all(&tmp_dir).unwrap();
        let cli = Cli::try_parse_from(["png-glitch", tmp_dir.to_str().unwrap()]).unwrap();
        let err = single_process(&cli, &[]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("--batch-output"),
            "expected '--batch-output' hint in error, got: {msg}"
        );
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_error_message_includes_path_without_duplicate_context() {
        let cli = Cli::try_parse_from([
            "png-glitch",
            "nonexistent_test_file_12345.png",
            "-o",
            "/tmp/nonexistent_out_12345.png",
            "--seed",
            "1",
        ])
        .unwrap();
        let err = single_process(&cli, &[]).unwrap_err();
        let chain = format!("{err:#}");
        assert!(
            chain.contains("nonexistent_test_file_12345.png"),
            "error should include file path, got: {chain}"
        );
        assert_eq!(
            chain.matches("failed to open").count(),
            1,
            "context should not be duplicated, got: {chain}"
        );
    }

    #[test]
    fn test_batch_case_insensitive_and_failure_exit() {
        let base = std::env::temp_dir().join("png_glitch_test_batch");
        let _ = fs::remove_dir_all(&base);
        let in_dir = base.join("in");
        let out_dir = base.join("out");
        fs::create_dir_all(in_dir.join("sub")).unwrap();

        let sample = include_bytes!("../crates/png-glitch/etc/sample00.png");
        fs::write(in_dir.join("IMG_001.PNG"), sample).unwrap();
        fs::write(in_dir.join("sub").join("corrupt.png"), b"not a png").unwrap();

        let found = find_batch_images(&in_dir);
        assert_eq!(found.len(), 2, "should find uppercase .PNG and .png");

        let cli = Cli::try_parse_from([
            "png-glitch",
            in_dir.to_str().unwrap(),
            "--batch-output",
            out_dir.to_str().unwrap(),
            "--seed",
            "1",
            "--invert",
        ])
        .unwrap();
        let steps = collect_ordered_steps(&cli, None);
        let res = batch_process(&cli, &steps, out_dir.to_str().unwrap());
        assert!(res.is_err(), "partial batch failure must return Err");
        assert!(
            out_dir.join("IMG_001.PNG").exists(),
            "valid uppercase PNG should still be processed"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_list_filters_does_not_require_input() {
        let cli = Cli::try_parse_from(["png-glitch", "--list-filters"])
            .expect("--list-filters should succeed without <INPUT>");
        assert!(cli.list_filters);
        assert!(cli.input.is_none());
        let list = format_filter_list();
        assert!(list.contains("chromatic-aberration"));
        assert!(list.contains("macroblock-glitch"));
        assert!(list.contains("sub-filter"));
    }

    #[test]
    fn test_repeatable_filter_flag_and_spec_parsing() {
        let matches = Cli::command()
            .try_get_matches_from([
                "png-glitch",
                "sample.png",
                "-f",
                "sub-filter",
                "-f",
                "replace=0.05",
                "-f",
                "chromatic-aberration:magnitude=0.2,r=4,b=-4",
                "--invert",
                "-f",
                "replace=0.1",
            ])
            .expect("repeatable -f flags should parse");
        let args = Cli::from_arg_matches(&matches).unwrap();
        let steps = collect_ordered_steps(&args, Some(&matches));
        assert_eq!(steps.len(), 5);
        assert!(matches!(
            steps[0],
            OrderedStep::Filter(FilterConfig::SubFilter)
        ));
        assert!(matches!(
            steps[1],
            OrderedStep::Filter(FilterConfig::Replace { magnitude }) if (magnitude - 0.05).abs() < 1e-6
        ));
        assert!(matches!(
            steps[2],
            OrderedStep::Filter(FilterConfig::ChromaticAberration {
                magnitude,
                r_offset: Some(4),
                b_offset: Some(-4),
                ..
            }) if (magnitude - 0.2).abs() < 1e-6
        ));
        assert!(matches!(
            steps[3],
            OrderedStep::Filter(FilterConfig::Invert)
        ));
        assert!(matches!(
            steps[4],
            OrderedStep::Filter(FilterConfig::Replace { magnitude }) if (magnitude - 0.1).abs() < 1e-6
        ));
    }

    #[test]
    fn test_filter_override_aliases_and_yaml_kebab_case() {
        let cli = Cli::try_parse_from([
            "png-glitch",
            "input.png",
            "--sub-filter",
            "--up-filter",
            "--average-filter",
            "--paeth-filter",
            "--darken",
            "500",
        ])
        .expect("filter override aliases and --darken should parse");
        assert!(cli.sub && cli.up && cli.average && cli.paeth);
        assert_eq!(cli.darken, Some(500));

        let yaml = r#"
filters:
  - type: remove-filter
  - type: block-scramble
    magnitude: 0.1
    block_size: 32
  - type: ChromaticAberration
    magnitude: 0.2
    r_offset: 3
    b_offset: -3
"#;
        let cfg: ConfigFile =
            serde_saphyr::from_str(yaml).expect("kebab and PascalCase YAML should parse");
        assert_eq!(cfg.filters.len(), 3);
    }

    #[test]
    fn test_invalid_filter_parameters_and_strict_flag() {
        assert!(
            Cli::try_parse_from([
                "png-glitch",
                "input.png",
                "--color-distortion",
                "0.5",
                "--color-distortion-strength",
                "-5"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "png-glitch",
                "input.png",
                "-f",
                "color-distortion:magnitude=0.5,strength=-5"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from(["png-glitch", "input.png", "--random-copy", "100001"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["png-glitch", "input.png", "-f", "random-copy=100001"]).is_err()
        );
        assert!(
            Cli::try_parse_from([
                "png-glitch",
                "input.png",
                "--color-space-glitch",
                "0.5",
                "--saturation-mult",
                "-1.0"
            ])
            .is_err()
        );
        let strict_cli = Cli::try_parse_from(["png-glitch", "input.png", "--strict"])
            .expect("--strict flag should parse");
        assert!(strict_cli.strict);
    }

    #[test]
    fn test_filter_display_round_trips_through_spec_parser() {
        use crate::cli::parse_filter_spec;
        use png_glitch_recipe::{BitOp, SortCriterion, SwapTarget};
        let configs = vec![
            FilterConfig::Replace { magnitude: 0.1 },
            FilterConfig::RandomCopy { times: 5 },
            FilterConfig::Substitute {
                index: 4,
                value: 255,
            },
            FilterConfig::PixelSort {
                magnitude: 0.2,
                criterion: Some(SortCriterion::Hue),
            },
            FilterConfig::Bitwise {
                magnitude: 0.3,
                op: Some(BitOp::And),
                value: Some(7),
            },
            FilterConfig::ChannelSwap {
                magnitude: 0.5,
                target: Some(SwapTarget::Br),
            },
            FilterConfig::BlockScramble {
                magnitude: 0.1,
                block_size: Some(32),
            },
            FilterConfig::ColorDistortion {
                magnitude: 0.2,
                strength: Some(30),
            },
            FilterConfig::ColorSpaceGlitch {
                magnitude: 0.2,
                hue_shift: Some(-90.5),
                saturation_mult: Some(1.5),
                lightness_mult: Some(0.5),
            },
            FilterConfig::ChromaticAberration {
                magnitude: 0.2,
                r_offset: Some(4),
                g_offset: None,
                b_offset: Some(-4),
            },
            FilterConfig::Invert,
            FilterConfig::Brighten { strength: 30 },
            FilterConfig::ShiftChannels {
                r: 15,
                g: 0,
                b: -15,
            },
            FilterConfig::PaethFilter,
            FilterConfig::MacroblockGlitch { magnitude: 0.1 },
            FilterConfig::LossyArtifact { quality: 15.5 },
        ];
        for config in configs {
            let spec = config.to_string();
            assert_eq!(parse_filter_spec(&spec), Ok(config), "{spec}");
        }
    }
}
