//! `png-glitch-bench`: per-stage timings for `png-glitch`, runnable natively and as
//! `wasm32-wasip1` under wasmtime. Numbers from shared CI runners are only meaningful relative
//! to a baseline from the same runner (see `compare`).

mod alloc;
mod bench;
mod compare;
mod fixture;
mod report;
mod stats;

use std::process::ExitCode;

#[global_allocator]
static GLOBAL: alloc::CountingAlloc = alloc::CountingAlloc;

const USAGE: &str = "\
usage:
  png-glitch-bench [--size 360p|720p|1080p] [--frames N] [--warmup N] [--pool N]
                   [--stages TEXT] [--json PATH] [--check-live]
  png-glitch-bench compare BASE HEAD [--threshold 0.25] [--stage-threshold STAGE=VALUE]...

  --frames   measured iterations per stage (default 60)
  --warmup   unmeasured iterations per stage (default 3)
  --pool     distinct fixture frames cycled through (default 8)
  --stages   run only stages whose name contains TEXT
  --check-live  exit 1 if a live stage (live/*, reload_pixels/*, render_into/*, the in-place
             effects) allocates in steady state or its live heap grows; the file workflow
             stages stay report-only
  --stage-threshold  a threshold for one stage instead of --threshold, e.g.
             effect/pixel-sort=0.40 (repeatable); use it only for a stage that needs it
  compare    BASE and HEAD are JSON reports; each may be a comma-separated list of runs of the
             same build (the best p50 per stage is used). Exits 1 on a regression.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.first().map(String::as_str) == Some("compare") {
        compare_command(&args[1..])
    } else {
        bench_command(&args)
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Splits `--flag value` pairs and bare positionals.
type Parsed = (Vec<(String, String)>, Vec<String>);

fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut flags = Vec::new();
    let mut positional = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--check-live" {
            flags.push(("check-live".to_string(), String::new()));
        } else if let Some(flag) = arg.strip_prefix("--") {
            let value = iter.next().ok_or(format!("`--{flag}` needs a value"))?;
            flags.push((flag.to_string(), value.clone()));
        } else {
            positional.push(arg.clone());
        }
    }
    Ok((flags, positional))
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value `{value}` for `--{flag}`"))
}

fn bench_command(args: &[String]) -> Result<ExitCode, String> {
    let (flags, positional) = parse_args(args)?;
    if !positional.is_empty() {
        return Err(format!("unexpected argument `{}`", positional[0]));
    }
    let mut size = fixture::Size::P1080;
    let mut frames = 60;
    let mut warmup = 3;
    let mut pool = 8;
    let mut only = None;
    let mut json = None;
    let mut check_live = false;
    for (flag, value) in &flags {
        match flag.as_str() {
            "check-live" => check_live = true,
            "size" => size = value.parse()?,
            "frames" => frames = number(flag, value)?,
            "warmup" => warmup = number(flag, value)?,
            "pool" => pool = number(flag, value)?,
            "stages" => only = Some(value.clone()),
            "json" => json = Some(value.clone()),
            other => return Err(format!("unknown flag `--{other}`")),
        }
    }
    if frames == 0 {
        return Err("`--frames` must be at least 1".into());
    }
    let (width, height) = size.dimensions();
    let report = bench::run_all(&bench::Config {
        size_name: size.name().to_string(),
        width,
        height,
        frames,
        warmup,
        pool,
        only,
    });
    println!("{}", report.table());
    if let Some(path) = json {
        let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("cannot write `{path}`: {e}"))?;
    }
    if check_live {
        let violations = report.live_violations();
        if !violations.is_empty() {
            eprintln!("\nFAILED: the live path allocates in steady state:");
            for v in &violations {
                eprintln!("  {v}");
            }
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn load(list: &str) -> Result<Vec<report::Report>, String> {
    list.split(',')
        .map(|path| {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
            serde_json::from_str(&text).map_err(|e| format!("invalid report `{path}`: {e}"))
        })
        .collect()
}

fn compare_command(args: &[String]) -> Result<ExitCode, String> {
    let (flags, positional) = parse_args(args)?;
    let [base, head] = positional.as_slice() else {
        return Err("`compare` needs BASE and HEAD".into());
    };
    let mut threshold = 0.25;
    let mut per_stage = Vec::new();
    for (flag, value) in &flags {
        match flag.as_str() {
            "threshold" => threshold = number(flag, value)?,
            "stage-threshold" => {
                let (stage, t) = value
                    .split_once('=')
                    .ok_or("`--stage-threshold` needs STAGE=VALUE")?;
                per_stage.push((stage.to_string(), number("stage-threshold", t)?));
            }
            other => return Err(format!("unknown flag `--{other}`")),
        }
    }
    let rows = compare::compare(&load(base)?, &load(head)?, threshold, &per_stage);
    print!("{}", compare::render(&rows, threshold, &per_stage));
    if rows.iter().any(|r| r.regressed) {
        println!(
            "\nFAILED: at least one stage regressed by more than {:.0}%",
            threshold * 100.0
        );
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}
