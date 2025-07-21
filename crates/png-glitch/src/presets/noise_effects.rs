use crate::presets::{Direction, NoiseConfig, NoiseDistribution, PresetError, Region};
use crate::png::ColorType;
use crate::{PngGlitch, ScanLine};

/// Apply noise injection effects to the PNG image
pub fn apply_noise(png_glitch: &mut PngGlitch, config: NoiseConfig) -> Result<(), PresetError> {
    let width = png_glitch.width();
    let height = png_glitch.height();
    
    // Determine the region to apply the effect
    let region = config.region.unwrap_or_else(|| Region::full_image(width, height));
    
    // Validate region bounds
    if region.x + region.width > width || region.y + region.height > height {
        return Err(PresetError::region_out_of_bounds(region));
    }
    
    // Apply noise based on distribution pattern
    match &config.distribution {
        NoiseDistribution::Uniform => {
            apply_uniform_noise(png_glitch, &config, &region)
        }
        NoiseDistribution::Clustered { cluster_size } => {
            apply_clustered_noise(png_glitch, &config, &region, *cluster_size)
        }
        NoiseDistribution::Linear { direction } => {
            apply_linear_noise(png_glitch, &config, &region, direction)
        }
        NoiseDistribution::Random => {
            apply_random_noise(png_glitch, &config, &region)
        }
    }
}

/// Apply uniform noise distribution across the region
fn apply_uniform_noise(
    png_glitch: &mut PngGlitch,
    config: &NoiseConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    // Create a pseudo-random seed based on region
    let mut seed = region.x.wrapping_mul(31).wrapping_add(region.y.wrapping_mul(17));
    
    for line_index in start_line..end_line {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            // Determine if this line should have noise based on intensity
            seed = next_random(seed);
            let line_probability = seed as f32 / u32::MAX as f32;
            
            if line_probability < config.intensity {
                apply_noise_to_scanline(&mut scanline, config, region, &mut seed)?;
            }
        }
    }
    
    Ok(())
}

/// Apply clustered noise distribution
fn apply_clustered_noise(
    png_glitch: &mut PngGlitch,
    config: &NoiseConfig,
    region: &Region,
    cluster_size: u32,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    let mut seed = region.x.wrapping_mul(31).wrapping_add(region.y.wrapping_mul(17));
    
    // Generate cluster centers
    let num_clusters = ((region.width * region.height) as f32 * config.intensity / (cluster_size * cluster_size) as f32) as u32;
    let mut cluster_centers = Vec::new();
    
    for _ in 0..num_clusters {
        seed = next_random(seed);
        let cluster_x = region.x + (seed % region.width);
        seed = next_random(seed);
        let cluster_y = region.y + (seed % region.height);
        cluster_centers.push((cluster_x, cluster_y));
    }
    
    // Apply noise around cluster centers
    for line_index in start_line..end_line {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            for &(cluster_x, cluster_y) in &cluster_centers {
                let distance_y = (line_index as i32 - cluster_y as i32).abs() as u32;
                if distance_y <= cluster_size / 2 {
                    apply_clustered_noise_to_scanline(&mut scanline, config, region, cluster_x, cluster_size, &mut seed)?;
                }
            }
        }
    }
    
    Ok(())
}

/// Apply linear noise distribution in a specific direction
fn apply_linear_noise(
    png_glitch: &mut PngGlitch,
    config: &NoiseConfig,
    region: &Region,
    direction: &Direction,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    let mut seed = region.x.wrapping_mul(31).wrapping_add(region.y.wrapping_mul(17));
    
    for line_index in start_line..end_line {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            // Calculate noise intensity based on direction and position
            let line_intensity = calculate_linear_intensity(line_index, region, direction, config.intensity);
            
            if line_intensity > 0.0 {
                let line_config = NoiseConfig {
                    intensity: line_intensity,
                    ..config.clone()
                };
                apply_noise_to_scanline(&mut scanline, &line_config, region, &mut seed)?;
            }
        }
    }
    
    Ok(())
}

/// Apply random noise distribution with varying density
fn apply_random_noise(
    png_glitch: &mut PngGlitch,
    config: &NoiseConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    let mut seed = region.x.wrapping_mul(31).wrapping_add(region.y.wrapping_mul(17));
    
    for line_index in start_line..end_line {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            // Random intensity variation for each line
            seed = next_random(seed);
            let intensity_variation = (seed as f32 / u32::MAX as f32) * 0.5 + 0.5; // 0.5 to 1.0
            let line_intensity = config.intensity * intensity_variation;
            
            seed = next_random(seed);
            let line_probability = seed as f32 / u32::MAX as f32;
            
            if line_probability < line_intensity {
                let line_config = NoiseConfig {
                    intensity: line_intensity,
                    ..config.clone()
                };
                apply_noise_to_scanline(&mut scanline, &line_config, region, &mut seed)?;
            }
        }
    }
    
    Ok(())
}

/// Apply noise to a specific scanline
fn apply_noise_to_scanline(
    scanline: &mut ScanLine,
    config: &NoiseConfig,
    region: &Region,
    seed: &mut u32,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        // Determine if this pixel should be corrupted
        *seed = next_random(*seed);
        let pixel_probability = *seed as f32 / u32::MAX as f32;
        
        if pixel_probability < config.intensity {
            apply_noise_to_pixel(scanline, pixel_start, config, color_type, seed)?;
        }
    }
    
    Ok(())
}

/// Apply clustered noise to a scanline around a cluster center
fn apply_clustered_noise_to_scanline(
    scanline: &mut ScanLine,
    config: &NoiseConfig,
    region: &Region,
    cluster_x: u32,
    cluster_size: u32,
    seed: &mut u32,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let cluster_start_x = cluster_x.saturating_sub(cluster_size / 2).max(region.x);
    let cluster_end_x = (cluster_x + cluster_size / 2).min(region.x + region.width);
    
    let start_x = cluster_start_x as usize * bytes_per_pixel;
    let end_x = (cluster_end_x as usize * bytes_per_pixel).min(scanline.size());
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        // Higher probability near cluster center
        let pixel_x = pixel_start / bytes_per_pixel;
        let distance_from_center = (pixel_x as i32 - cluster_x as i32).abs() as u32;
        let cluster_intensity = config.intensity * (1.0 - (distance_from_center as f32 / (cluster_size as f32 / 2.0))).max(0.0);
        
        *seed = next_random(*seed);
        let pixel_probability = *seed as f32 / u32::MAX as f32;
        
        if pixel_probability < cluster_intensity {
            apply_noise_to_pixel(scanline, pixel_start, config, color_type, seed)?;
        }
    }
    
    Ok(())
}

/// Apply noise corruption to a specific pixel
fn apply_noise_to_pixel(
    scanline: &mut ScanLine,
    pixel_start: usize,
    config: &NoiseConfig,
    color_type: ColorType,
    seed: &mut u32,
) -> Result<(), PresetError> {
    match color_type {
        ColorType::TrueColor => {
            // RGB format
            if pixel_start + 2 < scanline.size() {
                if config.preserve_structure {
                    // Preserve some structure by adding controlled noise
                    apply_structured_noise_rgb(scanline, pixel_start, seed);
                } else {
                    // Complete corruption
                    apply_random_noise_rgb(scanline, pixel_start, seed);
                }
            }
        }
        ColorType::TrueColorAlpha => {
            // RGBA format
            if pixel_start + 3 < scanline.size() {
                if config.preserve_structure {
                    apply_structured_noise_rgb(scanline, pixel_start, seed);
                } else {
                    apply_random_noise_rgb(scanline, pixel_start, seed);
                }
                // Keep alpha channel unchanged
            }
        }
        ColorType::GrayScale => {
            // Grayscale format
            if pixel_start < scanline.size() {
                if config.preserve_structure {
                    apply_structured_noise_gray(scanline, pixel_start, seed);
                } else {
                    apply_random_noise_gray(scanline, pixel_start, seed);
                }
            }
        }
        ColorType::GrayScaleAlpha => {
            // Grayscale with alpha
            if pixel_start + 1 < scanline.size() {
                if config.preserve_structure {
                    apply_structured_noise_gray(scanline, pixel_start, seed);
                } else {
                    apply_random_noise_gray(scanline, pixel_start, seed);
                }
                // Keep alpha channel unchanged
            }
        }
        ColorType::IndexColor => {
            // Indexed color - modify the index
            if pixel_start < scanline.size() {
                *seed = next_random(*seed);
                let noise_value = (*seed & 0xFF) as u8;
                if config.preserve_structure {
                    let original = scanline.index(pixel_start).unwrap_or(0);
                    let corrupted = original.wrapping_add(noise_value.wrapping_div(4));
                    scanline.update(pixel_start, corrupted);
                } else {
                    scanline.update(pixel_start, noise_value);
                }
            }
        }
    }
    
    Ok(())
}

/// Apply structured noise to RGB pixels (preserves some original data)
fn apply_structured_noise_rgb(scanline: &mut ScanLine, pixel_start: usize, seed: &mut u32) {
    let original_r = scanline.index(pixel_start).unwrap_or(0);
    let original_g = scanline.index(pixel_start + 1).unwrap_or(0);
    let original_b = scanline.index(pixel_start + 2).unwrap_or(0);
    
    // Generate noise values
    *seed = next_random(*seed);
    let noise_r = ((*seed >> 16) & 0x3F) as u8; // 6-bit noise
    *seed = next_random(*seed);
    let noise_g = ((*seed >> 16) & 0x3F) as u8;
    *seed = next_random(*seed);
    let noise_b = ((*seed >> 16) & 0x3F) as u8;
    
    // Apply noise while preserving structure
    let new_r = original_r.wrapping_add(noise_r).wrapping_sub(32);
    let new_g = original_g.wrapping_add(noise_g).wrapping_sub(32);
    let new_b = original_b.wrapping_add(noise_b).wrapping_sub(32);
    
    scanline.update(pixel_start, new_r);
    scanline.update(pixel_start + 1, new_g);
    scanline.update(pixel_start + 2, new_b);
}

/// Apply random noise to RGB pixels (complete corruption)
fn apply_random_noise_rgb(scanline: &mut ScanLine, pixel_start: usize, seed: &mut u32) {
    *seed = next_random(*seed);
    let noise_r = ((*seed >> 16) & 0xFF) as u8;
    *seed = next_random(*seed);
    let noise_g = ((*seed >> 16) & 0xFF) as u8;
    *seed = next_random(*seed);
    let noise_b = ((*seed >> 16) & 0xFF) as u8;
    
    scanline.update(pixel_start, noise_r);
    scanline.update(pixel_start + 1, noise_g);
    scanline.update(pixel_start + 2, noise_b);
}

/// Apply structured noise to grayscale pixels
fn apply_structured_noise_gray(scanline: &mut ScanLine, pixel_start: usize, seed: &mut u32) {
    let original = scanline.index(pixel_start).unwrap_or(0);
    
    *seed = next_random(*seed);
    let noise = ((*seed >> 16) & 0x3F) as u8; // 6-bit noise
    
    let new_value = original.wrapping_add(noise).wrapping_sub(32);
    scanline.update(pixel_start, new_value);
}

/// Apply random noise to grayscale pixels
fn apply_random_noise_gray(scanline: &mut ScanLine, pixel_start: usize, seed: &mut u32) {
    *seed = next_random(*seed);
    let noise = ((*seed >> 16) & 0xFF) as u8;
    scanline.update(pixel_start, noise);
}

/// Calculate linear intensity based on direction and position
fn calculate_linear_intensity(
    line_index: u32,
    region: &Region,
    direction: &Direction,
    base_intensity: f32,
) -> f32 {
    let relative_y = (line_index - region.y) as f32 / region.height as f32;
    
    match direction {
        Direction::Up => base_intensity * (1.0 - relative_y),
        Direction::Down => base_intensity * relative_y,
        Direction::Left | Direction::Right => base_intensity, // Uniform for horizontal
        Direction::UpLeft | Direction::UpRight => base_intensity * (1.0 - relative_y),
        Direction::DownLeft | Direction::DownRight => base_intensity * relative_y,
    }
}

/// Get a scanline at the specified index
fn get_scanline_at_index(png_glitch: &mut PngGlitch, line_index: u32) -> Option<ScanLine> {
    if line_index < png_glitch.height() {
        let scanlines = png_glitch.scan_lines_from(line_index, 1);
        scanlines.into_iter().next()
    } else {
        None
    }
}

/// Get bytes per pixel for the given color type and bit depth
fn get_bytes_per_pixel(color_type: ColorType, bit_depth: u8) -> usize {
    match color_type {
        ColorType::GrayScale => std::cmp::max(bit_depth / 8, 1) as usize,
        ColorType::GrayScaleAlpha => std::cmp::max(bit_depth * 2 / 8, 1) as usize,
        ColorType::TrueColor => std::cmp::max(bit_depth * 3 / 8, 1) as usize,
        ColorType::TrueColorAlpha => std::cmp::max(bit_depth * 4 / 8, 1) as usize,
        ColorType::IndexColor => (bit_depth / 8) as usize,
    }
}

/// Simple linear congruential generator for pseudo-random numbers
fn next_random(seed: u32) -> u32 {
    seed.wrapping_mul(1103515245).wrapping_add(12345)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{NoiseConfig, NoiseDistribution};
    use crate::presets::config::ConfigValidation;

    #[test]
    fn test_next_random() {
        let seed = 12345;
        let next1 = next_random(seed);
        let next2 = next_random(next1);
        
        // Should generate different values
        assert_ne!(seed, next1);
        assert_ne!(next1, next2);
        
        // Should be deterministic
        assert_eq!(next_random(seed), next1);
    }
    
    #[test]
    fn test_calculate_linear_intensity() {
        let region = Region::new(0, 0, 100, 100);
        
        // Test upward direction (intensity decreases as we go down)
        let intensity_top = calculate_linear_intensity(0, &region, &Direction::Up, 1.0);
        let intensity_bottom = calculate_linear_intensity(99, &region, &Direction::Down, 1.0);
        
        assert!(intensity_top > intensity_bottom);
        
        // Test downward direction (intensity increases as we go down)
        let intensity_top_down = calculate_linear_intensity(0, &region, &Direction::Down, 1.0);
        let intensity_bottom_down = calculate_linear_intensity(99, &region, &Direction::Down, 1.0);
        
        assert!(intensity_bottom_down > intensity_top_down);
    }
    
    #[test]
    fn test_get_bytes_per_pixel() {
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScale, 8), 1);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColor, 8), 3);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColorAlpha, 8), 4);
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScaleAlpha, 8), 2);
    }
    
    #[test]
    fn test_noise_config_validation() {
        let valid_config = NoiseConfig::default();
        assert!(valid_config.validate().is_ok());
        
        let invalid_config = NoiseConfig {
            intensity: 1.5, // Invalid: > 1.0
            ..NoiseConfig::default()
        };
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn test_noise_distributions() {
        // Test that different distributions can be created
        let uniform = NoiseDistribution::Uniform;
        let clustered = NoiseDistribution::Clustered { cluster_size: 10 };
        let linear = NoiseDistribution::Linear { direction: Direction::Up };
        let random = NoiseDistribution::Random;
        
        // These should all be valid distribution types
        let configs = vec![
            NoiseConfig { distribution: uniform, ..NoiseConfig::default() },
            NoiseConfig { distribution: clustered, ..NoiseConfig::default() },
            NoiseConfig { distribution: linear, ..NoiseConfig::default() },
            NoiseConfig { distribution: random, ..NoiseConfig::default() },
        ];
        
        for config in configs {
            assert!(config.validate().is_ok());
        }
    }
}