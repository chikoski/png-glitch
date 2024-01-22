use crate::png::png_error::PngError;

#[derive(Debug, Copy, Clone)]
/// FilterType represents filter methods defined in [the PNG specification](https://www.w3.org/TR/2003/REC-PNG-20031110/#9Filters).
/// Each variants represents different method.
pub enum FilterType {
    /// None method
    None,
    /// Sub method
    Sub,
    /// Up method
    Up,
    /// Average method
    Average,
    /// Paeth method
    Paeth,
}

impl TryFrom<u8> for FilterType {
    type Error = PngError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FilterType::None),
            1 => Ok(FilterType::Sub),
            2 => Ok(FilterType::Up),
            3 => Ok(FilterType::Average),
            4 => Ok(FilterType::Paeth),
            _ => Err(PngError::InvalidFilterType),
        }
    }
}

impl From<FilterType> for u8 {
    fn from(value: FilterType) -> Self {
        match value {
            FilterType::None => 0,
            FilterType::Sub => 1,
            FilterType::Up => 2,
            FilterType::Average => 3,
            FilterType::Paeth => 4,
        }
    }
}
