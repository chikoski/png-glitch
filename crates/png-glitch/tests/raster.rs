//! 2D pixel access (`pixel`, `set_pixel`, `column`, `block_mut`) and the `Pixel` conversions, over
//! the colour type × bit depth matrix and widths 1..=33.

mod common;

use common::{build_png, row_bytes};
use png_glitch::{ColorType, Pixel, PixelError, PngGlitch};

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

fn color_type(code: u8) -> ColorType {
    match code {
        0 => ColorType::GrayScale,
        2 => ColorType::TrueColor,
        3 => ColorType::IndexColor,
        4 => ColorType::GrayScaleAlpha,
        _ => ColorType::TrueColorAlpha,
    }
}

fn image(ct: u8, bd: u8, w: u32, h: u32) -> PngGlitch {
    let n = row_bytes(w, ct, bd);
    let rows: Vec<Vec<u8>> = (0..h as usize)
        .map(|y| {
            let mut row = vec![0u8];
            row.extend((0..n).map(|i| ((y * 37 + i * 19 + 5) % 251) as u8));
            row
        })
        .collect();
    let palette: Vec<u8> = (0..256 * 3).map(|i| i as u8).collect();
    let plte = (ct == 3).then(|| &palette[..3 * (1usize << bd)]);
    PngGlitch::from_bytes(build_png(w, h, ct, bd, &rows, plte, None)).expect("fixture parses")
}

/// A pixel whose samples fit `bd` bits, different for each position.
fn pixel_for(ct: u8, bd: u8, x: u32, y: u32) -> Pixel {
    let max = (1u32 << bd) - 1;
    let s = |k: u32| ((x * 7 + y * 3 + k * 5 + 1) % (max + 1)) as u16;
    match ct {
        0 => Pixel::Gray(s(0)),
        2 => Pixel::RGB(s(0), s(1), s(2)),
        3 => Pixel::Indexed(s(0) as u8),
        4 => Pixel::GrayAlpha(s(0), s(1)),
        _ => Pixel::RGBA(s(0), s(1), s(2), s(3)),
    }
}

#[test]
fn set_pixel_then_pixel_round_trips_and_column_matches() {
    for &(ct, bd) in MATRIX {
        for w in 1..=33u32 {
            let h = 4;
            let mut png = image(ct, bd, w, h);
            for y in 0..h {
                for x in 0..w {
                    png.set_pixel(x, y, pixel_for(ct, bd, x, y))
                        .unwrap_or_else(|e| panic!("ct{ct} bd{bd} w{w} ({x},{y}): {e}"));
                }
            }
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(
                        png.pixel(x, y),
                        Some(pixel_for(ct, bd, x, y)),
                        "ct{ct} bd{bd} w{w} ({x},{y})"
                    );
                }
            }
            let lines = png.scan_lines_mut();
            for x in 0..w {
                let column: Vec<Pixel> = lines.column(x).collect();
                let expected: Vec<Pixel> = (0..h).map(|y| pixel_for(ct, bd, x, y)).collect();
                assert_eq!(column, expected, "ct{ct} bd{bd} w{w} column {x}");
                for (y, p) in column.iter().enumerate() {
                    assert_eq!(lines.pixel(x, y as u32), Some(*p));
                }
            }
        }
    }
}

#[test]
fn writing_a_pixel_leaves_its_neighbours_alone() {
    for &(ct, bd) in MATRIX {
        let (w, h) = (13u32, 3u32);
        let mut png = image(ct, bd, w, h);
        let before: Vec<Option<Pixel>> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| png.pixel(x, y))
            .collect();
        png.set_pixel(6, 1, pixel_for(ct, bd, 0, 0)).unwrap();
        for y in 0..h {
            for x in 0..w {
                if (x, y) != (6, 1) {
                    assert_eq!(
                        png.pixel(x, y),
                        before[(y * w + x) as usize],
                        "ct{ct} bd{bd} ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn block_mut_clips_at_the_right_and_bottom_edges() {
    for &(ct, bd) in MATRIX {
        for w in [1u32, 2, 5, 17, 33] {
            let h = 6;
            let mut png = image(ct, bd, w, h);
            let mut lines = png.scan_lines_mut();
            // A block that sticks out past the right and bottom edges.
            let (bx, by) = (w.saturating_sub(2), h - 2);
            let mut block = lines.block_mut(bx, by, 5, 5).expect("corner is inside");
            let (bw, bh) = (block.width(), block.height());
            assert_eq!((bw, bh), (w - bx, 2), "ct{ct} bd{bd} w{w}");
            for y in 0..bh {
                for x in 0..bw {
                    block.set_pixel(x, y, pixel_for(ct, bd, x, y)).unwrap();
                    assert_eq!(block.pixel(x, y), Some(pixel_for(ct, bd, x, y)));
                }
            }
            // Outside the (clipped) block, in block coordinates: an error, never a write.
            assert_eq!(block.pixel(bw, 0), None);
            assert_eq!(block.pixel(0, bh), None);
            assert_eq!(
                block.set_pixel(bw, 0, pixel_for(ct, bd, 0, 0)),
                Err(PixelError::OutOfBounds { x: bw, y: 0 })
            );
            assert_eq!(
                block.set_pixel(0, bh, pixel_for(ct, bd, 0, 0)),
                Err(PixelError::OutOfBounds { x: 0, y: bh })
            );
            // The writes landed at the right place in the image.
            for y in 0..bh {
                for x in 0..bw {
                    assert_eq!(lines.pixel(bx + x, by + y), Some(pixel_for(ct, bd, x, y)));
                }
            }
        }
    }
}

#[test]
fn out_of_range_coordinates_never_panic() {
    for &(ct, bd) in MATRIX {
        let (w, h) = (7u32, 5u32);
        let mut png = image(ct, bd, w, h);
        let p = pixel_for(ct, bd, 0, 0);
        for (x, y) in [
            (w, 0),
            (0, h),
            (w, h),
            (u32::MAX, 0),
            (0, u32::MAX),
            (u32::MAX, u32::MAX),
        ] {
            assert_eq!(png.pixel(x, y), None, "ct{ct} bd{bd} ({x},{y})");
            assert_eq!(
                png.set_pixel(x, y, p),
                Err(PixelError::OutOfBounds { x, y })
            );
        }
        let mut lines = png.scan_lines_mut();
        assert_eq!(lines.column(w).count(), 0);
        assert_eq!(lines.column(u32::MAX).count(), 0);
        for (x, y, bw, bh) in [
            (w, 0, 1, 1),
            (0, h, 1, 1),
            (u32::MAX, u32::MAX, u32::MAX, u32::MAX),
            (0, 0, 0, 3),
            (0, 0, 3, 0),
        ] {
            assert!(
                lines.block_mut(x, y, bw, bh).is_none(),
                "({x},{y},{bw},{bh})"
            );
        }
        // A huge block from a valid corner clips to the image instead of overflowing.
        let block = lines
            .block_mut(1, 1, u32::MAX, u32::MAX)
            .expect("valid corner");
        assert_eq!((block.width(), block.height()), (w - 1, h - 1));
        let mut tail = lines.block_mut(w - 1, h - 1, u32::MAX, u32::MAX).unwrap();
        assert_eq!((tail.width(), tail.height()), (1, 1));
        assert_eq!(
            tail.set_pixel(u32::MAX, 0, p),
            Err(PixelError::OutOfBounds { x: u32::MAX, y: 0 })
        );
    }
}

#[test]
fn set_pixel_reports_why_it_failed() {
    let mut rgb = image(2, 8, 4, 2);
    assert_eq!(
        rgb.set_pixel(0, 0, Pixel::Gray(1)),
        Err(PixelError::ColorTypeMismatch)
    );
    assert_eq!(
        rgb.set_pixel(0, 0, Pixel::RGBA(1, 2, 3, 4)),
        Err(PixelError::ColorTypeMismatch)
    );
    assert_eq!(
        rgb.set_pixel(0, 0, Pixel::RGB(256, 0, 0)),
        Err(PixelError::SampleOutOfRange)
    );
    assert_eq!(rgb.set_pixel(0, 0, Pixel::RGB(255, 255, 255)), Ok(()));
    let mut two_bit = image(0, 2, 5, 1);
    assert_eq!(
        two_bit.set_pixel(0, 0, Pixel::Gray(5)),
        Err(PixelError::SampleOutOfRange)
    );
    assert_eq!(two_bit.set_pixel(0, 0, Pixel::Gray(3)), Ok(()));
    let mut gray8 = image(0, 8, 5, 1);
    assert_eq!(
        gray8.set_pixel(0, 0, Pixel::Gray(300)),
        Err(PixelError::SampleOutOfRange)
    );
    let mut gray16 = image(0, 16, 5, 1);
    assert_eq!(gray16.set_pixel(0, 0, Pixel::Gray(u16::MAX)), Ok(()));
    let mut indexed = image(3, 4, 5, 1);
    assert_eq!(
        indexed.set_pixel(0, 0, Pixel::Indexed(16)),
        Err(PixelError::SampleOutOfRange)
    );
    assert_eq!(indexed.set_pixel(0, 0, Pixel::Indexed(15)), Ok(()));
    // A failed write changes nothing.
    let before = rgb.pixel(1, 1);
    let _ = rgb.set_pixel(1, 1, Pixel::RGB(999, 0, 0));
    assert_eq!(rgb.pixel(1, 1), before);
}

#[test]
fn the_old_set_pixel_still_ignores_bad_input() {
    // `ScanLine::set_pixel` keeps its behaviour: it does nothing on a mismatch and masks or
    // narrows samples that are too large.
    let mut png = image(2, 8, 4, 1);
    let mut lines = png.scan_lines_mut();
    let mut row = lines.get_mut(0).unwrap();
    row.set_pixel(0, Pixel::RGB(1, 2, 3));
    row.set_pixel(0, Pixel::Gray(9)); // mismatch: ignored
    row.set_pixel(99, Pixel::RGB(9, 9, 9)); // out of range: ignored
    assert_eq!(row.get_pixel(0), Some(Pixel::RGB(1, 2, 3)));
    row.set_pixel(1, Pixel::RGB(256 + 7, 0, 0)); // narrowed with `as u8`
    assert_eq!(row.get_pixel(1), Some(Pixel::RGB(7, 0, 0)));
}

#[test]
fn rgba16_conversions_round_trip_for_every_depth() {
    for &(ct, bd) in MATRIX.iter().filter(|(ct, _)| *ct != 3) {
        let max = (1u32 << bd) - 1;
        let samples: Vec<u16> = (0..=max.min(300)).chain([max]).map(|v| v as u16).collect();
        for &v in &samples {
            let p = match ct {
                0 => Pixel::Gray(v),
                2 => Pixel::RGB(v, ((u32::from(v) + 1) % (max + 1)) as u16, v),
                4 => Pixel::GrayAlpha(v, v),
                _ => Pixel::RGBA(v, v, v, v),
            };
            let rgba = p.to_rgba16(bd).expect("not indexed");
            assert_eq!(
                Pixel::from_rgba16(rgba, color_type(ct), bd),
                Some(p),
                "ct{ct} bd{bd} v{v}"
            );
        }
    }
    // The ends map to 0 and 0xFFFF at every depth.
    for bd in [1u8, 2, 4, 8, 16] {
        let max = ((1u32 << bd) - 1) as u16;
        assert_eq!(Pixel::Gray(0).to_rgba16(bd), Some([0, 0, 0, 0xFFFF]));
        assert_eq!(Pixel::Gray(max).to_rgba16(bd), Some([0xFFFF; 4]));
    }
    assert_eq!(
        Pixel::Gray(200).to_rgba16(8),
        Some([200 * 257, 200 * 257, 200 * 257, 0xFFFF])
    );
    assert_eq!(
        Pixel::Gray(1).to_rgba16(4),
        Some([0x1111, 0x1111, 0x1111, 0xFFFF])
    );
    assert_eq!(
        Pixel::Gray(2).to_rgba16(2),
        Some([0xAAAA, 0xAAAA, 0xAAAA, 0xFFFF])
    );
}

#[test]
fn conversions_reject_what_they_cannot_do() {
    assert_eq!(Pixel::Indexed(3).to_rgba16(8), None);
    assert_eq!(Pixel::Indexed(3).luma16(8), None);
    assert_eq!(Pixel::Gray(1).to_rgba16(3), None);
    assert_eq!(Pixel::Gray(1).to_rgba16(0), None);
    assert_eq!(Pixel::from_rgba16([0; 4], ColorType::IndexColor, 8), None);
    assert_eq!(Pixel::from_rgba16([0; 4], ColorType::TrueColor, 4), None);
    assert_eq!(Pixel::from_rgba16([0; 4], ColorType::GrayScale, 5), None);
    // Gray from colour is the integer Rec. 601 luma of the 16-bit channels.
    let luma = |r: u32, g: u32, b: u32| ((299 * r + 587 * g + 114 * b + 500) / 1000) as u16;
    assert_eq!(
        Pixel::from_rgba16([0xFFFF, 0, 0, 0xFFFF], ColorType::GrayScale, 16),
        Some(Pixel::Gray(luma(0xFFFF, 0, 0)))
    );
    assert_eq!(Pixel::RGB(255, 0, 0).luma16(8), Some(luma(0xFFFF, 0, 0)));
    assert_eq!(Pixel::Gray(255).luma16(8), Some(0xFFFF));
}

#[test]
fn map_channels_leaves_alpha_and_indices_alone() {
    assert_eq!(Pixel::Gray(1).map_channels(|v| v + 1), Pixel::Gray(2));
    assert_eq!(
        Pixel::GrayAlpha(1, 9).map_channels(|v| v + 1),
        Pixel::GrayAlpha(2, 9)
    );
    assert_eq!(
        Pixel::RGB(1, 2, 3).map_channels(|v| v * 2),
        Pixel::RGB(2, 4, 6)
    );
    assert_eq!(
        Pixel::RGBA(1, 2, 3, 4).map_channels(|v| v * 2),
        Pixel::RGBA(2, 4, 6, 4)
    );
    assert_eq!(Pixel::Indexed(5).map_channels(|_| 0), Pixel::Indexed(5));
}
