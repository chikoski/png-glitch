//! `ScanLines::rows`, `WithinRows` and `RefilterRows`: the stored bytes of rows outside the
//! range never change, and inside the range the result equals the effect on a cropped image.

use png_glitch::effects::*;
use png_glitch::{Encoding, FilterStrategy, FilterType, PixelSource, PngGlitch};

const W: u32 = 16;
const H: u32 = 24;

fn pixels(rows: std::ops::Range<u32>) -> Vec<u8> {
    rows.flat_map(|y| (0..W * 4).map(move |i| (y * 31 + i * 7) as u8))
        .collect()
}

fn image(strategy: FilterStrategy, rows: std::ops::Range<u32>) -> PngGlitch {
    let h = rows.end - rows.start;
    PngGlitch::from_pixels(
        PixelSource::rgba8(W, h, &pixels(rows)),
        Encoding::rgba8(strategy),
    )
    .expect("supported encoding")
}

/// Every built-in effect with parameters that land inside rows `0..6` (the cropped height).
fn effects() -> Vec<(&'static str, Box<dyn ScanLinesEffect>)> {
    let rows: Vec<u32> = (0..6).collect();
    vec![
        (
            "SetFilterTypes",
            Box::new(SetFilterTypes::new(vec![
                Some(FilterType::Paeth),
                None,
                Some(FilterType::Sub),
            ])),
        ),
        (
            "SetBytes",
            Box::new(SetBytes::new(vec![ByteEdit {
                line: 3,
                index: 5,
                value: 9,
            }])),
        ),
        ("Substitute", Box::new(Substitute::new(7, 200))),
        ("SwapLines", Box::new(SwapLines::new(vec![(0, 5), (2, 99)]))),
        ("CopyLines", Box::new(CopyLines::new(vec![(1, 4), (7, 0)]))),
        (
            "ShiftLines",
            Box::new(ShiftLines::new(vec![LineShift { line: 2, shift: 11 }])),
        ),
        (
            "PixelSort",
            Box::new(PixelSort::new(SortCriterion::Brightness, rows.clone())),
        ),
        (
            "Bitwise",
            Box::new(Bitwise::new(BitOp::Xor, 0x5A, rows.clone())),
        ),
        (
            "SwapChannels",
            Box::new(SwapChannels::new(SwapTarget::Gb, rows.clone())),
        ),
        (
            "HslShift",
            Box::new(HslShift::new(40.0, 1.2, 0.9, rows.clone())),
        ),
        (
            "ChannelShift",
            Box::new(ChannelShift::new(3, 0, -3, rows.clone())),
        ),
        (
            "SwapBlocks",
            Box::new(SwapBlocks::new(
                8,
                vec![BlockSwap {
                    src: (0, 0),
                    dst: (1, 1),
                }],
            )),
        ),
        ("Invert", Box::new(Invert::new())),
        ("Brighten", Box::new(Brighten::new(10))),
        ("Darken", Box::new(Darken::new(10))),
        ("ShiftChannels", Box::new(ShiftChannels::new(1, -2, 3))),
    ]
}

struct Dyn<'a>(&'a dyn ScanLinesEffect);

impl ScanLinesEffect for Dyn<'_> {
    fn apply_to_scan_lines(&self, lines: &mut png_glitch::ScanLines<'_>) {
        self.0.apply_to_scan_lines(lines);
    }
}

#[test]
fn rows_outside_the_range_keep_their_stored_bytes() {
    let range = 9..15u32;
    for (name, effect) in effects() {
        for strategy in [
            FilterStrategy::Adaptive,
            FilterStrategy::Fixed(FilterType::Up),
        ] {
            let mut png = image(strategy, 0..H);
            let before = png.data().to_vec();
            let stride = before.len() / H as usize;
            png.apply(Dyn(&*effect).within_rows(range.clone()));
            let (a, b) = (range.start as usize * stride, range.end as usize * stride);
            assert_eq!(png.data()[..a], before[..a], "{name}: rows above changed");
            assert_eq!(png.data()[b..], before[b..], "{name}: rows below changed");
        }
    }
}

#[test]
fn rows_inside_the_range_match_the_effect_on_a_cropped_image() {
    // With filter type None the stored bytes are the pixels, so a crop is just the same rows.
    let range = 9..15u32;
    for (name, effect) in effects() {
        let mut full = image(FilterStrategy::Fixed(FilterType::None), 0..H);
        let mut cropped = image(FilterStrategy::Fixed(FilterType::None), range.clone());
        let stride = full.data().len() / H as usize;
        full.apply(Dyn(&*effect).within_rows(range.clone()));
        cropped.apply(Dyn(&*effect));
        let (a, b) = (range.start as usize * stride, range.end as usize * stride);
        assert_eq!(full.data()[a..b], *cropped.data(), "{name}");
    }
}

#[test]
fn ranges_are_clamped_and_never_panic() {
    for range in [
        0..0,
        5..5,
        9..3,
        H..H + 4,
        H + 10..H + 20,
        20..1000,
        0..u32::MAX,
    ] {
        let mut png = image(FilterStrategy::Adaptive, 0..H);
        let before = png.data().to_vec();
        png.apply(Invert::new().within_rows(range.clone()));
        let stride = before.len() / H as usize;
        let (a, b) = (
            (range.start as usize).min(H as usize) * stride,
            (range.end as usize)
                .min(H as usize)
                .max(range.start.min(H) as usize)
                * stride,
        );
        assert_eq!(png.data()[..a], before[..a]);
        assert_eq!(png.data()[b..], before[b..]);
        if a == b {
            assert_eq!(png.data(), &before[..], "empty range must be a no-op");
        }
        png.apply(Refilter::new(FilterType::Sub).within_rows(range));
    }
}

#[test]
fn refilter_within_rows_changes_only_the_range() {
    let range = 6..14u32;
    let mut png = image(FilterStrategy::Fixed(FilterType::Up), 0..H);
    // A glitch above the range that the re-encode must bake in without touching the rows.
    png.apply(SetBytes::new(vec![ByteEdit {
        line: 2,
        index: 3,
        value: 77,
    }]));
    let before = png.data().to_vec();
    let stride = before.len() / H as usize;
    png.apply(Refilter::new(FilterType::Paeth).within_rows(range.clone()));
    let (a, b) = (range.start as usize * stride, range.end as usize * stride);
    assert_eq!(png.data()[..a], before[..a]);
    assert_eq!(png.data()[b..], before[b..]);
    assert!(
        png.data()[a..b].chunks(stride).all(|r| r[0] == 4),
        "re-tagged Paeth"
    );

    // The decoded pixels of the range are the same as before the re-encode.
    let decoded = |mut p: PngGlitch| {
        p.remove_filter();
        p.data().to_vec()
    };
    let mut reference = image(FilterStrategy::Fixed(FilterType::Up), 0..H);
    reference.apply(SetBytes::new(vec![ByteEdit {
        line: 2,
        index: 3,
        value: 77,
    }]));
    assert_eq!(decoded(png)[a..b], decoded(reference)[a..b]);
}
