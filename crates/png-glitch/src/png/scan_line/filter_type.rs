use crate::png::png_error::PngError;

/// An enum representing the filter type of a scan line.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FilterType {
    /// No filter.
    None,
    /// Sub filter.
    Sub,
    /// Up filter.
    Up,
    /// Average filter.
    Average,
    /// Paeth filter.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_u8() {
        assert_eq!(FilterType::try_from(0).unwrap(), FilterType::None);
        assert_eq!(FilterType::try_from(1).unwrap(), FilterType::Sub);
        assert_eq!(FilterType::try_from(2).unwrap(), FilterType::Up);
        assert_eq!(FilterType::try_from(3).unwrap(), FilterType::Average);
        assert_eq!(FilterType::try_from(4).unwrap(), FilterType::Paeth);
        assert!(FilterType::try_from(5).is_err());
    }

    #[test]
    fn test_into_u8() {
        assert_eq!(u8::from(FilterType::None), 0);
        assert_eq!(u8::from(FilterType::Sub), 1);
        assert_eq!(u8::from(FilterType::Up), 2);
        assert_eq!(u8::from(FilterType::Average), 3);
        assert_eq!(u8::from(FilterType::Paeth), 4);
    }
}