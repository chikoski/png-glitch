use png_glitch::{GlitchPresets, PngGlitch, RgbShiftConfig, WrapMode};

fn main() -> anyhow::Result<()> {
    // Load the sample image
    let mut png_glitch = PngGlitch::open("./etc/sample00.png")?;
    
    println!("Original image size: {}x{}", png_glitch.width(), png_glitch.height());
    
    // Apply default RGB shift
    png_glitch.rgb_shift_default()?;
    png_glitch.save("./rgb_shift_default.png")?;
    println!("Saved rgb_shift_default.png");
    
    // Apply horizontal RGB separation
    let mut png_glitch2 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch2.rgb_shift(RgbShiftConfig {
        red_offset: (5, 0),
        green_offset: (0, 0),
        blue_offset: (-5, 0),
        intensity: 1.0,
        wrap_mode: WrapMode::Clamp,
    })?;
    png_glitch2.save("./rgb_shift_horizontal.png")?;
    println!("Saved rgb_shift_horizontal.png");
    
    // Apply vertical RGB separation
    let mut png_glitch3 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch3.rgb_shift(RgbShiftConfig {
        red_offset: (0, 3),
        green_offset: (0, 0),
        blue_offset: (0, -3),
        intensity: 0.9,
        wrap_mode: WrapMode::Mirror,
    })?;
    png_glitch3.save("./rgb_shift_vertical.png")?;
    println!("Saved rgb_shift_vertical.png");
    
    // Apply diagonal RGB separation
    let mut png_glitch4 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch4.rgb_shift(RgbShiftConfig {
        red_offset: (4, 4),
        green_offset: (0, 0),
        blue_offset: (-4, -4),
        intensity: 0.8,
        wrap_mode: WrapMode::Wrap,
    })?;
    png_glitch4.save("./rgb_shift_diagonal.png")?;
    println!("Saved rgb_shift_diagonal.png");
    
    // Apply subtle chromatic aberration
    let mut png_glitch5 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch5.rgb_shift(RgbShiftConfig {
        red_offset: (1, 0),
        green_offset: (0, 0),
        blue_offset: (-1, 0),
        intensity: 0.6,
        wrap_mode: WrapMode::Clamp,
    })?;
    png_glitch5.save("./rgb_shift_chromatic.png")?;
    println!("Saved rgb_shift_chromatic.png");
    
    // Combine RGB shift with other effects
    let mut png_glitch6 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch6
        .rgb_shift(RgbShiftConfig {
            red_offset: (3, 0),
            green_offset: (0, 0),
            blue_offset: (-3, 0),
            intensity: 0.7,
            wrap_mode: WrapMode::Clamp,
        })?
        .noise_default()?
        .scanlines_default()?;
    png_glitch6.save("./rgb_shift_combo.png")?;
    println!("Saved rgb_shift_combo.png");
    
    println!("All RGB shift preset demos completed successfully!");
    
    Ok(())
}