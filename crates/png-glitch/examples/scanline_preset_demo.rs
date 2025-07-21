use png_glitch::{GlitchPresets, PngGlitch, ScanlineColor, ScanlineConfig, ScanlinePattern};

fn main() -> anyhow::Result<()> {
    // Load the sample image
    let mut png_glitch = PngGlitch::open("./etc/sample00.png")?;
    
    println!("Original image size: {}x{}", png_glitch.width(), png_glitch.height());
    
    // Apply default scanlines
    png_glitch.scanlines_default()?;
    png_glitch.save("./scanline_default.png")?;
    println!("Saved scanline_default.png");
    
    // Apply custom red scanlines
    let mut png_glitch2 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch2.scanlines(ScanlineConfig {
        intensity: 0.8,
        spacing: 2,
        color: ScanlineColor::Custom(255, 0, 0),
        region: None,
    })?;
    png_glitch2.save("./scanline_red.png")?;
    println!("Saved scanline_red.png");
    
    // Apply dashed pattern
    let mut png_glitch3 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch3.scanlines(ScanlineConfig {
        intensity: 0.6,
        spacing: 4,
        color: ScanlineColor::Pattern(ScanlinePattern::Dashed),
        region: None,
    })?;
    png_glitch3.save("./scanline_dashed.png")?;
    println!("Saved scanline_dashed.png");
    
    // Apply noise pattern
    let mut png_glitch4 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch4.scanlines(ScanlineConfig {
        intensity: 0.4,
        spacing: 6,
        color: ScanlineColor::Pattern(ScanlinePattern::Noise),
        region: None,
    })?;
    png_glitch4.save("./scanline_noise.png")?;
    println!("Saved scanline_noise.png");
    
    // Apply dotted pattern with region
    let mut png_glitch5 = PngGlitch::open("./etc/sample00.png")?;
    let region = png_glitch::Region::new(50, 50, 200, 150);
    png_glitch5.scanlines(ScanlineConfig {
        intensity: 0.9,
        spacing: 3,
        color: ScanlineColor::Pattern(ScanlinePattern::Dotted),
        region: Some(region),
    })?;
    png_glitch5.save("./scanline_dotted_region.png")?;
    println!("Saved scanline_dotted_region.png");
    
    println!("All scanline preset demos completed successfully!");
    
    Ok(())
}