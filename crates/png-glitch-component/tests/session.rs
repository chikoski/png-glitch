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
    let initial_snapshot = s.snapshot().unwrap();
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
    assert_eq!(s.snapshot().unwrap(), initial_snapshot);
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
    assert!(Session::from_pixels(0, 0, &[], enc.clone()).is_err());
    assert_eq!(
        Session::from_png(b"not a png").err(),
        Some(GlitchError::UnsupportedFormat)
    );

    // process_frame* error cases: NoRecipe, wrong buffer lengths, and from_png sessions.
    let mut no_recipe = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
    let mut buf = frame(0);
    assert_eq!(
        no_recipe.process_frame(&frame(0)),
        Err(GlitchError::NoRecipe)
    );
    assert_eq!(
        no_recipe.process_frame_into(&frame(0), &mut out),
        Err(GlitchError::NoRecipe)
    );
    assert_eq!(
        no_recipe.process_frame_in_place(&mut buf),
        Err(GlitchError::NoRecipe)
    );

    no_recipe
        .set_recipe(RECIPES[0], 1, SeedMode::Static)
        .unwrap();
    assert!(matches!(
        no_recipe.process_frame(&[0; 12]),
        Err(GlitchError::InvalidImage(_))
    ));
    let mut valid_out = vec![0u8; (W * H * 4) as usize];
    assert!(matches!(
        no_recipe.process_frame_into(&[0; 12], &mut valid_out),
        Err(GlitchError::InvalidImage(_))
    ));
    assert!(matches!(
        no_recipe.process_frame_into(&frame(0), &mut out),
        Err(GlitchError::InvalidImage(_))
    ));
    let mut bad_buf = vec![0u8; 12];
    assert!(matches!(
        no_recipe.process_frame_in_place(&mut bad_buf),
        Err(GlitchError::InvalidImage(_))
    ));

    // A wrong-length `out` in `process_frame_into` must leave the live frame, snapshot, and
    // `SeedMode::PerFrame` counter untouched.
    let mut guarded = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
    let mut clean_ref = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
    guarded
        .set_recipe(RECIPES[0], 11, SeedMode::PerFrame)
        .unwrap();
    clean_ref
        .set_recipe(RECIPES[0], 11, SeedMode::PerFrame)
        .unwrap();
    let initial_render = guarded.render_rgba().unwrap();
    let initial_snapshot = guarded.snapshot().unwrap();
    assert!(matches!(
        guarded.process_frame_into(&frame(1), &mut out),
        Err(GlitchError::InvalidImage(_))
    ));
    assert_eq!(guarded.render_rgba().unwrap(), initial_render);
    assert_eq!(guarded.snapshot().unwrap(), initial_snapshot);
    assert!(matches!(
        guarded.process_frame_into(&[0; 12], &mut valid_out),
        Err(GlitchError::InvalidImage(_))
    ));
    assert_eq!(guarded.render_rgba().unwrap(), initial_render);
    assert_eq!(guarded.snapshot().unwrap(), initial_snapshot);
    assert_eq!(
        guarded.process_frame(&frame(2)).unwrap(),
        clean_ref.process_frame(&frame(2)).unwrap()
    );

    let png = include_bytes!("../../png-glitch/etc/sample00.png");
    let mut from_png = Session::from_png(png).unwrap();
    from_png
        .set_recipe(RECIPES[0], 1, SeedMode::Static)
        .unwrap();
    assert!(matches!(
        from_png.process_frame(&frame(0)),
        Err(GlitchError::InvalidImage(_))
    ));
    let mut png_out = vec![0u8; (from_png.width() * from_png.height() * 4) as usize];
    assert!(matches!(
        from_png.process_frame_into(&frame(0), &mut png_out),
        Err(GlitchError::InvalidImage(_))
    ));
    assert!(matches!(
        from_png.process_frame_in_place(&mut png_out),
        Err(GlitchError::InvalidImage(_))
    ));
}

#[test]
fn process_frame_matches_reload_apply_render_and_preserves_reset_baseline() {
    for enc in encodings() {
        for mode in [SeedMode::Static, SeedMode::PerFrame] {
            let mut ref_s = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
            let mut s_alloc = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
            let mut s_into = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
            let mut s_in_place = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();

            let baseline = ref_s.render_rgba().unwrap();
            let baseline_snapshot = ref_s.snapshot().unwrap();

            for s in [&mut ref_s, &mut s_alloc, &mut s_into, &mut s_in_place] {
                s.set_recipe(RECIPES[0], 7, mode).unwrap();
            }

            // Direct check of `from_pixels` clean baseline clone via `apply` + `reset`.
            let mut baseline_only = Session::from_pixels(W, H, &frame(0), enc.clone()).unwrap();
            baseline_only.set_recipe(RECIPES[0], 7, mode).unwrap();
            baseline_only.apply().unwrap();
            baseline_only.reset().unwrap();
            assert_eq!(baseline_only.render_rgba().unwrap(), baseline);
            assert_eq!(baseline_only.snapshot().unwrap(), baseline_snapshot);

            let mut out = vec![0u8; (W * H * 4) as usize];
            for k in 0..4 {
                let input = frame(k);
                ref_s.reload_pixels(&input).unwrap();
                ref_s.apply().unwrap();
                let expected = ref_s.render_rgba().unwrap();

                let got_alloc = s_alloc.process_frame(&input).unwrap();
                s_into.process_frame_into(&input, &mut out).unwrap();
                let mut in_place_buf = input;
                s_in_place
                    .process_frame_in_place(&mut in_place_buf)
                    .unwrap();

                assert_eq!(got_alloc, expected, "process_frame {enc:?} {mode:?} {k}");
                assert_eq!(out, expected, "process_frame_into {enc:?} {mode:?} {k}");
                assert_eq!(
                    in_place_buf, expected,
                    "process_frame_in_place {enc:?} {mode:?} {k}"
                );
            }

            // process_frame* must not update `clean`, so `reset()` still restores the initial frame.
            s_alloc.reset().unwrap();
            assert_eq!(s_alloc.render_rgba().unwrap(), baseline);

            // Updating the clean baseline via `reload_pixels` is preserved across `process_frame`.
            s_into.reload_pixels(&frame(2)).unwrap();
            let reloaded_baseline = s_into.render_rgba().unwrap();
            s_into.process_frame(&frame(3)).unwrap();
            s_into.reset().unwrap();
            assert_eq!(s_into.render_rgba().unwrap(), reloaded_baseline);
        }
    }
}
