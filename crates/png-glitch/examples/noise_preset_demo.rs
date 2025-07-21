use png_glitch::{Direction, GlitchPresets, NoiseConfig, NoiseDistribution, PngGlitch, Region};

fn main() -> anyhow::Result<()> {
    // Load the sample image
    let mut png_glitch = PngGlitch::open("./etc/sample00.png")?;
    
    println!("Original image size: {}x{}", png_glitch.width(), png_glitch.height());
    
    // Apply default noise
    png_glitch.noise_default()?;
    png_glitch.save("./noise_default.png")?;
    println!("Saved noise_default.png");
    
    // Apply uniform noise with higher intensity
    let mut png_glitch2 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch2.noise(NoiseConfig {
        intensity: 0.5,
        distribution: NoiseDistribution::Uniform,
        region: None,
        preserve_structure: false,
    })?;
    png_glitch2.save("./noise_uniform.png")?;
    println!("Saved noise_uniform.png");
    
    // Apply clustered noise
    let mut png_glitch3 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch3.noise(NoiseConfig {
        intensity: 0.4,
        distribution: NoiseDistribution::Clustered { cluster_size: 16 },
        region: None,
        preserve_structure: true,
    })?;
    png_glitch3.save("./noise_clustered.png")?;
    println!("Saved noise_clustered.png");
    
    // Apply linear noise (downward gradient)
    let mut png_glitch4 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch4.noise(NoiseConfig {
        intensity: 0.6,
        distribution: NoiseDistribution::Linear { direction: Direction::Down },
        region: None,
        preserve_structure: true,
    })?;
    png_glitch4.save("./noise_linear.png")?;
    println!("Saved noise_linear.png");
    
    // Apply random noise with region targeting
    let mut png_glitch5 = PngGlitch::open("./etc/sample00.png")?;
    let region = Region::new(100, 100, 300, 200);
    png_glitch5.noise(NoiseConfig {
        intensity: 0.8,
        distribution: NoiseDistribution::Random,
        region: Some(region),
        preserve_structure: false,
    })?;
    png_glitch5.save("./noise_random_region.png")?;
    println!("Saved noise_random_region.png");
    
    // Combine noise with scanlines
    let mut png_glitch6 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch6
        .noise(NoiseConfig {
            intensity: 0.3,
            distribution: NoiseDistribution::Uniform,
            region: None,
            preserve_structure: true,
        })?
        .scanlines_default()?;
    png_glitch6.save("./noise_scanlines_combo.png")?;
    println!("Saved noise_scanlines_combo.png");
    
    println!("All noise preset demos completed successfully!");
    
    Ok(())
}