extern crate png_glitch;

use png_glitch::{FilterType, PngGlitch};

fn main() {
    let mut glitch = PngGlitch::open("etc/sample00.png").expect("Failed to open file");

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
    glitch.apply_filter_from(FilterType::Paeth, region_height * 4, height - (region_height * 4));

    // 4. Change filter type for a region
    // This internally calls remove_filter_from and then apply_filter_from
    // Note: change_filter_type for the whole image exists, but let's just 
    // use the component methods to show the regional control.
    
    glitch.save("etc/region-glitch.png").expect("Failed to save file");
    
    println!("Region-glitched image saved to etc/region-glitch.png");
}
