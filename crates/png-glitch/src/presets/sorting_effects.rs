use crate::presets::{PixelSortConfig, PresetError, Region, SortCriteria, SortDirection};
use crate::png::ColorType;
use crate::{PngGlitch, ScanLine};

/// Apply pixel sorting effects to the PNG image
pub fn apply_pixel_sort(png_glitch: &mut PngGlitch, config: PixelSortConfig) -> Result<(), PresetError> {
    let width = png_glitch.width();
    let height = png_glitch.height();
    
    // Determine the region to apply the effect
    let region = config.region.unwrap_or_else(|| Region::full_image(width, height));
    
    // Validate region bounds
    if region.x + region.width > width || region.y + region.height > height {
        return Err(PresetError::region_out_of_bounds(region));
    }
    
    // Apply pixel sorting based on direction
    match &config.direction {
        SortDirection::Horizontal => {
            apply_horizontal_sort(png_glitch, &config, &region)
        }
        SortDirection::Vertical => {
            apply_vertical_sort(png_glitch, &config, &region)
        }
        SortDirection::Diagonal => {
            apply_diagonal_sort(png_glitch, &config, &region)
        }
        SortDirection::Radial { center } => {
            apply_radial_sort(png_glitch, &config, &region, *center)
        }
    }
}

/// Apply horizontal pixel sorting
fn apply_horizontal_sort(
    png_glitch: &mut PngGlitch,
    config: &PixelSortConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let start_line = region.y;
    let end_line = region.y + region.height;
    
    for line_index in start_line..end_line {
        let scanlines = png_glitch.scan_lines_from(line_index, 1);
        if let Some(mut scanline) = scanlines.into_iter().next() {
            sort_scanline_horizontal(&mut scanline, config, region)?;
        }
    }
    
    Ok(())
}

/// Apply vertical pixel sorting
fn apply_vertical_sort(
    png_glitch: &mut PngGlitch,
    config: &PixelSortConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let start_x = region.x;
    let end_x = region.x + region.width;
    
    // Extract vertical columns and sort them
    for x in start_x..end_x {
        let mut column_pixels = extract_vertical_column(png_glitch, x, region)?;
        sort_pixel_column(&mut column_pixels, config)?;
        apply_vertical_column(png_glitch, x, region, &column_pixels)?;
    }
    
    Ok(())
}

/// Apply diagonal pixel sorting
fn apply_diagonal_sort(
    png_glitch: &mut PngGlitch,
    config: &PixelSortConfig,
    region: &Region,
) -> Result<(), PresetError> {
    // Sort along diagonal lines from top-left to bottom-right
    let diagonals = generate_diagonal_lines(region);
    
    for diagonal in diagonals {
        let mut diagonal_pixels = extract_diagonal_pixels(png_glitch, &diagonal, region)?;
        sort_pixel_array(&mut diagonal_pixels, config)?;
        apply_diagonal_pixels(png_glitch, &diagonal, region, &diagonal_pixels)?;
    }
    
    Ok(())
}

/// Apply radial pixel sorting from a center point
fn apply_radial_sort(
    png_glitch: &mut PngGlitch,
    config: &PixelSortConfig,
    region: &Region,
    center: (u32, u32),
) -> Result<(), PresetError> {
    // Generate radial lines from center
    let radial_lines = generate_radial_lines(center, region);
    
    for line in radial_lines {
        let mut line_pixels = extract_line_pixels(png_glitch, &line, region)?;
        sort_pixel_array(&mut line_pixels, config)?;
        apply_line_pixels(png_glitch, &line, region, &line_pixels)?;
    }
    
    Ok(())
}

/// Sort pixels within a horizontal scanline
fn sort_scanline_horizontal(
    scanline: &mut ScanLine,
    config: &PixelSortConfig,
    region: &Region,
) -> Result<(), PresetError> {
    let color_type = scanline.color_type();
    let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
    
    let start_x = region.x as usize * bytes_per_pixel;
    let end_x = ((region.x + region.width) as usize * bytes_per_pixel).min(scanline.size());
    let pixels_in_region = (end_x - start_x) / bytes_per_pixel;
    
    if pixels_in_region <= 1 {
        return Ok(());
    }
    
    // Extract pixels with their sort keys
    let mut pixel_data = Vec::new();
    for pixel_index in 0..pixels_in_region {
        let pixel_start = start_x + pixel_index * bytes_per_pixel;
        let pixel = extract_pixel_from_scanline(scanline, pixel_start, color_type, bytes_per_pixel)?;
        let sort_key = calculate_sort_key(&pixel, &config.criteria);
        
        // Apply threshold if specified
        if let Some(threshold) = config.threshold {
            if sort_key < threshold * 255.0 {
                continue; // Skip pixels below threshold
            }
        }
        
        pixel_data.push((sort_key, pixel, pixel_start));
    }
    
    // Sort pixels by their sort keys
    pixel_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    
    // Apply sorted pixels back to scanline
    for (i, (_, pixel, _)) in pixel_data.iter().enumerate() {
        let target_pos = start_x + i * bytes_per_pixel;
        apply_pixel_to_scanline(scanline, target_pos, pixel, color_type, bytes_per_pixel)?;
    }
    
    Ok(())
}

/// Extract a vertical column of pixels
fn extract_vertical_column(
    png_glitch: &PngGlitch,
    x: u32,
    region: &Region,
) -> Result<Vec<PixelData>, PresetError> {
    let mut column_pixels = Vec::new();
    let start_y = region.y;
    let end_y = region.y + region.height;
    
    for y in start_y..end_y {
        let scanlines = png_glitch.scan_lines_from(y, 1);
        if let Some(scanline) = scanlines.into_iter().next() {
            let color_type = scanline.color_type();
            let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
            let pixel_start = x as usize * bytes_per_pixel;
            
            if pixel_start + bytes_per_pixel <= scanline.size() {
                let pixel = extract_pixel_from_scanline(&scanline, pixel_start, color_type, bytes_per_pixel)?;
                column_pixels.push(pixel);
            }
        }
    }
    
    Ok(column_pixels)
}

/// Sort a column of pixels
fn sort_pixel_column(
    pixels: &mut [PixelData],
    config: &PixelSortConfig,
) -> Result<(), PresetError> {
    // Calculate sort keys and filter by threshold
    let mut pixel_keys: Vec<(f32, usize)> = pixels
        .iter()
        .enumerate()
        .map(|(i, pixel)| (calculate_sort_key(pixel, &config.criteria), i))
        .filter(|(key, _)| {
            if let Some(threshold) = config.threshold {
                *key >= threshold * 255.0
            } else {
                true
            }
        })
        .collect();
    
    // Sort by keys
    pixel_keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    
    // Rearrange pixels based on sorted order
    let original_pixels = pixels.to_vec();
    for (new_index, (_, original_index)) in pixel_keys.iter().enumerate() {
        if new_index < pixels.len() {
            pixels[new_index] = original_pixels[*original_index].clone();
        }
    }
    
    Ok(())
}

/// Apply a sorted vertical column back to the image
fn apply_vertical_column(
    png_glitch: &mut PngGlitch,
    x: u32,
    region: &Region,
    pixels: &[PixelData],
) -> Result<(), PresetError> {
    let start_y = region.y;
    let end_y = region.y + region.height;
    
    for (i, y) in (start_y..end_y).enumerate() {
        if i < pixels.len() {
            let scanlines = png_glitch.scan_lines_from(y, 1);
            if let Some(mut scanline) = scanlines.into_iter().next() {
                let color_type = scanline.color_type();
                let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
                let pixel_start = x as usize * bytes_per_pixel;
                
                if pixel_start + bytes_per_pixel <= scanline.size() {
                    apply_pixel_to_scanline(&mut scanline, pixel_start, &pixels[i], color_type, bytes_per_pixel)?;
                }
            }
        }
    }
    
    Ok(())
}

/// Generate diagonal lines for diagonal sorting
fn generate_diagonal_lines(region: &Region) -> Vec<Vec<(u32, u32)>> {
    let mut diagonals = Vec::new();
    
    // Top-left to bottom-right diagonals
    // Starting from top edge
    for start_x in region.x..(region.x + region.width) {
        let mut diagonal = Vec::new();
        let mut x = start_x;
        let mut y = region.y;
        
        while x < region.x + region.width && y < region.y + region.height {
            diagonal.push((x, y));
            x += 1;
            y += 1;
        }
        
        if diagonal.len() > 1 {
            diagonals.push(diagonal);
        }
    }
    
    // Starting from left edge (excluding corner)
    for start_y in (region.y + 1)..(region.y + region.height) {
        let mut diagonal = Vec::new();
        let mut x = region.x;
        let mut y = start_y;
        
        while x < region.x + region.width && y < region.y + region.height {
            diagonal.push((x, y));
            x += 1;
            y += 1;
        }
        
        if diagonal.len() > 1 {
            diagonals.push(diagonal);
        }
    }
    
    diagonals
}

/// Generate radial lines from a center point
fn generate_radial_lines(center: (u32, u32), region: &Region) -> Vec<Vec<(u32, u32)>> {
    let mut radial_lines = Vec::new();
    let num_rays = 16; // Number of radial rays
    
    for i in 0..num_rays {
        let angle = (i as f32 * 2.0 * std::f32::consts::PI) / num_rays as f32;
        let mut line = Vec::new();
        
        // Cast ray from center outward
        let mut distance = 1.0;
        let max_distance = (region.width.max(region.height) as f32) * 1.5; // Reasonable maximum
        
        loop {
            if distance > max_distance {
                break; // Prevent infinite loops
            }
            
            let x = center.0 as f32 + distance * angle.cos();
            let y = center.1 as f32 + distance * angle.sin();
            
            let x_int = x.round() as u32;
            let y_int = y.round() as u32;
            
            // Check if point is within region
            if x_int >= region.x && x_int < region.x + region.width &&
               y_int >= region.y && y_int < region.y + region.height {
                line.push((x_int, y_int));
                distance += 1.0;
            } else {
                break;
            }
        }
        
        if line.len() > 1 {
            radial_lines.push(line);
        }
    }
    
    radial_lines
}

/// Extract pixels along a diagonal or line
fn extract_diagonal_pixels(
    png_glitch: &PngGlitch,
    diagonal: &[(u32, u32)],
    _region: &Region,
) -> Result<Vec<PixelData>, PresetError> {
    let mut pixels = Vec::new();
    
    for &(x, y) in diagonal {
        let scanlines = png_glitch.scan_lines_from(y, 1);
        if let Some(scanline) = scanlines.into_iter().next() {
            let color_type = scanline.color_type();
            let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
            let pixel_start = x as usize * bytes_per_pixel;
            
            if pixel_start + bytes_per_pixel <= scanline.size() {
                let pixel = extract_pixel_from_scanline(&scanline, pixel_start, color_type, bytes_per_pixel)?;
                pixels.push(pixel);
            }
        }
    }
    
    Ok(pixels)
}

/// Extract pixels along a line (used for radial sorting)
fn extract_line_pixels(
    png_glitch: &PngGlitch,
    line: &[(u32, u32)],
    region: &Region,
) -> Result<Vec<PixelData>, PresetError> {
    extract_diagonal_pixels(png_glitch, line, region)
}

/// Apply sorted pixels back to a diagonal
fn apply_diagonal_pixels(
    png_glitch: &mut PngGlitch,
    diagonal: &[(u32, u32)],
    _region: &Region,
    pixels: &[PixelData],
) -> Result<(), PresetError> {
    for (i, &(x, y)) in diagonal.iter().enumerate() {
        if i < pixels.len() {
            let scanlines = png_glitch.scan_lines_from(y, 1);
            if let Some(mut scanline) = scanlines.into_iter().next() {
                let color_type = scanline.color_type();
                let bytes_per_pixel = get_bytes_per_pixel(color_type, scanline.bit_depth());
                let pixel_start = x as usize * bytes_per_pixel;
                
                if pixel_start + bytes_per_pixel <= scanline.size() {
                    apply_pixel_to_scanline(&mut scanline, pixel_start, &pixels[i], color_type, bytes_per_pixel)?;
                }
            }
        }
    }
    
    Ok(())
}

/// Apply sorted pixels back to a line (used for radial sorting)
fn apply_line_pixels(
    png_glitch: &mut PngGlitch,
    line: &[(u32, u32)],
    region: &Region,
    pixels: &[PixelData],
) -> Result<(), PresetError> {
    apply_diagonal_pixels(png_glitch, line, region, pixels)
}

/// Sort an array of pixels
fn sort_pixel_array(
    pixels: &mut [PixelData],
    config: &PixelSortConfig,
) -> Result<(), PresetError> {
    // Calculate sort keys and create index mapping
    let mut pixel_keys: Vec<(f32, usize)> = pixels
        .iter()
        .enumerate()
        .map(|(i, pixel)| (calculate_sort_key(pixel, &config.criteria), i))
        .filter(|(key, _)| {
            if let Some(threshold) = config.threshold {
                *key >= threshold * 255.0
            } else {
                true
            }
        })
        .collect();
    
    // Sort by keys
    pixel_keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    
    // Rearrange pixels
    let original_pixels = pixels.to_vec();
    for (new_index, (_, original_index)) in pixel_keys.iter().enumerate() {
        if new_index < pixels.len() {
            pixels[new_index] = original_pixels[*original_index].clone();
        }
    }
    
    Ok(())
}

/// Pixel data structure for sorting
#[derive(Debug, Clone)]
struct PixelData {
    r: u8,
    g: u8,
    b: u8,
    a: Option<u8>,
}

/// Extract pixel data from scanline
fn extract_pixel_from_scanline(
    scanline: &ScanLine,
    pixel_start: usize,
    color_type: ColorType,
    _bytes_per_pixel: usize,
) -> Result<PixelData, PresetError> {
    match color_type {
        ColorType::TrueColor => {
            if pixel_start + 2 < scanline.size() {
                Ok(PixelData {
                    r: scanline.index(pixel_start).unwrap_or(0),
                    g: scanline.index(pixel_start + 1).unwrap_or(0),
                    b: scanline.index(pixel_start + 2).unwrap_or(0),
                    a: None,
                })
            } else {
                Ok(PixelData { r: 0, g: 0, b: 0, a: None })
            }
        }
        ColorType::TrueColorAlpha => {
            if pixel_start + 3 < scanline.size() {
                Ok(PixelData {
                    r: scanline.index(pixel_start).unwrap_or(0),
                    g: scanline.index(pixel_start + 1).unwrap_or(0),
                    b: scanline.index(pixel_start + 2).unwrap_or(0),
                    a: Some(scanline.index(pixel_start + 3).unwrap_or(255)),
                })
            } else {
                Ok(PixelData { r: 0, g: 0, b: 0, a: Some(255) })
            }
        }
        ColorType::GrayScale => {
            if pixel_start < scanline.size() {
                let gray = scanline.index(pixel_start).unwrap_or(0);
                Ok(PixelData {
                    r: gray,
                    g: gray,
                    b: gray,
                    a: None,
                })
            } else {
                Ok(PixelData { r: 0, g: 0, b: 0, a: None })
            }
        }
        ColorType::GrayScaleAlpha => {
            if pixel_start + 1 < scanline.size() {
                let gray = scanline.index(pixel_start).unwrap_or(0);
                let alpha = scanline.index(pixel_start + 1).unwrap_or(255);
                Ok(PixelData {
                    r: gray,
                    g: gray,
                    b: gray,
                    a: Some(alpha),
                })
            } else {
                Ok(PixelData { r: 0, g: 0, b: 0, a: Some(255) })
            }
        }
        ColorType::IndexColor => {
            if pixel_start < scanline.size() {
                let index = scanline.index(pixel_start).unwrap_or(0);
                Ok(PixelData {
                    r: index,
                    g: index,
                    b: index,
                    a: None,
                })
            } else {
                Ok(PixelData { r: 0, g: 0, b: 0, a: None })
            }
        }
    }
}

/// Apply pixel data back to scanline
fn apply_pixel_to_scanline(
    scanline: &mut ScanLine,
    pixel_start: usize,
    pixel: &PixelData,
    color_type: ColorType,
    _bytes_per_pixel: usize,
) -> Result<(), PresetError> {
    match color_type {
        ColorType::TrueColor => {
            if pixel_start + 2 < scanline.size() {
                scanline.update(pixel_start, pixel.r);
                scanline.update(pixel_start + 1, pixel.g);
                scanline.update(pixel_start + 2, pixel.b);
            }
        }
        ColorType::TrueColorAlpha => {
            if pixel_start + 3 < scanline.size() {
                scanline.update(pixel_start, pixel.r);
                scanline.update(pixel_start + 1, pixel.g);
                scanline.update(pixel_start + 2, pixel.b);
                if let Some(alpha) = pixel.a {
                    scanline.update(pixel_start + 3, alpha);
                }
            }
        }
        ColorType::GrayScale => {
            if pixel_start < scanline.size() {
                let gray = rgb_to_grayscale(pixel.r, pixel.g, pixel.b);
                scanline.update(pixel_start, gray);
            }
        }
        ColorType::GrayScaleAlpha => {
            if pixel_start + 1 < scanline.size() {
                let gray = rgb_to_grayscale(pixel.r, pixel.g, pixel.b);
                scanline.update(pixel_start, gray);
                if let Some(alpha) = pixel.a {
                    scanline.update(pixel_start + 1, alpha);
                }
            }
        }
        ColorType::IndexColor => {
            if pixel_start < scanline.size() {
                let index = (pixel.r as u32 + pixel.g as u32 + pixel.b as u32) / 3;
                scanline.update(pixel_start, index as u8);
            }
        }
    }
    
    Ok(())
}

/// Calculate sort key based on criteria
fn calculate_sort_key(pixel: &PixelData, criteria: &SortCriteria) -> f32 {
    match criteria {
        SortCriteria::Brightness => {
            // Standard luminance formula
            0.299 * pixel.r as f32 + 0.587 * pixel.g as f32 + 0.114 * pixel.b as f32
        }
        SortCriteria::Hue => {
            let (h, _, _) = rgb_to_hsv(pixel.r, pixel.g, pixel.b);
            h
        }
        SortCriteria::Saturation => {
            let (_, s, _) = rgb_to_hsv(pixel.r, pixel.g, pixel.b);
            s * 255.0
        }
        SortCriteria::Red => pixel.r as f32,
        SortCriteria::Green => pixel.g as f32,
        SortCriteria::Blue => pixel.b as f32,
    }
}

/// Convert RGB to HSV
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    
    // Hue
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    
    // Saturation
    let s = if max == 0.0 { 0.0 } else { delta / max };
    
    // Value
    let v = max;
    
    (h, s, v)
}

/// Convert RGB to grayscale
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
    use crate::presets::{PixelSortConfig, SortCriteria, SortDirection};
    use crate::presets::config::ConfigValidation;

    #[test]
    fn test_calculate_sort_key() {
        let pixel = PixelData { r: 128, g: 64, b: 192, a: None };
        
        // Test brightness calculation
        let brightness = calculate_sort_key(&pixel, &SortCriteria::Brightness);
        assert!(brightness > 0.0 && brightness <= 255.0);
        
        // Test individual color channels
        assert_eq!(calculate_sort_key(&pixel, &SortCriteria::Red), 128.0);
        assert_eq!(calculate_sort_key(&pixel, &SortCriteria::Green), 64.0);
        assert_eq!(calculate_sort_key(&pixel, &SortCriteria::Blue), 192.0);
    }
    
    #[test]
    fn test_rgb_to_hsv() {
        // Test pure red
        let (h, s, v) = rgb_to_hsv(255, 0, 0);
        assert!((h - 0.0).abs() < 1.0); // Hue should be around 0
        assert!((s - 1.0).abs() < 0.1); // Saturation should be 1
        assert!((v - 1.0).abs() < 0.1); // Value should be 1
        
        // Test gray
        let (_h, s, _v) = rgb_to_hsv(128, 128, 128);
        assert_eq!(s, 0.0); // Gray should have 0 saturation
    }
    
    #[test]
    fn test_rgb_to_grayscale() {
        assert_eq!(rgb_to_grayscale(255, 255, 255), 255);
        assert_eq!(rgb_to_grayscale(0, 0, 0), 0);
        
        // Test standard gray conversion
        let gray = rgb_to_grayscale(128, 128, 128);
        assert!(gray > 120 && gray < 135);
    }
    
    #[test]
    fn test_get_bytes_per_pixel() {
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScale, 8), 1);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColor, 8), 3);
        assert_eq!(get_bytes_per_pixel(ColorType::TrueColorAlpha, 8), 4);
        assert_eq!(get_bytes_per_pixel(ColorType::GrayScaleAlpha, 8), 2);
    }
    
    #[test]
    fn test_pixel_sort_config_validation() {
        let valid_config = PixelSortConfig::default();
        assert!(valid_config.validate().is_ok());
        
        // Test that default config has reasonable values
        assert_eq!(valid_config.direction, SortDirection::Horizontal);
        assert_eq!(valid_config.criteria, SortCriteria::Brightness);
        assert_eq!(valid_config.threshold, Some(0.5));
    }
    
    #[test]
    fn test_generate_diagonal_lines() {
        let region = Region::new(0, 0, 4, 4);
        let diagonals = generate_diagonal_lines(&region);
        
        // Should generate multiple diagonal lines
        assert!(!diagonals.is_empty());
        
        // Each diagonal should have at least 2 points
        for diagonal in &diagonals {
            assert!(diagonal.len() >= 2);
        }
    }
    
    #[test]
    fn test_generate_radial_lines() {
        let region = Region::new(0, 0, 10, 10);
        let center = (5, 5);
        let radial_lines = generate_radial_lines(center, &region);
        
        // Should generate multiple radial lines
        assert!(!radial_lines.is_empty());
        
        // Check that we have a reasonable number of lines (16 rays)
        assert!(radial_lines.len() <= 16);
        
        // Each line should have at least one point
        for line in &radial_lines {
            assert!(!line.is_empty());
        }
    }
}