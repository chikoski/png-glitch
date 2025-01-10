extern crate png_glitch;

use png_glitch::FilterType;

fn main() {
    let mut glitch = png_glitch::PngGlitch::open("etc/sample00.png").unwrap();

    glitch.remove_filter();
    let src = glitch.height() as usize / 3;
    let dest = src * 2;
    let width = glitch.height() as usize / 4;

    let mut lines = glitch.scan_lines_from(src, width);
    let mut previous = lines.pop();
    while lines.len() > 0 {
        if let Some(mut line) = previous {
            previous = lines.pop();
            line.apply_filter(FilterType::Average, previous.as_ref());
            line[0] = 5;
        }
    }
    glitch.transpose(src, dest, width as u32);

    let src = glitch.height() as usize / 5 * 2;
    let lines = glitch.scan_lines_from(src, width);
    for mut line in lines {
        line.set_filter_type(FilterType::Sub);
    }

    glitch.save("etc/example-glitch.png").unwrap()
}