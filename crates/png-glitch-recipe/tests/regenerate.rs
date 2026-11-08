//! `set-filter-type` (tag only) and `regenerate` (new generation) steps (#139).

use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
use png_glitch_recipe::{
    FilterConfig, ImageHeader, LiveRecipe, PngGlitch, PreProcess, Recipe, RecipeError,
    RegenerateStrategy, SeedMode,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

fn run(config: FilterConfig) -> Vec<u8> {
    let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
    Recipe::new().then(config).run(&mut png, Some(7)).unwrap();
    png.to_bytes().unwrap()
}

#[test]
fn regenerate_fixed_matches_the_deprecated_steps() {
    let cases = [
        (FilterConfig::RemoveFilter, PreProcess::RemoveFilter),
        (FilterConfig::SubFilter, PreProcess::SubFilter),
        (FilterConfig::UpFilter, PreProcess::UpFilter),
        (FilterConfig::AverageFilter, PreProcess::AverageFilter),
        (FilterConfig::PaethFilter, PreProcess::PaethFilter),
    ];
    for (old, filter) in cases {
        let new = FilterConfig::Regenerate {
            strategy: RegenerateStrategy::Fixed(filter),
        };
        assert_eq!(run(old.clone()), run(new), "{old:?}");
    }
}

#[test]
fn set_filter_type_only_overwrites_the_tag() {
    let bytes = run(FilterConfig::SetFilterType {
        filter: PreProcess::UpFilter,
    });
    let mut expected = PngGlitch::from_bytes(FIXTURE).unwrap();
    expected.set_filter_type(FilterType::Up);
    assert_eq!(bytes, expected.to_bytes().unwrap());
    // ... which differs from recomputing the values.
    assert_ne!(
        bytes,
        run(FilterConfig::Regenerate {
            strategy: RegenerateStrategy::Fixed(PreProcess::UpFilter),
        })
    );
}

#[test]
fn regenerate_strategies_match_png_glitch() {
    let strategies = [
        (RegenerateStrategy::Adaptive, FilterStrategy::Adaptive),
        (
            RegenerateStrategy::Pattern(vec![PreProcess::SubFilter, PreProcess::PaethFilter]),
            FilterStrategy::Pattern(vec![FilterType::Sub, FilterType::Paeth]),
        ),
    ];
    for (strategy, expected) in strategies {
        let mut png = PngGlitch::from_bytes(FIXTURE).unwrap();
        png.regenerate(&expected).unwrap();
        assert_eq!(
            run(FilterConfig::Regenerate { strategy }),
            png.to_bytes().unwrap()
        );
    }
}

#[test]
fn empty_pattern_is_an_error_when_the_recipe_is_resolved_or_parsed() {
    let config = FilterConfig::Regenerate {
        strategy: RegenerateStrategy::Pattern(vec![]),
    };
    let png = PngGlitch::from_bytes(FIXTURE).unwrap();
    let err = Recipe::new()
        .then(config)
        .resolve(&ImageHeader::of(&png), Some(0))
        .unwrap_err();
    assert!(matches!(err, RecipeError::InvalidSpec(_)), "{err}");

    let yaml = "type: regenerate\nstrategy: { pattern: [] }\n";
    assert!(serde_saphyr::from_str::<FilterConfig>(yaml).is_err());
}

#[test]
fn yaml_forms() {
    let parse = |yaml: &str| serde_saphyr::from_str::<FilterConfig>(yaml).unwrap();
    assert_eq!(
        parse("type: set-filter-type\nfilter: paeth\n"),
        FilterConfig::SetFilterType {
            filter: PreProcess::PaethFilter
        }
    );
    assert_eq!(
        parse("type: regenerate\nstrategy: adaptive\n"),
        FilterConfig::Regenerate {
            strategy: RegenerateStrategy::Adaptive
        }
    );
    assert_eq!(
        parse("type: regenerate\nstrategy: { fixed: paeth }\n"),
        FilterConfig::Regenerate {
            strategy: RegenerateStrategy::Fixed(PreProcess::PaethFilter)
        }
    );
    assert_eq!(
        parse("type: regenerate\nstrategy: { pattern: [sub, up, paeth] }\n"),
        FilterConfig::Regenerate {
            strategy: RegenerateStrategy::Pattern(vec![
                PreProcess::SubFilter,
                PreProcess::UpFilter,
                PreProcess::PaethFilter
            ])
        }
    );
    // Old types keep loading.
    assert_eq!(parse("type: sub-filter\n"), FilterConfig::SubFilter);
}

#[test]
fn regenerate_runs_as_a_generation_boundary_in_a_live_loop() {
    let (w, h) = (16u32, 12u32);
    let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i * 7) as u8).collect();
    let live_out = |config: FilterConfig| {
        let mut live = LivePipeline::new(
            PixelSource::rgba8(w, h, &pixels),
            Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
        )
        .unwrap();
        let header = ImageHeader::of(live.model());
        let mut runner = LiveRecipe::new(
            Recipe::new().then(config),
            header,
            Some(1),
            SeedMode::Static,
        )
        .unwrap();
        let mut out = vec![0u8; pixels.len()];
        live.encode(&pixels).unwrap();
        runner.run(&mut live).unwrap();
        live.decode_into(&mut out).unwrap();
        out
    };
    let old = live_out(FilterConfig::SubFilter);
    let new = live_out(FilterConfig::Regenerate {
        strategy: RegenerateStrategy::Fixed(PreProcess::SubFilter),
    });
    assert_eq!(old, new);
    // The tag-only step runs as a scan-line effect.
    live_out(FilterConfig::SetFilterType {
        filter: PreProcess::UpFilter,
    });
}
