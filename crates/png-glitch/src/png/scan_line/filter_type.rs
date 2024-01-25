pub use crate::bindings::chikoski::png_glitch::types::FilterType;
use crate::png::png_error::PngError;

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
