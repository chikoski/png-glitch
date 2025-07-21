use crate::presets::error::PresetError;

/// Common region specification for targeting specific areas of an image
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: u32,
    pub y: u32, 
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
    
    pub fn full_image(width: u32, height: u32) -> Self {
        Self { x: 0, y: 0, width, height }
    }
}

/// Direction enumeration for various effects
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

/// Validation trait for configuration structures
pub trait ConfigValidation {
    fn validate(&self) -> Result<(), PresetError>;
}

// Scanline preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct ScanlineConfig {
    pub intensity: f32,
    pub spacing: u32,
    pub color: ScanlineColor,
    pub region: Option<Region>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScanlineColor {
    Black,
    White, 
    Transparent,
    Custom(u8, u8, u8),
    Pattern(ScanlinePattern),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScanlinePattern {
    Solid,
    Dashed,
    Dotted,
    Noise,
}

impl Default for ScanlineConfig {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            spacing: 4,
            color: ScanlineColor::Black,
            region: None,
        }
    }
}

impl ConfigValidation for ScanlineConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if !(0.0..=1.0).contains(&self.intensity) {
            return Err(PresetError::intensity_out_of_range(self.intensity));
        }
        if self.spacing == 0 {
            return Err(PresetError::invalid_config("Spacing must be greater than 0"));
        }
        Ok(())
    }
}

// Noise preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseConfig {
    pub intensity: f32,
    pub distribution: NoiseDistribution,
    pub region: Option<Region>,
    pub preserve_structure: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NoiseDistribution {
    Uniform,
    Clustered { cluster_size: u32 },
    Linear { direction: Direction },
    Random,
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            intensity: 0.2,
            distribution: NoiseDistribution::Uniform,
            region: None,
            preserve_structure: true,
        }
    }
}

impl ConfigValidation for NoiseConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if !(0.0..=1.0).contains(&self.intensity) {
            return Err(PresetError::intensity_out_of_range(self.intensity));
        }
        if let NoiseDistribution::Clustered { cluster_size } = &self.distribution {
            if *cluster_size == 0 {
                return Err(PresetError::invalid_config("Cluster size must be greater than 0"));
            }
        }
        Ok(())
    }
}

// RGB shift preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RgbShiftConfig {
    pub red_offset: (i32, i32),
    pub green_offset: (i32, i32),
    pub blue_offset: (i32, i32),
    pub intensity: f32,
    pub wrap_mode: WrapMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrapMode {
    Clamp,
    Wrap,
    Mirror,
}

impl Default for RgbShiftConfig {
    fn default() -> Self {
        Self {
            red_offset: (2, 0),
            green_offset: (0, 0),
            blue_offset: (-2, 0),
            intensity: 0.8,
            wrap_mode: WrapMode::Clamp,
        }
    }
}

impl ConfigValidation for RgbShiftConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if !(0.0..=1.0).contains(&self.intensity) {
            return Err(PresetError::intensity_out_of_range(self.intensity));
        }
        // Offset validation will be done against actual image dimensions during application
        Ok(())
    }
}

// Digital degradation preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DegradationConfig {
    pub level: f32,
    pub artifacts: Vec<ArtifactType>,
    pub preserve_readability: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactType {
    Blocking { block_size: u32 },
    Banding { bands: u32 },
    Compression { quality: f32 },
    Posterization { levels: u32 },
}

impl Default for DegradationConfig {
    fn default() -> Self {
        Self {
            level: 0.3,
            artifacts: vec![
                ArtifactType::Blocking { block_size: 8 },
                ArtifactType::Banding { bands: 16 },
            ],
            preserve_readability: true,
        }
    }
}

impl ConfigValidation for DegradationConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if !(0.0..=1.0).contains(&self.level) {
            return Err(PresetError::intensity_out_of_range(self.level));
        }
        
        for artifact in &self.artifacts {
            match artifact {
                ArtifactType::Blocking { block_size } => {
                    if *block_size == 0 {
                        return Err(PresetError::invalid_config("Block size must be greater than 0"));
                    }
                }
                ArtifactType::Banding { bands } => {
                    if *bands == 0 {
                        return Err(PresetError::invalid_config("Number of bands must be greater than 0"));
                    }
                }
                ArtifactType::Compression { quality } => {
                    if !(0.0..=1.0).contains(quality) {
                        return Err(PresetError::invalid_config("Compression quality must be between 0.0 and 1.0"));
                    }
                }
                ArtifactType::Posterization { levels } => {
                    if *levels == 0 || *levels > 256 {
                        return Err(PresetError::invalid_config("Posterization levels must be between 1 and 256"));
                    }
                }
            }
        }
        Ok(())
    }
}

// Pixel sorting preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct PixelSortConfig {
    pub direction: SortDirection,
    pub criteria: SortCriteria,
    pub region: Option<Region>,
    pub threshold: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Horizontal,
    Vertical,
    Diagonal,
    Radial { center: (u32, u32) },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortCriteria {
    Brightness,
    Hue,
    Saturation,
    Red,
    Green, 
    Blue,
}

impl Default for PixelSortConfig {
    fn default() -> Self {
        Self {
            direction: SortDirection::Horizontal,
            criteria: SortCriteria::Brightness,
            region: None,
            threshold: Some(0.5),
        }
    }
}

impl ConfigValidation for PixelSortConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if let Some(threshold) = self.threshold {
            if !(0.0..=1.0).contains(&threshold) {
                return Err(PresetError::invalid_threshold(threshold));
            }
        }
        Ok(())
    }
}

// Datamosh preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DatamoshConfig {
    pub intensity: f32,
    pub motion_direction: Direction,
    pub duplication_factor: f32,
    pub preserve_structure: bool,
}

impl Default for DatamoshConfig {
    fn default() -> Self {
        Self {
            intensity: 0.4,
            motion_direction: Direction::Right,
            duplication_factor: 1.5,
            preserve_structure: true,
        }
    }
}

impl ConfigValidation for DatamoshConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if !(0.0..=1.0).contains(&self.intensity) {
            return Err(PresetError::intensity_out_of_range(self.intensity));
        }
        if self.duplication_factor < 1.0 || self.duplication_factor > 10.0 {
            return Err(PresetError::invalid_config("Duplication factor must be between 1.0 and 10.0"));
        }
        Ok(())
    }
}

// Block corruption preset configuration
#[derive(Debug, Clone, PartialEq)]
pub struct BlockCorruptionConfig {
    pub block_size: (u32, u32),
    pub corruption_type: CorruptionType,
    pub distribution: f32,
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CorruptionType {
    SolidColor(u8, u8, u8),
    Noise,
    Displacement { offset: (i32, i32) },
    Duplication { source_offset: (i32, i32) },
}

impl Default for BlockCorruptionConfig {
    fn default() -> Self {
        Self {
            block_size: (16, 16),
            corruption_type: CorruptionType::Noise,
            distribution: 0.1,
            intensity: 0.7,
        }
    }
}

impl ConfigValidation for BlockCorruptionConfig {
    fn validate(&self) -> Result<(), PresetError> {
        if self.block_size.0 == 0 || self.block_size.1 == 0 {
            return Err(PresetError::invalid_block_size(self.block_size.0, self.block_size.1));
        }
        if !(0.0..=1.0).contains(&self.distribution) {
            return Err(PresetError::invalid_config("Distribution must be between 0.0 and 1.0"));
        }
        if !(0.0..=1.0).contains(&self.intensity) {
            return Err(PresetError::intensity_out_of_range(self.intensity));
        }
        Ok(())
    }
}