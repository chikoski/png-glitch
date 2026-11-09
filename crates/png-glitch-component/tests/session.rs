//! `Session`: decoder equivalence over encodings, strategies, recipes and frames; reset;
//! error cases.

use png_glitch_component::{
    ColorType, Encoding, FilterStrategy, FilterType, GlitchError, SeedMode, Session,
};

const W: u32 = 40;
const H: u32 = 24;

fn frame(k: u32) -> Vec<u8> {
    (0..W * H * 4)
        .map(|i| (i.wrapping_mul(7 + k)).wrapping_add(k * 53) as u8)
        .collect()
}

fn decode(png: &[u8]) -> Vec<u8> {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8()
        .into_raw()
}

const RECIPES: &[&str] = &[
    "filters:\n  - type: Replace\n    magnitude: 0.2\n  - type: transpose\n    magnitude: 0.2\n",
    "filters:\n  - type: pixel-sort\n    magnitude: 0.5\n  - type: invert\n    rows: 3..10\n",
    "filters:\n  - type: sub-filter\n  - type: set-zero\n    magnitude: 0.1\n",
    "filters:\n  - type: regenerate\n    strategy: adaptive\n  - type: bitwise\n    magnitude: 0.2\n",
    "filters:\n  - type: change-filter-type\n    magnitude: 0.4\n  - type: brighten\n    strength: 30\n    rect: 4,4,20,10\n",
];

fn strategies() -> Vec<FilterStrategy> {
    vec![
        FilterStrategy::Fixed(FilterType::Paeth),
        FilterStrategy::Pattern(vec![FilterType::Sub, FilterType::Up, FilterType::None]),
        FilterStrategy::Adaptive,
    ]
}

fn encodings() -> Vec<Encoding> {
    let mut v = vec![];
    for color_type in [
        ColorType::GrayScale,
        ColorType::GrayScaleAlpha,
        ColorType::TrueColor,
        ColorType::TrueColorAlpha,
    ] {
        for filter in strategies() {
            v.push(Encoding {
                color_type,
                bit_depth: 8,
                filter,
            });
        }
    }
    v
}

#[test]
fn render_equals_decoding_the_snapshot_for_every_frame() {
    for enc in encodings() {
        for (r, recipe) in RECIPES.iter().enumerate() {
            for mode in [SeedMode::Static, SeedMode::PerFrame] {
                let mut s = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
                s.set_recipe(recipe, 5, mode).unwrap();
                for k in 0..10 {
                    s.reload_pixels(&frame(k)).unwrap();
                    s.apply().unwrap();
                    let rendered = s.render_rgba().unwrap();
                    assert_eq!(
                        rendered,
                        decode(&s.snapshot().unwrap()),
                        "{enc:?}, recipe {r}, {mode:?}, frame {k}"
                    );
                }
            }
        }
    }
}

#[test]
fn from_png_sessions_work_on_a_real_file() {
    let png = include_bytes!("../../png-glitch/etc/sample00.png");
    let mut s = Session::from_png(png).unwrap();
    assert_eq!(s.render_rgba().unwrap(), decode(png));
    s.set_recipe(RECIPES[0], 1, SeedMode::Static).unwrap();
    s.apply().unwrap();
    let glitched = s.render_rgba().unwrap();
    assert_ne!(glitched, decode(png));
    assert_eq!(glitched, decode(&s.snapshot().unwrap()));
    assert!(matches!(
        s.reload_pixels(&[0; 4]),
        Err(GlitchError::InvalidImage(_))
    ));
    s.reset().unwrap();
    assert_eq!(s.render_rgba().unwrap(), decode(png));
}

#[test]
fn same_seed_same_frame_sequence() {
    let run = || {
        let mut s = Session::from_pixels(W, H, &frame(0), encodings().remove(11)).unwrap();
        s.set_recipe(RECIPES[0], 9, SeedMode::PerFrame).unwrap();
        (0..4)
            .map(|k| {
                s.reload_pixels(&frame(k)).unwrap();
                s.apply().unwrap();
                s.render_rgba().unwrap()
            })
            .collect::<Vec<_>>()
    };
    let a = run();
    assert_eq!(a, run());
    // Per-frame seeds: the same input frame glitches differently on the next frame.
    let mut s = Session::from_pixels(W, H, &frame(0), encodings().remove(11)).unwrap();
    s.set_recipe(RECIPES[0], 9, SeedMode::PerFrame).unwrap();
    let mut outs = vec![];
    for _ in 0..2 {
        s.reload_pixels(&frame(0)).unwrap();
        s.apply().unwrap();
        outs.push(s.render_rgba().unwrap());
    }
    assert_ne!(outs[0], outs[1]);
}

#[test]
fn reset_restores_the_state_after_the_last_load() {
    let mut s = Session::from_pixels(W, H, &frame(0), encodings().remove(11)).unwrap();
    s.reload_pixels(&frame(3)).unwrap();
    let before = s.render_rgba().unwrap();
    let snapshot = s.snapshot().unwrap();
    s.set_recipe(RECIPES[0], 2, SeedMode::Static).unwrap();
    s.apply().unwrap();
    assert_ne!(s.render_rgba().unwrap(), before);
    s.reset().unwrap();
    assert_eq!(s.render_rgba().unwrap(), before);
    assert_eq!(s.snapshot().unwrap(), snapshot);
}

#[test]
fn errors_are_values_not_traps() {
    let enc = encodings().remove(11);
    let mut s = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
    assert_eq!(s.apply(), Err(GlitchError::NoRecipe));
    assert!(matches!(
        s.reload_pixels(&[0; 12]),
        Err(GlitchError::InvalidImage(_))
    ));
    assert!(matches!(
        s.set_recipe("nope", 1, SeedMode::Static),
        Err(GlitchError::InvalidRecipe(_))
    ));
    assert!(matches!(
        s.set_recipe(
            "filters: [{type: invert, mask: {path: m.png, crc32: 1}}]",
            1,
            SeedMode::Static
        ),
        Err(GlitchError::InvalidRecipe(_))
    ));
    let mut out = vec![0u8; 3];
    assert!(s.render_into(&mut out).is_err());

    let deep = Encoding {
        bit_depth: 16,
        ..enc.clone()
    };
    assert!(matches!(
        Session::from_pixels(W, H, &frame(0), deep),
        Err(GlitchError::UnsupportedEncoding)
    ));
    let empty = Encoding {
        filter: FilterStrategy::Pattern(vec![]),
        ..enc.clone()
    };
    assert!(matches!(
        Session::from_pixels(W, H, &frame(0), empty),
        Err(GlitchError::UnsupportedEncoding)
    ));
    assert!(Session::from_pixels(W, H, &[0; 5], enc.clone()).is_err());
    assert!(Session::from_pixels(0, 0, &[], enc).is_err());
    assert_eq!(
        Session::from_png(b"not a png").err(),
        Some(GlitchError::UnsupportedFormat)
    );
}
