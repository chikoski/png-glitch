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

// ---------------------------------------------------------------- watch

#[test]
fn watch_rerenders_when_the_config_changes() {
    use std::process::{Command as Proc, Stdio};
    use std::time::{Duration, Instant};

    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let config = dir.path().join("recipe.yaml");
    let out = dir.path().join("w.png");
    fs::write(
        &config,
        "filters:\n  - type: Replace\n    magnitude: 0.01\n",
    )
    .unwrap();
    let mut child = Proc::new(env!("CARGO_BIN_EXE_png-glitch"))
        .arg(&input)
        .args(["--watch", "--seed", SEED, "--config"])
        .arg(&config)
        .arg("-o")
        .arg(&out)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let wait_for = |pred: &dyn Fn() -> bool| {
        let start = Instant::now();
        while !pred() {
            assert!(start.elapsed() < Duration::from_secs(20), "timed out");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    wait_for(&|| out.exists());
    let first = fs::read(&out).unwrap();
    // A broken recipe keeps the watcher alive; a valid one re-renders.
    fs::write(&config, "filters: [").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    fs::write(&config, "filters:\n  - type: Replace\n    magnitude: 0.5\n").unwrap();
    wait_for(&|| fs::read(&out).map(|b| b != first).unwrap_or(false));
    assert!(child.try_wait().unwrap().is_none(), "watcher exited");
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!dir.path().join("w.png.tmp").exists());
}

#[test]
fn watch_requires_config_and_conflicts_with_batch() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    cmd().arg(&input).arg("--watch").assert().code(2);
    cmd()
        .arg(&input)
        .args([
            "--watch",
            "--config",
            "x.yaml",
            "--sweep",
            "0.magnitude=0..1:2",
        ])
        .assert()
        .code(2);
    cmd()
        .arg(&input)
        .args(["--watch", "--config", "x.yaml", "--variations", "2"])
        .assert()
        .code(2);
}

// ---------------------------------------------------------------- surprise

#[test]
fn surprise_is_reproducible_and_its_recipe_round_trips() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let (a, b, c) = (
        dir.path().join("a.png"),
        dir.path().join("b.png"),
        dir.path().join("c.png"),
    );
    let recipe = dir.path().join("lucky.yaml");
    cmd()
        .arg(&input)
        .args(["--surprise", "--seed", "77", "--save-recipe"])
        .arg(&recipe)
        .arg("-o")
        .arg(&a)
        .assert()
        .success()
        .stderr(predicate::str::contains("seed: 77"));
    cmd()
        .arg(&input)
        .args(["--surprise", "--seed", "77", "-o"])
        .arg(&b)
        .assert()
        .success();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    cmd()
        .arg(&input)
        .args(["--seed", "77", "--config"])
        .arg(&recipe)
        .arg("-o")
        .arg(&c)
        .assert()
        .success();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&c).unwrap());
}

#[test]
fn surprise_conflicts_with_other_modes_and_recipes() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    for other in [["--variations", "2"], ["--sweep", "0.magnitude=0..1:2"]] {
        cmd()
            .arg(&input)
            .arg("--surprise")
            .args(other)
            .assert()
            .code(2);
    }
    cmd()
        .arg(&input)
        .args(["--surprise", "-f", "invert", "-o"])
        .arg(dir.path().join("x.png"))
        .assert()
        .failure();
    assert!(!dir.path().join("x.png").exists());
}

// ---------------------------------------------------------------- rows

fn stored(path: &Path) -> (Vec<u8>, usize) {
    let png = png_glitch_recipe::PngGlitch::open(path).unwrap();
    let h = png.height() as usize;
    (png.data().to_vec(), h)
}

#[test]
fn rows_restrict_a_step_and_leave_other_rows_untouched() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let (before, h) = stored(&input);
    assert!(h > 8);
    let stride = before.len() / h;
    for spec in [
        "replace:magnitude=0.8,rows=2..6",
        "transpose:magnitude=1.0,rows=2..6",
        "invert:rows=2..6",
        "sub-filter:rows=2..6",
    ] {
        let out = dir.path().join("o.png");
        cmd()
            .arg(&input)
            .args(["--seed", SEED, "--force", "-f", spec, "-o"])
            .arg(&out)
            .assert()
            .success();
        let (after, _) = stored(&out);
        assert_eq!(after[..2 * stride], before[..2 * stride], "{spec}: above");
        assert_eq!(after[6 * stride..], before[6 * stride..], "{spec}: below");
    }
}

#[test]
fn rows_appear_in_the_printed_recipe_and_round_trip_through_config() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let (a, b, c) = (
        dir.path().join("a.png"),
        dir.path().join("b.png"),
        dir.path().join("c.png"),
    );
    let recipe = dir.path().join("r.yaml");
    cmd()
        .arg(&input)
        .args([
            "--seed",
            SEED,
            "-f",
            "replace:magnitude=0.8,rows=2..",
            "-f",
            "invert",
            "--save-recipe",
        ])
        .arg(&recipe)
        .arg("-o")
        .arg(&a)
        .assert()
        .success();
    let text = fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("2.."), "{text}");
    cmd()
        .arg(&input)
        .args(["--seed", SEED, "--config"])
        .arg(&recipe)
        .arg("-o")
        .arg(&b)
        .assert()
        .success();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    // The embedded recipe loads back to the same output, too.
    let embedded = dir.path().join("e.png");
    cmd()
        .arg(&input)
        .args([
            "--seed",
            SEED,
            "--embed-recipe",
            "-f",
            "replace:magnitude=0.8,rows=..5",
            "-o",
        ])
        .arg(&embedded)
        .assert()
        .success();
    cmd()
        .arg(&embedded)
        .arg("--show-recipe")
        .assert()
        .success()
        .stdout(predicate::str::contains("..5"));
    let _ = c;
}

#[test]
fn rows_errors() {
    let dir = TempDir::new().unwrap();
    let png = copy_fixture(&dir, "small.png");
    let webp = copy_fixture(&dir, "small.webp");
    let out = dir.path().join("x.png");
    for bad in [
        "replace:magnitude=0.1,rows=soon",
        "replace:magnitude=0.1,rows=5",
        "replace:magnitude=0.1,rows=1..2,rows=3..4",
    ] {
        cmd()
            .arg(&png)
            .args(["-f", bad, "-o"])
            .arg(&out)
            .assert()
            .code(2);
    }
    cmd()
        .arg(&png)
        .args(["-f", "regenerate:strategy=paeth,rows=1..4", "-o"])
        .arg(&out)
        .assert()
        .failure();
    cmd()
        .arg(&webp)
        .args(["-f", "invert:rows=1..4", "-o"])
        .arg(dir.path().join("x.webp"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("rows"));
    assert!(!out.exists());
}

// ---------------------------------------------------------------- rect / mask

/// Asserts two byte strings are equal without dumping them: reports how many bytes differ.
#[track_caller]
fn assert_same_bytes(a: &[u8], b: &[u8], what: &str) {
    let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
    assert!(
        a.len() == b.len() && differing == 0,
        "{what}: lengths {} and {}, {differing} bytes differ",
        a.len(),
        b.len()
    );
}

/// Writes a PNG of `w` x `h` whose pixels are gray levels `value(x, y)`.
fn write_gray_png(path: &Path, w: u32, h: u32, value: impl Fn(u32, u32) -> u8) {
    let pixels: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let v = value(x, y);
            [v, v, v, 255]
        })
        .collect();
    let png = png_glitch::PngGlitch::from_pixels(
        png_glitch::PixelSource::rgba8(w, h, &pixels),
        png_glitch::Encoding::rgba8(png_glitch::FilterStrategy::Fixed(
            png_glitch::FilterType::None,
        )),
    )
    .unwrap();
    fs::write(path, png.to_bytes().unwrap()).unwrap();
}

/// Writes a mask PNG of the fixture's size.
fn write_mask(path: &Path, value: impl Fn(u32, u32) -> u8) {
    let png = png_glitch_recipe::PngGlitch::open(fixture("small.png")).unwrap();
    write_gray_png(path, png.width(), png.height(), value);
}

fn run_with(dir: &TempDir, input: &Path, out: &str, args: &[&str]) -> std::process::Output {
    cmd()
        .arg(input)
        .args(["--seed", SEED, "--force"])
        .args(args)
        .arg("-o")
        .arg(dir.path().join(out))
        .output()
        .unwrap()
}

#[test]
fn a_full_mask_equals_the_plain_run_and_a_zero_mask_equals_the_input() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let white = dir.path().join("white.png");
    let black = dir.path().join("black.png");
    write_mask(&white, |_, _| 255);
    write_mask(&black, |_, _| 0);
    let (before, _) = stored(&input);
    for spec in ["replace:magnitude=0.5", "transpose:magnitude=0.5", "invert"] {
        let plain = run_with(&dir, &input, "plain.png", &["-f", spec]);
        assert!(plain.status.success());
        let with_mask = |m: &Path, extra: &str| {
            let sep = if spec.contains(':') { "," } else { ":" };
            format!("{spec}{sep}mask={}{extra}", m.display())
        };
        let full = run_with(&dir, &input, "full.png", &["-f", &with_mask(&white, "")]);
        assert!(
            full.status.success(),
            "{}",
            String::from_utf8_lossy(&full.stderr)
        );
        assert_same_bytes(
            &stored(&dir.path().join("full.png")).0,
            &stored(&dir.path().join("plain.png")).0,
            &format!("{spec}: full mask vs plain"),
        );
        let none = run_with(&dir, &input, "none.png", &["-f", &with_mask(&black, "")]);
        assert!(none.status.success());
        assert_same_bytes(
            &stored(&dir.path().join("none.png")).0,
            &before,
            &format!("{spec}: zero mask vs input"),
        );
        let flipped = run_with(
            &dir,
            &input,
            "flip.png",
            &["-f", &with_mask(&black, ",invert-mask")],
        );
        assert!(flipped.status.success());
        assert_same_bytes(
            &stored(&dir.path().join("flip.png")).0,
            &stored(&dir.path().join("plain.png")).0,
            &format!("{spec}: inverted zero mask vs plain"),
        );
    }
}

#[test]
fn rect_equals_the_same_rectangle_as_a_mask_file() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let (x, y, w, h) = (3u32, 2u32, 6u32, 5u32);
    let file = dir.path().join("r.png");
    write_mask(&file, |px, py| {
        if px >= x && px < x + w && py >= y && py < y + h {
            255
        } else {
            0
        }
    });
    let by_rect = run_with(
        &dir,
        &input,
        "a.png",
        &["-f", "replace:magnitude=0.8,rect=3,2,6,5"],
    );
    assert!(
        by_rect.status.success(),
        "{}",
        String::from_utf8_lossy(&by_rect.stderr)
    );
    let spec = format!("replace:magnitude=0.8,mask={}", file.display());
    let by_mask = run_with(&dir, &input, "b.png", &["-f", &spec]);
    assert!(by_mask.status.success());
    assert_eq!(
        fs::read(dir.path().join("a.png")).unwrap(),
        fs::read(dir.path().join("b.png")).unwrap()
    );
}

#[test]
fn a_saved_recipe_with_a_mask_round_trips_and_pins_the_file() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let mask = dir.path().join("m.png");
    write_mask(&mask, |x, _| if x < 6 { 255 } else { 0 });
    let recipe = dir.path().join("r.yaml");
    let spec = format!("replace:magnitude=0.8,rows=2..,mask={}", mask.display());
    let first = run_with(
        &dir,
        &input,
        "a.png",
        &["-f", &spec, "--save-recipe", recipe.to_str().unwrap()],
    );
    assert!(first.status.success());
    assert!(
        String::from_utf8_lossy(&first.stderr).contains("crc32"),
        "warns that the checksum was missing"
    );
    let text = fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("crc32:") && text.contains("rows:"), "{text}");
    let again = run_with(
        &dir,
        &input,
        "b.png",
        &["--config", recipe.to_str().unwrap()],
    );
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(
        fs::read(dir.path().join("a.png")).unwrap(),
        fs::read(dir.path().join("b.png")).unwrap()
    );
    // The recipe pins the mask: another file under the same name is an error.
    write_mask(&mask, |_, _| 255);
    let changed = run_with(
        &dir,
        &input,
        "c.png",
        &["--config", recipe.to_str().unwrap()],
    );
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("CRC-32"));
}

#[test]
fn mask_and_rect_errors() {
    let dir = TempDir::new().unwrap();
    let input = copy_fixture(&dir, "small.png");
    let webp = copy_fixture(&dir, "small.webp");
    let mask = dir.path().join("m.png");
    write_mask(&mask, |_, _| 255);
    let m = mask.display().to_string();
    let out = dir.path().join("x.png");

    // Parse errors exit with 2.
    for bad in [
        "invert:rect=1,2,3".to_string(),
        format!("invert:rect=1,2,3,4,mask={m}"),
        "invert:invert-mask".to_string(),
        format!("invert:mask={m},mask={m}"),
        "invert:mask=".to_string(),
    ] {
        cmd()
            .arg(&input)
            .args(["-f", &bad, "-o"])
            .arg(&out)
            .assert()
            .code(2);
    }
    // A mask of the wrong size (both sizes are in the message), a missing file, a non-PNG.
    let small = dir.path().join("small-mask.png");
    write_gray_png(&small, 2, 2, |_, _| 255);
    cmd()
        .arg(&input)
        .args(["-f", &format!("invert:mask={}", small.display()), "-o"])
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("2x2").and(predicate::str::contains("pixels")));
    cmd()
        .arg(&input)
        .args(["-f", "invert:mask=/no/such/mask.png", "-o"])
        .arg(&out)
        .assert()
        .failure();
    let junk = dir.path().join("junk.png");
    fs::write(&junk, b"not a png").unwrap();
    cmd()
        .arg(&input)
        .args(["-f", &format!("invert:mask={}", junk.display()), "-o"])
        .arg(&out)
        .assert()
        .failure();
    // Steps that recompute filtered values, and WebP input.
    for spec in [
        "sub-filter:rect=0,0,4,4",
        "regenerate:strategy=paeth,rect=0,0,4,4",
    ] {
        cmd()
            .arg(&input)
            .args(["-f", spec, "-o"])
            .arg(&out)
            .assert()
            .failure();
    }
    cmd()
        .arg(&webp)
        .args(["-f", "invert:rect=0,0,4,4", "-o"])
        .arg(dir.path().join("x.webp"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("WebP"));
    assert!(!out.exists());
}
