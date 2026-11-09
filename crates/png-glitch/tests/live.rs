//! `LivePipeline` (#160): the laws of ADR 0005 as tests.
//!
//! 1. `D(E(x)) == x` after the documented colour conversion.
//! 2. `decode_into` equals `Renderer::render_into` on a glitched model.
//! 3. `snapshot()` is a valid PNG and equals `to_bytes()` of the same model.

use png_glitch::{
    ColorType, Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource, PngGlitch, Renderer,
};
use proptest::prelude::*;

const COLOR_TYPES: [ColorType; 4] = [
    ColorType::TrueColorAlpha,
    ColorType::TrueColor,
    ColorType::GrayScaleAlpha,
    ColorType::GrayScale,
];

const ALL_FILTERS: [FilterType; 5] = [
    FilterType::None,
    FilterType::Sub,
    FilterType::Up,
    FilterType::Average,
    FilterType::Paeth,
];

fn strategies() -> Vec<FilterStrategy> {
    let mut all: Vec<_> = ALL_FILTERS
        .iter()
        .map(|&t| FilterStrategy::Fixed(t))
        .collect();
    all.push(FilterStrategy::Pattern(vec![
        FilterType::Sub,
        FilterType::Up,
        FilterType::Paeth,
    ]));
    all.push(FilterStrategy::Adaptive);
    all
}

fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// The documented conversion followed by widening back to RGBA8, written independently of the
/// library.
fn expected_rgba(color_type: ColorType, rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, a]| {
            let y = ((77 * u32::from(r) + 150 * u32::from(g) + 29 * u32::from(b) + 128) >> 8) as u8;
            match color_type {
                ColorType::TrueColorAlpha => [r, g, b, a],
                ColorType::TrueColor => [r, g, b, 255],
                ColorType::GrayScaleAlpha => [y, y, y, a],
                _ => [y, y, y, 255],
            }
        })
        .collect()
}

fn reference_decode(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("valid PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("decodes");
    buf.truncate(info.buffer_size());
    match reader.output_color_type().0 {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => unreachable!(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Law 1, plus: a second frame overwrites the first.
    #[test]
    fn decode_of_encode_is_the_input(
        width in 1u32..=33,
        height in 1u32..=6,
        seed in any::<u64>(),
    ) {
        let len = (width * height * 4) as usize;
        let first = noise(seed, len);
        let second = noise(seed.wrapping_add(1), len);
        for color_type in COLOR_TYPES {
            for filter in strategies() {
                let enc = Encoding { color_type, bit_depth: 8, filter };
                let mut live =
                    LivePipeline::new(PixelSource::rgba8(width, height, &first), enc.clone())
                        .unwrap();
                let mut out = vec![0u8; len];
                for frame in [&first, &second, &first] {
                    live.encode(frame).unwrap();
                    live.decode_into(&mut out).unwrap();
                    prop_assert_eq!(&out, &expected_rgba(color_type, frame), "{:?}", enc);
                }
            }
        }
    }

    /// Law 3.
    #[test]
    fn snapshot_is_a_valid_png_equal_to_to_bytes(
        width in 1u32..=33,
        height in 1u32..=6,
        seed in any::<u64>(),
        glitch in any::<bool>(),
    ) {
        let len = (width * height * 4) as usize;
        let frame = noise(seed, len);
        for color_type in COLOR_TYPES {
            for filter in strategies() {
                let enc = Encoding { color_type, bit_depth: 8, filter };
                let mut live =
                    LivePipeline::new(PixelSource::rgba8(width, height, &frame), enc).unwrap();
                {
                    let mut lines = live.encode(&frame).unwrap();
                    if glitch {
                        lines.swap(0, (height - 1) as usize);
                        lines.set_filter_type(0, FilterType::Average);
                    }
                }
                let snapshot = live.snapshot().unwrap();
                prop_assert_eq!(&snapshot, &live.model().to_bytes().unwrap());
                // The independent decoder reads it, and sees what `decode_into` shows.
                let mut out = vec![0u8; len];
                live.decode_into(&mut out).unwrap();
                prop_assert_eq!(reference_decode(&snapshot), out);
            }
        }
    }
}

/// Law 2, on a glitched model: tags overwritten, rows swapped and bytes changed.
#[test]
fn decode_into_equals_render_into_on_a_glitched_model() {
    let (w, h) = (23u32, 9u32);
    let frame = noise(7, (w * h * 4) as usize);
    for color_type in COLOR_TYPES {
        let enc = Encoding {
            color_type,
            bit_depth: 8,
            filter: FilterStrategy::Fixed(FilterType::Paeth),
        };
        let mut live = LivePipeline::new(PixelSource::rgba8(w, h, &frame), enc.clone()).unwrap();
        {
            let mut lines = live.encode(&frame).unwrap();
            lines.set_filter_type(1, FilterType::Sub);
            lines.set_filter_type(4, FilterType::Average);
            lines.swap(2, 6);
            lines.copy(0, 8);
            lines.for_each_mut(|row| {
                row.update(3, 0xAA);
            });
        }
        let mut via_live = vec![0u8; frame.len()];
        live.decode_into(&mut via_live).unwrap();

        // The same glitch on a plain model, rendered by the Renderer directly.
        let mut model = PngGlitch::from_pixels(PixelSource::rgba8(w, h, &frame), enc).unwrap();
        {
            let mut lines = model.scan_lines_mut();
            lines.set_filter_type(1, FilterType::Sub);
            lines.set_filter_type(4, FilterType::Average);
            lines.swap(2, 6);
            lines.copy(0, 8);
            lines.for_each_mut(|row| {
                row.update(3, 0xAA);
            });
        }
        let mut renderer = Renderer::new(&model);
        let mut direct = vec![0u8; frame.len()];
        renderer.render_into(&model, &mut direct).unwrap();
        assert_eq!(via_live, direct, "{color_type:?}");
        assert_ne!(
            via_live,
            expected_rgba(color_type, &frame),
            "glitch had no effect"
        );
    }
}

#[test]
fn encode_discards_the_previous_glitch() {
    let (w, h) = (8u32, 8u32);
    let frame = noise(3, (w * h * 4) as usize);
    let mut live = LivePipeline::new(
        PixelSource::rgba8(w, h, &frame),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Up)),
    )
    .unwrap();
    live.encode(&frame).unwrap().swap(0, 7);
    let mut out = vec![0u8; frame.len()];
    live.encode(&frame).unwrap();
    live.decode_into(&mut out).unwrap();
    assert_eq!(out, frame);
}

#[test]
fn scan_lines_does_not_reload() {
    let (w, h) = (8u32, 4u32);
    let frame = noise(5, (w * h * 4) as usize);
    let mut live = LivePipeline::new(
        PixelSource::rgba8(w, h, &frame),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::None)),
    )
    .unwrap();
    live.encode(&frame).unwrap();
    live.scan_lines().swap(0, 1);
    let mut out = vec![0u8; frame.len()];
    live.decode_into(&mut out).unwrap();
    let row = (w * 4) as usize;
    assert_eq!(&out[..row], &frame[row..2 * row]);
}

#[test]
fn wrong_frame_length_is_an_error() {
    let frame = vec![0u8; 4 * 4 * 4];
    let mut live = LivePipeline::new(
        PixelSource::rgba8(4, 4, &frame),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub)),
    )
    .unwrap();
    assert!(live.encode(&frame[..10]).is_err());
    assert!(live.decode_into(&mut [0u8; 3]).is_err());
}

#[test]
fn view_operations_are_total() {
    let frame = noise(9, 5 * 5 * 4);
    let mut model = PngGlitch::from_pixels(
        PixelSource::rgba8(5, 5, &frame),
        Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub)),
    )
    .unwrap();
    let before = model.data().to_vec();
    {
        let mut lines = model.scan_lines_mut();
        assert_eq!(lines.len(), 5);
        assert!(lines.scratch().len() >= lines.stride());
        assert!(lines.get_mut(5).is_none());
        assert!(lines.pair_mut(1, 1).is_none());
        assert!(lines.pair_mut(1, 9).is_none());
        assert!(!lines.swap(0, 5));
        assert!(!lines.swap_range(0, 2, 3)); // overlapping
        assert!(!lines.swap_range(3, 0, 3)); // past the end
        assert!(!lines.copy(5, 0));
        assert!(!lines.copy_range(0, 3, 3));
        assert_eq!(lines.filter_type(5), None);
        lines.set_filter_type(5, FilterType::Up);
        assert!(lines.swap(2, 2));
    }
    assert_eq!(model.data(), before.as_slice());

    // pair_mut keeps the argument order.
    let mut lines = model.scan_lines_mut();
    let (mut a, mut b) = lines.pair_mut(3, 1).unwrap();
    a.set_filter_type(FilterType::Paeth);
    b.set_filter_type(FilterType::Up);
    assert_eq!(lines.filter_type(3), Some(FilterType::Paeth));
    assert_eq!(lines.filter_type(1), Some(FilterType::Up));
    // Range swap and copy move whole rows, tags included.
    let stride = 5 * 4 + 1;
    let row = |model: &PngGlitch, i: usize| model.data()[i * stride..(i + 1) * stride].to_vec();
    let rows: Vec<Vec<u8>> = (0..5).map(|i| row(&model, i)).collect();
    assert!(model.scan_lines_mut().swap_range(0, 3, 2));
    assert_eq!(
        (0..5).map(|i| row(&model, i)).collect::<Vec<_>>(),
        [&rows[3], &rows[4], &rows[2], &rows[0], &rows[1]]
            .map(|r| r.clone())
            .to_vec()
    );
    let rows: Vec<Vec<u8>> = (0..5).map(|i| row(&model, i)).collect();
    assert!(model.scan_lines_mut().copy_range(0, 1, 3));
    assert_eq!(
        (0..5).map(|i| row(&model, i)).collect::<Vec<_>>(),
        [&rows[0], &rows[0], &rows[1], &rows[2], &rows[4]]
            .map(|r| r.clone())
            .to_vec()
    );
}

#[test]
fn par_for_each_matches_for_each_mut() {
    let frame = noise(11, 17 * 13 * 4);
    let make = || {
        PngGlitch::from_pixels(
            PixelSource::rgba8(17, 13, &frame),
            Encoding::rgba8(FilterStrategy::Fixed(FilterType::Up)),
        )
        .unwrap()
    };
    let (mut a, mut b) = (make(), make());
    a.scan_lines_mut().for_each_mut(|row| row.update(5, 0x42));
    b.scan_lines_mut().par_for_each(|row| row.update(5, 0x42));
    assert_eq!(a.data(), b.data());
}

#[test]
fn copy_data_from_restores_a_glitched_model_and_checks_the_header() {
    let frame = noise(3, 8 * 6 * 4);
    let enc = Encoding::rgba8(FilterStrategy::Fixed(FilterType::Sub));
    let clean = PngGlitch::from_pixels(PixelSource::rgba8(8, 6, &frame), enc.clone()).unwrap();
    let mut model = PngGlitch::from_pixels(PixelSource::rgba8(8, 6, &frame), enc.clone()).unwrap();
    model.scan_lines_mut().swap(0, 5);
    assert_ne!(model.data(), clean.data());
    model.copy_data_from(&clean).unwrap();
    assert_eq!(model.data(), clean.data());

    let other =
        PngGlitch::from_pixels(PixelSource::rgba8(6, 8, &noise(3, 6 * 8 * 4)), enc).unwrap();
    assert!(matches!(
        model.copy_data_from(&other),
        Err(png_glitch::PngError::HeaderMismatch)
    ));
}

#[test]
fn a_pipeline_from_a_png_decodes_regenerates_and_snapshots() {
    let png = include_bytes!("../etc/sample00.png");
    let mut live = LivePipeline::from_png(PngGlitch::from_bytes(png).unwrap());
    assert!(live.encoding().is_none());
    let (w, h) = {
        let m = live.model();
        (m.width(), m.height())
    };
    let mut out = vec![0u8; (w * h * 4) as usize];
    live.decode_into(&mut out).unwrap();
    live.regenerate(&FilterStrategy::Fixed(FilterType::Paeth))
        .unwrap();
    let mut again = vec![0u8; out.len()];
    live.decode_into(&mut again).unwrap();
    assert_eq!(out, again);
    assert!(live.snapshot().unwrap().starts_with(b"\x89PNG"));
    assert!(matches!(
        live.encode(&out),
        Err(png_glitch::PngError::NotCreatedFromPixels)
    ));
}
