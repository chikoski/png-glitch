use png_glitch::PngGlitch;
use std::fs;
use std::path::{Path, PathBuf};

fn example_output_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&dir).expect("Failed to create example output directory");
    dir
}

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = example_output_dir();

    remove_filter(
        manifest_dir.join("etc/none.png"),
        out_dir.join("removed-none.png"),
    );
    remove_filter(
        manifest_dir.join("etc/sub.png"),
        out_dir.join("removed-sub.png"),
    );
    remove_filter(
        manifest_dir.join("etc/up.png"),
        out_dir.join("removed-up.png"),
    );
    remove_filter(
        manifest_dir.join("etc/average.png"),
        out_dir.join("removed-average.png"),
    );
    remove_filter(
        manifest_dir.join("etc/paeth.png"),
        out_dir.join("removed-paeth.png"),
    );
    remove_filter(
        manifest_dir.join("etc/sample00.png"),
        out_dir.join("removed-sample00.png"),
    );
}

fn remove_filter(path: impl AsRef<Path>, removed: impl AsRef<Path>) {
    let removed = removed.as_ref();
    let mut png = PngGlitch::open(path).unwrap();
    png.remove_filter();
    png.save(removed).unwrap();
    println!("Saved {}", removed.display());
}
