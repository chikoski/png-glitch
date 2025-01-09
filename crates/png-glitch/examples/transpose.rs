extern crate png_glitch;

fn main() {
    let mut glitch = png_glitch::PngGlitch::open("etc/sample00.png").unwrap();
    glitch.remove_filter();

    let scan_lines = glitch.scan_lines();

    let src = scan_lines.len() / 3;
    let dest = src * 2;
    let width = scan_lines.len() / 10;

    glitch.transpose(src, dest, width as u32);

    glitch.save("etc/example-transpose.png").unwrap()
}