use png_glitch::PngGlitch;
use std::fs;
use std::path::Path;

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("example-transpose.png");

    let mut glitch = PngGlitch::open(&input_path).unwrap();
    glitch.remove_filter();

    let src = glitch.height() / 3;
    let dest = src * 2;
    let width = glitch.height() / 10;

    glitch.transpose(src, dest, width);

    glitch.save(&output_path).unwrap();
    println!("Saved {}", output_path.display());
}
