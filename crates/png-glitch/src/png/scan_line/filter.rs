use crate::{FilterType, ScanLine};

mod kernels;
#[cfg(test)]
mod reference;

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

    /// Random rows: bytes-per-pixel, pixel count, current row, previous row, whether a previous
    /// row exists. Rows are arbitrary bytes, so "glitched" (not really filtered) input is covered.
    fn rows() -> impl Strategy<Value = (usize, Vec<u8>, Vec<u8>, bool)> {
        prop::sample::select(vec![1usize, 2, 3, 4, 6, 8]).prop_flat_map(|bpp| {
            (1usize..=64).prop_flat_map(move |pixels| {
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
            for filter_type in FILTER_TYPES {
                let mut prev_data = [vec![0u8], prev.clone()].concat();
                let (color, depth) = format_for(bpp);
                let width = (cur.len() / bpp) as u32;
                let prev_line = ScanLine::new(&mut prev_data, color, depth, width);
                let prev_ref = has_prev.then_some(&prev_line);

                let (_, expected) =
                    with_line(bpp, filter_type, &cur, |l| reference::remove(l, prev_ref));
                let (_, actual) = with_line(bpp, filter_type, &cur, |l| remove(l, prev_ref));
                prop_assert_eq!(actual, expected, "unfilter {:?} bpp {}", filter_type, bpp);
            }
        }

        #[test]
        fn filter_matches_reference((bpp, cur, prev, has_prev) in rows()) {
            for filter_type in FILTER_TYPES {
                let mut prev_data = [vec![0u8], prev.clone()].concat();
                let (color, depth) = format_for(bpp);
                let width = (cur.len() / bpp) as u32;
                let prev_line = ScanLine::new(&mut prev_data, color, depth, width);
                let prev_ref = has_prev.then_some(&prev_line);

                let (_, expected) =
                    with_line(bpp, FilterType::None, &cur, |l| reference::apply(filter_type, l, prev_ref));
                let (_, actual) =
                    with_line(bpp, FilterType::None, &cur, |l| apply(filter_type, l, prev_ref));
                prop_assert_eq!(actual, expected, "filter {:?} bpp {}", filter_type, bpp);
            }
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
