//! `PngGlitch::regenerate` (#162): `D` then `E` in place.

mod common;

use common::{build_png, row_bytes};
use png_glitch::{
    ColorType, Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource, PngError,
    PngGlitch, Renderer,
};
use proptest::collection::vec;
use proptest::prelude::*;

/// Every valid (colour type code, bit depth).
const FORMATS: &[(u8, u8)] = &[
    (0, 1),
    (0, 2),
    (0, 4),
    (0, 8),
    (0, 16),
    (2, 8),
    (2, 16),
    (3, 1),
    (3, 2),
    (3, 4),
    (3, 8),
    (4, 8),
    (4, 16),
    (6, 8),
    (6, 16),
];

const TYPES: [FilterType; 5] = [
    FilterType::None,
    FilterType::Sub,
    FilterType::Up,
    FilterType::Average,
    FilterType::Paeth,
];

fn strategies() -> Vec<FilterStrategy> {
    let mut all: Vec<_> = TYPES.iter().map(|&t| FilterStrategy::Fixed(t)).collect();
    all.push(FilterStrategy::Pattern(vec![
        FilterType::Sub,
        FilterType::Paeth,
        FilterType::None,
    ]));
    all.push(FilterStrategy::Adaptive);
    all
}

/// A model of any format whose scan lines are arbitrary (filter types 0..=4, random bytes).
fn model() -> impl Strategy<Value = Vec<u8>> {
    (0..FORMATS.len(), 1u32..=19, 1u32..=6).prop_flat_map(|(format, width, height)| {
        let (color_type, bit_depth) = FORMATS[format];
        let len = row_bytes(width, color_type, bit_depth);
        vec((0u8..=4, vec(any::<u8>(), len)), height as usize).prop_map(move |rows| {
            let rows: Vec<Vec<u8>> = rows
                .into_iter()
                .map(|(ft, bytes)| std::iter::once(ft).chain(bytes).collect())
                .collect();
            let plte = (color_type == 3).then(|| vec![0x40; 3 << bit_depth.min(8)]);
            build_png(
                width,
                height,
                color_type,
                bit_depth,
                &rows,
                plte.as_deref(),
                None,
            )
        })
    })
}

fn render(png: &PngGlitch) -> Vec<u8> {
    let mut out = vec![0; png.width() as usize * png.height() as usize * 4];
    Renderer::new(png)
        .render_into(png, &mut out)
        .expect("render");
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn fixed_equals_reencode_with(bytes in model(), t in 0usize..5) {
        let mut a = PngGlitch::from_bytes(&bytes).unwrap();
        let mut b = PngGlitch::from_bytes(&bytes).unwrap();
        a.regenerate(&FilterStrategy::Fixed(TYPES[t])).unwrap();
        b.reencode_with(TYPES[t]);
        prop_assert_eq!(a.data(), b.data());
    }

    #[test]
    fn regenerate_keeps_the_reconstructed_pixels(bytes in model()) {
        let mut png = PngGlitch::from_bytes(&bytes).unwrap();
        let before = render(&png);
        for strategy in strategies() {
            png.regenerate(&strategy).unwrap();
            prop_assert_eq!(&render(&png), &before, "{:?}", strategy);
        }
    }
}

#[test]
fn matches_from_pixels_with_the_same_strategy() {
    let (w, h) = (13u32, 9u32);
    let pixels: Vec<u8> = (0..w * h * 4)
        .map(|i| (i as u8).wrapping_mul(37).wrapping_add((i >> 4) as u8))
        .collect();
    for color_type in [
        ColorType::TrueColorAlpha,
        ColorType::TrueColor,
        ColorType::GrayScaleAlpha,
        ColorType::GrayScale,
    ] {
        for filter in strategies() {
            let enc = |filter| Encoding {
                color_type,
                bit_depth: 8,
                filter,
            };
            let src = PixelSource::rgba8(w, h, &pixels);
            let expected = PngGlitch::from_pixels(src, enc(filter.clone())).unwrap();
            let mut png =
                PngGlitch::from_pixels(src, enc(FilterStrategy::Fixed(FilterType::Sub))).unwrap();
            png.regenerate(&filter).unwrap();
            assert_eq!(png.data(), expected.data(), "{color_type:?} {filter:?}");
        }
    }
}

#[test]
fn an_empty_pattern_is_rejected_before_anything_changes() {
    let pixels = vec![9u8; 4 * 4 * 4];
    let mut png = PngGlitch::from_pixels(
        PixelSource::rgba8(4, 4, &pixels),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Up)),
    )
    .unwrap();
    let before = png.data().to_vec();
    let empty = FilterStrategy::Pattern(vec![]);
    assert!(matches!(
        png.regenerate(&empty),
        Err(PngError::EmptyFilterPattern)
    ));
    assert_eq!(png.data(), before);

    let mut live = LivePipeline::new(
        PixelSource::rgba8(4, 4, &pixels),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Up)),
    )
    .unwrap();
    assert!(matches!(
        live.regenerate(&empty),
        Err(PngError::EmptyFilterPattern)
    ));
}

#[test]
fn live_pipeline_regenerate_keeps_its_encoding_and_pixels() {
    let pixels: Vec<u8> = (0..8 * 8 * 4).map(|i| (i * 11) as u8).collect();
    let mut live = LivePipeline::new(
        PixelSource::rgba8(8, 8, &pixels),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub)),
    )
    .unwrap();
    live.scan_lines().set_filter_type(2, FilterType::Paeth); // a glitch
    let mut before = vec![0; pixels.len()];
    live.decode_into(&mut before).unwrap();
    live.regenerate(&FilterStrategy::Fixed(FilterType::Up))
        .unwrap();
    let mut after = vec![0; pixels.len()];
    live.decode_into(&mut after).unwrap();
    assert_eq!(before, after);
    assert_eq!(
        live.encoding().expect("made from pixels").filter,
        FilterStrategy::Fixed(FilterType::Sub)
    );
}
