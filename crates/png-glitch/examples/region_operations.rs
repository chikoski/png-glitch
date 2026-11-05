use png_glitch::{FilterType, PngGlitch};
use std::fs;
use std::path::Path;

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("region-glitch.png");

    let mut glitch = PngGlitch::open(&input_path).expect("Failed to open file");

    let height = glitch.height();
    let region_height = height / 5;

    // 1. Remove filter from a specific region (the top part)
    println!("Removing filter from top region...");
    glitch.remove_filter_from(0, region_height);

    // 2. Apply a specific filter to a middle region
    println!("Applying Sub filter to middle region...");
    glitch.apply_filter_from(FilterType::Sub, region_height * 2, region_height);

    // 3. Apply a different filter to the bottom region
    println!("Applying Paeth filter to bottom region...");
    glitch.apply_filter_from(
        FilterType::Paeth,
        region_height * 4,
        height - (region_height * 4),
    );

    glitch.save(&output_path).expect("Failed to save file");

    println!("Region-glitched image saved to {}", output_path.display());
}
