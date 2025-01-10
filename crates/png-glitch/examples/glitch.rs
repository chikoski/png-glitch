extern crate png_glitch;

use png_glitch::FilterType;

fn main() {
    let mut glitch = png_glitch::PngGlitch::open("etc/sample00.png").unwrap();

    glitch.remove_filter();
    let src = glitch.height() / 3;
    let dest = src * 2;
    let width = glitch.height() / 4;

    glitch.transpose(src, dest, width);
    glitch.apply_filter_from(FilterType::Paeth, dest, width);
    for mut line in glitch.scan_lines_from(dest, width) {
        line[0] = 0;
    }

    let src = glitch.height() / 5 * 2;
    glitch.apply_filter_from(FilterType::Sub, src, width);
    let lines = glitch.scan_lines_from(src, width);
    for mut line in lines {
        for i in 0..line.size() {
            if line[i] == 0  {
                line[i] = 1;
            }
        }
    }

    glitch.save("etc/example-glitch.png").unwrap()
}