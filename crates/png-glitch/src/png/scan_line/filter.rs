use crate::{FilterType, ScanLine};

pub(crate) mod kernels;
#[cfg(test)]
mod reference;
mod wavefront;

/// The bytes of `line` after the filter-type byte, or `None` for an empty line.
fn pixel_bytes<'a>(line: &'a mut ScanLine) -> Option<&'a mut [u8]> {
    line.data.split_first_mut().map(|(_, bytes)| bytes)
}

/// Runs `f` with the current row's bytes and the previous row's bytes (if any).
///
/// A previous row of a different length cannot occur for rows of one image, but if it did the
/// per-byte code treated missing bytes as zero; keep that by padding / truncating it.
fn with_rows(
    line: &mut ScanLine,
    previous: Option<&ScanLine>,
    f: impl FnOnce(usize, &mut [u8], Option<&[u8]>),
) {
    let bpp = line.bytes_per_pixel();
    let Some(cur) = pixel_bytes(line) else {
        return;
    };
    let prev = previous.and_then(|p| p.data.get(1..));
    match prev {
        Some(prev) if prev.len() != cur.len() => {
            let padded: Vec<u8> = prev
                .iter()
                .copied()
                .chain(std::iter::repeat(0))
                .take(cur.len())
                .collect();
            f(bpp, cur, Some(&padded));
        }
        prev => f(bpp, cur, prev),
    }
}

/// The function removes a filter from a scan line.
/// The `line` parameter is the scan line to remove the filter from.
/// The `previous` parameter is the previous scan line.
pub fn remove(line: &mut ScanLine, previous_line: Option<&ScanLine>) {
    let filter_type = line.filter_type();
    with_rows(line, previous_line, |bpp, cur, prev| {
        kernels::unfilter(filter_type, bpp, cur, prev)
    });
}

/// Whether [`remove_rows`] uses the multi-row kernels. They need 8-lane SIMD to pay off, so only
/// wasm with `simd128` turns them on; native targets keep the one-row kernels.
const MULTI_ROW: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

/// Removes the filters from the first rows of `block` together with a multi-row wavefront kernel
/// (`wavefront.rs`, #152), if one exists for them and for this target ([`MULTI_ROW`]).
///
/// `block` holds consecutive rows, `stride` bytes apart, each with its filter-type byte;
/// `previous` is the (already reconstructed) row above the first one, with its filter-type
/// byte, and `bpp` the bytes per pixel. Returns how many rows were reconstructed and had their
/// filter type set to `None`: 0 (nothing changed; reconstruct the first row alone), 2 or 4. The
/// bytes are the same as reconstructing the rows one at a time.
pub(crate) fn remove_rows(
    block: &mut [u8],
    stride: usize,
    bpp: usize,
    previous: Option<&[u8]>,
) -> usize {
    if MULTI_ROW {
        try_remove_rows(block, stride, bpp, previous)
    } else {
        0
    }
}

/// [`remove_rows`] without the target check, so native tests can run the multi-row kernels.
fn try_remove_rows(block: &mut [u8], stride: usize, bpp: usize, previous: Option<&[u8]>) -> usize {
    if bpp != 4 || stride == 0 {
        return 0;
    }
    // The first image row has no row above; it is reconstructed on its own.
    let Some(prev) = previous.and_then(|p| p.get(1..)) else {
        return 0;
    };
    let Some(&first) = block.first() else {
        return 0;
    };
    let filter_type = FilterType::try_from(first).unwrap_or(FilterType::None);
    if !matches!(filter_type, FilterType::Paeth | FilterType::Average) {
        return 0;
    }
    let run = block
        .chunks_exact(stride)
        .take(4)
        .take_while(|row| row.first() == Some(&first))
        .count();
    let rows = match run {
        4 => 4,
        2 | 3 => 2,
        _ => return 0,
    };
    let Some(block) = block.get_mut(..rows * stride) else {
        return 0;
    };
    use wavefront::{Average, Paeth, unfilter_4_rows};
    let done = match (filter_type, rows) {
        (FilterType::Paeth, 4) => unfilter_4_rows::<Paeth, 2>(prev, block, stride),
        (FilterType::Paeth, _) => unfilter_4_rows::<Paeth, 1>(prev, block, stride),
        (_, 4) => unfilter_4_rows::<Average, 2>(prev, block, stride),
        _ => unfilter_4_rows::<Average, 1>(prev, block, stride),
    };
    if !done {
        return 0;
    }
    for row in block.chunks_exact_mut(stride) {
        if let Some(ft) = row.first_mut() {
            *ft = FilterType::None.into();
        }
    }
    rows
}

/// The function applies a filter to a scan line.
/// The `filter_type` parameter is the type of the filter to apply.
/// The `line` parameter is the scan line to apply the filter to.
/// The `previous` parameter is the previous scan line.
pub fn apply(filter_type: FilterType, line: &mut ScanLine, previous: Option<&ScanLine>) {
    with_rows(line, previous, |bpp, cur, prev| {
        kernels::filter(filter_type, bpp, cur, prev)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::ColorType;
    use proptest::prelude::*;

    #[test]
    fn test_filter_roundtrip() {
        let filter_types = [
            FilterType::None,
            FilterType::Sub,
            FilterType::Up,
            FilterType::Average,
            FilterType::Paeth,
        ];

        for &filter_type in &filter_types {
            let mut previous_data = vec![0, 10, 20, 30, 40, 50, 60, 70, 80];
            let mut current_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
            let current_copy = current_data.clone();

            let previous_line = ScanLine::new(&mut previous_data, ColorType::TrueColorAlpha, 8, 2);
            let mut current_line =
                ScanLine::new(&mut current_data, ColorType::TrueColorAlpha, 8, 2);

            apply(filter_type, &mut current_line, Some(&previous_line));
            current_line.data[0] = filter_type.into();
            remove(&mut current_line, Some(&previous_line));

            assert_eq!(
                current_line.data[1..],
                current_copy[1..],
                "Failed for {:?}",
                filter_type
            );
        }
    }

    const FILTER_TYPES: [FilterType; 5] = [
        FilterType::None,
        FilterType::Sub,
        FilterType::Up,
        FilterType::Average,
        FilterType::Paeth,
    ];

    /// A header (color type, bit depth) whose bytes-per-pixel is `bpp`.
    fn format_for(bpp: usize) -> (ColorType, u8) {
        match bpp {
            1 => (ColorType::GrayScale, 8),
            2 => (ColorType::GrayScale, 16),
            3 => (ColorType::TrueColor, 8),
            4 => (ColorType::TrueColorAlpha, 8),
            6 => (ColorType::TrueColor, 16),
            _ => (ColorType::TrueColorAlpha, 16),
        }
    }

    /// Random rows of 1..=64 pixels.
    fn rows() -> impl Strategy<Value = (usize, Vec<u8>, Vec<u8>, bool)> {
        rows_up_to(64)
    }

    /// Random rows: bytes-per-pixel, current row, previous row, whether a previous row exists.
    /// Rows hold 1..=`max_pixels` pixels of arbitrary bytes, so "glitched" (not really
    /// filtered) input is covered.
    fn rows_up_to(max_pixels: usize) -> impl Strategy<Value = (usize, Vec<u8>, Vec<u8>, bool)> {
        prop::sample::select(vec![1usize, 2, 3, 4, 6, 8]).prop_flat_map(move |bpp| {
            (1usize..=max_pixels).prop_flat_map(move |pixels| {
                let len = pixels * bpp;
                (
                    Just(bpp),
                    prop::collection::vec(any::<u8>(), len),
                    prop::collection::vec(any::<u8>(), len),
                    any::<bool>(),
                )
            })
        })
    }

    /// Builds a scan line (filter-type byte + `bytes`) and runs `f` on it.
    fn with_line<R>(
        bpp: usize,
        filter_type: FilterType,
        bytes: &[u8],
        f: impl FnOnce(&mut ScanLine) -> R,
    ) -> (R, Vec<u8>) {
        let (color, depth) = format_for(bpp);
        let mut data = vec![filter_type.into()];
        data.extend_from_slice(bytes);
        let width = (bytes.len() / bpp) as u32;
        let mut line = ScanLine::new(&mut data, color, depth, width);
        assert_eq!(line.bytes_per_pixel(), bpp);
        let r = f(&mut line);
        (r, data[1..].to_vec())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn unfilter_matches_reference((bpp, cur, prev, has_prev) in rows()) {
            check_unfilter_matches_reference(bpp, &cur, &prev, has_prev)?;
        }

        #[test]
        fn filter_matches_reference((bpp, cur, prev, has_prev) in rows()) {
            check_filter_matches_reference(bpp, &cur, &prev, has_prev)?;
        }

        #[test]
        fn unfilter_inverts_filter((bpp, cur, prev, has_prev) in rows()) {
            for filter_type in FILTER_TYPES {
                let prev_row = has_prev.then_some(prev.as_slice());
                let mut bytes = cur.clone();
                kernels::filter(filter_type, bpp, &mut bytes, prev_row);
                kernels::unfilter(filter_type, bpp, &mut bytes, prev_row);
                prop_assert_eq!(&bytes, &cur, "{:?} bpp {}", filter_type, bpp);
            }
        }
    }

    fn check_unfilter_matches_reference(
        bpp: usize,
        cur: &[u8],
        prev: &[u8],
        has_prev: bool,
    ) -> Result<(), TestCaseError> {
        for filter_type in FILTER_TYPES {
            let mut prev_data = [&[0u8][..], prev].concat();
            let (color, depth) = format_for(bpp);
            let width = (cur.len() / bpp) as u32;
            let prev_line = ScanLine::new(&mut prev_data, color, depth, width);
            let prev_ref = has_prev.then_some(&prev_line);

            let (_, expected) =
                with_line(bpp, filter_type, cur, |l| reference::remove(l, prev_ref));
            let (_, actual) = with_line(bpp, filter_type, cur, |l| remove(l, prev_ref));
            prop_assert_eq!(actual, expected, "unfilter {:?} bpp {}", filter_type, bpp);
        }
        Ok(())
    }

    fn check_filter_matches_reference(
        bpp: usize,
        cur: &[u8],
        prev: &[u8],
        has_prev: bool,
    ) -> Result<(), TestCaseError> {
        for filter_type in FILTER_TYPES {
            let mut prev_data = [&[0u8][..], prev].concat();
            let (color, depth) = format_for(bpp);
            let width = (cur.len() / bpp) as u32;
            let prev_line = ScanLine::new(&mut prev_data, color, depth, width);
            let prev_ref = has_prev.then_some(&prev_line);

            let (_, expected) = with_line(bpp, FilterType::None, cur, |l| {
                reference::apply(filter_type, l, prev_ref)
            });
            let (_, actual) = with_line(bpp, FilterType::None, cur, |l| {
                apply(filter_type, l, prev_ref)
            });
            prop_assert_eq!(actual, expected, "filter {:?} bpp {}", filter_type, bpp);
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Rows longer than the Paeth filter's block (`PAETH_BLOCK` bytes), so that block
        /// boundaries, and a short last block, are crossed for every pixel size.
        #[test]
        fn long_rows_match_reference((bpp, cur, prev, has_prev) in rows_up_to(400)) {
            check_unfilter_matches_reference(bpp, &cur, &prev, has_prev)?;
            check_filter_matches_reference(bpp, &cur, &prev, has_prev)?;
        }

        /// The lane-wise RGBA Paeth reconstruction that wasm builds use. Native builds don't
        /// dispatch to it, so it is checked here directly against the reference.
        #[test]
        fn paeth_unfilter_4_lanes_matches_reference(
            (cur, prev, has_prev) in (1usize..=400).prop_flat_map(|pixels| (
                prop::collection::vec(any::<u8>(), pixels * 4),
                prop::collection::vec(any::<u8>(), pixels * 4),
                any::<bool>(),
            ))
        ) {
            let mut prev_data = [&[0u8][..], &prev].concat();
            let width = (cur.len() / 4) as u32;
            let prev_line = ScanLine::new(&mut prev_data, ColorType::TrueColorAlpha, 8, width);
            let prev_ref = has_prev.then_some(&prev_line);
            let (_, expected) =
                with_line(4, FilterType::Paeth, &cur, |l| reference::remove(l, prev_ref));

            let mut actual = cur.clone();
            kernels::paeth_unfilter_4_lanes(&mut actual, has_prev.then_some(prev.as_slice()));
            prop_assert_eq!(actual, expected);
        }
    }

    /// Unfilters `rows` (each with its filter-type byte) top-down with the reference
    /// implementation, `above` being the row above the first one (if any). Returns the bytes.
    fn reference_unfilter_rows(bpp: usize, above: Option<&[u8]>, rows: &[Vec<u8>]) -> Vec<u8> {
        let (color, depth) = format_for(bpp);
        let mut out = Vec::new();
        let mut prev: Option<Vec<u8>> = above.map(|a| [&[0u8][..], a].concat());
        for row in rows {
            let width = ((row.len() - 1) / bpp) as u32;
            let mut data = row.clone();
            let mut line = ScanLine::new(&mut data, color, depth, width);
            match prev.as_mut() {
                Some(p) => {
                    let p = ScanLine::new(p, color, depth, width);
                    reference::remove(&mut line, Some(&p));
                }
                None => reference::remove(&mut line, None),
            }
            out.extend_from_slice(&data[1..]);
            prev = Some(data);
        }
        out
    }

    /// Runs the multi-row kernels on `rows` (each with its filter-type byte) below `above`,
    /// without the target check. Returns how many rows they reconstructed and the bytes of all
    /// rows (filter-type bytes included).
    fn try_rows(bpp: usize, above: Option<&[u8]>, rows: &[Vec<u8>]) -> (usize, Vec<Vec<u8>>) {
        let stride = rows.first().map_or(1, Vec::len);
        let mut block = rows.concat();
        let above = above.map(|a| [&[0u8][..], a].concat());
        let done = try_remove_rows(&mut block, stride, bpp, above.as_deref());
        (done, block.chunks(stride).map(<[u8]>::to_vec).collect())
    }

    /// 1..=6 rows of `bpp`-byte pixels (1..=`max_pixels` of them) and the row above them
    /// (`None` for the first image row). Each block picks Paeth or Average; each row has that
    /// filter type with probability `same`, otherwise an arbitrary one.
    #[allow(clippy::type_complexity)]
    fn row_blocks(
        bpps: Vec<usize>,
        max_pixels: usize,
        same: f64,
    ) -> impl Strategy<Value = (usize, Option<Vec<u8>>, Vec<Vec<u8>>)> {
        let dominant = prop::sample::select(vec![FilterType::Paeth, FilterType::Average]);
        (prop::sample::select(bpps), dominant).prop_flat_map(move |(bpp, dominant)| {
            (1usize..=max_pixels).prop_flat_map(move |pixels| {
                let len = pixels * bpp;
                let row = (
                    prop::bool::weighted(same),
                    0u8..5,
                    prop::collection::vec(any::<u8>(), len),
                )
                    .prop_map(move |(is_same, ft, bytes)| {
                        let ft = if is_same { dominant.into() } else { ft };
                        [&[ft][..], &bytes].concat()
                    });
                (
                    Just(bpp),
                    prop::option::weighted(0.9, prop::collection::vec(any::<u8>(), len)),
                    prop::collection::vec(row, 1..=6),
                )
            })
        })
    }

    /// Checks one block: the kernels reconstruct the first 0, 2 or 4 rows exactly as the
    /// reference does (filter type set to `None`), leave the other rows untouched, and take
    /// every run of RGBA Paeth rows below a row that they can.
    fn check_rows(bpp: usize, above: Option<&[u8]>, rows: &[Vec<u8>]) -> Result<(), TestCaseError> {
        let (done, actual) = try_rows(bpp, above, rows);
        let first = rows[0][0];
        let wavefront =
            first == u8::from(FilterType::Paeth) || first == u8::from(FilterType::Average);
        let run = if wavefront {
            rows.iter().take_while(|r| r[0] == first).count()
        } else {
            0
        };
        let expected_done = match (bpp, above.is_some(), run) {
            (4, true, 4..) => 4,
            (4, true, 2 | 3) => 2,
            _ => 0,
        };
        prop_assert_eq!(done, expected_done);
        let expected = reference_unfilter_rows(bpp, above, &rows[..done]);
        for (r, (got, original)) in actual.iter().zip(rows).enumerate() {
            if r < done {
                prop_assert_eq!(got[0], u8::from(FilterType::None));
                let width = got.len() - 1;
                prop_assert_eq!(
                    &got[1..],
                    &expected[r * width..(r + 1) * width],
                    "row {}",
                    r
                );
            } else {
                prop_assert_eq!(got, original, "row {} must be untouched", r);
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// Mixed filter types and pixel sizes, so declined blocks are covered too.
        #[test]
        fn rows_match_reference((bpp, above, rows) in row_blocks(vec![1, 3, 4, 8], 40, 0.5)) {
            check_rows(bpp, above.as_deref(), &rows)?;
        }

        /// RGBA rows that are mostly Paeth or mostly Average, and long, so 2- and 4-row blocks
        /// are taken often.
        #[test]
        fn wavefront_rows_match_reference((bpp, above, rows) in row_blocks(vec![4], 300, 0.9)) {
            check_rows(bpp, above.as_deref(), &rows)?;
        }
    }

    /// Blocks whose stride doesn't match the row above, or whose rows aren't whole pixels, are
    /// declined unchanged.
    #[test]
    fn rows_declines_inconsistent_blocks() {
        let paeth = u8::from(FilterType::Paeth);
        let rows = [
            vec![paeth, 1, 2, 3, 4, 5, 6, 7, 8],
            vec![paeth, 1, 2, 3, 4, 5, 6, 7, 8],
        ];
        let mut block = rows.concat();
        // Row above too short.
        assert_eq!(try_remove_rows(&mut block, 9, 4, Some(&[0, 1, 2, 3, 4])), 0);
        // Rows of 6 bytes are not whole RGBA pixels.
        let mut ragged = [paeth, 1, 2, 3, 4, 5, 6, paeth, 1, 2, 3, 4, 5, 6];
        assert_eq!(try_remove_rows(&mut ragged, 7, 4, Some(&[0; 7])), 0);
        assert_eq!(block, rows.concat());
        assert_eq!(ragged, [paeth, 1, 2, 3, 4, 5, 6, paeth, 1, 2, 3, 4, 5, 6]);
    }

    /// Rows that are not a whole number of pixels, and pixel sizes outside the specialised set,
    /// take the generic path; it must still invert itself and agree with the fast path where
    /// both apply.
    #[test]
    fn generic_path_round_trips_and_matches_fast_path() {
        let prev: Vec<u8> = (0..24u32).map(|i| (i * 37 + 11) as u8).collect();
        let cur: Vec<u8> = (0..24u32).map(|i| (i * 91 + 5) as u8).collect();
        for filter_type in FILTER_TYPES {
            for prev_row in [None, Some(prev.as_slice())] {
                // 5 bytes per pixel and a ragged tail (24 % 5 != 0).
                let mut bytes = cur.clone();
                kernels::filter(filter_type, 5, &mut bytes, prev_row);
                kernels::unfilter(filter_type, 5, &mut bytes, prev_row);
                assert_eq!(bytes, cur, "{filter_type:?} generic");
            }
        }
    }

    #[test]
    fn previous_row_of_another_length_is_zero_padded() {
        let mut short = vec![0u8, 9, 9];
        let mut data = vec![FilterType::Up.into(), 1, 2, 3, 4];
        let prev = ScanLine::new(&mut short, ColorType::GrayScale, 8, 2);
        let mut line = ScanLine::new(&mut data, ColorType::GrayScale, 8, 4);
        remove(&mut line, Some(&prev));
        assert_eq!(&data[1..], [10, 11, 3, 4]);
    }
}
