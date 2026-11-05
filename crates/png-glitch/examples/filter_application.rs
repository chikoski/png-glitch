use png_glitch::{FilterType, PngGlitch};
use std::fs;
use std::path::{Path, PathBuf};

fn example_output_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&dir).expect("Failed to create example output directory");
    dir
}

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/none.png");
    let out_dir = example_output_dir();

    apply(&src, out_dir.join("applied-sub.png"), FilterType::Sub);
    apply(&src, out_dir.join("applied-up.png"), FilterType::Up);
    apply(
        &src,
        out_dir.join("applied-average.png"),
        FilterType::Average,
    );
    apply(&src, out_dir.join("applied-paeth.png"), FilterType::Paeth);
}

fn apply(src: impl AsRef<Path>, dst: impl AsRef<Path>, filter_type: FilterType) {
    let dst = dst.as_ref();
    let mut png = PngGlitch::open(src).unwrap();
    png.remove_filter();
    png.apply_filter(filter_type);
    png.save(dst).unwrap();
    println!("Saved {}", dst.display());
}
