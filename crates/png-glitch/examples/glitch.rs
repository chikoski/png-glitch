use png_glitch::{FilterType, PngGlitch};
use std::fs;
use std::path::Path;

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("example-glitch.png");

    let mut glitch = PngGlitch::open(&input_path).unwrap();

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
            if line[i] == 0 {
                line[i] = 1;
            }
        }
    }

    glitch.save(&output_path).unwrap();
    println!("Saved {}", output_path.display());
}
