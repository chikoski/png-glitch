use png_glitch::{Pixel, PngGlitch};
use std::fs;
use std::path::Path;

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("high-perf-glitch.png");

    // Open the sample PNG file
    let mut glitch = PngGlitch::open(&input_path).expect("Failed to open file");

    println!(
        "Applying custom high-performance glitch to {}x{} image...",
        glitch.width(),
        glitch.height()
    );

    // process_pixels is the most efficient way to do custom manipulation.
    // It resolves the bit depth and color type ONCE per scanline,
    // rather than for every single pixel access.
    glitch.foreach_scanline(|scan_line| {
        scan_line.process_pixels(|x, pixel| {
            // A simple high-performance threshold effect
            match pixel {
                Pixel::RGB(r, g, b) => {
                    if (r as u32 + g as u32 + b as u32) / 3 > 128 {
                        // Swap channels for bright pixels
                        Pixel::RGB(g, b, r)
                    } else {
                        // Shift x-coordinate into the red channel for dark pixels
                        Pixel::RGB((x % 255) as u16, g, b)
                    }
                }
                Pixel::RGBA(r, g, b, a) => {
                    // Same logic for RGBA
                    if (r as u32 + g as u32 + b as u32) / 3 > 128 {
                        Pixel::RGBA(g, b, r, a)
                    } else {
                        Pixel::RGBA((x % 255) as u16, g, b, a)
                    }
                }
                // Pass-through for other types (Grayscale, Indexed)
                _ => pixel,
            }
        });
    });

    // Save the result
    glitch.save(&output_path).expect("Failed to save file");

    println!(
        "High-performance glitched image saved to {}",
        output_path.display()
    );
}
