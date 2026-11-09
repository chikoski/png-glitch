//! `rect`, `mask` and `invert-mask` on a step (ADR 0006): serialisation, errors, and the bytes.

use png_glitch::effects::Mask;
use png_glitch_recipe::{
    FilterConfig, FilterEntry, GlitchContext, ImageHeader, LiveRecipe, MaskRef, MaskSet, PngGlitch,
    Recipe, RecipeError, RectSpec, RowRange, SeedMode,
};
use serde::{Deserialize, Serialize};

const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct File {
    filters: Vec<FilterEntry>,
}

fn image() -> PngGlitch {
    PngGlitch::from_bytes(FIXTURE).unwrap()
}

fn dims() -> (u32, u32) {
    let p = image();
    (p.width(), p.height())
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> RectSpec {
    RectSpec { x, y, w, h }
}

fn run(recipe: &Recipe, seed: u64) -> Vec<u8> {
    let mut png = image();
    recipe.run(&mut png, Some(seed)).unwrap();
    png.data().to_vec()
}

fn entry(config: FilterConfig) -> FilterEntry {
    config.into()
}

fn with_rect(config: FilterConfig, r: RectSpec) -> FilterEntry {
    FilterEntry {
        rect: Some(r),
        ..entry(config)
    }
}

fn with_mask(config: FilterConfig, path: &str, crc32: Option<u32>) -> FilterEntry {
    FilterEntry {
        mask: Some(MaskRef {
            path: path.into(),
            crc32,
        }),
        ..entry(config)
    }
}

fn masks(path: &str, mask: Mask, crc: Option<u32>) -> MaskSet {
    let mut set = MaskSet::new();
    set.insert(path, mask, crc);
    set
}

fn uniform(value: u8) -> Mask {
    let (w, h) = dims();
    Mask::from_values(w, h, vec![value; (w * h) as usize]).unwrap()
}

#[test]
fn yaml_keys_round_trip() {
    let yaml = "filters:\n  - type: replace\n    magnitude: 0.1\n    rect: 1,2,30,40\n  - type: invert\n    mask:\n      path: m.png\n      crc32: 305419896\n    invert-mask: true\n  - type: invert\n    rows: 2..5\n    rect: 0,0,4,4\n";
    let file: File = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(file.filters[0].rect, Some(rect(1, 2, 30, 40)));
    let m = file.filters[1].mask.as_ref().unwrap();
    assert_eq!((m.path.as_str(), m.crc32), ("m.png", Some(0x1234_5678)));
    assert!(file.filters[1].invert_mask);
    assert_eq!(file.filters[2].rows, Some(RowRange::new(2, 5)));
    let text = serde_saphyr::to_string(&file).unwrap();
    assert!(text.contains("invert-mask"), "{text}");
    assert_eq!(serde_saphyr::from_str::<File>(&text).unwrap(), file);
    // The snake_case spelling is accepted too.
    let alias: File = serde_saphyr::from_str(
        "filters:\n  - type: invert\n    rect: 0,0,1,1\n    invert_mask: true\n",
    )
    .unwrap();
    assert!(alias.filters[0].invert_mask);
    // Plain entries are written exactly as before.
    let plain = serde_saphyr::to_string(&File {
        filters: vec![entry(FilterConfig::Invert)],
    })
    .unwrap();
    assert!(
        !plain.contains("rect") && !plain.contains("mask"),
        "{plain}"
    );
}

#[test]
fn bad_values_are_rejected() {
    for bad in [
        "rect: 1,2,3",
        "rect: a,b,c,d",
        "rect: -1,0,1,1",
        "mask: m.png",
        "mask: { path: m.png, extra: 1 }",
    ] {
        let yaml = format!("filters:\n  - type: invert\n    {bad}\n");
        assert!(serde_saphyr::from_str::<File>(&yaml).is_err(), "{bad}");
    }
}

#[test]
fn recipe_serde_keeps_rows_and_regions() {
    let recipe = Recipe::from_entries(vec![
        entry(FilterConfig::Invert),
        FilterEntry::with_rows(
            FilterConfig::Replace { magnitude: 0.2 },
            RowRange::new(1, 4),
        ),
        with_rect(FilterConfig::Invert, rect(0, 0, 4, 4)),
    ]);
    let yaml = serde_saphyr::to_string(&recipe).unwrap();
    let back: Recipe = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back, recipe, "{yaml}");
}

#[test]
fn a_full_mask_equals_the_unmasked_run_and_a_zero_mask_changes_nothing() {
    let (w, h) = dims();
    let before = image().data().to_vec();
    for config in [
        FilterConfig::Replace { magnitude: 0.5 },
        FilterConfig::Transpose { magnitude: 0.5 },
        FilterConfig::PixelSort {
            magnitude: 0.5,
            criterion: None,
        },
        FilterConfig::RandomCopy { times: 5 },
        FilterConfig::Invert,
    ] {
        for seed in 0..3 {
            let plain = run(&Recipe::new().then(config.clone()), seed);
            let full = run(
                &Recipe::from_entries(vec![with_mask(config.clone(), "m.png", None)])
                    .with_masks(masks("m.png", uniform(255), None)),
                seed,
            );
            assert_eq!(full, plain, "{config:?} seed {seed}: full mask");
            let none = run(
                &Recipe::from_entries(vec![with_mask(config.clone(), "m.png", None)])
                    .with_masks(masks("m.png", uniform(0), None)),
                seed,
            );
            assert_eq!(none, before, "{config:?} seed {seed}: zero mask");
            // `invert-mask` flips them.
            let mut inverted = with_mask(config.clone(), "m.png", None);
            inverted.invert_mask = true;
            let flipped = run(
                &Recipe::from_entries(vec![inverted]).with_masks(masks("m.png", uniform(0), None)),
                seed,
            );
            assert_eq!(flipped, plain, "{config:?} seed {seed}: inverted zero mask");
        }
    }
    let _ = (w, h);
}

#[test]
fn rect_equals_the_same_rectangle_as_a_mask_image() {
    let (w, h) = dims();
    let r = rect(2, 3, w / 2, h / 2);
    let as_mask = Mask::rect(w, h, r.x, r.y, r.w, r.h);
    for config in [
        FilterConfig::Replace { magnitude: 0.6 },
        FilterConfig::HorizontalShift { magnitude: 0.6 },
    ] {
        let a = run(&Recipe::from_entries(vec![with_rect(config.clone(), r)]), 9);
        let b = run(
            &Recipe::from_entries(vec![with_mask(config.clone(), "m.png", None)])
                .with_masks(masks("m.png", as_mask.clone(), None)),
            9,
        );
        assert_eq!(a, b, "{config:?}");
    }
}

#[test]
fn rows_and_a_rect_together_leave_everything_else_alone() {
    let (w, h) = dims();
    assert!(h >= 12);
    let before = image().data().to_vec();
    let stride = before.len() / h as usize;
    let entry = FilterEntry {
        rows: Some(RowRange::new(3, 9)),
        rect: Some(rect(0, 0, w, 6)), // rows 0..6 of the image; only rows 3..6 are in both
        ..FilterEntry::from(FilterConfig::Replace { magnitude: 1.0 })
    };
    let got = run(&Recipe::from_entries(vec![entry]), 4);
    assert_eq!(got[..3 * stride], before[..3 * stride]);
    assert_eq!(got[6 * stride..], before[6 * stride..]);
    assert_ne!(got[3 * stride..6 * stride], before[3 * stride..6 * stride]);
}

#[test]
fn the_context_path_equals_the_recipe_path() {
    let (w, h) = dims();
    let r = rect(1, 1, w / 2, h - 2);
    let entries = [
        with_rect(FilterConfig::Replace { magnitude: 0.7 }, r),
        FilterEntry {
            invert_mask: true,
            ..with_rect(FilterConfig::Transpose { magnitude: 0.5 }, r)
        },
        FilterEntry {
            rows: Some(RowRange::new(2, 8)),
            ..with_rect(FilterConfig::Invert, r)
        },
    ];
    let mut ctx = GlitchContext::new(FIXTURE, Some(3)).unwrap();
    for e in &entries {
        ctx.add_from_entry(e.clone(), &MaskSet::new()).unwrap();
    }
    ctx.execute();
    let via_recipe = {
        let mut png = image();
        Recipe::from_entries(entries.to_vec())
            .run(&mut png, Some(3))
            .unwrap();
        png.to_bytes().unwrap()
    };
    assert_eq!(ctx.buffer().unwrap(), via_recipe);
}

#[test]
fn errors() {
    let (w, h) = dims();
    let header = ImageHeader::of(&image());
    let resolve = |e: FilterEntry, set: MaskSet| {
        Recipe::from_entries(vec![e])
            .with_masks(set)
            .resolve(&header, Some(1))
    };

    // Both rect and mask.
    let mut both = with_rect(FilterConfig::Invert, rect(0, 0, 1, 1));
    both.mask = Some(MaskRef {
        path: "m.png".into(),
        crc32: None,
    });
    assert!(matches!(
        resolve(both, MaskSet::new()),
        Err(RecipeError::InvalidSpec(_))
    ));

    // invert-mask alone.
    let alone = FilterEntry {
        invert_mask: true,
        ..entry(FilterConfig::Invert)
    };
    assert!(matches!(
        resolve(alone, MaskSet::new()),
        Err(RecipeError::InvalidSpec(_))
    ));

    // A mask that was not supplied.
    let missing = with_mask(FilterConfig::Invert, "m.png", None);
    assert!(matches!(
        resolve(missing, MaskSet::new()),
        Err(RecipeError::MissingMask { .. })
    ));

    // A checksum that does not match, and one that does.
    let mask = Mask::rect(w, h, 0, 0, 2, 2);
    let wrong = with_mask(FilterConfig::Invert, "m.png", Some(1));
    let err = resolve(wrong, masks("m.png", mask.clone(), Some(2))).unwrap_err();
    assert!(
        matches!(
            err,
            RecipeError::MaskChecksum {
                expected: 1,
                actual: 2,
                ..
            }
        ),
        "{err}"
    );
    let right = with_mask(FilterConfig::Invert, "m.png", Some(7));
    assert!(resolve(right, masks("m.png", mask, Some(7))).is_ok());

    // Steps that recompute filtered values cannot be masked.
    for config in [
        FilterConfig::SubFilter,
        FilterConfig::Regenerate {
            strategy: png_glitch_recipe::RegenerateStrategy::Fixed(
                png_glitch_recipe::PreProcess::SubFilter,
            ),
        },
    ] {
        let e = with_rect(config.clone(), rect(0, 0, 2, 2));
        assert!(
            matches!(resolve(e, MaskSet::new()), Err(RecipeError::InvalidSpec(_))),
            "{config:?}"
        );
        let mut ctx = GlitchContext::new(FIXTURE, Some(1)).unwrap();
        assert!(
            ctx.add_from_entry(with_rect(config, rect(0, 0, 2, 2)), &MaskSet::new())
                .is_err()
        );
    }
    let mut ctx = GlitchContext::new(FIXTURE, Some(1)).unwrap();
    assert!(matches!(
        ctx.add_from_entry(
            with_mask(FilterConfig::Invert, "m.png", None),
            &MaskSet::new()
        ),
        Err(RecipeError::MissingMask { .. })
    ));
}

#[test]
fn live_recipes_run_masked_steps_like_the_file_workflow() {
    use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};

    let (w, h) = (24u32, 20u32);
    let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(7)).collect();
    let encoding = Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth));
    let mut live = LivePipeline::new(PixelSource::rgba8(w, h, &pixels), encoding.clone()).unwrap();
    let header = ImageHeader::of(live.model());
    let recipe = Recipe::from_entries(vec![
        with_rect(FilterConfig::Replace { magnitude: 0.5 }, rect(4, 2, 10, 12)),
        with_mask(FilterConfig::Invert, "m.png", None),
    ])
    .with_masks(masks("m.png", Mask::rect(w, h, 0, 0, 12, 20), None));
    let mut runner = LiveRecipe::new(recipe.clone(), header, Some(5), SeedMode::Static).unwrap();
    live.encode(&pixels).unwrap();
    runner.run(&mut live).unwrap();
    let mut file = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &pixels), encoding).unwrap();
    recipe.run(&mut file, Some(5)).unwrap();
    assert_eq!(live.model().data(), file.data());
}
