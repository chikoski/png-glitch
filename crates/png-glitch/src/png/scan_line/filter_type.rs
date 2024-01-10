use crate::png::png_error::PngError;

#[derive(Debug, Copy, Clone)]
pub enum FilterType {
    None,
    Sub,
    Up,
    Average,
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
            _ => Err(PngError::InvalidFilterType)
        }
    }
}

impl Into<u8> for FilterType {
    fn into(self) -> u8 {
        match self {
            FilterType::None => 0,
            FilterType::Sub => 1,
            FilterType::Up => 2,
            FilterType::Average => 3,
            FilterType::Paeth => 4,
        }
    }
}
