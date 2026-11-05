use png_glitch::presets::{Brighten, Invert};
use png_glitch::{Pixel, PngGlitch};
use std::fs;
use std::path::Path;

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("sample00-glitched.png");

    // Open the sample PNG file
    let mut glitch = PngGlitch::open(&input_path).expect("Failed to open file");

    println!("Original image: {}x{}", glitch.width(), glitch.height());

    // 1. Use the fluent API to remove filters and apply presets
    glitch
        .remove_filter()
        .apply(Invert)
        .apply(Brighten { strength: 50 });

    // 2. Use par_foreach_scanline for custom parallel manipulation
    // We'll zero out the green channel for all pixels in a specific range
    glitch.par_foreach_scanline(move |scan_line| {
        // Just a silly condition to show we can do anything per-line
        for x in 0..(scan_line.size() / scan_line.bytes_per_pixel()) {
            if let Some(pixel) = scan_line.get_pixel(x) {
                let modified = match pixel {
                    Pixel::RGB(r, _g, b) => Pixel::RGB(r, 0, b),
                    Pixel::RGBA(r, _g, b, a) => Pixel::RGBA(r, 0, b, a),
                    _ => pixel,
                };
                scan_line.set_pixel(x, modified);
            }
        }
    });

    // 3. Save the result
    glitch.save(&output_path).expect("Failed to save file");

    println!("Glitched image saved to {}", output_path.display());
}
