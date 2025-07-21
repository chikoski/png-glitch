use thiserror::Error;
use crate::presets::config::Region;

/// Errors that can occur when applying presets
#[derive(Error, Debug)]
pub enum PresetError {
    #[error("Invalid configuration parameter: {0}")]
    InvalidConfig(String),
    
    #[error("Region out of bounds: {region:?}")]
    RegionOutOfBounds { region: Region },
    
    #[error("Unsupported color type for preset: {preset}")]
    UnsupportedColorType { preset: String },
    
    #[error("Intensity value out of range: {value} (expected 0.0-1.0)")]
    IntensityOutOfRange { value: f32 },
    
    #[error("Invalid offset value: {value} (too large for image dimensions)")]
    InvalidOffset { value: i32 },
    
    #[error("Invalid block size: width={width}, height={height} (must be > 0)")]
    InvalidBlockSize { width: u32, height: u32 },
    
    #[error("Invalid threshold value: {value} (expected 0.0-1.0)")]
    InvalidThreshold { value: f32 },
}

impl PresetError {
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        PresetError::InvalidConfig(msg.into())
    }
    
    pub fn region_out_of_bounds(region: Region) -> Self {
        PresetError::RegionOutOfBounds { region }
    }
    
    pub fn unsupported_color_type(preset: impl Into<String>) -> Self {
        PresetError::UnsupportedColorType { preset: preset.into() }
    }
    
    pub fn intensity_out_of_range(value: f32) -> Self {
        PresetError::IntensityOutOfRange { value }
    }
    
    pub fn invalid_offset(value: i32) -> Self {
        PresetError::InvalidOffset { value }
    }
    
    pub fn invalid_block_size(width: u32, height: u32) -> Self {
        PresetError::InvalidBlockSize { width, height }
    }
    
    pub fn invalid_threshold(value: f32) -> Self {
        PresetError::InvalidThreshold { value }
    }
}