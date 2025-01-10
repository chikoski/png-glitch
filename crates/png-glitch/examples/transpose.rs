extern crate png_glitch;

fn main() {
    let mut glitch = png_glitch::PngGlitch::open("etc/sample00.png").unwrap();
    glitch.remove_filter();

    let src = glitch.height()/ 3;
    let dest = src * 2;
    let width = glitch.height() / 10;

    glitch.transpose(src, dest, width);

    glitch.save("etc/example-transpose.png").unwrap()
}