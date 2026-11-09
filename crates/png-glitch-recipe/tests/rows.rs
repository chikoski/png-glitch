//! `rows: a..b` targeting: parsing, round trips, and the stored bytes outside the range.

use png_glitch_recipe::{
    FilterConfig, FilterEntry, GlitchContext, ImageHeader, LiveRecipe, PngGlitch, PreProcess,
    Recipe, RecipeError, RegenerateStrategy, RowRange,
};
use serde::{Deserialize, Serialize};

const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct File {
    filters: Vec<FilterEntry>,
}

fn rows(a: u32, b: u32) -> RowRange {
    RowRange::new(a, b)
}

#[test]
fn row_range_text_forms() {
    for (text, want) in [
        ("3..9", RowRange::new(3, 9)),
        (
            "3..",
            RowRange {
                start: Some(3),
                end: None,
            },
        ),
        (
            "..9",
            RowRange {
                start: None,
                end: Some(9),
            },
        ),
        ("..", RowRange::default()),
    ] {
        let r: RowRange = text.parse().unwrap();
        assert_eq!(r, want);
        assert_eq!(r.to_string(), text);
    }
    for bad in ["", "5", "a..b", "1...2", "-1..3", "1..2..3"] {
        assert!(bad.parse::<RowRange>().is_err(), "{bad}");
    }
    assert_eq!(rows(9, 3).resolve(100), 9..9);
    assert_eq!(rows(200, 300).resolve(100), 100..100);
    assert_eq!(rows(5, 300).resolve(100), 5..100);
}

#[test]
fn yaml_rows_key_round_trips_and_is_optional() {
    let yaml = "filters:\n  - type: replace\n    magnitude: 0.1\n    rows: 3..9\n  - type: invert\n  - type: random-copy\n    times: 4\n    rows: 5..\n";
    let file: File = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(file.filters[0].rows, Some(rows(3, 9)));
    assert_eq!(file.filters[1].rows, None);
    assert_eq!(
        file.filters[2].rows,
        Some(RowRange {
            start: Some(5),
            end: None
        })
    );
    let text = serde_saphyr::to_string(&file).unwrap();
    assert!(text.contains("3..9"), "{text}");
    assert!(!text.contains("rows: null"), "{text}");
    assert_eq!(serde_saphyr::from_str::<File>(&text).unwrap(), file);
}

#[test]
fn yaml_still_rejects_unknown_fields_and_bad_ranges() {
    let unknown = "filters:\n  - type: replace\n    magnitude: 0.1\n    nope: 1\n";
    assert!(serde_saphyr::from_str::<File>(unknown).is_err());
    let bad = "filters:\n  - type: replace\n    magnitude: 0.1\n    rows: soon\n";
    assert!(serde_saphyr::from_str::<File>(bad).is_err());
    let out_of_range = "filters:\n  - type: replace\n    magnitude: 7\n    rows: 1..2\n";
    assert!(serde_saphyr::from_str::<File>(out_of_range).is_err());
}

fn stride_and_height(png: &PngGlitch) -> (usize, usize) {
    let h = png.height() as usize;
    (png.data().len() / h, h)
}

fn targeted_steps() -> Vec<FilterConfig> {
    vec![
        FilterConfig::Replace { magnitude: 0.5 },
        FilterConfig::Transpose { magnitude: 0.5 },
        FilterConfig::SetZero { magnitude: 0.5 },
        FilterConfig::ChangeFilterType { magnitude: 0.5 },
        FilterConfig::RandomCopy { times: 6 },
        FilterConfig::HorizontalShift { magnitude: 0.5 },
        FilterConfig::BlockScramble {
            magnitude: 0.5,
            block_size: None,
        },
        FilterConfig::PixelSort {
            magnitude: 0.5,
            criterion: None,
        },
        FilterConfig::ColorDistortion {
            magnitude: 0.5,
            strength: None,
        },
        FilterConfig::Invert,
        FilterConfig::SubFilter, // a Refilter
        FilterConfig::SetFilterType {
            filter: PreProcess::PaethFilter,
        },
    ]
}

#[test]
fn rows_outside_the_range_keep_their_stored_bytes() {
    let height = PngGlitch::from_bytes(FIXTURE).unwrap().height();
    assert!(height >= 8, "fixture too small");
    let range = rows(2, height - 2);
    for config in targeted_steps() {
        for seed in 0..4 {
            let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
            let before = png.data().to_vec();
            let (stride, h) = stride_and_height(&png);
            Recipe::new()
                .then_rows(config.clone(), range)
                .run(&mut png, Some(seed))
                .unwrap();
            let (a, b) = (2 * stride, (h - 2) * stride);
            assert_eq!(
                png.data()[..a],
                before[..a],
                "{config:?} seed {seed}: above"
            );
            assert_eq!(
                png.data()[b..],
                before[b..],
                "{config:?} seed {seed}: below"
            );
        }
    }
}

#[test]
fn a_recipe_without_rows_is_unchanged_and_the_entry_api_agrees() {
    let plain = Recipe::new()
        .then(FilterConfig::Replace { magnitude: 0.3 })
        .then(FilterConfig::Invert);
    let entries = Recipe::from_entries(vec![
        FilterConfig::Replace { magnitude: 0.3 }.into(),
        FilterConfig::Invert.into(),
    ]);
    assert_eq!(plain, entries);
    assert_eq!(plain.entries().unwrap().len(), 2);
    assert!(plain.configs().is_some());
    let targeted = Recipe::new().then_rows(FilterConfig::Invert, rows(1, 3));
    assert!(targeted.configs().is_none());
    assert_eq!(
        targeted.entries().unwrap(),
        vec![FilterEntry::with_rows(FilterConfig::Invert, rows(1, 3))]
    );
}

#[test]
fn context_and_recipe_paths_give_the_same_bytes() {
    for config in targeted_steps() {
        let range = rows(3, 12);
        let mut ctx = GlitchContext::new(FIXTURE, Some(5)).unwrap();
        ctx.add_from_config_in_rows(config.clone(), range).unwrap();
        ctx.execute();

        let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
        Recipe::new()
            .then_rows(config.clone(), range)
            .run(&mut png, Some(5))
            .unwrap();
        assert_eq!(ctx.buffer().unwrap(), png.to_bytes().unwrap(), "{config:?}");
    }
}

#[test]
fn a_targeted_step_draws_relative_to_the_range() {
    // Row numbers are relative: swapping within 4..8 touches only rows 4..8.
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let before = png.data().to_vec();
    let (stride, _) = stride_and_height(&png);
    Recipe::new()
        .then_rows(FilterConfig::Transpose { magnitude: 1.0 }, rows(4, 8))
        .run(&mut png, Some(1))
        .unwrap();
    assert_eq!(png.data()[..4 * stride], before[..4 * stride]);
    assert_ne!(
        png.data()[4 * stride..8 * stride],
        before[4 * stride..8 * stride]
    );
    assert_eq!(png.data()[8 * stride..], before[8 * stride..]);
}

#[test]
fn empty_and_out_of_range_rows_do_nothing() {
    for range in [
        rows(5, 5),
        rows(9, 3),
        rows(10_000, 20_000),
        RowRange {
            start: Some(10_000),
            end: None,
        },
    ] {
        let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
        let before = png.data().to_vec();
        Recipe::new()
            .then_rows(FilterConfig::Replace { magnitude: 1.0 }, range)
            .run(&mut png, Some(1))
            .unwrap();
        assert_eq!(png.data(), &before[..], "{range}");
    }
}

#[test]
fn regenerate_cannot_be_targeted() {
    let step = FilterConfig::Regenerate {
        strategy: RegenerateStrategy::Fixed(PreProcess::SubFilter),
    };
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let err = Recipe::new()
        .then_rows(step.clone(), rows(1, 4))
        .run(&mut png, Some(1))
        .unwrap_err();
    assert!(matches!(err, RecipeError::InvalidSpec(_)), "{err}");
    let mut ctx = GlitchContext::new(FIXTURE, Some(1)).unwrap();
    assert!(ctx.add_from_config_in_rows(step, rows(1, 4)).is_err());
}

#[test]
fn live_recipes_run_targeted_scan_line_steps_and_reject_refilter() {
    use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
    use png_glitch_recipe::SeedMode;

    let (w, h) = (24u32, 20u32);
    let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i as u8).wrapping_mul(7)).collect();
    let encoding = Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth));
    let mut live = LivePipeline::new(PixelSource::rgba8(w, h, &pixels), encoding.clone()).unwrap();
    let header = ImageHeader::of(live.model());

    let ok = Recipe::new().then_rows(FilterConfig::Replace { magnitude: 0.5 }, rows(3, 11));
    let mut runner = LiveRecipe::new(ok.clone(), header, Some(9), SeedMode::Static).unwrap();
    live.encode(&pixels).unwrap();
    runner.run(&mut live).unwrap();
    let mut file = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &pixels), encoding).unwrap();
    ok.run(&mut file, Some(9)).unwrap();
    assert_eq!(live.model().data(), file.data());

    let refilter = Recipe::new().then_rows(FilterConfig::SubFilter, rows(3, 11));
    let err = LiveRecipe::new(refilter, header, Some(9), SeedMode::Static).unwrap_err();
    assert!(matches!(err, RecipeError::NotLive { .. }), "{err}");
}
