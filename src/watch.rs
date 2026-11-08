//! `--watch`: re-render whenever the recipe file or the input image changes.

use crate::any_context::AnyGlitchContext;
use crate::cli::Cli;
use crate::{
    OrderedStep, apply_steps, check_overwrite, recipe_io, report_recipe, resolve_output_path,
    resolve_seed, validate_flags_for_format,
};
use anyhow::{Context, bail};
use png_glitch_recipe::ParseOptions;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Modification time and size of a file; `None` while it cannot be read (e.g. mid-save).
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Calls `render` once, then again each time `stamps()` changes, sleeping `interval` between
/// polls. Errors from `render` are printed and do not stop the loop. Returns after
/// `max_iterations` renders (`None` runs until the process is killed).
pub fn watch_loop(
    stamps: impl Fn() -> Vec<Stamp>,
    mut render: impl FnMut() -> anyhow::Result<()>,
    interval: Duration,
    max_iterations: Option<usize>,
) {
    let mut last = stamps();
    let mut done = 0usize;
    loop {
        if let Err(e) = render() {
            eprintln!("error: {e:#} (still watching)");
        }
        done += 1;
        if max_iterations.is_some_and(|max| done >= max) {
            return;
        }
        loop {
            std::thread::sleep(interval);
            let now = stamps();
            if now != last {
                last = now;
                break;
            }
        }
    }
}

/// Writes `bytes` to `<path>.tmp` and renames it over `path`, so a viewer never sees a partial file.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes).with_context(|| format!("failed to save '{}'", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to save '{}'", path.display()))
}

fn render_once(
    args: &Cli,
    steps: &[OrderedStep],
    input: &str,
    output: &Path,
    seed: u64,
) -> anyhow::Result<()> {
    let options = ParseOptions::default().with_strict(args.strict);
    let mut context = AnyGlitchContext::open_with_options(input, Some(seed), options)?;
    validate_flags_for_format(args, context.is_webp())?;
    for w in context.warnings() {
        eprintln!("warning: {w}");
    }
    let recipe = report_recipe(args, steps, seed)?;
    apply_steps(args, steps, &mut context)?;
    context.execute();
    let bytes = if args.embed_recipe {
        recipe_io::embed_in_png(&context.buffer()?, &recipe)?
    } else {
        context.buffer()?
    };
    write_atomic(output, &bytes)?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

pub fn watch_process(args: &Cli, steps: &[OrderedStep]) -> anyhow::Result<()> {
    let input = args
        .input
        .as_deref()
        .context("missing required <INPUT> file path")?;
    if Path::new(input).is_dir() {
        bail!("'{input}' is a directory; --watch works on a single image");
    }
    let output = resolve_output_path(args, input);
    check_overwrite(&output, args.force)?;
    let seed = resolve_seed(args.seed);

    let mut watched: Vec<PathBuf> = vec![PathBuf::from(input)];
    for step in steps {
        if let OrderedStep::Config(path) = step {
            watched.push(PathBuf::from(path));
        }
    }
    eprintln!(
        "watching {} (Ctrl-C to stop)",
        watched
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    watch_loop(
        || watched.iter().map(|p| stamp(p)).collect(),
        || render_once(args, steps, input, &output, seed),
        POLL_INTERVAL,
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn loop_renders_on_start_and_on_each_change_and_survives_errors() {
        let version = Cell::new(0u64);
        let renders = Cell::new(0usize);
        watch_loop(
            || vec![Some((SystemTime::UNIX_EPOCH, version.get()))],
            || {
                renders.set(renders.get() + 1);
                version.set(version.get() + 1); // pretend the file changes after every render
                if renders.get() == 2 {
                    bail!("bad recipe");
                }
                Ok(())
            },
            Duration::from_millis(1),
            Some(4),
        );
        assert_eq!(renders.get(), 4);
    }

    #[test]
    fn write_atomic_leaves_no_tmp_file() {
        let dir = std::env::temp_dir().join("png_glitch_watch_atomic");
        fs::create_dir_all(&dir).unwrap();
        let out = dir.join("o.png");
        write_atomic(&out, b"x").unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"x");
        assert!(!dir.join("o.png.tmp").exists());
        fs::remove_dir_all(&dir).ok();
    }
}
