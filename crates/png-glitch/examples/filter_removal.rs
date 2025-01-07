extern crate png_glitch;

use png_glitch::PngGlitch;
use std::path::Path;

fn main() {
    remove_filter("etc/none.png", "etc/removed-none.png");
    remove_filter("etc/sub.png", "etc/removed-sub.png");
    remove_filter("etc/up.png", "etc/removed-up.png");
    remove_filter("etc/average.png", "etc/removed-average.png");
    remove_filter("etc/paeth.png", "etc/removed-paeth.png");

    remove_filter("etc/sample00.png", "etc/removed-sample00.png");
}

fn remove_filter(path: impl AsRef<Path>, removed: impl AsRef<Path>) {
    let mut png = PngGlitch::open(path).unwrap();
    png.remove_filter();
    png.save(removed).unwrap();
}