use crate::presets::{PresetError, RgbShiftConfig, WrapMode};
use crate::png::ColorType;
use crate::{PngGlitch, ScanLine};

/// Apply RGB channel separation effects to the PNG image
pub fn apply_rgb_shift(png_glitch: &mut PngGlitch, config: RgbShiftConfig) -> Result<(), PresetError> {
    let width = png_glitch.width();
    let height = png_glitch.height();
    
    // Validate offsets against image dimensions
    validate_offsets(&config, width, height)?;
    
    // Extract original image data for channel separation
    let original_data = extract_image_data(png_glitch)?;
    
    // Apply RGB channel shifts
    apply_channel_shifts(png_glitch, &config, &original_data, width, height)?;
    
    Ok(())
}

/// Validate that offsets are reasonable for the image dimensions
fn validate_offsets(config: &RgbShiftConfig, width: u32, height: u32) -> Result<(), PresetError> {
    let max_offset = (width.max(height) / 2) as i32;
    
    for &(x_offset, y_offset) in &[config.red_offset, config.green_offset, config.blue_offset] {
        if x_offset.abs() > max_offset || y_offset.abs() > max_offset {
            return Err(PresetError::invalid_offset(x_offset.max(y_offset)));
        }
    }
    
    Ok(())
}

/// Extract image data for channel manipulation
fn extract_image_data(png_glitch: &PngGlitch) -> Result<Vec<Vec<u8>>, PresetError> {
    let height = png_glitch.height();
    let mut image_data = Vec::new();
    
    for line_index in 0..height {
        let scanlines = png_glitch.scan_lines_from(line_index, 1);
        if let Some(scanline) = scanlines.into_iter().next() {
            let mut line_data = Vec::new();
            for i in 0..scanline.size() {
                line_data.push(scanline.index(i).unwrap_or(0));
            }
            image_data.push(line_data);
        }
    }
    
    Ok(image_data)
}

/// Apply channel shifts to the image
fn apply_channel_shifts(
    png_glitch: &mut PngGlitch,
    config: &RgbShiftConfig,
    original_data: &[Vec<u8>],
    width: u32,
    height: u32,
) -> Result<(), PresetError> {
    for line_index in 0..height {
        let scanlines = png_glitch.scan_lines_from(line_index, 1);
        if let Some(mut scanline) = scanlines.into_iter().next() {
            apply_channel_shift_to_scanline(
                &mut scanline,
                config,
                original_data,
                line_index,
                width,
                height,
            )?;
        }
    }
    
    Ok(())
}

/// Apply channel shift to a specific scanline
fn apply_channel_shift_to_scanline(
    scanline: &mut ScanLine,
    config: &RgbShiftConfig,
    original_data: &[Vec<u8>],
    line_index: u32,
    width: u32,
    height: u32,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    match color_type {
        ColorType::TrueColor => {
            apply_rgb_shift_to_scanline(scanline, config, original_data, line_index, width, height, bytes_per_pixel, false)?;
        }
        ColorType::TrueColorAlpha => {
            apply_rgb_shift_to_scanline(scanline, config, original_data, line_index, width, height, bytes_per_pixel, true)?;
        }
        ColorType::GrayScale | ColorType::GrayScaleAlpha | ColorType::IndexColor => {
            // RGB shift doesn't apply to non-RGB formats, but we can simulate it
            apply_simulated_rgb_shift(scanline, config, original_data, line_index, width, height, color_type)?;
        }
    }
    
    Ok(())
}

/// Apply RGB channel shift to RGB/RGBA scanlines
fn apply_rgb_shift_to_scanline(
    scanline: &mut ScanLine,
    config: &RgbShiftConfig,
    original_data: &[Vec<u8>],
    line_index: u32,
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
    has_alpha: bool,
) -> Result<(), PresetError> {
    let scanline_size = scanline.size();
    let pixels_per_line = scanline_size / bytes_per_pixel;
    
    for pixel_index in 0..pixels_per_line {
        let pixel_start = pixel_index * bytes_per_pixel;
        
        if pixel_start + 2 < scanline_size {
            // Get shifted channel values
            let red_value = get_shifted_channel_value(
                original_data,
                line_index,
                pixel_index,
                0, // Red channel
                config.red_offset,
                width,
                height,
                bytes_per_pixel,
                &config.wrap_mode,
            );
            
            let green_value = get_shifted_channel_value(
                original_data,
                line_index,
                pixel_index,
                1, // Green channel
                config.green_offset,
                width,
                height,
                bytes_per_pixel,
                &config.wrap_mode,
            );
            
            let blue_value = get_shifted_channel_value(
                original_data,
                line_index,
                pixel_index,
                2, // Blue channel
                config.blue_offset,
                width,
                height,
                bytes_per_pixel,
                &config.wrap_mode,
            );
            
            // Blend with original values based on intensity
            let original_r = scanline.index(pixel_start).unwrap_or(0);
            let original_g = scanline.index(pixel_start + 1).unwrap_or(0);
            let original_b = scanline.index(pixel_start + 2).unwrap_or(0);
            
            let new_r = blend_channel(original_r, red_value, config.intensity);
            let new_g = blend_channel(original_g, green_value, config.intensity);
            let new_b = blend_channel(original_b, blue_value, config.intensity);
            
            scanline.update(pixel_start, new_r);
            scanline.update(pixel_start + 1, new_g);
            scanline.update(pixel_start + 2, new_b);
            
            // Keep alpha channel unchanged if present
            if has_alpha && pixel_start + 3 < scanline_size {
                // Alpha channel remains unchanged
            }
        }
    }
    
    Ok(())
}

/// Apply simulated RGB shift to non-RGB formats
fn apply_simulated_rgb_shift(
    scanline: &mut ScanLine,
    config: &RgbShiftConfig,
    original_data: &[Vec<u8>],
    line_index: u32,
    width: u32,
    height: u32,
    color_type: ColorType,
) -> Result<(), PresetError> {
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    let scanline_size = scanline.size();
    let pixels_per_line = scanline_size / bytes_per_pixel;
    
    for pixel_index in 0..pixels_per_line {
        let pixel_start = pixel_index * bytes_per_pixel;
        
        if pixel_start < scanline_size {
            // Simulate RGB shift by using different offset patterns for grayscale
            let shift_offset = calculate_average_offset(config);
            let shifted_value = get_shifted_grayscale_value(
                original_data,
                line_index,
                pixel_index,
                shift_offset,
                width,
                height,
                bytes_per_pixel,
                &config.wrap_mode,
            );
            
            let original_value = scanline.index(pixel_start).unwrap_or(0);
            let new_value = blend_channel(original_value, shifted_value, config.intensity);
            
            scanline.update(pixel_start, new_value);
            
            // Keep alpha channel unchanged for grayscale+alpha
            if color_type == ColorType::GrayScaleAlpha && pixel_start + 1 < scanline_size {
                // Alpha channel remains unchanged
            }
        }
    }
    
    Ok(())
}

/// Get shifted channel value with boundary handling
fn get_shifted_channel_value(
    original_data: &[Vec<u8>],
    line_index: u32,
    pixel_index: usize,
    channel_index: usize,
    offset: (i32, i32),
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
    wrap_mode: &WrapMode,
) -> u8 {
    let (x_offset, y_offset) = offset;
    
    // Calculate source coordinates
    let src_x = pixel_index as i32 + x_offset;
    let src_y = line_index as i32 + y_offset;
    
    // Handle boundary conditions
    let (final_x, final_y) = handle_boundaries(src_x, src_y, width as i32, height as i32, wrap_mode);
    
    // Extract value if coordinates are valid
    if final_x >= 0 && final_y >= 0 && final_x < width as i32 && final_y < height as i32 {
        let y_index = final_y as usize;
        let x_index = final_x as usize;
        
        if y_index < original_data.len() {
            let line_data = &original_data[y_index];
            let pixel_start = x_index * bytes_per_pixel + channel_index;
            
            if pixel_start < line_data.len() {
                return line_data[pixel_start];
            }
        }
    }
    
    // Return default value if out of bounds
    0
}

/// Get shifted grayscale value for non-RGB formats
fn get_shifted_grayscale_value(
    original_data: &[Vec<u8>],
    line_index: u32,
    pixel_index: usize,
    offset: (i32, i32),
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
    wrap_mode: &WrapMode,
) -> u8 {
    let (x_offset, y_offset) = offset;
    
    // Calculate source coordinates
    let src_x = pixel_index as i32 + x_offset;
    let src_y = line_index as i32 + y_offset;
    
    // Handle boundary conditions
    let (final_x, final_y) = handle_boundaries(src_x, src_y, width as i32, height as i32, wrap_mode);
    
    // Extract value if coordinates are valid
    if final_x >= 0 && final_y >= 0 && final_x < width as i32 && final_y < height as i32 {
        let y_index = final_y as usize;
        let x_index = final_x as usize;
        
        if y_index < original_data.len() {
            let line_data = &original_data[y_index];
            let pixel_start = x_index * bytes_per_pixel;
            
            if pixel_start < line_data.len() {
                return line_data[pixel_start];
            }
        }
    }
    
    // Return default value if out of bounds
    0
}

/// Handle boundary conditions based on wrap mode
fn handle_boundaries(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    wrap_mode: &WrapMode,
) -> (i32, i32) {
    match wrap_mode {
        WrapMode::Clamp => {
            let clamped_x = x.clamp(0, width - 1);
            let clamped_y = y.clamp(0, height - 1);
            (clamped_x, clamped_y)
        }
        WrapMode::Wrap => {
            let wrapped_x = if x < 0 {
                width + (x % width)
            } else {
                x % width
            };
            let wrapped_y = if y < 0 {
                height + (y % height)
            } else {
                y % height
            };
            (wrapped_x, wrapped_y)
        }
        WrapMode::Mirror => {
            let mirrored_x = if x < 0 {
                (-x - 1).min(width - 1)
            } else if x >= width {
                (2 * width - x - 1).max(0)
            } else {
                x
            };
            let mirrored_y = if y < 0 {
                (-y - 1).min(height - 1)
            } else if y >= height {
                (2 * height - y - 1).max(0)
            } else {
                y
            };
            (mirrored_x, mirrored_y)
        }
    }
}

/// Calculate average offset for simulated RGB shift
fn calculate_average_offset(config: &RgbShiftConfig) -> (i32, i32) {
    let avg_x = (config.red_offset.0 + config.green_offset.0 + config.blue_offset.0) / 3;
    let avg_y = (config.red_offset.1 + config.green_offset.1 + config.blue_offset.1) / 3;
    (avg_x, avg_y)
}

/// Blend channel values with intensity
fn blend_channel(original: u8, shifted: u8, intensity: f32) -> u8 {
    let original_f = original as f32;
    let shifted_f = shifted as f32;
    let blended = original_f * (1.0 - intensity) + shifted_f * intensity;
    blended.round().clamp(0.0, 255.0) as u8
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
    use crate::presets::{RgbShiftConfig, WrapMode};
    use crate::presets::config::ConfigValidation;

    #[test]
    fn test_handle_boundaries_clamp() {
        let (x, y) = handle_boundaries(-5, -3, 100, 100, &WrapMode::Clamp);
        assert_eq!((x, y), (0, 0));
        
        let (x, y) = handle_boundaries(105, 103, 100, 100, &WrapMode::Clamp);
        assert_eq!((x, y), (99, 99));
        
        let (x, y) = handle_boundaries(50, 50, 100, 100, &WrapMode::Clamp);
        assert_eq!((x, y), (50, 50));
    }
    
    #[test]
    fn test_handle_boundaries_wrap() {
        let (x, y) = handle_boundaries(-5, -3, 100, 100, &WrapMode::Wrap);
        assert_eq!((x, y), (95, 97));
        
        let (x, y) = handle_boundaries(105, 103, 100, 100, &WrapMode::Wrap);
        assert_eq!((x, y), (5, 3));
        
        let (x, y) = handle_boundaries(50, 50, 100, 100, &WrapMode::Wrap);
        assert_eq!((x, y), (50, 50));
    }
    
    #[test]
    fn test_handle_boundaries_mirror() {
        let (x, y) = handle_boundaries(-5, -3, 100, 100, &WrapMode::Mirror);
        assert_eq!((x, y), (4, 2));
        
        let (x, y) = handle_boundaries(105, 103, 100, 100, &WrapMode::Mirror);
        assert_eq!((x, y), (94, 96));
        
        let (x, y) = handle_boundaries(50, 50, 100, 100, &WrapMode::Mirror);
        assert_eq!((x, y), (50, 50));
    }
    
    #[test]
    fn test_blend_channel() {
        assert_eq!(blend_channel(100, 200, 0.0), 100);
        assert_eq!(blend_channel(100, 200, 1.0), 200);
        assert_eq!(blend_channel(100, 200, 0.5), 150);
    }
    
    #[test]
    fn test_calculate_average_offset() {
        let config = RgbShiftConfig {
            red_offset: (3, 0),
            green_offset: (0, 0),
            blue_offset: (-3, 0),
            intensity: 1.0,
            wrap_mode: WrapMode::Clamp,
        };
        
        let (avg_x, avg_y) = calculate_average_offset(&config);
        assert_eq!((avg_x, avg_y), (0, 0));
    }
    
    #[test]
    fn test_get_bytes_per_pixel() {
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScale, 8), 1);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColor, 8), 3);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColorAlpha, 8), 4);
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScaleAlpha, 8), 2);
    }
    
    #[test]
    fn test_rgb_shift_config_validation() {
        let valid_config = RgbShiftConfig::default();
        assert!(valid_config.validate().is_ok());
        
        // Test that default config has reasonable offsets
        assert_eq!(valid_config.red_offset, (2, 0));
        assert_eq!(valid_config.green_offset, (0, 0));
        assert_eq!(valid_config.blue_offset, (-2, 0));
        assert_eq!(valid_config.intensity, 0.8);
    }
    
    #[test]
    fn test_validate_offsets() {
        let config = RgbShiftConfig {
            red_offset: (5, 0),
            green_offset: (0, 0),
            blue_offset: (-5, 0),
            intensity: 1.0,
            wrap_mode: WrapMode::Clamp,
        };
        
        // Should be valid for reasonable image size
        assert!(validate_offsets(&config, 100, 100).is_ok());
        
        // Should be invalid for very small image
        assert!(validate_offsets(&config, 8, 8).is_err());
    }
}