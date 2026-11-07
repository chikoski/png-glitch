//! Shared fixtures for the integration tests: a builder for PNG files from explicit scan lines.
#![allow(dead_code)]

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(kind);
    hasher.update(data);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
}

/// Samples per pixel of a PNG colour type code.
pub fn channels(color_type: u8) -> usize {
    match color_type {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => unreachable!("not a PNG colour type"),
    }
}

/// Bytes in one scan line, without the filter-type byte.
pub fn row_bytes(width: u32, color_type: u8, bit_depth: u8) -> usize {
    (width as usize * channels(color_type) * bit_depth as usize).div_ceil(8)
}

/// Builds a PNG whose scan lines are exactly `rows` (`filter-type byte + filtered bytes`, as
/// stored in the file), with an optional `PLTE` and `tRNS`.
pub fn build_png(
    width: u32,
    height: u32,
    color_type: u8,
    bit_depth: u8,
    rows: &[Vec<u8>],
    plte: Option<&[u8]>,
    trns: Option<&[u8]>,
) -> Vec<u8> {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[bit_depth, color_type, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    if let Some(plte) = plte {
        chunk(&mut png, b"PLTE", plte);
    }
    if let Some(trns) = trns {
        chunk(&mut png, b"tRNS", trns);
    }
    let raw: Vec<u8> = rows.iter().flatten().copied().collect();
    let mut compressor = fdeflate::Compressor::new(Vec::new()).expect("compressor");
    compressor.write_data(&raw).expect("write data");
    let idat = compressor.finish().expect("finish");
    chunk(&mut png, b"IDAT", &idat);
    chunk(&mut png, b"IEND", &[]);
    png
}
