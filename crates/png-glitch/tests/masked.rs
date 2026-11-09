//! `Mask` and `Masked<E>` (ADR 0006): 0 keeps the original stored bytes, 255 keeps the result,
//! in between blends. Checked over the colour type × bit depth matrix.

mod common;

use common::{build_png, row_bytes};
use png_glitch::effects::*;
use png_glitch::{
    ColorType, Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource, PngGlitch,
};

const W: u32 = 11;
const H: u32 = 7;

/// (PNG colour type code, bit depth) for every valid combination.
const MATRIX: &[(u8, u8)] = &[
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

fn png_for(color_type: u8, bit_depth: u8) -> PngGlitch {
    let n = row_bytes(W, color_type, bit_depth);
    let rows: Vec<Vec<u8>> = (0..H as usize)
        .map(|y| {
            let mut row = vec![(y % 5) as u8]; // varied filter-type tags
            row.extend((0..n).map(|i| ((y * 37 + i * 19 + 5) % 251) as u8));
            row
        })
        .collect();
    let plte: Vec<u8> = (0..256 * 3).map(|i| i as u8).collect();
    let plte = (color_type == 3).then(|| &plte[..3 * (1usize << bit_depth)]);
    let bytes = build_png(W, H, color_type, bit_depth, &rows, plte, None);
    PngGlitch::from_bytes(&bytes).expect("fixture parses")
}

struct Dyn<'a>(&'a dyn ScanLinesEffect);
impl ScanLinesEffect for Dyn<'_> {
    fn apply_to_scan_lines(&self, lines: &mut png_glitch::ScanLines<'_>) {
        self.0.apply_to_scan_lines(lines);
    }
}

fn effects() -> Vec<(&'static str, Box<dyn ScanLinesEffect>)> {
    vec![
        ("Invert", Box::new(Invert::new())),
        ("SwapLines", Box::new(SwapLines::new(vec![(0, 5), (2, 3)]))),
        (
            "ShiftLines",
            Box::new(ShiftLines::new(vec![LineShift { line: 1, shift: 3 }])),
        ),
        (
            "ChannelShift",
            Box::new(ChannelShift::new(3, 0, -3, (0..H).collect())),
        ),
    ]
}

fn bare(png: &PngGlitch, e: &dyn ScanLinesEffect) -> Vec<u8> {
    let mut p = PngGlitch::from_bytes(png.to_bytes().unwrap()).unwrap();
    p.apply(Dyn(e));
    p.data().to_vec()
}

fn masked(png: &PngGlitch, e: &dyn ScanLinesEffect, mask: Mask) -> Vec<u8> {
    let mut p = PngGlitch::from_bytes(png.to_bytes().unwrap()).unwrap();
    let wrapper = Masked::for_image(Dyn(e), mask, &p);
    p.apply(wrapper);
    p.data().to_vec()
}

#[test]
fn an_all_255_mask_equals_the_bare_effect_and_an_all_0_mask_changes_nothing() {
    for &(ct, bd) in MATRIX {
        let png = png_for(ct, bd);
        let orig = png.data().to_vec();
        for (name, e) in effects() {
            let full = Mask::from_values(W, H, vec![255; (W * H) as usize]).unwrap();
            assert_eq!(
                masked(&png, &*e, full),
                bare(&png, &*e),
                "{name} ct{ct} bd{bd}"
            );
            let none = Mask::from_values(W, H, vec![0; (W * H) as usize]).unwrap();
            assert_eq!(masked(&png, &*e, none), orig, "{name} ct{ct} bd{bd}: all-0");
        }
    }
}

/// The bits of pixel `x` in a row's data (without the tag), as a comparable value.
fn pixel_bits(data: &[u8], x: usize, bits: usize) -> Vec<bool> {
    (0..bits)
        .map(|k| {
            let bit = x * bits + k;
            data[bit / 8] >> (7 - bit % 8) & 1 == 1
        })
        .collect()
}

#[test]
fn a_rect_mask_selects_result_inside_and_original_outside() {
    let (rx, ry, rw, rh) = (3u32, 2u32, 4u32, 3u32);
    for &(ct, bd) in MATRIX {
        let png = png_for(ct, bd);
        let orig = png.data().to_vec();
        let stride = orig.len() / H as usize;
        let bits = ColorTypeCode(ct).bits(bd);
        for (name, e) in effects() {
            let result = bare(&png, &*e);
            let got = masked(&png, &*e, Mask::rect(W, H, rx, ry, rw, rh));
            for y in 0..H as usize {
                let row = |d: &[u8]| d[y * stride..(y + 1) * stride].to_vec();
                let (g, o, r) = (row(&got), row(&orig), row(&result));
                let intersects = y as u32 >= ry && (y as u32) < ry + rh;
                assert_eq!(
                    g[0],
                    if intersects { r[0] } else { o[0] },
                    "{name} ct{ct} bd{bd} tag row {y}"
                );
                for x in 0..W as usize {
                    let inside = intersects && x as u32 >= rx && (x as u32) < rx + rw;
                    let want = if inside { &r } else { &o };
                    assert_eq!(
                        pixel_bits(&g[1..], x, bits),
                        pixel_bits(&want[1..], x, bits),
                        "{name} ct{ct} bd{bd} pixel ({x},{y})"
                    );
                }
            }
        }
    }
}

struct ColorTypeCode(u8);
impl ColorTypeCode {
    fn bits(&self, bd: u8) -> usize {
        let ct = match self.0 {
            0 => ColorType::GrayScale,
            2 => ColorType::TrueColor,
            3 => ColorType::IndexColor,
            4 => ColorType::GrayScaleAlpha,
            _ => ColorType::TrueColorAlpha,
        };
        ct.bit_per_pixel(bd)
    }
}

#[test]
fn blending_matches_an_independent_computation() {
    let values = [0u8, 1, 128, 254, 255];
    for (ct, bd) in [(6u8, 8u8), (6, 16), (2, 8)] {
        let png = png_for(ct, bd);
        let orig = png.data().to_vec();
        let stride = orig.len() / H as usize;
        let result = bare(&png, &Invert::new());
        // One mask value per column, same on every row.
        let mut vals = vec![0u8; (W * H) as usize];
        for y in 0..H as usize {
            for (x, &m) in values.iter().enumerate() {
                vals[y * W as usize + x] = m;
            }
        }
        let got = masked(&png, &Invert::new(), Mask::from_values(W, H, vals).unwrap());
        let bpp = ColorTypeCode(ct).bits(bd) / 8;
        for y in 0..H as usize {
            for (x, &m) in values.iter().enumerate() {
                for s in 0..bpp / (bd as usize / 8) {
                    let at = y * stride + 1 + x * bpp + s * (bd as usize / 8);
                    let (o, r, g): (f64, f64, f64) = if bd == 16 {
                        let v = |d: &[u8]| f64::from(u16::from_be_bytes([d[at], d[at + 1]]));
                        (v(&orig), v(&result), v(&got))
                    } else {
                        (
                            f64::from(orig[at]),
                            f64::from(result[at]),
                            f64::from(got[at]),
                        )
                    };
                    let strength = if bd == 16 {
                        f64::from(m) * 257.0 / 65535.0
                    } else {
                        f64::from(m) / 255.0
                    };
                    let want = (o * (1.0 - strength) + r * strength).round();
                    assert_eq!(g, want, "ct{ct} bd{bd} m={m} sample {s} of ({x},{y})");
                }
            }
        }
    }
}

#[test]
fn indexed_and_packed_samples_are_thresholded() {
    for (ct, bd) in [(3u8, 8u8), (3, 4), (0, 2), (0, 1)] {
        let png = png_for(ct, bd);
        let result = bare(&png, &Invert::new());
        let orig = png.data().to_vec();
        let stride = orig.len() / H as usize;
        let bits = ColorTypeCode(ct).bits(bd);
        let vals: Vec<u8> = (0..W * H).map(|i| [127u8, 128][(i % 2) as usize]).collect();
        let got = masked(&png, &Invert::new(), Mask::from_values(W, H, vals).unwrap());
        for y in 0..H as usize {
            for x in 0..W as usize {
                let on = (y * W as usize + x) % 2 == 1;
                let src = if on { &result } else { &orig };
                assert_eq!(
                    pixel_bits(&got[y * stride + 1..(y + 1) * stride], x, bits),
                    pixel_bits(&src[y * stride + 1..(y + 1) * stride], x, bits),
                    "ct{ct} bd{bd} ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn masks_are_clipped_and_wrong_sizes_never_panic() {
    assert!(Mask::from_values(3, 3, vec![0; 8]).is_none());
    assert!(Mask::from_values(3, 3, vec![0; 10]).is_none());
    let clipped = Mask::rect(4, 4, 2, 2, 100, 100);
    assert_eq!(
        (
            clipped.value(3, 3),
            clipped.value(1, 1),
            clipped.value(4, 4)
        ),
        (255, 0, 0)
    );
    assert_eq!(Mask::rect(4, 4, 90, 90, 5, 5).value(0, 0), 0);
    assert_eq!(Mask::rect(4, 4, 0, 0, 4, 4).inverted().value(1, 1), 0);
    assert_eq!(Mask::rect(4, 4, 0, 0, 4, 4).inverted().value(9, 9), 0); // outside stays 0

    let png = png_for(6, 8);
    let orig = png.data().to_vec();
    // Smaller than the image: nothing happens outside it. Larger: cropped.
    let small = masked(&png, &Invert::new(), Mask::rect(2, 2, 0, 0, 2, 2));
    let stride = orig.len() / H as usize;
    assert_eq!(small[2 * stride..], orig[2 * stride..]);
    let large = masked(&png, &Invert::new(), Mask::rect(500, 500, 0, 0, 500, 500));
    assert_eq!(large, bare(&png, &Invert::new()));
    let _ = masked(
        &png,
        &Invert::new(),
        Mask::from_values(0, 0, vec![]).unwrap(),
    );
}

#[test]
fn a_masked_effect_runs_in_the_live_pipeline() {
    let (w, h) = (16u32, 8u32);
    let frame = |k: u8| -> Vec<u8> {
        (0..w * h * 4)
            .map(|i| (i as u8).wrapping_mul(7).wrapping_add(k))
            .collect()
    };
    let enc = Encoding::rgba8(FilterStrategy::Fixed(FilterType::None));
    let mut live = LivePipeline::new(PixelSource::rgba8(w, h, &frame(0)), enc.clone()).unwrap();
    let stride = live.model().data().len() / h as usize;
    let masked = Masked::new(Invert::new(), Mask::rect(w, h, 0, 0, 8, h), stride, h);
    for k in 0..4u8 {
        let pixels = frame(k);
        let mut lines = live.encode(&pixels).unwrap();
        masked.apply_to_scan_lines(&mut lines);
        let mut file =
            PngGlitch::from_pixels(PixelSource::rgba8(w, h, &pixels), enc.clone()).unwrap();
        file.apply(Masked::for_image(
            Invert::new(),
            Mask::rect(w, h, 0, 0, 8, h),
            &file,
        ));
        assert_eq!(live.model().data(), file.data(), "frame {k}");
    }
}
