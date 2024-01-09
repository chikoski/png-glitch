mod filter_type;

pub use filter_type::FilterType;

pub struct ScanLine {
    pub filter_type: FilterType,
    offset: usize,
}