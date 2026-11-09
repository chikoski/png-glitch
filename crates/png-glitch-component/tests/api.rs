//! The plain-Rust API behind the component: output equals the library's, errors map to the
//! WIT variants, and no input panics.

use png_glitch_component::{GlitchError, glitch, list_filters, validate_recipe};
use png_glitch_recipe::{FilterConfig, GlitchContext};

const IMAGE: &[u8] = include_bytes!("../../png-glitch/etc/sample00.png");
const RECIPE: &str = "filters:\n  - type: Replace\n    magnitude: 0.05\n  - type: invert\n    rows: 10..40\n  - type: pixel-sort\n    magnitude: 0.2\n    criterion: hue\n    rect: 5,5,30,30\n";

fn native(seed: u64) -> Vec<u8> {
    let mut ctx = GlitchContext::new(IMAGE, Some(seed)).unwrap();
    ctx.add_from_config(FilterConfig::Replace { magnitude: 0.05 })
        .unwrap();
    ctx.execute();
    ctx.buffer().unwrap()
}

#[test]
fn matches_the_library_for_the_same_recipe_and_seed() {
    let out = glitch(
        IMAGE,
        "filters:\n  - type: Replace\n    magnitude: 0.05\n",
        42,
    )
    .unwrap();
    assert_eq!(out, native(42));
    assert_ne!(out, IMAGE);
}

#[test]
fn same_input_same_bytes_and_seed_matters() {
    let a = glitch(IMAGE, RECIPE, 1).unwrap();
    assert_eq!(a, glitch(IMAGE, RECIPE, 1).unwrap());
    assert_ne!(a, glitch(IMAGE, RECIPE, 2).unwrap());
}

#[test]
fn json_recipes_work() {
    let json = r#"{"filters": [{"type": "Replace", "magnitude": 0.05}]}"#;
    let yaml = "filters:\n  - type: Replace\n    magnitude: 0.05\n";
    assert_eq!(
        glitch(IMAGE, json, 7).unwrap(),
        glitch(IMAGE, yaml, 7).unwrap()
    );
}

#[test]
fn errors_map_to_the_wit_variants() {
    assert_eq!(
        glitch(b"RIFF\0\0\0\0WEBPVP8 ", RECIPE, 1),
        Err(GlitchError::UnsupportedFormat)
    );
    assert_eq!(
        glitch(b"hello", RECIPE, 1),
        Err(GlitchError::UnsupportedFormat)
    );
    assert_eq!(glitch(b"", RECIPE, 1), Err(GlitchError::UnsupportedFormat));
    let mut truncated = IMAGE.to_vec();
    truncated.truncate(60);
    assert!(matches!(
        glitch(&truncated, RECIPE, 1),
        Err(GlitchError::InvalidImage(_))
    ));
    for bad in [
        "not: a recipe",
        "filters: [{type: Replace, magnitude: 1.5}]",
        "filters: [{type: Nope}]",
        "filters: [{type: random-copy, times: 100001}]",
        "filters: [{type: macroblock-glitch, magnitude: 0.1}]",
        "filters: [{type: regenerate, strategy: adaptive, rows: 0..5}]",
        "",
    ] {
        assert!(
            matches!(glitch(IMAGE, bad, 1), Err(GlitchError::InvalidRecipe(_))),
            "{bad}"
        );
        assert!(
            matches!(validate_recipe(bad), Err(GlitchError::InvalidRecipe(_))),
            "{bad}"
        );
    }
    assert!(validate_recipe(RECIPE).is_ok());
}

#[test]
fn mask_steps_are_rejected() {
    let recipe = "filters:\n  - type: invert\n    mask: {path: m.png, crc32: 1}\n";
    match glitch(IMAGE, recipe, 1) {
        Err(GlitchError::InvalidRecipe(m)) => assert!(m.contains("mask steps are not supported")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn oversized_ihdr_is_too_large_before_allocating() {
    // 40000 x 40000 = 1.6e9 pixels, over the 64e6 default limit.
    let mut png = IMAGE.to_vec();
    png[16..20].copy_from_slice(&40_000u32.to_be_bytes());
    png[20..24].copy_from_slice(&40_000u32.to_be_bytes());
    let crc = crc32(&png[12..29]);
    png[29..33].copy_from_slice(&crc.to_be_bytes());
    assert_eq!(glitch(&png, RECIPE, 1), Err(GlitchError::TooLarge));
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[test]
fn list_filters_is_the_metadata_table() {
    assert_eq!(list_filters().len(), png_glitch_recipe::filters().len());
    assert!(list_filters().iter().any(|f| f.name == "pixel-sort"));
}

/// Deterministic xorshift, so a failure reproduces.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn arbitrary_bytes_and_recipes_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let small = &IMAGE[..IMAGE.len().min(400)];
    for i in 0..300 {
        let mut image = if i % 3 == 0 {
            IMAGE.to_vec()
        } else {
            small.to_vec()
        };
        for _ in 0..(rng.next() % 6) {
            let n = image.len();
            image[(rng.next() as usize) % n] = rng.next() as u8;
        }
        if i % 7 == 0 {
            image = (0..(rng.next() % 64)).map(|_| rng.next() as u8).collect();
        }
        let mut recipe = RECIPE.as_bytes().to_vec();
        for _ in 0..(rng.next() % 4) {
            let n = recipe.len();
            recipe[(rng.next() as usize) % n] = rng.next() as u8;
        }
        let recipe = String::from_utf8_lossy(&recipe).into_owned();
        let _ = glitch(&image, &recipe, rng.next());
        let _ = validate_recipe(&recipe);
    }
}
