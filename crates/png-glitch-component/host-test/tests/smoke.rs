//! Runs the release component with wasmtime and compares it with the CLI.
//!
//! Build first: `cargo build -p png-glitch-component --target wasm32-wasip2 --release` and
//! `cargo build -p png-glitch-cli`. `PNG_GLITCH_COMPONENT` and `PNG_GLITCH_CLI` override the paths.

use png_glitch_component_host_test::{GlitchError, Host};
use std::process::Command;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn host() -> Host {
    let path = std::env::var("PNG_GLITCH_COMPONENT").unwrap_or_else(|_| {
        root()
            .join("target/wasm32-wasip2/release/png_glitch_component.wasm")
            .to_string_lossy()
            .into_owned()
    });
    Host::load(&path).expect("load the component")
}

fn cli() -> std::path::PathBuf {
    std::env::var("PNG_GLITCH_CLI")
        .map(Into::into)
        .unwrap_or_else(|_| root().join("target/debug/png-glitch"))
}

fn image() -> Vec<u8> {
    std::fs::read(root().join("crates/png-glitch/etc/sample00.png")).unwrap()
}

const RECIPES: &[&str] = &[
    "filters:\n  - type: Replace\n    magnitude: 0.05\n",
    "filters:\n  - type: sub-filter\n  - type: pixel-sort\n    magnitude: 0.2\n    criterion: hue\n",
    "filters:\n  - type: invert\n    rows: 10..40\n  - type: block-scramble\n    magnitude: 0.1\n    block_size: 8\n",
    r#"{"filters": [{"type": "ShiftChannels", "r": 10, "g": 0, "b": -10}]}"#,
];

#[test]
fn glitch_gives_the_same_bytes_as_the_cli() {
    let dir = std::env::temp_dir().join(format!("png-glitch-host-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.png");
    std::fs::write(&input, image()).unwrap();
    let mut host = host();
    for (i, recipe) in RECIPES.iter().enumerate() {
        let config = dir.join(format!("r{i}.yaml"));
        std::fs::write(&config, recipe).unwrap();
        for seed in [1u64, 42] {
            let out = dir.join(format!("out{i}-{seed}.png"));
            let status = Command::new(cli())
                .arg(&input)
                .args(["--config"])
                .arg(&config)
                .args(["--seed", &seed.to_string(), "-o"])
                .arg(&out)
                .arg("--force")
                .status()
                .expect("run the CLI");
            assert!(status.success(), "CLI failed for recipe {i}");
            let from_component = host.glitch(&image(), recipe, seed).unwrap().unwrap();
            assert_eq!(
                from_component,
                std::fs::read(&out).unwrap(),
                "recipe {i}, seed {seed}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bad_input_gives_errors_not_traps() {
    let mut host = host();
    let ok = RECIPES[0];
    assert!(matches!(
        host.glitch(b"not a png", ok, 1).unwrap(),
        Err(GlitchError::UnsupportedFormat)
    ));
    assert!(matches!(
        host.glitch(&image()[..60], ok, 1).unwrap(),
        Err(GlitchError::InvalidImage(_))
    ));
    assert!(matches!(
        host.glitch(&image(), "filters: [{type: Replace, magnitude: 9}]", 1)
            .unwrap(),
        Err(GlitchError::InvalidRecipe(_))
    ));
    assert!(matches!(
        host.validate_recipe("nope").unwrap(),
        Err(GlitchError::InvalidRecipe(_))
    ));
    assert!(host.validate_recipe(ok).unwrap().is_ok());

    // 40000 x 40000 pixels in the IHDR (with a fixed-up CRC).
    let mut png = image();
    png[16..20].copy_from_slice(&40_000u32.to_be_bytes());
    png[20..24].copy_from_slice(&40_000u32.to_be_bytes());
    let mut crc = !0u32;
    for &b in &png[12..29] {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    png[29..33].copy_from_slice(&(!crc).to_be_bytes());
    assert!(matches!(
        host.glitch(&png, ok, 1).unwrap(),
        Err(GlitchError::TooLarge)
    ));
}

#[test]
fn list_filters_has_one_entry_per_filter() {
    let mut host = host();
    let listed = host.list_filters().unwrap();
    let want = std::process::Command::new(cli())
        .args(["--list-filters", "--json"])
        .output()
        .unwrap();
    let json = String::from_utf8(want.stdout).unwrap();
    let count = json.matches("\"yaml_type\"").count();
    assert_eq!(listed.len(), count);
    assert!(
        listed
            .iter()
            .any(|f| f.name == "pixel-sort" && !f.params.is_empty())
    );
}
