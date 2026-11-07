//! Decoder equivalence of `Renderer` (#96): `render_into(m)` must equal the `png` crate decoding
//! `m.to_bytes()`, byte for byte, for every colour type × bit depth × width and any glitch.

mod common;

use common::{build_png, row_bytes};
use png_glitch::{PngError, PngGlitch, Renderer};
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

/// The reference decoder: `png` crate with EXPAND | STRIP_16, widened to RGBA8.
fn reference_decode(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("reference reads the info");
    let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("reference decodes");
    let (color, depth) = reader.output_color_type();
    assert_eq!(depth, png::BitDepth::Eight);
    buf.truncate(info.buffer_size());
    match color {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => unreachable!("EXPAND removes the palette"),
    }
}

#[derive(Debug, Clone)]
struct Case {
    width: u32,
    height: u32,
    color_type: u8,
    bit_depth: u8,
    rows: Vec<Vec<u8>>,
    plte: Option<Vec<u8>>,
    trns: Option<Vec<u8>>,
}

impl Case {
    fn bytes(&self) -> Vec<u8> {
        build_png(
            self.width,
            self.height,
            self.color_type,
            self.bit_depth,
            &self.rows,
            self.plte.as_deref(),
            self.trns.as_deref(),
        )
    }
}

fn case() -> impl Strategy<Value = Case> {
    (
        0..FORMATS.len(),
        1u32..=33,
        1u32..=8,
        // Small samples make palette indices and the tRNS key colour hit often.
        any::<bool>(),
    )
        .prop_flat_map(|(format, width, height, small)| {
            let (color_type, bit_depth) = FORMATS[format];
            let len = row_bytes(width, color_type, bit_depth);
            let sample = if small { 0u8..=3 } else { 0u8..=255 };
            let rows = vec((0u8..=4, vec(sample.clone(), len)), height as usize);
            let max_entries = (1usize << bit_depth.min(8)).min(256);
            let plte = vec(any::<[u8; 3]>(), 1..=max_entries);
            let trns_len = match color_type {
                0 => 2,
                2 => 6,
                _ => 0,
            };
            let key = vec(sample, trns_len);
            let alphas = vec(any::<u8>(), 0..=max_entries + 1);
            (
                Just((width, height, color_type, bit_depth)),
                rows,
                plte,
                key,
                alphas,
                any::<bool>(),
            )
        })
        .prop_map(
            |((width, height, color_type, bit_depth), rows, plte, key, alphas, with_trns)| {
                let rows = rows
                    .into_iter()
                    .map(|(ft, bytes)| std::iter::once(ft).chain(bytes).collect())
                    .collect();
                let (plte, trns) = match color_type {
                    3 => (Some(plte.concat()), with_trns.then_some(alphas)),
                    0 | 2 => (None, with_trns.then_some(key)),
                    _ => (None, None),
                };
                Case {
                    width,
                    height,
                    color_type,
                    bit_depth,
                    rows,
                    plte,
                    trns,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn render_into_equals_the_reference_decoder(case in case()) {
        let model = PngGlitch::from_bytes(case.bytes()).expect("the model parses");
        let expected = reference_decode(&model.to_bytes().expect("serialises"));

        let mut renderer = Renderer::new(&model);
        let mut out = vec![0xAA; case.width as usize * case.height as usize * 4];
        renderer.render_into(&model, &mut out).expect("renders");
        prop_assert_eq!(&out, &expected);

        // A second call gives the same pixels: the model was not modified.
        let mut again = vec![0x55; out.len()];
        renderer.render_into(&model, &mut again).expect("renders again");
        prop_assert_eq!(&again, &expected);
    }
}

#[test]
fn the_matrix_is_covered_by_a_plain_example_each() {
    for &(color_type, bit_depth) in FORMATS {
        let (width, height) = (5u32, 3u32);
        let len = row_bytes(width, color_type, bit_depth);
        let rows: Vec<Vec<u8>> = (0..height as usize)
            .map(|y| {
                std::iter::once((y % 5) as u8)
                    .chain((0..len).map(|x| (x * 37 + y * 11 + 5) as u8))
                    .collect()
            })
            .collect();
        let plte = (color_type == 3).then(|| {
            (0..(1usize << bit_depth.min(8)))
                .flat_map(|i| [i as u8, (i * 3) as u8, (i * 7) as u8])
                .collect::<Vec<u8>>()
        });
        let bytes = build_png(
            width,
            height,
            color_type,
            bit_depth,
            &rows,
            plte.as_deref(),
            None,
        );
        let model = PngGlitch::from_bytes(&bytes).expect("parses");
        let mut out = vec![0; 5 * 3 * 4];
        Renderer::new(&model)
            .render_into(&model, &mut out)
            .expect("renders");
        assert_eq!(out, reference_decode(&bytes), "{color_type}/{bit_depth}");
    }
}

#[test]
fn out_of_range_palette_indices_are_opaque_black() {
    // Two palette entries, 8-bit indices 0..=3; the reference decoder gives black for 2 and 3.
    let rows = vec![vec![0, 0, 1, 2, 3]];
    let plte = [10, 20, 30, 40, 50, 60];
    for trns in [None, Some(vec![7u8]), Some(vec![1u8, 2, 3])] {
        let bytes = build_png(4, 1, 3, 8, &rows, Some(&plte), trns.as_deref());
        let model = PngGlitch::from_bytes(&bytes).expect("parses");
        let mut out = vec![0; 16];
        Renderer::new(&model)
            .render_into(&model, &mut out)
            .expect("renders");
        assert_eq!(out, reference_decode(&bytes), "tRNS {trns:?}");
        assert_eq!(out.get(8..16), Some(&[0, 0, 0, 255, 0, 0, 0, 255][..]));
    }
}

#[test]
fn wrong_buffer_length_and_wrong_header_are_errors() {
    let bytes = build_png(4, 2, 0, 8, &[vec![0; 5], vec![0; 5]], None, None);
    let model = PngGlitch::from_bytes(&bytes).expect("parses");
    let mut renderer = Renderer::new(&model);
    for len in [0, 31, 33] {
        let mut out = vec![0; len];
        let err = renderer.render_into(&model, &mut out).unwrap_err();
        assert!(matches!(err, PngError::PixelDataLength { expected: 32, actual } if actual == len));
    }
    for other in [
        build_png(5, 2, 0, 8, &[vec![0; 6], vec![0; 6]], None, None),
        build_png(4, 2, 2, 8, &[vec![0; 13], vec![0; 13]], None, None),
        build_png(4, 2, 0, 16, &[vec![0; 9], vec![0; 9]], None, None),
    ] {
        let other = PngGlitch::from_bytes(&other).expect("parses");
        let mut out = vec![0; 32];
        let err = renderer.render_into(&other, &mut out).unwrap_err();
        assert!(matches!(err, PngError::HeaderMismatch), "{err:?}");
    }
}
