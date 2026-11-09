//! `-` as standard input and output (#75).
//!
//! `INPUT` = `-` reads the whole image from stdin (the format comes from its magic bytes) and
//! `-o -` writes the image to stdout. With `-` as input and no `-o`, the output also goes to
//! stdout, like a Unix filter. Nothing but the image is written to stdout: the seed, warnings
//! and `--print-recipe` go to stderr.

use crate::any_context::AnyGlitchContext;
use crate::cli::Cli;
use anyhow::{Context, bail};
use png_glitch_recipe::ParseOptions;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;

/// The name that stands for stdin or stdout.
pub const STDIO: &str = "-";

/// Whether `input` names stdin.
pub fn is_stdin(input: &str) -> bool {
    input == STDIO
}

/// Whether `path` names stdout.
pub fn is_stdout(path: &Path) -> bool {
    path == Path::new(STDIO)
}

/// Reads all of stdin.
pub fn read_stdin() -> anyhow::Result<Vec<u8>> {
    let mut data = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut data)
        .context("failed to read the image from standard input")?;
    Ok(data)
}

/// Reads `input`, which may be `-` for stdin.
pub fn read_input(input: &str) -> anyhow::Result<Vec<u8>> {
    if is_stdin(input) {
        read_stdin()
    } else {
        std::fs::read(input).with_context(|| format!("failed to open '{input}'"))
    }
}

/// Opens `input` (a path, or `-` for stdin) as a glitch context. An image from stdin is
/// recognised by its magic bytes and an unknown format is an error: there is no file extension
/// to fall back on.
pub fn open_context(
    input: &str,
    seed: Option<u64>,
    options: ParseOptions,
) -> anyhow::Result<AnyGlitchContext> {
    if is_stdin(input) {
        let data = read_stdin()?;
        AnyGlitchContext::from_bytes_with_options(&data, seed, options)
    } else {
        AnyGlitchContext::open_with_options(input, seed, options)
    }
}

/// Fails if binary data would go to a terminal, unless `force`. Like `gzip`.
pub fn refuse_tty(is_terminal: bool, force: bool) -> anyhow::Result<()> {
    if is_terminal && !force {
        bail!("refusing to write binary image data to a terminal (use --force or redirect stdout)");
    }
    Ok(())
}

/// Writes `bytes` to stdout, after the terminal check.
pub fn write_stdout(bytes: &[u8], force: bool) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    refuse_tty(out.is_terminal(), force)?;
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .context("failed to write the image to standard output")
}

/// Fails if the run reads stdin or writes stdout, for the modes that cannot (several outputs,
/// a directory, a file to watch).
pub fn reject_stdio(args: &Cli, mode: &str) -> anyhow::Result<()> {
    if args.input.as_deref().is_some_and(is_stdin) {
        bail!("'-' (stdin) cannot be used with {mode}");
    }
    if args.output_file.as_deref() == Some(STDIO) {
        bail!("'-o -' (stdout) cannot be used with {mode}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_is_refused_unless_forced() {
        assert!(refuse_tty(true, false).is_err());
        assert!(refuse_tty(true, true).is_ok());
        assert!(refuse_tty(false, false).is_ok());
        assert!(refuse_tty(false, true).is_ok());
        let message = refuse_tty(true, false).unwrap_err().to_string();
        assert!(message.contains("refusing to write binary image data to a terminal"));
    }

    #[test]
    fn dash_is_stdio() {
        assert!(is_stdin("-") && !is_stdin("a.png") && !is_stdin("-x"));
        assert!(is_stdout(Path::new("-")) && !is_stdout(Path::new("./-")));
    }
}
