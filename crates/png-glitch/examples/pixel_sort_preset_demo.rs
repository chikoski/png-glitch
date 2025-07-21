use png_glitch::{GlitchPresets, PixelSortConfig, PngGlitch, Region, SortCriteria, SortDirection};

fn main() -> anyhow::Result<()> {
    // Load the sample image
    let mut png_glitch = PngGlitch::open("./etc/sample00.png")?;
    
    println!("Original image size: {}x{}", png_glitch.width(), png_glitch.height());
    
    // Apply default pixel sort
    png_glitch.pixel_sort_default()?;
    png_glitch.save("./pixel_sort_default.png")?;
    println!("Saved pixel_sort_default.png");
    
    // Apply horizontal brightness sorting
    let mut png_glitch2 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch2.pixel_sort(PixelSortConfig {
        direction: SortDirection::Horizontal,
        criteria: SortCriteria::Brightness,
        region: None,
        threshold: Some(0.3),
    })?;
    png_glitch2.save("./pixel_sort_horizontal_brightness.png")?;
    println!("Saved pixel_sort_horizontal_brightness.png");
    
    // Apply vertical hue sorting
    let mut png_glitch3 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch3.pixel_sort(PixelSortConfig {
        direction: SortDirection::Vertical,
        criteria: SortCriteria::Hue,
        region: None,
        threshold: Some(0.4),
    })?;
    png_glitch3.save("./pixel_sort_vertical_hue.png")?;
    println!("Saved pixel_sort_vertical_hue.png");
    
    // Apply diagonal saturation sorting
    let mut png_glitch4 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch4.pixel_sort(PixelSortConfig {
        direction: SortDirection::Diagonal,
        criteria: SortCriteria::Saturation,
        region: None,
        threshold: Some(0.2),
    })?;
    png_glitch4.save("./pixel_sort_diagonal_saturation.png")?;
    println!("Saved pixel_sort_diagonal_saturation.png");
    
    // Apply radial red channel sorting
    let mut png_glitch5 = PngGlitch::open("./etc/sample00.png")?;
    let center = (png_glitch5.width() / 2, png_glitch5.height() / 2);
    png_glitch5.pixel_sort(PixelSortConfig {
        direction: SortDirection::Radial { center },
        criteria: SortCriteria::Red,
        region: None,
        threshold: Some(0.5),
    })?;
    png_glitch5.save("./pixel_sort_radial_red.png")?;
    println!("Saved pixel_sort_radial_red.png");
    
    // Apply regional pixel sorting
    let mut png_glitch6 = PngGlitch::open("./etc/sample00.png")?;
    let region = Region::new(100, 100, 300, 200);
    png_glitch6.pixel_sort(PixelSortConfig {
        direction: SortDirection::Horizontal,
        criteria: SortCriteria::Green,
        region: Some(region),
        threshold: None, // No threshold
    })?;
    png_glitch6.save("./pixel_sort_regional_green.png")?;
    println!("Saved pixel_sort_regional_green.png");
    
    // Apply blue channel sorting with high threshold
    let mut png_glitch7 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch7.pixel_sort(PixelSortConfig {
        direction: SortDirection::Vertical,
        criteria: SortCriteria::Blue,
        region: None,
        threshold: Some(0.7),
    })?;
    png_glitch7.save("./pixel_sort_blue_threshold.png")?;
    println!("Saved pixel_sort_blue_threshold.png");
    
    // Combine pixel sorting with other effects
    let mut png_glitch8 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch8
        .pixel_sort(PixelSortConfig {
            direction: SortDirection::Horizontal,
            criteria: SortCriteria::Brightness,
            region: None,
            threshold: Some(0.4),
        })?
        .rgb_shift_default()?
        .noise_default()?;
    png_glitch8.save("./pixel_sort_combo.png")?;
    println!("Saved pixel_sort_combo.png");
    
    println!("All pixel sort preset demos completed successfully!");
    
    Ok(())
}