use crate::presets::{PresetError, Region, ScanlineColor, ScanlineConfig, ScanlinePattern};
use crate::png::ColorType;
use crate::{PngGlitch, ScanLine};

/// Apply scanline effects to the PNG image
pub fn apply_scanlines(png_glitch: &mut PngGlitch, config: ScanlineConfig) -> Result<(), PresetError> {
    let width = png_glitch.width();
    let height = png_glitch.height();
    
    // Determine the region to apply the effect
    let region = config.region.unwrap_or_else(|| Region::full_image(width, height));
    
    // Validate region bounds
    if region.x + region.width > width || region.y + region.height > height {
        return Err(PresetError::region_out_of_bounds(region));
    }
    
    // Apply scanline effect based on configuration
    match &config.color {
        ScanlineColor::Pattern(pattern) => {
            apply_pattern_scanlines(png_glitch, &config, &region, pattern)
        }
        _ => {
            apply_solid_scanlines(png_glitch, &config, &region)
        }
    }
}

/// Apply solid color scanlines
fn apply_solid_scanlines(
    png_glitch: &mut PngGlitch, 
    config: &ScanlineConfig, 
    region: &Region
) -> Result<(), PresetError> {
    let color_values = get_color_values(&config.color)?;
    
    // Calculate which scanlines to affect based on spacing
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    for line_index in (start_line..end_line).step_by(config.spacing as usize) {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            apply_scanline_color(&mut scanline, &color_values, config.intensity, region)?;
        }
    }
    
    Ok(())
}

/// Apply pattern-based scanlines
fn apply_pattern_scanlines(
    png_glitch: &mut PngGlitch,
    config: &ScanlineConfig,
    region: &Region,
    pattern: &ScanlinePattern,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    for line_index in (start_line..end_line).step_by(config.spacing as usize) {
        if let Some(mut scanline) = get_scanline_at_index(png_glitch, line_index) {
            match pattern {
                ScanlinePattern::Solid => {
                    let color_values = get_color_values(&config.color)?;
                    apply_scanline_color(&mut scanline, &color_values, config.intensity, region)?;
                }
                ScanlinePattern::Dashed => {
                    apply_dashed_scanline(&mut scanline, config, region)?;
                }
                ScanlinePattern::Dotted => {
                    apply_dotted_scanline(&mut scanline, config, region)?;
                }
                ScanlinePattern::Noise => {
                    apply_noise_scanline(&mut scanline, config, region)?;
                }
            }
        }
    }
    
    Ok(())
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

/// Convert ScanlineColor to RGB values
fn get_color_values(color: &ScanlineColor) -> Result<(u8, u8, u8), PresetError> {
    match color {
        ScanlineColor::Black => Ok((0, 0, 0)),
        ScanlineColor::White => Ok((255, 255, 255)),
        ScanlineColor::Transparent => Ok((0, 0, 0)), // Will be handled specially
        ScanlineColor::Custom(r, g, b) => Ok((*r, *g, *b)),
        ScanlineColor::Pattern(_) => Ok((0, 0, 0)), // Default for patterns
    }
}

/// Apply color to a scanline with intensity blending
fn apply_scanline_color(
    scanline: &mut ScanLine,
    color_values: &(u8, u8, u8),
    intensity: f32,
    region: &Region,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        apply_color_to_pixel(scanline, pixel_start, color_values, intensity, color_type)?;
    }
    
    Ok(())
}

/// Apply dashed pattern to scanline
fn apply_dashed_scanline(
    scanline: &mut ScanLine,
    config: &ScanlineConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let color_values = get_color_values(&ScanlineColor::Black)?;
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let dash_length = 8; // pixels
    let gap_length = 4;  // pixels
    let pattern_length = dash_length + gap_length;
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        let pixel_index = (pixel_start - start_x) / bytes_per_pixel;
        let pattern_pos = pixel_index % pattern_length;
        
        if pattern_pos < dash_length {
            apply_color_to_pixel(scanline, pixel_start, &color_values, config.intensity, color_type)?;
        }
    }
    
    Ok(())
}

/// Apply dotted pattern to scanline
fn apply_dotted_scanline(
    scanline: &mut ScanLine,
    config: &ScanlineConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let color_values = get_color_values(&ScanlineColor::Black)?;
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let dot_spacing = 6; // pixels between dots
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        let pixel_index = (pixel_start - start_x) / bytes_per_pixel;
        
        if pixel_index % dot_spacing == 0 {
            apply_color_to_pixel(scanline, pixel_start, &color_values, config.intensity, color_type)?;
        }
    }
    
    Ok(())
}

/// Apply noise pattern to scanline
fn apply_noise_scanline(
    scanline: &mut ScanLine,
    config: &ScanlineConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    
    // Simple pseudo-random number generator for deterministic results
    let mut seed = region.x.wrapping_mul(31).wrapping_add(region.y.wrapping_mul(17));
    
    for pixel_start in (start_x..end_x).step_by(bytes_per_pixel) {
        // Generate pseudo-random values
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let random_r = ((seed >> 16) & 0xFF) as u8;
        
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let random_g = ((seed >> 16) & 0xFF) as u8;
        
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let random_b = ((seed >> 16) & 0xFF) as u8;
        
        let noise_color = (random_r, random_g, random_b);
        apply_color_to_pixel(scanline, pixel_start, &noise_color, config.intensity, color_type)?;
    }
    
    Ok(())
}

/// Apply color to a specific pixel in the scanline
fn apply_color_to_pixel(
    scanline: &mut ScanLine,
    pixel_start: usize,
    color_values: &(u8, u8, u8),
    intensity: f32,
    color_type: ColorType,
) -> Result<(), PresetError> {
    match color_type {
        ColorType::TrueColor => {
            // RGB format
            if pixel_start + 2 < scanline.size() {
                let original_r = scanline.index(pixel_start).unwrap_or(0);
                let original_g = scanline.index(pixel_start + 1).unwrap_or(0);
                let original_b = scanline.index(pixel_start + 2).unwrap_or(0);
                
                let new_r = blend_color(original_r, color_values.0, intensity);
                let new_g = blend_color(original_g, color_values.1, intensity);
                let new_b = blend_color(original_b, color_values.2, intensity);
                
                scanline.update(pixel_start, new_r);
                scanline.update(pixel_start + 1, new_g);
                scanline.update(pixel_start + 2, new_b);
            }
        }
        ColorType::TrueColorAlpha => {
            // RGBA format
            if pixel_start + 3 < scanline.size() {
                let original_r = scanline.index(pixel_start).unwrap_or(0);
                let original_g = scanline.index(pixel_start + 1).unwrap_or(0);
                let original_b = scanline.index(pixel_start + 2).unwrap_or(0);
                
                let new_r = blend_color(original_r, color_values.0, intensity);
                let new_g = blend_color(original_g, color_values.1, intensity);
                let new_b = blend_color(original_b, color_values.2, intensity);
                
                scanline.update(pixel_start, new_r);
                scanline.update(pixel_start + 1, new_g);
                scanline.update(pixel_start + 2, new_b);
                // Keep alpha channel unchanged
            }
        }
        ColorType::GrayScale => {
            // Grayscale format - convert RGB to grayscale
            if pixel_start < scanline.size() {
                let original = scanline.index(pixel_start).unwrap_or(0);
                let gray_value = rgb_to_grayscale(color_values.0, color_values.1, color_values.2);
                let new_value = blend_color(original, gray_value, intensity);
                scanline.update(pixel_start, new_value);
            }
        }
        ColorType::GrayScaleAlpha => {
            // Grayscale with alpha
            if pixel_start + 1 < scanline.size() {
                let original = scanline.index(pixel_start).unwrap_or(0);
                let gray_value = rgb_to_grayscale(color_values.0, color_values.1, color_values.2);
                let new_value = blend_color(original, gray_value, intensity);
                scanline.update(pixel_start, new_value);
                // Keep alpha channel unchanged
            }
        }
        ColorType::IndexColor => {
            // Indexed color - limited support, just modify the index
            if pixel_start < scanline.size() {
                let original = scanline.index(pixel_start).unwrap_or(0);
                let new_index = ((original as f32 * (1.0 - intensity)) + (color_values.0 as f32 * intensity)) as u8;
                scanline.update(pixel_start, new_index);
            }
        }
    }
    
    Ok(())
}

/// Blend two color values with the given intensity
fn blend_color(original: u8, target: u8, intensity: f32) -> u8 {
    let original_f = original as f32;
    let target_f = target as f32;
    let blended = original_f * (1.0 - intensity) + target_f * intensity;
    blended.round().clamp(0.0, 255.0) as u8
}

/// Convert RGB to grayscale using standard luminance formula
fn rgb_to_grayscale(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::ScanlineColor;

    #[test]
    fn test_get_color_values() {
        assert_eq!(get_color_values(&ScanlineColor::Black).unwrap(), (0, 0, 0));
        assert_eq!(get_color_values(&ScanlineColor::White).unwrap(), (255, 255, 255));
        assert_eq!(get_color_values(&ScanlineColor::Custom(128, 64, 192)).unwrap(), (128, 64, 192));
    }
    
    #[test]
    fn test_blend_color() {
        assert_eq!(blend_color(100, 200, 0.0), 100);
        assert_eq!(blend_color(100, 200, 1.0), 200);
        assert_eq!(blend_color(100, 200, 0.5), 150);
    }
    
    #[test]
    fn test_rgb_to_grayscale() {
        assert_eq!(rgb_to_grayscale(255, 255, 255), 255);
        assert_eq!(rgb_to_grayscale(0, 0, 0), 0);
        // Test standard gray conversion
        let gray = rgb_to_grayscale(128, 128, 128);
        assert!(gray > 120 && gray < 135); // Should be close to 128
    }
    
    #[test]
    fn test_get_bytes_per_pixel() {
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScale, 8), 1);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColor, 8), 3);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColorAlpha, 8), 4);
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScaleAlpha, 8), 2);
    }
}