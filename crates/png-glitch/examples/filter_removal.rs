extern crate png_glitch;

use png_glitch::PngGlitch;

fn main() {
    let mut png = PngGlitch::open("./etc/sample00.png").unwrap();
    png.remove_filter();
    png.save("./etc/filter_removal.png").unwrap();
}