use png_glitch::{ArtifactType, DegradationConfig, GlitchPresets, PngGlitch};

fn main() -> anyhow::Result<()> {
    // Load the sample image
    let mut png_glitch = PngGlitch::open("./etc/sample00.png")?;
    
    println!("Original image size: {}x{}", png_glitch.width(), png_glitch.height());
    
    // Apply default degradation
    png_glitch.degrade_default()?;
    png_glitch.save("./degradation_default.png")?;
    println!("Saved degradation_default.png");
    
    // Apply blocking artifacts (DCT compression simulation)
    let mut png_glitch2 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch2.degrade(DegradationConfig {
        level: 0.7,
        artifacts: vec![ArtifactType::Blocking { block_size: 8 }],
        preserve_readability: true,
    })?;
    png_glitch2.save("./degradation_blocking.png")?;
    println!("Saved degradation_blocking.png");
    
    // Apply banding artifacts (quantization)
    let mut png_glitch3 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch3.degrade(DegradationConfig {
        level: 0.8,
        artifacts: vec![ArtifactType::Banding { bands: 8 }],
        preserve_readability: true,
    })?;
    png_glitch3.save("./degradation_banding.png")?;
    println!("Saved degradation_banding.png");
    
    // Apply compression artifacts
    let mut png_glitch4 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch4.degrade(DegradationConfig {
        level: 0.6,
        artifacts: vec![ArtifactType::Compression { quality: 0.3 }],
        preserve_readability: true,
    })?;
    png_glitch4.save("./degradation_compression.png")?;
    println!("Saved degradation_compression.png");
    
    // Apply posterization (color depth reduction)
    let mut png_glitch5 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch5.degrade(DegradationConfig {
        level: 0.9,
        artifacts: vec![ArtifactType::Posterization { levels: 6 }],
        preserve_readability: false,
    })?;
    png_glitch5.save("./degradation_posterization.png")?;
    println!("Saved degradation_posterization.png");
    
    // Apply multiple artifacts combined
    let mut png_glitch6 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch6.degrade(DegradationConfig {
        level: 0.5,
        artifacts: vec![
            ArtifactType::Blocking { block_size: 16 },
            ArtifactType::Banding { bands: 12 },
            ArtifactType::Posterization { levels: 8 },
        ],
        preserve_readability: true,
    })?;
    png_glitch6.save("./degradation_combined.png")?;
    println!("Saved degradation_combined.png");
    
    // Combine degradation with other effects
    let mut png_glitch7 = PngGlitch::open("./etc/sample00.png")?;
    png_glitch7
        .degrade(DegradationConfig {
            level: 0.4,
            artifacts: vec![
                ArtifactType::Blocking { block_size: 8 },
                ArtifactType::Compression { quality: 0.6 },
            ],
            preserve_readability: true,
        })?
        .rgb_shift_default()?
        .noise_default()?;
    png_glitch7.save("./degradation_combo.png")?;
    println!("Saved degradation_combo.png");
    
    println!("All degradation preset demos completed successfully!");
    
    Ok(())
}