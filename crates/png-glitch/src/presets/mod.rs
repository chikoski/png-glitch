//! High-level preset API for common glitch art effects
//! 
//! This module provides easy-to-use preset methods that build upon the low-level
//! scanline manipulation capabilities to create popular glitch art styles.

pub mod config;
pub mod error;
pub mod scanline_effects;
pub mod noise_effects;
pub mod color_effects;
pub mod degradation_effects;
pub mod sorting_effects;

use crate::PngGlitch;
pub use config::*;
pub use error::*;

/// Trait providing high-level preset methods for creating common glitch effects
pub trait GlitchPresets {
    // Scanline effects (Priority 1)
    fn scanlines(&mut self, config: ScanlineConfig) -> Result<&mut Self, PresetError>;
    fn scanlines_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // Noise injection (Priority 2)  
    fn noise(&mut self, config: NoiseConfig) -> Result<&mut Self, PresetError>;
    fn noise_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // RGB separation (Priority 3)
    fn rgb_shift(&mut self, config: RgbShiftConfig) -> Result<&mut Self, PresetError>;
    fn rgb_shift_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // Digital degradation (Priority 4)
    fn degrade(&mut self, config: DegradationConfig) -> Result<&mut Self, PresetError>;
    fn degrade_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // Pixel sorting (Priority 5)
    fn pixel_sort(&mut self, config: PixelSortConfig) -> Result<&mut Self, PresetError>;
    fn pixel_sort_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // Datamoshing (Priority 6)
    fn datamosh(&mut self, config: DatamoshConfig) -> Result<&mut Self, PresetError>;
    fn datamosh_default(&mut self) -> Result<&mut Self, PresetError>;
    
    // Block corruption (Priority 7)
    fn corrupt_blocks(&mut self, config: BlockCorruptionConfig) -> Result<&mut Self, PresetError>;
    fn corrupt_blocks_default(&mut self) -> Result<&mut Self, PresetError>;
}

impl GlitchPresets for PngGlitch {
    fn scanlines(&mut self, config: ScanlineConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        scanline_effects::apply_scanlines(self, config)?;
        Ok(self)
    }
    
    fn scanlines_default(&mut self) -> Result<&mut Self, PresetError> {
        self.scanlines(ScanlineConfig::default())
    }
    
    fn noise(&mut self, config: NoiseConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        noise_effects::apply_noise(self, config)?;
        Ok(self)
    }
    
    fn noise_default(&mut self) -> Result<&mut Self, PresetError> {
        self.noise(NoiseConfig::default())
    }
    
    fn rgb_shift(&mut self, config: RgbShiftConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        color_effects::apply_rgb_shift(self, config)?;
        Ok(self)
    }
    
    fn rgb_shift_default(&mut self) -> Result<&mut Self, PresetError> {
        self.rgb_shift(RgbShiftConfig::default())
    }
    
    fn degrade(&mut self, config: DegradationConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        degradation_effects::apply_degradation(self, config)?;
        Ok(self)
    }
    
    fn degrade_default(&mut self) -> Result<&mut Self, PresetError> {
        self.degrade(DegradationConfig::default())
    }
    
    fn pixel_sort(&mut self, config: PixelSortConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        sorting_effects::apply_pixel_sort(self, config)?;
        Ok(self)
    }
    
    fn pixel_sort_default(&mut self) -> Result<&mut Self, PresetError> {
        self.pixel_sort(PixelSortConfig::default())
    }
    
    fn datamosh(&mut self, config: DatamoshConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        // Implementation will be added in subsequent tasks
        todo!("Datamosh preset implementation")
    }
    
    fn datamosh_default(&mut self) -> Result<&mut Self, PresetError> {
        self.datamosh(DatamoshConfig::default())
    }
    
    fn corrupt_blocks(&mut self, config: BlockCorruptionConfig) -> Result<&mut Self, PresetError> {
        config.validate()?;
        // Implementation will be added in subsequent tasks
        todo!("Block corruption preset implementation")
    }
    
    fn corrupt_blocks_default(&mut self) -> Result<&mut Self, PresetError> {
        self.corrupt_blocks(BlockCorruptionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        // Test ScanlineConfig validation
        let valid_config = ScanlineConfig::default();
        assert!(valid_config.validate().is_ok());
        
        let invalid_config = ScanlineConfig {
            intensity: 1.5, // Invalid: > 1.0
            ..ScanlineConfig::default()
        };
        assert!(invalid_config.validate().is_err());
        
        // Test NoiseConfig validation
        let valid_noise = NoiseConfig::default();
        assert!(valid_noise.validate().is_ok());
        
        let invalid_noise = NoiseConfig {
            intensity: -0.1, // Invalid: < 0.0
            ..NoiseConfig::default()
        };
        assert!(invalid_noise.validate().is_err());
    }
    
    #[test]
    fn test_default_configs() {
        // Ensure all default configurations are valid
        assert!(ScanlineConfig::default().validate().is_ok());
        assert!(NoiseConfig::default().validate().is_ok());
        assert!(RgbShiftConfig::default().validate().is_ok());
        assert!(DegradationConfig::default().validate().is_ok());
        assert!(PixelSortConfig::default().validate().is_ok());
        assert!(DatamoshConfig::default().validate().is_ok());
        assert!(BlockCorruptionConfig::default().validate().is_ok());
    }
    
    #[test]
    fn test_error_types() {
        let error = PresetError::intensity_out_of_range(1.5);
        assert!(matches!(error, PresetError::IntensityOutOfRange { value: 1.5 }));
        
        let region = Region::new(10, 10, 100, 100);
        let error = PresetError::region_out_of_bounds(region);
        assert!(matches!(error, PresetError::RegionOutOfBounds { .. }));
    }
} 
   
    #[test]
    fn test_scanline_preset_integration() {
        // This test would require a sample PNG file, so we'll test the configuration flow
        let config = ScanlineConfig {
            intensity: 0.5,
            spacing: 2,
            color: ScanlineColor::Custom(255, 0, 0),
            region: Some(Region::new(10, 10, 50, 50)),
        };
        
        // Test that configuration validation passes
        assert!(config.validate().is_ok());
        
        // Test default configuration
        let default_config = ScanlineConfig::default();
        assert!(default_config.validate().is_ok());
        assert_eq!(default_config.intensity, 0.3);
        assert_eq!(default_config.spacing, 4);
    }
    
    #[test]
    fn test_scanline_patterns() {
        use crate::presets::{ScanlineColor, ScanlinePattern};
        
        // Test different scanline patterns
        let patterns = vec![
            ScanlinePattern::Solid,
            ScanlinePattern::Dashed,
            ScanlinePattern::Dotted,
            ScanlinePattern::Noise,
        ];
        
        for pattern in patterns {
            let config = ScanlineConfig {
                color: ScanlineColor::Pattern(pattern),
                ..ScanlineConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_scanline_colors() {
        let colors = vec![
            ScanlineColor::Black,
            ScanlineColor::White,
            ScanlineColor::Transparent,
            ScanlineColor::Custom(128, 64, 192),
            ScanlineColor::Pattern(ScanlinePattern::Solid),
        ];
        
        for color in colors {
            let config = ScanlineConfig {
                color,
                ..ScanlineConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_scanline_intensity_validation() {
        // Test valid intensities
        for intensity in [0.0, 0.5, 1.0] {
            let config = ScanlineConfig {
                intensity,
                ..ScanlineConfig::default()
            };
            assert!(config.validate().is_ok());
        }
        
        // Test invalid intensities
        for intensity in [-0.1, 1.1, 2.0] {
            let config = ScanlineConfig {
                intensity,
                ..ScanlineConfig::default()
            };
            assert!(config.validate().is_err());
        }
    }
    
    #[test]
    fn test_scanline_spacing_validation() {
        // Test valid spacing
        let config = ScanlineConfig {
            spacing: 1,
            ..ScanlineConfig::default()
        };
        assert!(config.validate().is_ok());
        
        // Test invalid spacing (zero)
        let config = ScanlineConfig {
            spacing: 0,
            ..ScanlineConfig::default()
        };
        assert!(config.validate().is_err());
    } 
   
    #[test]
    fn test_scanline_preset_with_real_image() {
        use std::env;
        
        // Set working directory for test
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        // Test with actual PNG file
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test default scanlines
            let result = png_glitch.scanlines_default();
            assert!(result.is_ok());
            
            // Test custom scanlines
            let config = ScanlineConfig {
                intensity: 0.7,
                spacing: 3,
                color: ScanlineColor::Custom(255, 0, 0),
                region: None,
            };
            
            let result = png_glitch.scanlines(config);
            assert!(result.is_ok());
            
            // Test method chaining
            let result = png_glitch
                .scanlines_default()
                .and_then(|png| png.scanlines(ScanlineConfig {
                    intensity: 0.2,
                    spacing: 8,
                    color: ScanlineColor::White,
                    region: Some(Region::new(0, 0, 100, 100)),
                }));
            
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_scanline_region_bounds_validation() {
        use std::env;
        
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            let width = png_glitch.width();
            let height = png_glitch.height();
            
            // Test valid region
            let valid_config = ScanlineConfig {
                region: Some(Region::new(0, 0, width / 2, height / 2)),
                ..ScanlineConfig::default()
            };
            
            let result = png_glitch.scanlines(valid_config);
            assert!(result.is_ok());
            
            // Test invalid region (out of bounds)
            let invalid_config = ScanlineConfig {
                region: Some(Region::new(width, height, 100, 100)),
                ..ScanlineConfig::default()
            };
            
            let result = png_glitch.scanlines(invalid_config);
            assert!(result.is_err());
            
            if let Err(PresetError::RegionOutOfBounds { .. }) = result {
                // Expected error type
            } else {
                panic!("Expected RegionOutOfBounds error");
            }
        }
    }    
   
 #[test]
    fn test_noise_preset_integration() {
        // Test noise configuration flow
        let config = NoiseConfig {
            intensity: 0.3,
            distribution: NoiseDistribution::Uniform,
            region: Some(Region::new(20, 20, 100, 80)),
            preserve_structure: true,
        };
        
        // Test that configuration validation passes
        assert!(config.validate().is_ok());
        
        // Test default configuration
        let default_config = NoiseConfig::default();
        assert!(default_config.validate().is_ok());
        assert_eq!(default_config.intensity, 0.2);
        assert!(default_config.preserve_structure);
    }
    
    #[test]
    fn test_noise_distributions() {
        use crate::presets::{Direction, NoiseDistribution};
        
        // Test different noise distributions
        let distributions = vec![
            NoiseDistribution::Uniform,
            NoiseDistribution::Clustered { cluster_size: 8 },
            NoiseDistribution::Linear { direction: Direction::Down },
            NoiseDistribution::Random,
        ];
        
        for distribution in distributions {
            let config = NoiseConfig {
                distribution,
                ..NoiseConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_noise_intensity_validation() {
        // Test valid intensities
        for intensity in [0.0, 0.5, 1.0] {
            let config = NoiseConfig {
                intensity,
                ..NoiseConfig::default()
            };
            assert!(config.validate().is_ok());
        }
        
        // Test invalid intensities
        for intensity in [-0.1, 1.1, 2.0] {
            let config = NoiseConfig {
                intensity,
                ..NoiseConfig::default()
            };
            assert!(config.validate().is_err());
        }
    }
    
    #[test]
    fn test_noise_preset_with_real_image() {
        use std::env;
        
        // Set working directory for test
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        // Test with actual PNG file
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test default noise
            let result = png_glitch.noise_default();
            assert!(result.is_ok());
            
            // Test custom noise with different distributions
            let config = NoiseConfig {
                intensity: 0.4,
                distribution: NoiseDistribution::Clustered { cluster_size: 12 },
                region: None,
                preserve_structure: false,
            };
            
            let result = png_glitch.noise(config);
            assert!(result.is_ok());
            
            // Test method chaining with scanlines
            let result = png_glitch
                .noise_default()
                .and_then(|png| png.scanlines_default());
            
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_noise_region_bounds_validation() {
        use std::env;
        
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            let width = png_glitch.width();
            let height = png_glitch.height();
            
            // Test valid region
            let valid_config = NoiseConfig {
                region: Some(Region::new(0, 0, width / 3, height / 3)),
                ..NoiseConfig::default()
            };
            
            let result = png_glitch.noise(valid_config);
            assert!(result.is_ok());
            
            // Test invalid region (out of bounds)
            let invalid_config = NoiseConfig {
                region: Some(Region::new(width, height, 50, 50)),
                ..NoiseConfig::default()
            };
            
            let result = png_glitch.noise(invalid_config);
            assert!(result.is_err());
            
            if let Err(PresetError::RegionOutOfBounds { .. }) = result {
                // Expected error type
            } else {
                panic!("Expected RegionOutOfBounds error");
            }
        }
    }    

    #[test]
    fn test_rgb_shift_preset_integration() {
        // Test RGB shift configuration flow
        let config = RgbShiftConfig {
            red_offset: (3, 0),
            green_offset: (0, 0),
            blue_offset: (-3, 0),
            intensity: 0.9,
            wrap_mode: WrapMode::Clamp,
        };
        
        // Test that configuration validation passes
        assert!(config.validate().is_ok());
        
        // Test default configuration
        let default_config = RgbShiftConfig::default();
        assert!(default_config.validate().is_ok());
        assert_eq!(default_config.red_offset, (2, 0));
        assert_eq!(default_config.green_offset, (0, 0));
        assert_eq!(default_config.blue_offset, (-2, 0));
        assert_eq!(default_config.intensity, 0.8);
    }
    
    #[test]
    fn test_rgb_shift_wrap_modes() {
        use crate::presets::WrapMode;
        
        // Test different wrap modes
        let wrap_modes = vec![
            WrapMode::Clamp,
            WrapMode::Wrap,
            WrapMode::Mirror,
        ];
        
        for wrap_mode in wrap_modes {
            let config = RgbShiftConfig {
                wrap_mode,
                ..RgbShiftConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_rgb_shift_intensity_validation() {
        // Test valid intensities
        for intensity in [0.0, 0.5, 1.0] {
            let config = RgbShiftConfig {
                intensity,
                ..RgbShiftConfig::default()
            };
            assert!(config.validate().is_ok());
        }
        
        // Test invalid intensities
        for intensity in [-0.1, 1.1, 2.0] {
            let config = RgbShiftConfig {
                intensity,
                ..RgbShiftConfig::default()
            };
            assert!(config.validate().is_err());
        }
    }
    
    #[test]
    fn test_rgb_shift_preset_with_real_image() {
        use std::env;
        
        // Set working directory for test
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        // Test with actual PNG file
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test default RGB shift
            let result = png_glitch.rgb_shift_default();
            assert!(result.is_ok());
            
            // Test custom RGB shift with different wrap modes
            let config = RgbShiftConfig {
                red_offset: (5, 0),
                green_offset: (0, 2),
                blue_offset: (-3, -1),
                intensity: 0.7,
                wrap_mode: WrapMode::Wrap,
            };
            
            let result = png_glitch.rgb_shift(config);
            assert!(result.is_ok());
            
            // Test method chaining with other effects
            let result = png_glitch
                .rgb_shift_default()
                .and_then(|png| png.noise_default())
                .and_then(|png| png.scanlines_default());
            
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_rgb_shift_offset_validation() {
        use std::env;
        
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            let width = png_glitch.width();
            let height = png_glitch.height();
            
            // Test valid offsets
            let valid_config = RgbShiftConfig {
                red_offset: (5, 0),
                green_offset: (0, 5),
                blue_offset: (-5, -5),
                intensity: 0.8,
                wrap_mode: WrapMode::Clamp,
            };
            
            let result = png_glitch.rgb_shift(valid_config);
            assert!(result.is_ok());
            
            // Test invalid offsets (too large)
            let invalid_config = RgbShiftConfig {
                red_offset: (width as i32, 0),
                green_offset: (0, height as i32),
                blue_offset: (0, 0),
                intensity: 0.8,
                wrap_mode: WrapMode::Clamp,
            };
            
            let result = png_glitch.rgb_shift(invalid_config);
            assert!(result.is_err());
            
            if let Err(PresetError::InvalidOffset { .. }) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidOffset error");
            }
        }
    }    
  
  #[test]
    fn test_degradation_preset_integration() {
        // Test degradation configuration flow
        let config = DegradationConfig {
            level: 0.5,
            artifacts: vec![
                ArtifactType::Blocking { block_size: 8 },
                ArtifactType::Banding { bands: 16 },
            ],
            preserve_readability: true,
        };
        
        // Test that configuration validation passes
        assert!(config.validate().is_ok());
        
        // Test default configuration
        let default_config = DegradationConfig::default();
        assert!(default_config.validate().is_ok());
        assert_eq!(default_config.level, 0.3);
        assert!(default_config.preserve_readability);
    }
    
    #[test]
    fn test_degradation_artifact_types() {
        use crate::presets::ArtifactType;
        
        // Test different artifact types
        let artifacts = vec![
            ArtifactType::Blocking { block_size: 8 },
            ArtifactType::Banding { bands: 16 },
            ArtifactType::Compression { quality: 0.5 },
            ArtifactType::Posterization { levels: 8 },
        ];
        
        for artifact in artifacts {
            let config = DegradationConfig {
                artifacts: vec![artifact],
                ..DegradationConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_degradation_level_validation() {
        // Test valid levels
        for level in [0.0, 0.5, 1.0] {
            let config = DegradationConfig {
                level,
                ..DegradationConfig::default()
            };
            assert!(config.validate().is_ok());
        }
        
        // Test invalid levels
        for level in [-0.1, 1.1, 2.0] {
            let config = DegradationConfig {
                level,
                ..DegradationConfig::default()
            };
            assert!(config.validate().is_err());
        }
    }
    
    #[test]
    fn test_degradation_preset_with_real_image() {
        use std::env;
        
        // Set working directory for test
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        // Test with actual PNG file
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test default degradation
            let result = png_glitch.degrade_default();
            assert!(result.is_ok());
            
            // Test custom degradation with different artifacts
            let config = DegradationConfig {
                level: 0.6,
                artifacts: vec![
                    ArtifactType::Blocking { block_size: 16 },
                    ArtifactType::Posterization { levels: 8 },
                ],
                preserve_readability: false,
            };
            
            let result = png_glitch.degrade(config);
            assert!(result.is_ok());
            
            // Test method chaining with other effects
            let result = png_glitch
                .degrade_default()
                .and_then(|png| png.rgb_shift_default())
                .and_then(|png| png.noise_default());
            
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_degradation_preserve_readability() {
        use std::env;
        
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test with readability preservation
            let preserve_config = DegradationConfig {
                level: 0.8,
                artifacts: vec![ArtifactType::Posterization { levels: 4 }],
                preserve_readability: true,
            };
            
            let result = png_glitch.degrade(preserve_config);
            assert!(result.is_ok());
            
            // Test without readability preservation
            let no_preserve_config = DegradationConfig {
                level: 0.8,
                artifacts: vec![ArtifactType::Posterization { levels: 4 }],
                preserve_readability: false,
            };
            
            let result = png_glitch.degrade(no_preserve_config);
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_pixel_sort_preset_integration() {
        // Test pixel sort configuration flow
        let config = PixelSortConfig {
            direction: SortDirection::Horizontal,
            criteria: SortCriteria::Brightness,
            region: Some(Region::new(10, 10, 100, 100)),
            threshold: Some(0.3),
        };
        
        // Test that configuration validation passes
        assert!(config.validate().is_ok());
        
        // Test default configuration
        let default_config = PixelSortConfig::default();
        assert!(default_config.validate().is_ok());
        assert_eq!(default_config.direction, SortDirection::Horizontal);
        assert_eq!(default_config.criteria, SortCriteria::Brightness);
        assert_eq!(default_config.threshold, Some(0.5));
    }
    
    #[test]
    fn test_pixel_sort_directions() {
        use crate::presets::SortDirection;
        
        // Test different sort directions
        let directions = vec![
            SortDirection::Horizontal,
            SortDirection::Vertical,
            SortDirection::Diagonal,
            SortDirection::Radial { center: (50, 50) },
        ];
        
        for direction in directions {
            let config = PixelSortConfig {
                direction,
                ..PixelSortConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_pixel_sort_criteria() {
        use crate::presets::SortCriteria;
        
        // Test different sort criteria
        let criteria = vec![
            SortCriteria::Brightness,
            SortCriteria::Hue,
            SortCriteria::Saturation,
            SortCriteria::Red,
            SortCriteria::Green,
            SortCriteria::Blue,
        ];
        
        for criterion in criteria {
            let config = PixelSortConfig {
                criteria: criterion,
                ..PixelSortConfig::default()
            };
            assert!(config.validate().is_ok());
        }
    }
    
    #[test]
    fn test_pixel_sort_threshold_validation() {
        // Test valid thresholds
        for threshold in [0.0, 0.5, 1.0] {
            let config = PixelSortConfig {
                threshold: Some(threshold),
                ..PixelSortConfig::default()
            };
            assert!(config.validate().is_ok());
        }
        
        // Test invalid thresholds
        for threshold in [-0.1, 1.1, 2.0] {
            let config = PixelSortConfig {
                threshold: Some(threshold),
                ..PixelSortConfig::default()
            };
            assert!(config.validate().is_err());
        }
        
        // Test no threshold
        let config = PixelSortConfig {
            threshold: None,
            ..PixelSortConfig::default()
        };
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_pixel_sort_preset_with_real_image() {
        use std::env;
        
        // Set working directory for test
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        // Test with actual PNG file
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            // Test default pixel sort
            let result = png_glitch.pixel_sort_default();
            assert!(result.is_ok());
            
            // Test custom pixel sort with different directions
            let config = PixelSortConfig {
                direction: SortDirection::Vertical,
                criteria: SortCriteria::Hue,
                region: None,
                threshold: Some(0.3),
            };
            
            let result = png_glitch.pixel_sort(config);
            assert!(result.is_ok());
            
            // Test method chaining with other effects
            let result = png_glitch
                .pixel_sort_default()
                .and_then(|png| png.rgb_shift_default())
                .and_then(|png| png.noise_default());
            
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_pixel_sort_region_bounds_validation() {
        use std::env;
        
        let _ = env::set_current_dir(env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string()));
        
        if let Ok(mut png_glitch) = crate::PngGlitch::open("./etc/sample00.png") {
            let width = png_glitch.width();
            let height = png_glitch.height();
            
            // Test valid region
            let valid_config = PixelSortConfig {
                region: Some(Region::new(0, 0, width / 2, height / 2)),
                ..PixelSortConfig::default()
            };
            
            let result = png_glitch.pixel_sort(valid_config);
            assert!(result.is_ok());
            
            // Test invalid region (out of bounds)
            let invalid_config = PixelSortConfig {
                region: Some(Region::new(width, height, 100, 100)),
                ..PixelSortConfig::default()
            };
            
            let result = png_glitch.pixel_sort(invalid_config);
            assert!(result.is_err());
            
            if let Err(PresetError::RegionOutOfBounds { .. }) = result {
                // Expected error type
            } else {
                panic!("Expected RegionOutOfBounds error");
            }
        }
    }