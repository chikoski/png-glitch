//! `--sweep STEP.PARAM=FROM..TO:N`: N outputs from one input where one numeric parameter of one
//! recipe step takes N evenly spaced values and the seed stays fixed.

use crate::any_context::{AnyGlitchContext, ImageFormat, detect_format};
use crate::cli::Cli;
use crate::{
    OrderedStep, check_overwrite, prepare_filters, recipe_io, resolve_output_path, resolve_seed,
    save_output, validate_flags_for_format,
};
use anyhow::{Context, anyhow, bail};
use indicatif::{ProgressBar, ProgressStyle};
use png_glitch_recipe::{FilterConfig, FilterEntry, ParseOptions};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A parsed `STEP.PARAM=FROM..TO:N`.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepSpec {
    /// A 0-based step index or a step type name.
    pub step: String,
    pub param: String,
    pub from: f64,
    pub to: f64,
    pub count: u32,
}

/// Parses `STEP.PARAM=FROM..TO:N` (used as the clap value parser).
pub fn parse_sweep(s: &str) -> Result<SweepSpec, String> {
    const USAGE: &str = "expected STEP.PARAM=FROM..TO:N (e.g. replace.magnitude=0.01..0.2:5)";
    let bad = |what: &str| format!("{what}; {USAGE}");
    let (target, range) = s.split_once('=').ok_or_else(|| bad("missing '='"))?;
    let (step, param) = target
        .split_once('.')
        .ok_or_else(|| bad("missing '.' between STEP and PARAM"))?;
    if step.is_empty() || param.is_empty() {
        return Err(bad("empty STEP or PARAM"));
    }
    let (bounds, count) = range.rsplit_once(':').ok_or_else(|| bad("missing ':N'"))?;
    let (from, to) = bounds
        .split_once("..")
        .ok_or_else(|| bad("missing '..' between FROM and TO"))?;
    let num = |t: &str, name: &str| {
        t.trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| bad(&format!("'{t}' is not a valid {name}")))
    };
    let count: u32 = count
        .trim()
        .parse()
        .ok()
        .filter(|n| (1..=1000).contains(n))
        .ok_or_else(|| bad("N must be an integer from 1 to 1000"))?;
    Ok(SweepSpec {
        step: step.to_string(),
        param: param.to_string(),
        from: num(from, "FROM")?,
        to: num(to, "TO")?,
        count,
    })
}

/// `count` evenly spaced values from `from` to `to`, both ends included (just `from` for 1).
pub fn sweep_values(from: f64, to: f64, count: u32) -> Vec<f64> {
    if count <= 1 {
        return vec![from];
    }
    let last = f64::from(count - 1);
    (0..count)
        .map(|i| {
            if i + 1 == count {
                to
            } else {
                from + (to - from) * f64::from(i) / last
            }
        })
        .collect()
}

fn normalize(name: &str) -> String {
    name.to_ascii_lowercase().replace(['_', '-'], "")
}

/// The `type` tag of a step, as it appears in a recipe.
fn step_type(yaml: &str) -> Option<String> {
    yaml.lines()
        .find_map(|l| l.strip_prefix("type:"))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
}

fn serialize_step(f: &FilterConfig) -> anyhow::Result<String> {
    serde_saphyr::to_string(f).context("failed to serialize the step")
}

/// Finds the step a sweep targets: a 0-based index, or the first step with that type name.
fn find_step(filters: &[FilterEntry], step: &str) -> anyhow::Result<usize> {
    if let Ok(i) = step.parse::<usize>() {
        if i < filters.len() {
            return Ok(i);
        }
        bail!(
            "--sweep: step {i} does not exist; the recipe has {} step(s)",
            filters.len()
        );
    }
    let wanted = normalize(step);
    for (i, f) in filters.iter().enumerate() {
        if step_type(&serialize_step(&f.config)?).is_some_and(|t| normalize(&t) == wanted) {
            return Ok(i);
        }
    }
    bail!("--sweep: no step of type '{step}' in the recipe")
}

/// Returns `filter` with its numeric field `param` set to `value`. Integer fields get the
/// value rounded to the nearest integer.
pub fn set_param(filter: &FilterConfig, param: &str, value: f64) -> anyhow::Result<FilterConfig> {
    let yaml = serialize_step(filter)?;
    let key = format!("{param}:");
    let mut found = false;
    let mut lines = Vec::new();
    for line in yaml.lines() {
        if let Some(old) = line.strip_prefix(&key) {
            let old = old.trim();
            if old.parse::<f64>().is_err() {
                bail!("--sweep: '{param}' is not a numeric parameter of this step");
            }
            found = true;
            let new = if old.parse::<i64>().is_ok() {
                format!("{}", value.round() as i64)
            } else {
                format!("{value}")
            };
            lines.push(format!("{key} {new}"));
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        bail!("--sweep: this step has no numeric parameter '{param}'");
    }
    let edited = lines.join("\n") + "\n";
    serde_saphyr::from_str(&edited)
        .with_context(|| format!("--sweep: {param}={value} is not valid for this step"))
}

/// `<stem>-<param>-<value>.<ext>` next to `base`.
pub fn sweep_path(base: &Path, param: &str, value: f64) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("glitched");
    let name = match base.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!("{stem}-{param}-{value}.{ext}"),
        None => format!("{stem}-{param}-{value}"),
    };
    base.with_file_name(name)
}

pub fn sweep_process(args: &Cli, steps: &[OrderedStep], spec: &SweepSpec) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    let input_path = Path::new(input);
    if input_path.is_dir() {
        bail!("'{input}' is a directory; --sweep works on a single image");
    }
    let data = fs::read(input).with_context(|| format!("failed to open '{input}'"))?;
    let webp = detect_format(&data) == Some(ImageFormat::Webp)
        || input_path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("webp"));
    validate_flags_for_format(args, webp)?;

    let (base_filters, masks) = prepare_filters(args, steps)?;
    let idx = find_step(&base_filters, &spec.step)?;
    let values = sweep_values(spec.from, spec.to, spec.count);
    // Validate every value before writing anything.
    let variants: Vec<Vec<FilterEntry>> = values
        .iter()
        .map(|&v| {
            let mut filters = base_filters.clone();
            filters[idx] = FilterEntry {
                config: set_param(&base_filters[idx].config, &spec.param, v)?,
                ..base_filters[idx].clone()
            };
            Ok(filters)
        })
        .collect::<anyhow::Result<_>>()?;

    let base = resolve_output_path(args, input);
    let seed = resolve_seed(args.seed);
    let paths: Vec<PathBuf> = values
        .iter()
        .map(|&v| sweep_path(&base, &spec.param, v))
        .collect();
    for path in &paths {
        check_overwrite(path, args.force)?;
    }
    if args.print_recipe {
        eprint!("{}", recipe_io::render_recipe(seed, &base_filters)?);
    }
    if let Some(path) = &args.save_recipe {
        fs::write(path, recipe_io::render_recipe(seed, &base_filters)?)
            .with_context(|| format!("failed to write recipe to '{path}'"))?;
    }

    let pb = ProgressBar::new(values.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );
    let succeeded = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    (0..values.len()).into_par_iter().for_each(|i| {
        let result = (|| -> anyhow::Result<()> {
            let options = ParseOptions::default().with_strict(args.strict);
            let mut context = AnyGlitchContext::open_with_options(input, Some(seed), options)?;
            for f in &variants[i] {
                context.add_from_entry(f.clone(), &masks)?;
            }
            context.execute();
            let recipe = recipe_io::render_recipe(seed, &variants[i])?;
            save_output(args, &context, &paths[i], &recipe)
        })();
        match result {
            Ok(()) => {
                succeeded.fetch_add(1, Ordering::Relaxed);
                pb.println(format!(
                    "{}: {}={} seed {seed}",
                    paths[i].display(),
                    spec.param,
                    values[i]
                ));
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                pb.println(format!("Error generating '{}': {e:#}", paths[i].display()));
            }
        }
        pb.inc(1);
    });

    let ok_count = succeeded.load(Ordering::Relaxed);
    let err_count = failed.load(Ordering::Relaxed);
    pb.finish_with_message(format!(
        "Sweep complete: {ok_count} succeeded, {err_count} failed"
    ));
    if err_count > 0 {
        return Err(anyhow!(
            "sweep finished with {err_count} failure(s) ({ok_count} succeeded)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spec() {
        let s = parse_sweep("replace.magnitude=0.01..0.2:5").unwrap();
        assert_eq!(
            (s.step.as_str(), s.param.as_str()),
            ("replace", "magnitude")
        );
        assert_eq!((s.from, s.to, s.count), (0.01, 0.2, 5));
        assert!(parse_sweep("replace.magnitude=0.1:5").is_err());
        assert!(parse_sweep("magnitude=0.1..0.2:5").is_err());
        assert!(parse_sweep("0.magnitude=0.1..0.2:0").is_err());
        assert!(parse_sweep("0.magnitude=a..0.2:3").is_err());
    }

    #[test]
    fn values_include_both_ends() {
        assert_eq!(sweep_values(0.0, 1.0, 5), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(sweep_values(3.0, 9.0, 1), vec![3.0]);
    }

    #[test]
    fn set_param_changes_one_field() {
        let f = FilterConfig::Replace { magnitude: 0.1 };
        assert_eq!(
            set_param(&f, "magnitude", 0.5).unwrap(),
            FilterConfig::Replace { magnitude: 0.5 }
        );
        assert!(set_param(&f, "nope", 1.0).is_err());
        assert!(set_param(&f, "magnitude", 7.0).is_err()); // out of range
        let r = FilterConfig::RandomCopy { times: 2 };
        assert_eq!(
            set_param(&r, "times", 6.0).unwrap(),
            FilterConfig::RandomCopy { times: 6 }
        );
    }

    #[test]
    fn finds_steps_by_index_or_type() {
        let fs: [FilterEntry; 2] = [
            FilterConfig::Replace { magnitude: 0.1 }.into(),
            FilterConfig::RandomCopy { times: 2 }.into(),
        ];
        assert_eq!(find_step(&fs, "1").unwrap(), 1);
        assert_eq!(find_step(&fs, "random-copy").unwrap(), 1);
        assert_eq!(find_step(&fs, "replace").unwrap(), 0);
        assert!(find_step(&fs, "2").is_err());
        assert!(find_step(&fs, "invert").is_err());
    }

    #[test]
    fn path_inserts_param_and_value() {
        assert_eq!(
            sweep_path(Path::new("d/out.png"), "magnitude", 0.25),
            PathBuf::from("d/out-magnitude-0.25.png")
        );
    }
}
