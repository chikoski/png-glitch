//! Integration tests that run the real `png-glitch` binary.

use assert_cmd::Command;
use insta::assert_snapshot;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SEED: &str = "12345";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn cmd() -> Command {
    Command::cargo_bin("png-glitch").expect("binary builds")
}

/// Copies a fixture into `dir` and returns the new path.
fn copy_fixture(dir: &TempDir, name: &str) -> PathBuf {
    let dest = dir.path().join(name);
    fs::copy(fixture(name), &dest).unwrap();
    dest
}

// ---------------------------------------------------------------- exit codes

#[test]
fn invalid_arguments_exit_with_2() {
    cmd()
        .arg("--no-such-flag")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("error"));
}

#[test]
fn out_of_range_magnitude_exits_with_2() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["--replace", "1.5"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("between 0.0 and 1.0"));
}

#[test]
fn missing_input_argument_exits_with_2() {
    cmd().assert().code(2);
}

#[test]
fn unreadable_input_exits_with_1() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("does-not-exist.png");
    cmd()
        .arg(&missing)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("does-not-exist.png"));
}

#[test]
fn corrupt_input_exits_with_1() {
    let dir = TempDir::new().unwrap();
    let bad = dir.path().join("bad.png");
    fs::write(&bad, b"this is not a png").unwrap();
    cmd().arg(&bad).args(["--seed", SEED]).assert().code(1);
}

#[test]
fn success_exits_with_0_and_writes_default_output() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert"])
        .assert()
        .success();
    assert!(dir.path().join("small-glitched.png").exists());
}

#[test]
fn list_filters_does_not_require_input() {
    cmd()
        .arg("--list-filters")
        .assert()
        .success()
        .stdout(predicate::str::contains("invert"));
}

#[test]
fn directory_without_batch_output_is_an_error() {
    let dir = TempDir::new().unwrap();
    cmd()
        .arg(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--batch-output"));
}

// ------------------------------------------------------- overwrite protection

#[test]
fn refuses_to_overwrite_without_force() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let out = dir.path().join("out.png");
    fs::write(&out, b"precious").unwrap();

    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert", "-o"])
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(fs::read(&out).unwrap(), b"precious");

    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert", "--force", "-o"])
        .arg(&out)
        .assert()
        .success();
    assert_ne!(fs::read(&out).unwrap(), b"precious");
}

// ---------------------------------------------------------------------- seed

#[test]
fn prints_seed_when_omitted() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["-f", "replace=0.1"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(r"seed: \d+").unwrap());
}

#[test]
fn explicit_seed_is_not_printed() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "replace=0.1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("seed:").not());
}

#[test]
fn same_seed_gives_identical_output_and_different_seed_differs() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let run = |seed: &str, name: &str| {
        let out = dir.path().join(name);
        cmd()
            .arg(&input)
            .args(["--seed", seed, "-f", "replace=0.3", "-o"])
            .arg(&out)
            .assert()
            .success();
        fs::read(out).unwrap()
    };
    let a = run("1", "a.png");
    let b = run("1", "b.png");
    let c = run("2", "c.png");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ------------------------------------------------------ format compatibility

#[test]
fn png_only_filter_on_webp_input_is_an_error() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.webp");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "--replace", "0.1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not supported for WebP"));
    assert!(!dir.path().join("small-glitched.webp").exists());
}

#[test]
fn webp_only_filter_on_png_input_is_an_error() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "--macroblock-glitch", "0.1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not supported for PNG"));
    assert!(!dir.path().join("small-glitched.png").exists());
}

#[test]
fn unsupported_filter_spec_for_format_is_an_error() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.webp");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "transpose=0.1"])
        .assert()
        .failure()
        .code(1);
}

// -------------------------------------------------------------------- batch

#[test]
fn batch_processes_directory_and_keeps_structure() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("in");
    fs::create_dir_all(input.join("sub")).unwrap();
    fs::copy(fixture("small.png"), input.join("a.png")).unwrap();
    fs::copy(fixture("small.webp"), input.join("sub/b.WEBP")).unwrap();
    let out = dir.path().join("out");

    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert", "--batch-output"])
        .arg(&out)
        .assert()
        .success();
    assert!(out.join("a.png").exists());
    assert!(out.join("sub/b.WEBP").exists());
}

#[test]
fn batch_fails_when_any_file_fails() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("in");
    fs::create_dir_all(&input).unwrap();
    fs::copy(fixture("small.png"), input.join("good.png")).unwrap();
    fs::write(input.join("bad.png"), b"garbage").unwrap();
    let out = dir.path().join("out");

    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert", "--batch-output"])
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("1 failure"));
    // The healthy file is still processed.
    assert!(out.join("good.png").exists());
}

#[test]
fn batch_without_images_is_an_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("empty");
    fs::create_dir_all(&input).unwrap();
    cmd()
        .arg(&input)
        .arg("--batch-output")
        .arg(dir.path().join("out"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("No PNG or WebP files"));
}

#[test]
fn batch_refuses_to_overwrite_without_force() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("in");
    let out = dir.path().join("out");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&out).unwrap();
    fs::copy(fixture("small.png"), input.join("a.png")).unwrap();
    fs::write(out.join("a.png"), b"precious").unwrap();

    cmd()
        .arg(&input)
        .args(["--seed", SEED, "-f", "invert", "--batch-output"])
        .arg(&out)
        .assert()
        .code(1);
    assert_eq!(fs::read(out.join("a.png")).unwrap(), b"precious");
}

// ------------------------------------------------------------ golden outputs

/// Runs the binary with a fixed seed and returns the BLAKE3 hash of the output.
fn golden_hash(fixture_name: &str, filter: &str) -> String {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, fixture_name);
    let out = dir.path().join(format!("out-{fixture_name}"));
    cmd()
        .arg(&input)
        .args(["--seed", "42", "-f", filter, "-o"])
        .arg(&out)
        .assert()
        .success();
    blake3::hash(&fs::read(out).unwrap()).to_hex().to_string()
}

macro_rules! golden {
    ($name:ident, $fixture:expr, $filter:expr) => {
        #[test]
        fn $name() {
            assert_snapshot!(golden_hash($fixture, $filter));
        }
    };
}

// Shared filters (PNG)
golden!(golden_png_invert, "small.png", "invert");
golden!(golden_png_brighten, "small.png", "brighten=30");
golden!(golden_png_darken, "small.png", "darken=30");
golden!(
    golden_png_shift_channels,
    "small.png",
    "shift-channels=15,0,-15"
);
golden!(
    golden_png_pixel_sort,
    "small.png",
    "pixel-sort:magnitude=1.0,criterion=brightness"
);
golden!(
    golden_png_bitwise,
    "small.png",
    "bitwise:magnitude=0.2,op=xor,value=255"
);
golden!(
    golden_png_channel_swap,
    "small.png",
    "channel-swap:magnitude=0.5,target=br"
);
golden!(
    golden_png_horizontal_shift,
    "small.png",
    "horizontal-shift=0.3"
);
golden!(
    golden_png_color_distortion,
    "small.png",
    "color-distortion:magnitude=0.3,strength=40"
);
golden!(
    golden_png_chromatic_aberration,
    "small.png",
    "chromatic-aberration:magnitude=0.5,r=4,b=-4"
);
// PNG-only filters
golden!(
    golden_png_block_scramble,
    "small.png",
    "block-scramble:magnitude=0.3,block_size=8"
);
golden!(
    golden_png_color_space_glitch,
    "small.png",
    "color-space-glitch:magnitude=0.5,hue_shift=90"
);
golden!(
    golden_png_change_filter_type,
    "small.png",
    "change-filter-type=0.3"
);
golden!(golden_png_replace, "small.png", "replace=0.1");
golden!(golden_png_transpose, "small.png", "transpose=0.3");
golden!(golden_png_set_zero, "small.png", "set-zero=0.5");
golden!(golden_png_random_copy, "small.png", "random-copy=5");
golden!(golden_png_substitute, "small.png", "substitute=3:255");
golden!(golden_png_remove_filter, "small.png", "remove-filter");
golden!(golden_png_sub_filter, "small.png", "sub-filter");
golden!(golden_png_up_filter, "small.png", "up-filter");
golden!(golden_png_average_filter, "small.png", "average-filter");
golden!(golden_png_paeth_filter, "small.png", "paeth-filter");
// Shared filters (WebP)
golden!(golden_webp_invert, "small.webp", "invert");
golden!(golden_webp_brighten, "small.webp", "brighten=30");
golden!(
    golden_webp_shift_channels,
    "small.webp",
    "shift-channels=15,0,-15"
);
golden!(golden_webp_pixel_sort, "small.webp", "pixel-sort=0.5");
golden!(
    golden_webp_color_distortion,
    "small.webp",
    "color-distortion=0.3"
);
// WebP-only filters
golden!(
    golden_webp_macroblock_glitch,
    "small.webp",
    "macroblock-glitch=0.5"
);
golden!(
    golden_webp_alpha_glitch,
    "small.webp",
    "alpha-glitch=invert"
);
golden!(
    golden_webp_lossy_artifact,
    "small.webp",
    "lossy-artifact=10"
);

// ---------------------------------------------------------------------- help

#[test]
fn help_output_snapshot() {
    let out = cmd().arg("--help").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // On Windows clap prints the executable name with its `.exe` suffix.
    let text = text.replace("png-glitch.exe", "png-glitch");
    assert_snapshot!(text);
}

// ------------------------------------------------- thread-count independence

/// Filters that consume the RNG, with arguments that make them do real work on the fixture.
const RNG_FILTERS: &[&str] = &[
    "change-filter-type=0.5",
    "replace=0.3",
    "transpose=0.5",
    "set-zero=0.5",
    "random-copy=5",
    "pixel-sort=0.5",
    "bitwise:magnitude=0.5,op=xor,value=255",
    "channel-swap:magnitude=0.5,target=br",
    "horizontal-shift=0.5",
    "block-scramble:magnitude=0.5,block_size=8",
    "color-distortion:magnitude=0.5,strength=40",
    "color-space-glitch:magnitude=0.5,hue_shift=90",
    "chromatic-aberration:magnitude=0.5,r=4,b=-4",
];

/// Runs the binary in its own process with the given rayon thread count.
fn run_with_threads(threads: &str, input: &Path, args: &[&str], out: &Path) -> Vec<u8> {
    cmd()
        .env("RAYON_NUM_THREADS", threads)
        .arg(input)
        .args(["--seed", "7"])
        .args(args)
        .arg(out)
        .assert()
        .success();
    fs::read(out).unwrap()
}

#[test]
fn rng_filters_are_independent_of_thread_count() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    for (i, filter) in RNG_FILTERS.iter().enumerate() {
        let one = run_with_threads(
            "1",
            &input,
            &["-f", filter, "-o"],
            &dir.path().join(format!("one{i}.png")),
        );
        let many = run_with_threads(
            "8",
            &input,
            &["-f", filter, "-o"],
            &dir.path().join(format!("many{i}.png")),
        );
        assert_eq!(
            one, many,
            "output of `{filter}` depends on the thread count"
        );
    }
}

#[test]
fn chained_filters_are_independent_of_thread_count() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let args: Vec<&str> = RNG_FILTERS
        .iter()
        .flat_map(|f| ["-f", *f])
        .chain(["-o"])
        .collect();
    let one = run_with_threads("1", &input, &args, &dir.path().join("one.png"));
    let many = run_with_threads("8", &input, &args, &dir.path().join("many.png"));
    assert_eq!(one, many);
}

#[test]
fn batch_output_is_independent_of_thread_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("in");
    fs::create_dir_all(&input).unwrap();
    for i in 0..4 {
        fs::copy(fixture("small.png"), input.join(format!("{i}.png"))).unwrap();
    }
    fs::copy(fixture("small.webp"), input.join("w.webp")).unwrap();
    let run = |threads: &str, out: &Path| {
        cmd()
            .env("RAYON_NUM_THREADS", threads)
            .arg(&input)
            .args([
                "--seed",
                "7",
                "-f",
                "pixel-sort=0.5",
                "-f",
                "color-distortion=0.3",
                "--batch-output",
            ])
            .arg(out)
            .assert()
            .success();
    };
    let (one, many) = (dir.path().join("one"), dir.path().join("many"));
    run("1", &one);
    run("8", &many);
    for name in ["0.png", "1.png", "2.png", "3.png", "w.webp"] {
        assert_eq!(
            fs::read(one.join(name)).unwrap(),
            fs::read(many.join(name)).unwrap(),
            "{name} differs between thread counts"
        );
    }
}

// ------------------------------------------------------------------- recipes

fn run(input: &Path, output: &Path, args: &[&str]) {
    cmd()
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(["--seed", SEED])
        .args(args)
        .assert()
        .success();
}

#[test]
fn saved_recipe_reproduces_the_output_byte_for_byte() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let first = dir.path().join("first.png");
    let again = dir.path().join("again.png");
    let recipe = dir.path().join("recipe.yaml");

    run(
        &input,
        &first,
        &[
            "--pre-process",
            "sub-filter",
            "--replace",
            "0.2",
            "-f",
            "pixel-sort:magnitude=0.3,criterion=hue",
            "--invert",
            "--save-recipe",
            recipe.to_str().unwrap(),
        ],
    );
    let text = fs::read_to_string(&recipe).unwrap();
    assert!(text.starts_with(&format!("# seed: {SEED}\n")), "{text}");

    run(&input, &again, &["--config", recipe.to_str().unwrap()]);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&again).unwrap());
}

#[test]
fn print_recipe_goes_to_stderr_in_order() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .arg("-o")
        .arg(dir.path().join("out.png"))
        .args([
            "--seed",
            SEED,
            "--print-recipe",
            "-f",
            "replace=0.1",
            "-f",
            "invert",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("# seed: 12345"))
        .stderr(predicate::str::is_match("(?s)Replace.*Invert").unwrap());
}

#[test]
fn embedded_recipe_can_be_shown_and_survives_a_rerun() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let out = dir.path().join("embedded.png");
    run(&input, &out, &["--replace", "0.2", "--embed-recipe"]);

    let shown = cmd().arg(&out).arg("--show-recipe").output().unwrap();
    assert!(shown.status.success());
    let text = String::from_utf8(shown.stdout).unwrap();
    assert!(
        text.contains("# seed: 12345") && text.contains("Replace"),
        "{text}"
    );

    // The embedded text is itself a valid recipe that reproduces the image.
    let recipe = dir.path().join("recipe.yaml");
    fs::write(&recipe, &text).unwrap();
    let again = dir.path().join("again.png");
    run(
        &input,
        &again,
        &["--config", recipe.to_str().unwrap(), "--embed-recipe"],
    );
    assert_eq!(fs::read(&out).unwrap(), fs::read(&again).unwrap());

    // Re-running on the glitched output still decodes and replaces the recipe.
    let rerun = dir.path().join("rerun.png");
    run(&out, &rerun, &["--invert", "--embed-recipe"]);
    let shown = cmd().arg(&rerun).arg("--show-recipe").output().unwrap();
    let text = String::from_utf8(shown.stdout).unwrap();
    assert!(
        text.contains("Invert") && !text.contains("Replace"),
        "{text}"
    );
    assert_eq!(text.matches("# seed").count(), 1);
}

#[test]
fn show_recipe_without_embedded_recipe_fails() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .arg("--show-recipe")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no embedded recipe"));
}

#[test]
fn embed_recipe_is_rejected_for_webp() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.webp");
    cmd()
        .arg(&input)
        .arg("-o")
        .arg(dir.path().join("out.webp"))
        .args(["--seed", SEED, "--invert", "--embed-recipe"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--embed-recipe"));
}

// ------------------------------------------------ set-filter-type / regenerate (#139)

/// Runs the CLI on `small.png` with `args` and returns (output bytes, stdout, stderr).
fn run_png(args: &[&str]) -> (Vec<u8>, Vec<u8>, String) {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let output = dir.path().join("out.png");
    let assert = cmd()
        .arg(&input)
        .args(["--seed", SEED, "-o"])
        .arg(&output)
        .args(args)
        .assert()
        .success();
    let out = assert.get_output();
    (
        fs::read(&output).unwrap(),
        out.stdout.clone(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn regenerate_matches_the_deprecated_flags_and_specs() {
    for t in ["none", "sub", "up", "average", "paeth"] {
        let old_spec = if t == "none" {
            "remove-filter".to_string()
        } else {
            format!("{t}-filter")
        };
        let (old, _, _) = run_png(&["-f", &old_spec]);
        let (new, _, _) = run_png(&["-f", &format!("regenerate={t}")]);
        assert_eq!(old, new, "{t}");
    }
}

#[test]
fn set_filter_type_and_regenerate_are_different_operations() {
    let (tag, _, _) = run_png(&["-f", "set-filter-type=up"]);
    let (values, _, _) = run_png(&["-f", "regenerate=up"]);
    assert_ne!(tag, values);
    let (key_value, _, _) = run_png(&["-f", "set-filter-type:filter=up"]);
    assert_eq!(tag, key_value);
}

#[test]
fn regenerate_accepts_adaptive_and_patterns() {
    run_png(&["-f", "regenerate=adaptive"]);
    let (a, _, _) = run_png(&["-f", "regenerate=sub,up,paeth"]);
    let (b, _, _) = run_png(&["-f", "regenerate=SUB,Up,paeth"]);
    assert_eq!(a, b);
    let (one, _, _) = run_png(&["-f", "regenerate=sub,"]);
    let (fixed, _, _) = run_png(&["-f", "regenerate=sub"]);
    assert_eq!(one, fixed);
}

#[test]
fn invalid_set_filter_type_and_regenerate_specs_exit_with_2() {
    for spec in [
        "set-filter-type",
        "set-filter-type=bogus",
        "regenerate",
        "regenerate=",
        "regenerate=sub,,up",
    ] {
        let dir = TempDir::new().unwrap();
        let input = copy_fixture(&dir, "small.png");
        cmd().arg(&input).args(["-f", spec]).assert().code(2);
    }
}

#[test]
fn deprecated_flags_warn_on_stderr_only() {
    let (flag_out, stdout, stderr) = run_png(&["--paeth"]);
    assert!(stderr.contains("`--paeth` is deprecated"), "{stderr}");
    assert!(stderr.contains("regenerate=paeth"), "{stderr}");
    let (spec_out, spec_stdout, spec_stderr) = run_png(&["-f", "paeth-filter"]);
    assert_eq!(flag_out, spec_out);
    assert_eq!(stdout, spec_stdout);
    assert!(!spec_stderr.contains("deprecated"), "{spec_stderr}");
}

#[test]
fn webp_rejects_the_new_png_only_steps() {
    for spec in ["set-filter-type=paeth", "regenerate=paeth"] {
        let dir = TempDir::new().unwrap();
        let input = copy_fixture(&dir, "small.webp");
        cmd()
            .arg(&input)
            .args(["-f", spec])
            .assert()
            .failure()
            .stderr(predicate::str::contains("not supported for WebP"));
    }
}

// ---------------------------------------------------------------- variations

fn png_size(path: &Path) -> (u32, u32) {
    let b = fs::read(path).unwrap();
    (
        u32::from_be_bytes(b[16..20].try_into().unwrap()),
        u32::from_be_bytes(b[20..24].try_into().unwrap()),
    )
}

#[test]
fn variations_match_plain_runs_with_the_derived_seeds() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let out = dir.path().join("v.png");
    cmd()
        .arg(&input)
        .args([
            "--variations",
            "3",
            "--seed",
            "42",
            "-f",
            "replace=0.1",
            "-o",
        ])
        .arg(&out)
        .assert()
        .success();
    for i in 0..3u64 {
        let seed = png_glitch_recipe::frame_seed(42, i);
        let varied = dir.path().join(format!("v-s{seed}.png"));
        assert!(varied.exists(), "missing {}", varied.display());
        let plain = dir.path().join(format!("plain{i}.png"));
        cmd()
            .arg(&input)
            .args(["--seed", &seed.to_string(), "-f", "replace=0.1", "-o"])
            .arg(&plain)
            .assert()
            .success();
        assert_eq!(fs::read(&varied).unwrap(), fs::read(&plain).unwrap());
    }
}

#[test]
fn variations_are_reproducible_and_refuse_to_overwrite() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let run = |name: &str, extra: &[&str]| {
        let out = dir.path().join(name);
        let mut c = cmd();
        c.arg(&input)
            .args([
                "--variations",
                "2",
                "--seed",
                "7",
                "-f",
                "replace=0.1",
                "-o",
            ])
            .arg(&out)
            .args(extra);
        c
    };
    run("a.png", &[]).assert().success();
    run("a.png", &[])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    run("a.png", &["--force"]).assert().success();
}

#[test]
fn contact_sheet_has_expected_dimensions() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let (w, h) = png_size(&input);
    let sheet = dir.path().join("sheet.png");
    cmd()
        .arg(&input)
        .args([
            "--variations",
            "5",
            "--seed",
            "1",
            "-f",
            "replace=0.05",
            "--contact-cell-width",
            "16",
            "--contact-sheet",
        ])
        .arg(&sheet)
        .assert()
        .success();
    let cell_h = ((h as u64 * 16 + w as u64 / 2) / w as u64).max(1) as u32;
    assert_eq!(png_size(&sheet), (3 * 16, 2 * cell_h)); // 5 cells -> 3 columns x 2 rows
}

#[test]
fn variations_conflict_with_batch_output_and_sheet_needs_variations() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd()
        .arg(&input)
        .args(["--variations", "2", "--batch-output"])
        .arg(dir.path().join("o"))
        .assert()
        .code(2);
    cmd()
        .arg(&input)
        .arg("--contact-sheet")
        .arg(dir.path().join("s.png"))
        .assert()
        .code(2);
    cmd()
        .arg(&input)
        .args(["--variations", "0"])
        .assert()
        .code(2);
}

// ---------------------------------------------------------------- sweep

#[test]
fn sweep_writes_n_files_and_the_middle_one_matches_a_plain_run() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let out = dir.path().join("w.png");
    cmd()
        .arg(&input)
        .args([
            "--seed",
            "9",
            "-f",
            "replace=0.1",
            "--sweep",
            "replace.magnitude=0.1..0.3:3",
            "-o",
        ])
        .arg(&out)
        .assert()
        .success();
    for v in ["0.1", "0.2", "0.3"] {
        assert!(dir.path().join(format!("w-magnitude-{v}.png")).exists());
    }
    let plain = dir.path().join("plain.png");
    cmd()
        .arg(&input)
        .args(["--seed", "9", "-f", "replace=0.2", "-o"])
        .arg(&plain)
        .assert()
        .success();
    assert_eq!(
        fs::read(dir.path().join("w-magnitude-0.2.png")).unwrap(),
        fs::read(&plain).unwrap()
    );
}

#[test]
fn sweep_rejects_unknown_steps_parameters_and_conflicts() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let run = |sweep: &str| {
        let mut c = cmd();
        c.arg(&input)
            .args(["-f", "replace=0.1", "--sweep", sweep, "-o"])
            .arg(dir.path().join("e.png"));
        c
    };
    run("invert.magnitude=0..1:2").assert().failure();
    run("5.magnitude=0..1:2").assert().failure();
    run("replace.nope=0..1:2").assert().failure();
    run("replace.magnitude=0..9:2").assert().failure();
    run("replace.magnitude=bad").assert().code(2);
    assert!(!dir.path().join("e-magnitude-0.png").exists());
    cmd()
        .arg(&input)
        .args(["--sweep", "0.magnitude=0..1:2", "--variations", "2"])
        .assert()
        .code(2);
}
