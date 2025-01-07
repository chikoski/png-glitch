extern crate png_glitch;

use std::path::Path;
use png_glitch::{FilterType, PngGlitch};

fn main() {
    apply("etc/none.png", "etc/applied-sub.png", FilterType::Sub);
    apply("etc/none.png", "etc/applied-up.png", FilterType::Up);
    apply("etc/none.png", "etc/applied-average.png", FilterType::Average);
    apply("etc/none.png", "etc/applied-paeth.png", FilterType::Paeth);
}

fn apply(src: impl AsRef<Path>, dst: impl AsRef<Path>, filter_type: FilterType) {
    let mut png = PngGlitch::open(src).unwrap();
    png.remove_filter();
    png.apply_filter(filter_type);
    png.save(dst).unwrap();
}