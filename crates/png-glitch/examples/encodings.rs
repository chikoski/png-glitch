//! The same frame, encoded three ways and glitched the same way.
//!
//! A PNG encoder's choices (color type, filter strategy) decide what the filter bytes and the
//! pixel bytes look like, so the same glitch lands differently. Here every output gets exactly
//! the same two operations: a fixed `Bitwise` XOR on every 8th row and every row's filter type
//! switched to `Sub` without recomputing values.
//!
//! Run with `cargo run --example encodings`; the PNGs go to a temporary directory.

use png_glitch::effects::{BitOp, Bitwise};
use png_glitch::{
    ColorType, Encoding, FilterStrategy, FilterType, PixelSource, PngError, PngGlitch,
};
use std::fs;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

/// A synthetic RGBA frame: gradients and a bright disc.
fn frame() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (dx, dy) = (x as i32 - 200, y as i32 - 100);
            let disc = dx * dx + dy * dy < 60 * 60;
            let r = if disc { 255 } else { (x * 255 / WIDTH) as u8 };
            let g = if disc { 220 } else { (y * 255 / HEIGHT) as u8 };
            let b = if disc {
                120
            } else {
                ((x + y) * 255 / (WIDTH + HEIGHT)) as u8
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    rgba
}

fn main() -> Result<(), PngError> {
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir)?;
    let rgba = frame();

    let encodings = [
        (
            "rgba-paeth",
            Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
        ),
        (
            "rgb-pattern",
            Encoding {
                color_type: ColorType::TrueColor,
                bit_depth: 8,
                filter: FilterStrategy::Pattern(vec![
                    FilterType::Sub,
                    FilterType::Up,
                    FilterType::Paeth,
                ]),
            },
        ),
        (
            "gray-adaptive",
            Encoding {
                color_type: ColorType::GrayScale,
                bit_depth: 8,
                filter: FilterStrategy::Adaptive,
            },
        ),
    ];

    for (name, encoding) in encodings {
        let mut png = PngGlitch::from_pixels(PixelSource::rgba8(WIDTH, HEIGHT, &rgba), encoding)?;

        // The same glitch for every encoding.
        let rows: Vec<u32> = (0..HEIGHT).step_by(8).collect();
        png.apply(Bitwise::new(BitOp::Xor, 0x5a, rows))
            .set_filter_type(FilterType::Sub);

        let path = out_dir.join(format!("encodings-{name}.png"));
        png.save(&path)?;
        println!("{name}: {}", path.display());
    }
    Ok(())
}
